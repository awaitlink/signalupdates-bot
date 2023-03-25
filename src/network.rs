use std::fmt;

use anyhow::{anyhow, Context};
use serde::de::DeserializeOwned;
use worker::{wasm_bindgen::JsValue, Fetch, Headers, Method, Request, RequestInit, Response, Url};

pub const USER_AGENT: &str = "updates-bot";

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
        tracing::debug!(response.status_code = response.status_code());
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
    tracing::debug!(url.domain = url.domain(), ?method, "creating request");

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
