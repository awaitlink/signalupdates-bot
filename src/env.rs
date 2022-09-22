use anyhow::{anyhow, Context};
use worker::{wasm_bindgen::JsValue, Env};

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

pub trait EnvExt {
    fn discourse_api_key(&self) -> anyhow::Result<String>;
    fn discord_webhook_url(&self) -> anyhow::Result<String>;

    fn user_id(&self) -> anyhow::Result<u64>;
    fn topic_id_override(&self) -> anyhow::Result<Option<u64>>;
    fn is_dry_run(&self) -> anyhow::Result<bool>;
}

impl EnvExt for Env {
    fn discourse_api_key(&self) -> anyhow::Result<String> {
        get_env_string(self, Secret, "DISCOURSE_API_KEY")
    }

    fn discord_webhook_url(&self) -> anyhow::Result<String> {
        get_env_string(self, Secret, "DISCORD_WEBHOOK_URL")
    }

    fn user_id(&self) -> anyhow::Result<u64> {
        get_env_string(self, Var, "USER_ID").map(|string| {
            string
                .parse()
                .context("couldn't parse user ID from the environment")
        })?
    }

    fn topic_id_override(&self) -> anyhow::Result<Option<u64>> {
        get_env_string(self, Var, "TOPIC_ID_OVERRIDE").map(|string| string.parse().ok())
    }

    fn is_dry_run(&self) -> anyhow::Result<bool> {
        get_env_string(self, Var, "DRY_RUN").map(|string| string == "true")
    }
}
