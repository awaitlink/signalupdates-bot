use std::time::{Duration, SystemTime};

use anyhow::{anyhow, Context};
use serde::de::DeserializeOwned;
use serde_json::Value;
use sha2::{Digest, Sha256};
use worker::{
    console_log, wasm_bindgen::JsValue, Delay, Env, Fetch, Headers, Method, Request, RequestInit,
    Response, Url,
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

pub fn is_dry_run(env: &Env) -> anyhow::Result<bool> {
    get_env_string(env, Var, "DRY_RUN").map(|string| string == "true")
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

pub async fn fetch(configuration: Fetch) -> anyhow::Result<Response> {
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

pub async fn json_from_response<T: DeserializeOwned>(response: &mut Response) -> anyhow::Result<T> {
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

pub fn sha256_string(input: &str) -> String {
    let result = Sha256::digest(input.as_bytes());
    base16ct::lower::encode_string(&result)
}

/// Asynchronously waits for the specified number of milliseconds.
pub async fn delay(milliseconds: u64) {
    console_log!("waiting {milliseconds} milliseconds");

    Delay::from(Duration::from_millis(milliseconds)).await;

    console_log!("done waiting {milliseconds} milliseconds");
}

pub fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_millis(worker::Date::now().as_millis())
}
