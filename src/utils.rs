use anyhow::{anyhow, Context};
use semver::Version;
use serde::de::DeserializeOwned;
use serde_json::Value;
use worker::{
    console_log, wasm_bindgen::JsValue, Env, Fetch, Headers, Method, Request, RequestInit,
    Response, Url,
};

use crate::platform::Platform;

pub const USER_AGENT: &str = "updates-bot";

pub fn version_from_tag(tag: &str) -> anyhow::Result<Version> {
    lenient_semver::parse(tag)
        .map_err(|e| anyhow!(e.to_string()))
        .context("could not parse version from tag")
}

pub fn api_key(env: &Env) -> String {
    let string_binding = env.secret("DISCOURSE_API_KEY").unwrap();
    JsValue::from(string_binding).as_string().unwrap()
}

pub async fn get_topic_id(
    api_key: String,
    platform: Platform,
    version: &Version,
) -> anyhow::Result<Option<u64>> {
    console_log!("getting topic id for version {version}");

    let url =
        Url::parse(&platform.discourse_topic_slug_url(version)).context("could not parse URL")?;

    let request = create_request(url, Method::Get, None, Some(api_key))?;
    let response: crate::types::discourse::TopicResponse = get_json_from_request(request).await?;

    match response.post_stream.posts.first() {
        Some(post) => Ok(Some(post.topic_id)),
        None => {
            console_log!("no posts in topic");
            Ok(None)
        }
    }
}

pub async fn get_json_from_url<T: DeserializeOwned>(url: impl Into<String>) -> anyhow::Result<T> {
    let url = Url::parse(&url.into()).context("could not parse URL")?;
    let request = create_request(url, Method::Get, None, None)?;
    json_from_configuration(Fetch::Request(request)).await
}

pub async fn get_json_from_request<T: DeserializeOwned>(request: Request) -> anyhow::Result<T> {
    json_from_configuration(Fetch::Request(request)).await
}

async fn json_from_configuration<T: DeserializeOwned>(configuration: Fetch) -> anyhow::Result<T> {
    fetch(configuration)
        .await?
        .json()
        .await
        .map_err(|e| anyhow!(e.to_string()))
        .context("could not get JSON")
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

pub fn create_request(
    url: Url,
    method: Method,
    body: Option<Value>,
    discourse_api_key: Option<String>,
) -> anyhow::Result<Request> {
    console_log!("constructing request for url {url}");

    let mut headers = Headers::new();

    if let Some(api_key) = discourse_api_key {
        headers.set("User-Api-Key", &api_key).unwrap();
    }

    headers.set("Content-Type", "application/json").unwrap();
    headers.set("Accept", "application/json").unwrap();
    headers.set("User-Agent", USER_AGENT).unwrap();

    let mut request_init = RequestInit::new();
    request_init.with_method(method).with_headers(headers);

    if let Some(body) = body {
        request_init.with_body(Some(JsValue::from_str(&body.to_string())));
    }

    Request::new_with_init(url.as_ref(), &request_init).map_err(|e| anyhow!(e.to_string()))
}
