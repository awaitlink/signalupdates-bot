use anyhow::{anyhow, bail, Context};
use semver::Version;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use worker::Env;
use worker_kv::KvStore;

use crate::{
    localization::{Completeness, LocalizationChange},
    platform::Platform::{self, *},
    types::github::Tag,
};

const STATE_KV_BINDING: &str = "STATE";
const STATE_KV_KEY: &str = "state";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct State {
    pub android: PlatformState,
    pub ios: PlatformState,
    pub desktop: PlatformState,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlatformState {
    pub last_posted_tag_previous_release: Tag,
    pub last_posted_tag: Tag,

    #[serde(default)]
    pub last_post_number: Option<u64>,

    #[serde(default)]
    pub posted_archiving_message: bool,

    #[serde(default)]
    pub localization_changes_completeness: Completeness,
    #[serde(default)]
    pub localization_changes: Vec<LocalizationChange>,
}

pub struct StateController {
    kv_store: KvStore,
    state: State,
}

impl StateController {
    pub async fn from_kv(env: &Env) -> anyhow::Result<Self> {
        let kv_store = env
            .kv(STATE_KV_BINDING)
            .map_err(|e| anyhow!(e.to_string()))
            .context("could not get KV store")?;

        let state: Option<State> = kv_store
            .get(STATE_KV_KEY)
            .json()
            .await
            .map_err(|e| anyhow!(e.to_string()))
            .with_context(|| format!("could not get value for key {STATE_KV_KEY}"))?;

        match state {
            Some(state) => {
                let controller = Self { kv_store, state };
                controller.validate_state().context("invalid state")?;
                Ok(controller)
            }
            None => bail!("no state in KV"),
        }
    }

    fn validate_state(&self) -> anyhow::Result<()> {
        for platform in Platform::iter() {
            let state = self.platform_state(platform);

            let last_posted_version_previous_release: Version = state
                .last_posted_tag_previous_release
                .clone()
                .try_into()
                .context("couldn't convert last_posted_tag_previous_release into Version")?;

            let last_posted_version: Version = state
                .last_posted_tag
                .clone()
                .try_into()
                .context("couldn't convert last_posted_tag into Version")?;

            if last_posted_version_previous_release >= last_posted_version {
                bail!("last_posted_version_previous_release >= last_posted_version for {platform}");
            }
        }

        Ok(())
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn platform_state(&self, platform: Platform) -> &PlatformState {
        match platform {
            Android => &self.state.android,
            Ios => &self.state.ios,
            Desktop => &self.state.desktop,
        }
    }

    pub async fn set_platform_state(
        &mut self,
        platform: Platform,
        state: PlatformState,
    ) -> anyhow::Result<()> {
        match platform {
            Android => self.state.android = state,
            Ios => self.state.ios = state,
            Desktop => self.state.desktop = state,
        }

        self.commit_changes().await
    }

    async fn commit_changes(&mut self) -> anyhow::Result<()> {
        self.kv_store
            .put(STATE_KV_KEY, &self.state)
            .map_err(|e| anyhow!(e.to_string()))
            .context("could not create request to put to KV")?
            .execute()
            .await
            .map_err(|e| anyhow!(e.to_string()))
            .context("could not put to KV")
    }
}
