use anyhow::Context;
use serde_json::Value;
use tracing::debug;
use worker::{Env, Method, Url};

use crate::{
    network::{self, ContentType},
    utils,
};

pub async fn send_error_message(env: &Env, log: &str) -> anyhow::Result<()> {
    let url = utils::discord_webhook_url(env).context("could not get Discord webhook URL")?;
    let url = Url::parse(&url).context("could not parse url")?;

    let boundary = "721640C74F194C8C9F795C59A371A868";
    let error_message = "Error occurred:";

    let body = vec![
        format!("--{boundary}"),
        String::from(r#"Content-Disposition: form-data; name="content""#),
        String::from(""),
        String::from(error_message),
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

    debug!("response from Discord: {:?}", response);

    Ok(())
}
