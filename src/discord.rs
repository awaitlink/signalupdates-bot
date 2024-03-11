use anyhow::{bail, Context};
use serde_json::{json, Value};
use worker::{Env, Method, Url};

use crate::{
    env::EnvExt,
    markdown::Post,
    network::{self, ContentType},
    utils,
    Platform::*,
};

pub async fn notify(
    env: &Env,
    post: &Post<'_>,
    discourse_topic_id: Option<u64>,
    discourse_post_number: Option<u64>,
) -> anyhow::Result<()> {
    let url = env
        .discord_webhook_url_updates()
        .context("could not get Discord updates webhook URL")?;

    let role = match post.platform() {
        Android | Ios | Desktop => env
            .discord_updates_mention_role()
            .context("could not get Discord updates mention role")?,
        Server => env
            .discord_server_updates_mention_role()
            .context("could not get Discord server updates mention role")?,
    };

    let url = Url::parse(&url).context("could not parse url")?;

    let title = format!("{} {}", post.platform(), post.new_tag().name);
    let (post_url, notice) = match (discourse_topic_id, discourse_post_number) {
        (Some(topic_id), Some(post_number)) => {
            (utils::discourse_post_url(topic_id, post_number), None)
        }
        (Some(topic_id), None) => (
            utils::discourse_topic_url(topic_id),
            Some("The update notice is not posted yet, likely awaiting moderation"),
        ),
        (None, Some(post_number)) => {
            bail!("have post number {post_number} but not a topic id")
        }
        (None, None) => (
            String::from("https://community.signalusers.org/c/25"),
            Some("Beta feedback thread could not be located. Not created yet?"),
        ),
    };

    let github_comparison_name = post.github_comparison_name();
    let github_comparison_url = post.platform().github_comparison_url(
        &post.old_tag().name,
        &post.new_tag().name,
        None,
        true,
    );

    let content = format!(
        "New {} version: `{}` <@&{}>",
        post.platform(),
        post.new_tag().exact_version_string(),
        role
    );

    let body = json!({
        "content": content,
        "embeds": [{
            "color": post.platform().color(),
            "title": title,
            "url": post_url,
            "description": notice,
            "author": {
                "name": github_comparison_name,
                "url": github_comparison_url,
            },
            "allowed_mentions": {
                "roles": [role]
            },
            "fields": [
                {
                    "name": "Build number",
                    "value": post.new_build_configuration()
                        .as_ref()
                        .map(|conf| conf.canonical_version_code.to_string())
                        .unwrap_or(String::from("n/a")),
                    "inline": true
                },
                {
                    "name": "Commits",
                    "value": post.commits().len().to_string(),
                    "inline": true
                },
                {
                    "name": "Total commits",
                    "value": post.unfiltered_commits_len().to_string(),
                    "inline": true
                },
                {
                    "name": "Languages changed (build)",
                    "value": post.localization_change_collection()
                        .build_changes
                        .unsorted_changes
                        .len()
                        .to_string(),
                    "inline": true
                },
                {
                    "name": "Languages changed (release so far)",
                    "value": post.localization_change_collection()
                        .release_changes
                        .as_ref()
                        .map(|changes| changes.unsorted_changes.len())
                        .unwrap_or(0)
                        .to_string(),
                    "inline": true
                },
            ],
        }],
    });

    tracing::trace!(?body, "will send to discord");

    let request = network::create_request(
        url,
        Method::Post,
        ContentType::ApplicationJson,
        ContentType::ApplicationJson,
        Some(body.to_string()),
        None,
    )
    .context("could not create request to Discord")?;

    let mut response = network::fetch(worker::Fetch::Request(request))
        .await
        .context("could not send request to Discord")?;

    if response.status_code() < 200 || response.status_code() > 299 {
        let json: Value = network::json_from_response(&mut response).await?;
        tracing::debug!(?json, "got response from Discord");

        bail!(
            "discord responded with {} status code",
            response.status_code()
        )
    };

    Ok(())
}

pub async fn send_error_message(env: &Env, error: &anyhow::Error, log: &str) -> anyhow::Result<()> {
    let role = env
        .discord_errors_mention_role()
        .context("could not get Discord errors mention role")?;

    let message = format!("**[error]** <@&{role}>\n```\n{error:?}\n```");
    notify_with_log(env, &message, log).await
}

pub async fn send_misc_message(env: &Env, content: &str, log: &str) -> anyhow::Result<()> {
    let role = env
        .discord_errors_mention_role()
        .context("could not get Discord errors mention role")?;

    let message = format!("**[misc]** {content} <@&{role}>");
    notify_with_log(env, &message, log).await
}

async fn notify_with_log(env: &Env, message: &str, log: &str) -> anyhow::Result<()> {
    let url = env
        .discord_webhook_url_errors()
        .context("could not get Discord errors webhook URL")?;

    let role = env
        .discord_errors_mention_role()
        .context("could not get Discord errors mention role")?;

    let url = Url::parse(&url).context("could not parse url")?;

    let boundary = "721640C74F194C8C9F795C59A371A868";

    let body = vec![
        format!("--{boundary}"),
        String::from(r#"Content-Disposition: form-data; name="payload_json""#),
        String::from(""),
        json!({
            "content": message,
            "allowed_mentions": {
                "roles": [role]
            }
        })
        .to_string(),
        format!("--{boundary}"),
        String::from(r#"Content-Disposition: form-data; name="files[0]"; filename="log.txt""#),
        String::from(r#"Content-Type: text/plain"#),
        String::from(""),
        String::from(log),
        format!("--{boundary}--"),
    ]
    .join("\r\n");

    let request = network::create_request(
        url,
        Method::Post,
        ContentType::MultipartFormData(boundary.to_string()),
        ContentType::ApplicationJson,
        Some(body),
        None,
    )
    .context("could not create request to Discord")?;

    let response: Value = network::get_json_from_request(request)
        .await
        .context("could not send request to Discord")?;

    tracing::debug!(?response, "got response from Discord");

    Ok(())
}
