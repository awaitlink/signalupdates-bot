use std::collections::HashMap;

use anyhow::{anyhow, bail, Context};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::json;
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
const STATE_KV_MOST_RECENT_ANDROID_FIREBASE_VERSION_KEY: &str = "mostRecentAndroidFirebaseVersion";

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
    most_recent_android_firebase_version_tag: Tag,
}

impl StateController {
    async fn get_json<T>(kv_store: &KvStore, key: &str) -> anyhow::Result<Option<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        kv_store
            .get(key)
            .json()
            .await
            .map_err(|e| anyhow!(e.to_string()))
            .with_context(|| format!("could not get value for key {key}"))
    }

    pub async fn from_kv(env: &Env) -> anyhow::Result<Self> {
        let kv_store = env
            .kv(STATE_KV_BINDING)
            .map_err(|e| anyhow!(e.to_string()))
            .context("could not get KV store")?;

        let state: Option<State> = Self::get_json(&kv_store, STATE_KV_KEY).await?;

        match state {
            Some(state) => {
                let most_recent_android_firebase_version: String =
                    Self::get_json(&kv_store, STATE_KV_MOST_RECENT_ANDROID_FIREBASE_VERSION_KEY)
                        .await?
                        .unwrap_or(String::from("0.0.0"));

                let controller = Self {
                    kv_store,
                    state,
                    most_recent_android_firebase_version_tag: Tag::from_exact_version_string(
                        &most_recent_android_firebase_version,
                    ),
                };

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

    pub fn most_recent_android_firebase_version_tag(&self) -> &Tag {
        &self.most_recent_android_firebase_version_tag
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

            match self.commit_state_changes().await {
                Ok(_) => tracing::debug!("saved state to KV"),
                Err(e) => return Err(e.context("could not save state to KV")),
            }
        } else {
            tracing::warn!(%platform, "platform state did not change");
        }

        Ok(())
    }

    pub async fn set_firebase(
        &mut self,
        most_recent_android_firebase_version: Tag,
    ) -> anyhow::Result<()> {
        let existing = &mut self.most_recent_android_firebase_version_tag;

        if *existing != most_recent_android_firebase_version {
            *existing = most_recent_android_firebase_version;
            tracing::debug!(new=?existing, "changed most recent android firebase version");

            match self.commit_firebase_changes().await {
                Ok(_) => tracing::debug!("saved firebase state to KV"),
                Err(e) => return Err(e.context("could not save firebase state to KV")),
            }
        } else {
            tracing::warn!(
                ?existing,
                "most recent android firebase version did not change"
            );
        }

        Ok(())
    }

    async fn put_json<T>(&self, key: &str, value: &T) -> anyhow::Result<()>
    where
        T: Serialize,
    {
        self.kv_store
            .put(key, value)
            .map_err(|e| anyhow!(e.to_string()))
            .context("could not create request to put to KV")?
            .execute()
            .await
            .map_err(|e| anyhow!(e.to_string()))
            .context("could not put to KV")
    }

    async fn commit_state_changes(&self) -> anyhow::Result<()> {
        self.put_json(STATE_KV_KEY, &self.state).await
    }

    async fn commit_firebase_changes(&self) -> anyhow::Result<()> {
        self.put_json(
            STATE_KV_MOST_RECENT_ANDROID_FIREBASE_VERSION_KEY,
            &json!(self
                .most_recent_android_firebase_version_tag
                .exact_version_string())
            .to_string(),
        )
        .await
    }
}
