use std::{
    fmt,
    sync::mpsc,
    time::{Duration, SystemTime},
};

use anyhow::{anyhow, Context};
use chrono::prelude::*;
use log::*;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use strum::IntoEnumIterator;
use worker::{
    wasm_bindgen::JsValue, Delay, Env, Fetch, Headers, Method, Request, RequestInit, Response, Url,
};

pub const USER_AGENT: &str = "updates-bot";

#[derive(Debug)]
enum StringBindingKind {
    Secret,
    Var,
}

use StringBindingKind::*;

use crate::platform::Platform;

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

pub fn discourse_api_key(env: &Env) -> anyhow::Result<String> {
    get_env_string(env, Secret, "DISCOURSE_API_KEY")
}

pub fn discord_webhook_url(env: &Env) -> anyhow::Result<String> {
    get_env_string(env, Secret, "DISCORD_WEBHOOK_URL")
}

pub fn topic_id_override(env: &Env) -> anyhow::Result<Option<u64>> {
    get_env_string(env, Var, "TOPIC_ID_OVERRIDE").map(|string| string.parse().ok())
}

pub fn is_dry_run(env: &Env) -> anyhow::Result<bool> {
    get_env_string(env, Var, "DRY_RUN").map(|string| string == "true")
}

pub async fn get_json_from_url<T: DeserializeOwned>(url: &str) -> anyhow::Result<T> {
    let url = Url::parse(url).context("could not parse URL")?;
    let request = create_request(
        url,
        Method::Get,
        ContentType::ApplicationJson,
        ContentType::ApplicationJson,
        None,
        None,
    )?;
    json_from_configuration(Fetch::Request(request)).await
}

pub async fn get_json_from_request<T: DeserializeOwned>(request: Request) -> anyhow::Result<T> {
    json_from_configuration(Fetch::Request(request)).await
}

async fn json_from_configuration<T: DeserializeOwned>(configuration: Fetch) -> anyhow::Result<T> {
    let mut response = fetch(configuration).await?;
    json_from_response(&mut response).await
}

pub async fn fetch(configuration: Fetch) -> anyhow::Result<Response> {
    let result = configuration
        .send()
        .await
        .map_err(|e| anyhow!(e.to_string()))
        .context("could not fetch");

    if let Ok(response) = &result {
        debug!("response.status_code() = {}", response.status_code());
    }

    result
}

pub async fn json_from_response<T: DeserializeOwned>(response: &mut Response) -> anyhow::Result<T> {
    response
        .json()
        .await
        .map_err(|e| anyhow!(e.to_string()))
        .context("could not get JSON")
}

pub enum ContentType {
    ApplicationJson,
    MultipartFormData(String),
}

impl fmt::Display for ContentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ContentType::ApplicationJson => String::from("application/json"),
                ContentType::MultipartFormData(boundary) =>
                    format!(r#"multipart/form-data; boundary="{boundary}""#),
            }
        )
    }
}

pub fn create_request(
    url: Url,
    method: Method,
    content_type: ContentType,
    accept: ContentType,
    body: Option<String>,
    discourse_api_key: Option<&str>,
) -> anyhow::Result<Request> {
    debug!("constructing request for url {url}");

    let mut headers = Headers::new();

    if let Some(api_key) = discourse_api_key {
        headers.set("User-Api-Key", api_key).unwrap();
    }

    headers
        .set("Content-Type", &content_type.to_string())
        .unwrap();
    headers.set("Accept", &accept.to_string()).unwrap();
    headers.set("User-Agent", USER_AGENT).unwrap();

    let mut request_init = RequestInit::new();
    request_init.with_method(method).with_headers(headers);

    if let Some(body) = body {
        request_init.with_body(Some(JsValue::from_str(&body)));
    }

    Request::new_with_init(url.as_ref(), &request_init)
        .map_err(|e| anyhow!(e.to_string()))
        .context("could not create request")
}

pub fn sha256_string(input: &str) -> String {
    let result = Sha256::digest(input.as_bytes());
    base16ct::lower::encode_string(&result)
}

/// Asynchronously waits for the specified number of milliseconds.
pub async fn delay(milliseconds: u64) {
    debug!("waiting {milliseconds} milliseconds");

    Delay::from(Duration::from_millis(milliseconds)).await;

    debug!("done waiting {milliseconds} milliseconds");
}

pub fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_millis(worker::Date::now().as_millis())
}

pub fn platforms_order(time: NaiveTime) -> anyhow::Result<Vec<Platform>> {
    let platforms = Platform::iter().collect::<Vec<_>>();

    let index = (time.minute() / 10)
        .try_into()
        .context("should be able to convert to usize")?;

    let platforms = permute::permutations_of(&platforms)
        .nth(index)
        .context("there should be >= 6 permutations")?
        .copied()
        .collect::<Vec<_>>();

    Ok(platforms)
}

pub fn log_separator() {
    debug!("----------------------------------------------------------------------");
}

pub fn initialize_logger(env: &Env) -> mpsc::Receiver<String> {
    let to_redact = [
        (
            "[redacted: Discourse API key]",
            discourse_api_key(env).expect("should able to get Discourse API key"),
        ),
        (
            "[redacted: Discord webhook URL]",
            discord_webhook_url(env).expect("should able to get Discord webhook URL"),
        ),
    ];

    let (tx, rx) = mpsc::channel();

    let dispatch = fern::Dispatch::new()
        .level_for("locale_codes", log::LevelFilter::Info)
        .format(move |out, message, record| {
            out.finish(format_args!(
                "[{}][{}] {}",
                record.level(),
                record.target(),
                {
                    let mut message = message.to_string();
                    for (name, string) in to_redact.iter() {
                        message = message.replace(string, name);
                    }
                    message
                },
            ))
        })
        .chain(tx);

    #[cfg(target_family = "wasm")]
    let dispatch = dispatch.chain(fern::Output::call(console_log::log));

    #[cfg(not(target_family = "wasm"))]
    let dispatch = dispatch.chain(fern::Output::stderr("\n"));

    dispatch
        .apply()
        .expect("should be able to initialize logger");

    rx
}

pub fn recv_log(rx: mpsc::Receiver<String>) -> String {
    let mut log = Vec::new();
    while let Ok(message) = rx.try_recv() {
        log.push(message);
    }

    log.join("")
}

pub fn escape_html(string: &str) -> String {
    askama_escape::escape(string, askama_escape::Html).to_string()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use pretty_assertions::assert_eq;
    use test_case::test_case;

    use super::*;

    #[test]
    fn platforms_order_len() {
        let mut set = HashSet::new();

        for minute in 0..=59 {
            set.insert(platforms_order(NaiveTime::from_hms(0, minute, 0)).unwrap());
        }

        assert_eq!(set.len(), 6);
    }

    #[test_case(
        "Test commit & message <HtmlTag/>'s \"continuation\"",
        "Test commit &amp; message &lt;HtmlTag/&gt;&#x27;s &quot;continuation&quot;";
        "basic"
    )]
    fn escape_html_ok(input: &str, output: &str) {
        assert_eq!(escape_html(input), output);
    }
}
