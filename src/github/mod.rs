use anyhow::{bail, Context};
use serde::de::DeserializeOwned;
use worker::{console_log, console_warn, Fetch, Method, Url};

use crate::{platform::Platform, utils};

mod types;
pub use types::*;

pub async fn get_comparison(
    platform: Platform,
    old_tag: &str,
    new_tag: &str,
) -> anyhow::Result<Comparison> {
    console_log!("getting comparison between {old_tag} and {new_tag} for {platform} from GitHub");

    let initial_url = platform.github_api_comparison_url(old_tag, new_tag);

    let comparison = get_paginated_response(
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

pub async fn get_commit(platform: Platform, sha: &str) -> anyhow::Result<Commit> {
    console_log!("getting commit {sha} for {platform} from GitHub");

    let initial_url = platform.github_api_commit_url(sha);

    let commit = get_paginated_response(
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
async fn get_paginated_response<T, F>(
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
        let request = utils::create_request(url, Method::Get, None, None)?;

        let mut response = utils::fetch(Fetch::Request(request))
            .await
            .context("could not fetch from GitHub")?;

        let mut part: T = utils::json_from_response(&mut response)
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
