use std::time::{Duration, SystemTime};

use anyhow::{anyhow, Context};
use chrono::prelude::*;
use sha2::{Digest, Sha256};
use strum::IntoEnumIterator;
use tracing::debug;
use worker::{wasm_bindgen::JsValue, Delay, Env};

use crate::platform::Platform;

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
