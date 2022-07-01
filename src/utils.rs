use std::time::Duration;

use anyhow::{anyhow, bail, Context};
use semver::Version;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use worker::{
    console_error, console_log, console_warn, wasm_bindgen::JsValue, Delay, Env, Fetch, Headers,
    Method, Request, RequestInit, Response, Url,
};

use crate::{
    platform::Platform,
    types::{
        discourse::PostApiResponse,
        github::{Commit, CommitData, Comparison},
    },
};

pub const USER_AGENT: &str = "updates-bot";

#[derive(Debug)]
enum StringBindingKind {
    Secret,
    Var,
}

use StringBindingKind::*;

fn get_env_string(env: &Env, kind: StringBindingKind, name: &str) -> anyhow::Result<String> {
    let string_binding = match kind {
        Secret => env.secret(name),
        Var => env.var(name),
    }
    .map_err(|e| anyhow!(e.to_string()))
    .with_context(|| anyhow!("couldn't get string binding kind = {kind:?}, name = {name}"))?;

    JsValue::from(string_binding)
        .as_string()
        .ok_or_else(|| anyhow!("couldn't get value of string binding"))
}

pub fn api_key(env: &Env) -> anyhow::Result<String> {
    get_env_string(env, Secret, "DISCOURSE_API_KEY")
}

pub fn topic_id_override(env: &Env) -> anyhow::Result<Option<u64>> {
    get_env_string(env, Var, "TOPIC_ID_OVERRIDE").map(|string| string.parse().ok())
}

pub async fn get_topic_id(
    api_key: &str,
    platform: Platform,
    version: &Version,
) -> anyhow::Result<Option<u64>> {
    console_log!("getting topic id for version {version}");

    let url =
        Url::parse(&platform.discourse_topic_slug_url(version)).context("could not parse URL")?;

    let request = create_request(url, Method::Get, None, Some(api_key))?;
    let response: crate::types::discourse::TopicResponse = get_json_from_request(request).await?;

    match (&response.post_stream, &response.error_type) {
        (Some(post_stream), _) => match post_stream.posts.first() {
            Some(post) => Ok(Some(post.topic_id)),
            None => {
                console_error!("response = {:?}", response);
                bail!("no posts in topic")
            }
        },
        (None, Some(error_type)) if error_type == "not_found" => {
            console_warn!("topic not found, response = {:?}", response);
            Ok(None)
        }
        (None, _) => {
            console_error!("response = {:?}", response);
            bail!("discourse API request likely failed")
        }
    }
}

pub async fn get_topic_id_or_override(
    env: &Env,
    api_key: &str,
    platform: Platform,
    version: &Version,
) -> anyhow::Result<Option<u64>> {
    match topic_id_override(env)? {
        Some(id) => {
            console_warn!("using topic id override: {id}");
            Ok(Some(id))
        }
        None => get_topic_id(api_key, platform, version)
            .await
            .context("could not find topic_id"),
    }
}

pub async fn get_json_from_url<T: DeserializeOwned>(url: &str) -> anyhow::Result<T> {
    let url = Url::parse(url).context("could not parse URL")?;
    let request = create_request(url, Method::Get, None, None)?;
    json_from_configuration(Fetch::Request(request)).await
}

pub async fn get_json_from_request<T: DeserializeOwned>(request: Request) -> anyhow::Result<T> {
    json_from_configuration(Fetch::Request(request)).await
}

async fn json_from_configuration<T: DeserializeOwned>(configuration: Fetch) -> anyhow::Result<T> {
    let mut response = fetch(configuration).await?;
    json_from_response(&mut response).await
}

async fn fetch(configuration: Fetch) -> anyhow::Result<Response> {
    let result = configuration
        .send()
        .await
        .map_err(|e| anyhow!(e.to_string()))
        .context("could not fetch");

    if let Ok(response) = &result {
        console_log!("response.status_code() = {}", response.status_code());
    }

    result
}

async fn json_from_response<T: DeserializeOwned>(response: &mut Response) -> anyhow::Result<T> {
    response
        .json()
        .await
        .map_err(|e| anyhow!(e.to_string()))
        .context("could not get JSON")
}

pub fn create_request(
    url: Url,
    method: Method,
    body: Option<Value>,
    discourse_api_key: Option<&str>,
) -> anyhow::Result<Request> {
    console_log!("constructing request for url {url}");

    let mut headers = Headers::new();

    if let Some(api_key) = discourse_api_key {
        headers.set("User-Api-Key", api_key).unwrap();
    }

    headers.set("Content-Type", "application/json").unwrap();
    headers.set("Accept", "application/json").unwrap();
    headers.set("User-Agent", USER_AGENT).unwrap();

    let mut request_init = RequestInit::new();
    request_init.with_method(method).with_headers(headers);

    if let Some(body) = body {
        request_init.with_body(Some(JsValue::from_str(&body.to_string())));
    }

    Request::new_with_init(url.as_ref(), &request_init)
        .map_err(|e| anyhow!(e.to_string()))
        .context("could not create request")
}

pub async fn get_github_comparison(
    platform: Platform,
    old_tag: &str,
    new_tag: &str,
) -> anyhow::Result<Comparison> {
    console_log!("getting comparison between {old_tag} and {new_tag} for {platform} from GitHub");

    let initial_url = platform.github_api_comparison_url(old_tag, new_tag);

    let comparison = get_paginated_github_response(
        &initial_url,
        Comparison {
            total_commits: 0,
            commits: Vec::new(),
            files: Some(Vec::new()),
        },
        |target, source| {
            target.total_commits = source.total_commits; // always the total number of commits
            target.commits.append(&mut source.commits);
            if let Some(part_files) = &mut source.files {
                target.files.as_mut().unwrap().append(part_files);
            }
        },
    )
    .await?;

    if comparison.total_commits != comparison.commits.len() {
        bail!(
            "incomplete full comparison: total_commits = {} but commits.len() = {}, commits = {:?}",
            comparison.total_commits,
            comparison.commits.len(),
            comparison.commits
        )
    };

    Ok(comparison)
}

pub async fn get_github_commit(platform: Platform, sha: &str) -> anyhow::Result<Commit> {
    console_log!("getting commit {sha} for {platform} from GitHub");

    let initial_url = platform.github_api_commit_url(sha);

    let commit = get_paginated_github_response(
        &initial_url,
        Commit {
            sha: sha.to_string(),
            commit: CommitData {
                message: String::new(),
            },
            files: Some(Vec::new()),
        },
        |target, source| {
            target.sha = source.sha.clone();
            target.commit = source.commit.clone();
            if let Some(part_files) = &mut source.files {
                target.files.as_mut().unwrap().append(part_files);
            }
        },
    )
    .await?;

    Ok(commit)
}

/// `merge`: `Fn(&mut target, &mut source)`
pub async fn get_paginated_github_response<T, F>(
    initial_url: &str,
    initial_result: T,
    merge: F,
) -> anyhow::Result<T>
where
    T: DeserializeOwned,
    F: Fn(&mut T, &mut T),
{
    console_log!("getting paginated response from GitHub");

    let mut page = 1;
    let per_page = 100;

    let mut url_string = format!("{initial_url}?page={page}&per_page={per_page}");

    let mut result: T = initial_result;

    loop {
        console_log!("getting page = {page}, url = {url_string}");

        let url = Url::parse(&url_string).context("could not parse URL")?;
        let request = create_request(url, Method::Get, None, None)?;

        let mut response = fetch(Fetch::Request(request))
            .await
            .context("could not fetch from GitHub")?;

        let mut part: T = json_from_response(&mut response)
            .await
            .context("could not get JSON for part")?;

        merge(&mut result, &mut part);

        let link_header_string = match response.headers().get("Link").unwrap() {
            Some(header_string) => header_string,
            None => {
                console_warn!(
                    "no `Link` header in GitHub's response, likely done getting paginated response"
                );
                break;
            }
        };

        let link_header = parse_link_header::parse_with_rel(&link_header_string)
            .context("could not parse `Link` header")?;

        match link_header.get("next") {
            Some(link) => {
                url_string = link.raw_uri.clone();
                page += 1;
            }
            None => {
                console_log!("no `next` link, done getting full response");
                break;
            }
        }
    }

    Ok(result)
}

pub fn sha256_string(input: &str) -> String {
    let result = Sha256::digest(input.as_bytes());
    base16ct::lower::encode_string(&result)
}

pub fn archiving_post_markdown(new_topic_id: u64) -> String {
    format!(
        "Beta testing for this release has concluded. If you find any further bugs related to this release or earlier releases, please report them on GitHub (read https://community.signalusers.org/t/27 for more information on how to do that).

If you have feedback specifically related to the new beta version, please post it in the following topic: https://community.signalusers.org/t/{new_topic_id}"
    )
}

/// Makes a post in Discourse.
///
/// If successful, returns the post number.
pub async fn post_to_discourse(
    markdown_text: &str,
    api_key: &str,
    topic_id: u64,
    reply_to_post_number: Option<u64>,
) -> anyhow::Result<u64> {
    let url = Url::parse("https://community.signalusers.org/posts.json")
        .context("could not parse URL")?;

    let body = json!({
        "topic_id": topic_id,
        "reply_to_post_number": reply_to_post_number,
        "raw": markdown_text,
    });

    let request = create_request(url, Method::Post, Some(body), Some(api_key))?;
    let api_response: PostApiResponse = get_json_from_request(request).await?;

    match api_response.post_number {
        Some(number) => Ok(number),
        None => {
            console_error!("api_response = {:?}", api_response);
            bail!("discourse API response did not include the post number, posting likely failed")
        }
    }
}

/// Asynchronously waits for the specified number of milliseconds.
pub async fn delay(milliseconds: u64) {
    console_log!("waiting {milliseconds} milliseconds");

    Delay::from(Duration::from_millis(milliseconds)).await;

    console_log!("done waiting {milliseconds} milliseconds");
}
