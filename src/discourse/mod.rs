use anyhow::{bail, Context};
use semver::Version;
use serde_json::json;
use worker::{Env, Method, Url};

use crate::{
    env::EnvExt,
    network::{self, ContentType},
    platform::Platform,
};

mod types;
pub use types::*;

pub async fn get_topic_id(
    api_key: &str,
    platform: Platform,
    version: &Version,
    topic_id_for_server_updates: u64,
) -> anyhow::Result<Option<u64>> {
    tracing::debug!(?version, "getting topic id for version");

    let url = Url::parse(&platform.discourse_topic_slug_url(version, topic_id_for_server_updates))
        .context("could not parse URL")?;

    let request = network::create_request(
        url,
        Method::Get,
        ContentType::ApplicationJson,
        ContentType::ApplicationJson,
        None,
        Some(api_key),
    )?;
    let response: ApiResponse<Topic> = network::get_json_from_request(request).await?;

    match &response {
        ApiResponse::Ok(response) => match response.post_stream.posts.first() {
            Some(post) => Ok(Some(post.topic_id)),
            None => {
                tracing::warn!(?response);
                bail!("no posts in topic")
            }
        },
        ApiResponse::Err(Error::NotFound) => {
            tracing::warn!("topic not found");
            Ok(None)
        }
        ApiResponse::Unknown(value) => bail!("unexpected response = {value:?}"),
    }
}

pub async fn get_topic_id_or_override(
    env: &Env,
    api_key: &str,
    platform: Platform,
    version: &Version,
) -> anyhow::Result<Option<u64>> {
    match env.topic_id_override()? {
        Some(id) => {
            tracing::warn!(id, "using topic id override");
            Ok(Some(id))
        }
        None => get_topic_id(
            api_key,
            platform,
            version,
            env.topic_id_for_server_updates()?,
        )
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
    Posted { id: u64, number: u64 },
    Enqueued,
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

    let request = network::create_request(
        url,
        Method::Post,
        ContentType::ApplicationJson,
        ContentType::ApplicationJson,
        Some(body.to_string()),
        Some(api_key),
    )?;
    let api_response: ApiResponse<CreatePostResponse> =
        network::get_json_from_request(request).await?;

    match api_response {
        ApiResponse::Ok(CreatePostResponse::Posted(post)) => Ok(PostingOutcome::Posted {
            id: post.id,
            number: post.post_number,
        }),
        ApiResponse::Ok(CreatePostResponse::Action(PostAction::Enqueued { .. })) => {
            Ok(PostingOutcome::Enqueued)
        }
        ApiResponse::Err(error) => bail!("error = {error:?}"),
        ApiResponse::Unknown(value) => bail!("unexpected response = {value:?}"),
    }
}

pub async fn get_replies_to_post(post_id: u64) -> anyhow::Result<Vec<Post>> {
    let url = Url::parse(&format!(
        "https://community.signalusers.org/posts/{post_id}/replies.json"
    ))
    .context("could not parse URL")?;

    // Without API key, in case the post is returned for the author even while it's enqueued
    let request = network::create_request(
        url,
        Method::Get,
        ContentType::ApplicationJson,
        ContentType::ApplicationJson,
        None,
        None,
    )?;
    let posts: ApiResponse<Vec<Post>> = network::get_json_from_request(request).await?;

    Ok(match posts {
        ApiResponse::Ok(posts) => posts,
        ApiResponse::Err(error) => bail!("error = {error:?}"),
        ApiResponse::Unknown(value) => bail!("unexpected response = {value:?}"),
    })
}
