use anyhow::{bail, Context};
use semver::Version;
use serde_json::json;
use worker::{console_error, console_log, console_warn, Env, Method, Url};

use crate::{platform::Platform, utils};

mod types;
pub use types::*;

pub const ERROR_TYPE_NOT_FOUND: &str = "not_found";

pub async fn get_topic_id(
    api_key: &str,
    platform: Platform,
    version: &Version,
) -> anyhow::Result<Option<u64>> {
    console_log!("getting topic id for version {version}");

    let url =
        Url::parse(&platform.discourse_topic_slug_url(version)).context("could not parse URL")?;

    let request = utils::create_request(url, Method::Get, None, Some(api_key))?;
    let response: Result<TopicResponse, Error> = utils::get_json_from_request(request).await?;

    match &response {
        Ok(response) => match response.post_stream.posts.first() {
            Some(post) => Ok(Some(post.topic_id)),
            None => {
                console_error!("response = {:?}", response);
                bail!("no posts in topic")
            }
        },
        Err(error) if error.error_type == ERROR_TYPE_NOT_FOUND => {
            console_warn!("topic not found, response = {:?}", response);
            Ok(None)
        }
        Err(error) => {
            console_error!("unexpected error = {:?}", error);
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
    match utils::topic_id_override(env)? {
        Some(id) => {
            console_warn!("using topic id override: {id}");
            Ok(Some(id))
        }
        None => get_topic_id(api_key, platform, version)
            .await
            .context("could not find topic_id"),
    }
}

pub fn archiving_post_markdown(new_topic_id: u64) -> String {
    format!(
        "Beta testing for this release has concluded. If you find any further bugs related to this release or earlier releases, please report them on GitHub (read https://community.signalusers.org/t/27 for more information on how to do that).

If you have feedback specifically related to the new beta version, please post it in the following topic: https://community.signalusers.org/t/{new_topic_id}."
    )
}

#[derive(Debug)]
pub enum PostingOutcome {
    Posted { number: u64 },
    Enqueued { id: u64 },
}

pub async fn post(
    markdown_text: &str,
    api_key: &str,
    topic_id: u64,
    reply_to_post_number: Option<u64>,
) -> anyhow::Result<PostingOutcome> {
    let url = Url::parse("https://community.signalusers.org/posts.json")
        .context("could not parse URL")?;

    let body = json!({
        "topic_id": topic_id,
        "reply_to_post_number": reply_to_post_number,
        "raw": markdown_text,
    });

    let request = utils::create_request(url, Method::Post, Some(body), Some(api_key))?;
    let api_response: PostApiResponse = utils::get_json_from_request(request).await?;

    match api_response.post_number {
        Some(number) => Ok(PostingOutcome::Posted { number }),
        None => {
            match (&api_response.action, &api_response.pending_post) {
                (Some(action), Some(pending_post)) if action == "enqueued" => {
                    return Ok(PostingOutcome::Enqueued {
                        id: pending_post.id,
                    })
                }
                _ => {}
            }

            console_error!("api_response = {:?}", api_response);
            bail!("discourse API response did not include the post identifiers, posting likely failed")
        }
    }
}

pub async fn get_post_number(post_id: u64) -> anyhow::Result<Option<u64>> {
    let url = Url::parse(&format!(
        "https://community.signalusers.org/posts/{post_id}.json"
    ))
    .context("could not parse URL")?;

    // Without API key, in case the post is returned for the author even while it's enqueued
    let request = utils::create_request(url, Method::Get, None, None)?;
    let post: Result<Post, Error> = utils::get_json_from_request(request).await?;

    Ok(match post {
        Ok(post) => Some(post.post_number),
        Err(error) if error.error_type == ERROR_TYPE_NOT_FOUND => None,
        Err(error) => bail!("unexpected error when getting post: {error:?}"),
    })
}
