use std::collections::HashMap;

use anyhow::{anyhow, bail, Context};
use semver::Version;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use worker::Env;
use worker_kv::KvStore;

use crate::{
    github::Tag,
    localization::{Completeness, UnsortedChanges},
    platform::Platform,
};

const STATE_KV_BINDING: &str = "STATE";
const STATE_KV_KEY: &str = "state";

pub type State = HashMap<String, PlatformState>;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PlatformState {
    pub last_posted_tag_previous_release: Tag,
    pub last_posted_tag: Tag,

    #[serde(default)]
    pub last_post: Option<PostInformation>,

    #[serde(default)]
    pub posted_archiving_message: bool,

    #[serde(default)]
    pub localization_changes_completeness: Completeness,
    #[serde(default)]
    pub localization_changes: UnsortedChanges,

    #[serde(default)]
    pub pending_state: Option<Box<PlatformState>>,
}

impl PlatformState {
    fn validate(&self) -> anyhow::Result<(Version, Version)> {
        let last_posted_version_previous_release: Version = self
            .last_posted_tag_previous_release
            .to_version()
            .context("couldn't convert last_posted_tag_previous_release to a Version")?;

        let last_posted_version: Version = self
            .last_posted_tag
            .to_version()
            .context("couldn't convert last_posted_tag to a Version")?;

        if last_posted_version_previous_release >= last_posted_version {
            bail!("last_posted_version_previous_release >= last_posted_version");
        }

        if let Some(platform_state) = self.pending_state.as_deref() {
            let (pending_last_posted_version_previous_release, pending_last_posted_version) =
                platform_state.validate().context("invalid pending state")?;

            if platform_state.last_post.is_some() {
                bail!("pending state unexpectedly has last post");
            }

            if pending_last_posted_version_previous_release < last_posted_version_previous_release {
                bail!("pending_last_posted_version_previous_release < last_posted_version_previous_release");
            }

            if pending_last_posted_version <= last_posted_version {
                bail!("pending_last_posted_version <= last_posted_version");
            }
        }

        Ok((last_posted_version_previous_release, last_posted_version))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PostInformation {
    pub id: u64,
    pub number: u64,
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
                controller.log_state("loaded state from KV");
                controller.validate_state().context("invalid state")?;
                tracing::trace!("state appears to be valid");

                Ok(controller)
            }
            None => bail!("no state in KV"),
        }
    }

    fn validate_state(&self) -> anyhow::Result<()> {
        for platform in Platform::iter() {
            tracing::trace!(%platform, "validating platform state");

            self.platform_state(platform)
                .validate()
                .context("invalid platform state")?;
        }

        Ok(())
    }

    pub fn platform_state(&self, platform: Platform) -> &PlatformState {
        self.state
            .get(platform.state_key().as_str())
            .expect("state to be available for all platforms")
    }

    fn platform_state_mut(&mut self, platform: Platform) -> &mut PlatformState {
        self.state
            .get_mut(platform.state_key().as_str())
            .expect("state to be available for all platforms")
    }

    pub async fn set_platform_state(
        &mut self,
        platform: Platform,
        state: PlatformState,
    ) -> anyhow::Result<()> {
        let platform_state = self.platform_state_mut(platform);

        if *platform_state != state {
            state.validate().context("tried to set invalid state")?;

            *platform_state = state;
            tracing::debug!(%platform, ?platform_state, "changed platform state");

            match self.commit_changes().await {
                Ok(_) => tracing::debug!("saved state to KV"),
                Err(e) => return Err(e.context("could not save state to KV")),
            }
        } else {
            tracing::warn!(%platform, "platform state did not change");
        }

        Ok(())
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

    fn log_state(&self, message: &str) {
        tracing::debug!("{message}:");

        for platform in Platform::iter() {
            tracing::debug!(%platform, state = ?self.platform_state(platform));
            crate::logging::separator();
        }
    }
}
