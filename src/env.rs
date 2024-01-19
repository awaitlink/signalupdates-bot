use anyhow::{anyhow, Context};
use strum::IntoEnumIterator;
use worker::{wasm_bindgen::JsValue, Env};

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

fn filter_platforms(string: &str) -> Vec<Platform> {
    Platform::iter()
        .filter(|platform| {
            string.contains(platform.to_string().to_lowercase().chars().next().unwrap())
        })
        .collect()
}

pub trait EnvExt {
    fn discourse_api_key(&self) -> anyhow::Result<String>;

    fn discord_webhook_url_errors(&self) -> anyhow::Result<String>;
    fn discord_webhook_url_updates(&self) -> anyhow::Result<String>;
    fn discord_errors_mention_role(&self) -> anyhow::Result<String>;
    fn discord_updates_mention_role(&self) -> anyhow::Result<String>;

    fn user_id(&self) -> anyhow::Result<u64>;
    fn topic_id_override(&self) -> anyhow::Result<Option<u64>>;
    fn is_dry_run(&self) -> anyhow::Result<bool>;
    fn enabled_platforms(&self) -> anyhow::Result<Vec<Platform>>;
}

impl EnvExt for Env {
    fn discourse_api_key(&self) -> anyhow::Result<String> {
        get_env_string(self, Secret, "DISCOURSE_API_KEY")
    }

    fn discord_webhook_url_errors(&self) -> anyhow::Result<String> {
        get_env_string(self, Secret, "DISCORD_WEBHOOK_URL")
    }

    fn discord_webhook_url_updates(&self) -> anyhow::Result<String> {
        get_env_string(self, Secret, "DISCORD_WEBHOOK_URL_UPDATES")
    }

    fn discord_errors_mention_role(&self) -> anyhow::Result<String> {
        get_env_string(self, Var, "DISCORD_ERRORS_MENTION_ROLE")
    }

    fn discord_updates_mention_role(&self) -> anyhow::Result<String> {
        get_env_string(self, Var, "DISCORD_UPDATES_MENTION_ROLE")
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

    fn enabled_platforms(&self) -> anyhow::Result<Vec<Platform>> {
        get_env_string(self, Var, "ENABLED_PLATFORMS").map(|s| filter_platforms(&s))
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use test_case::test_case;

    use super::*;
    use crate::platform::Platform::*;

    #[test_case("aid", &[Android, Ios, Desktop])]
    #[test_case("ai", &[Android, Ios])]
    #[test_case("id", &[Ios, Desktop])]
    #[test_case("a", &[Android])]
    #[test_case("", &[])]
    fn filter_platforms(string: &str, output: &[Platform]) {
        assert_eq!(super::filter_platforms(string), output);
    }
}
