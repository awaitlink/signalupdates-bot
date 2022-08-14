use anyhow::Context;
use log::*;
use serde_json::Value;
use worker::{Env, Method, Url};

use crate::utils::{self, ContentType};

pub async fn send_error_message(env: &Env, message: &str, log: &str) -> anyhow::Result<()> {
    let url = utils::discord_webhook_url(env).context("could not get Discord webhook URL")?;
    let url = Url::parse(&url).context("could not parse url")?;

    let boundary = "721640C74F194C8C9F795C59A371A868";

    let body = vec![
        format!("--{boundary}"),
        String::from(r#"Content-Disposition: form-data; name="content""#),
        String::from(""),
        String::from(message),
        format!("--{boundary}"),
        String::from(r#"Content-Disposition: form-data; name="files[0]"; filename="log.txt""#),
        String::from(r#"Content-Type: text/plain"#),
        String::from(""),
        String::from(log),
        format!("--{boundary}--"),
    ]
    .join("\r\n");

    let request = utils::create_request(
        url,
        Method::Post,
        ContentType::MultipartFormData(boundary.to_string()),
        ContentType::ApplicationJson,
        Some(body),
        None,
    )
    .context("could not create request to Discord")?;

    let response: Value = utils::get_json_from_request(request)
        .await
        .context("could not send request to Discord")?;

    debug!("response from Discord: {:?}", response);

    Ok(())
}
