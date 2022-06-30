#![feature(array_windows)]

use std::rc::Rc;

use anyhow::{anyhow, Context};
use semver::Version;
use strum::IntoEnumIterator;
use worker::{
    console_error, console_log, console_warn, event, Env, ScheduleContext, ScheduledEvent,
};

mod localization;
mod panic_hook;
mod platform;
mod post;
mod state;
mod types;
mod utils;

use localization::{
    Language, LocalizationChange, LocalizationChangeCollection, LocalizationChanges,
};
use platform::Platform;
use state::StateController;

enum PlatformCheckOutcome {
    LatestVersionIsAlreadyPosted,
    TopicNotFound,
    Posted,
}

use PlatformCheckOutcome::*;

// Used for debugging, to manually trigger the bot outside of schedule.
#[event(fetch)]
pub async fn fetch(
    _req: worker::Request,
    env: Env,
    _ctx: worker::Context,
) -> worker::Result<worker::Response> {
    main(&env).await;
    worker::Response::empty()
}

#[event(scheduled)]
pub async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    main(&env).await;
}

async fn main(env: &Env) {
    panic_hook::set_panic_hook();

    match check_all_platforms(env).await {
        Err(e) => console_error!("{e:?}"),
        Ok(_) => console_log!("finished successfully"),
    }
}

async fn check_all_platforms(env: &Env) -> anyhow::Result<()> {
    let mut state_controller = state::StateController::from_kv(env).await?;
    console_log!("loaded state from KV: {:?}", state_controller.state());

    for platform in Platform::iter() {
        let outcome = check_platform(&mut state_controller, env, platform).await?;

        match outcome {
            LatestVersionIsAlreadyPosted => console_log!("latest version is already posted"),
            TopicNotFound => console_warn!("no topic found, may be not created yet"),
            Posted => {
                console_warn!("already posted for {platform} and currently doing only one post per invocation, done");
                break;
            }
        }
    }

    Ok(())
}

async fn check_platform(
    state_controller: &mut StateController,
    env: &Env,
    platform: Platform,
) -> anyhow::Result<PlatformCheckOutcome> {
    console_log!("checking platform = {platform}");

    let all_tags: Vec<types::github::Tag> =
        utils::get_json_from_url(platform.github_api_tags_url())
            .await
            .context("could not fetch tags from GitHub")?;

    console_log!("all_tags = {:?}", all_tags);

    let tags: Vec<(types::github::Tag, Version)> = all_tags
        .iter()
        .filter_map(|tag| tag.try_into().ok().map(|version| (tag.clone(), version)))
        .filter(|(_, version)| platform.should_post_version(version))
        .collect();

    console_log!("tags = {:?}", tags);

    // TODO: assumes the last posted tag can be found on this GitHub API page
    let tags_to_post: Vec<(types::github::Tag, Version)> = tags
        .iter()
        .rev()
        .skip_while(|(tag, _)| tag != &state_controller.platform_state(platform).last_posted_tag)
        .cloned()
        .collect();

    console_log!("tags_to_post = {:?}", tags_to_post);

    if let Some([(old_tag, old_version), (new_tag, new_version)]) =
        tags_to_post.array_windows().next()
    {
        console_log!(
            "looking at [old_tag: {:?}, new_tag: {:?}]",
            old_tag,
            new_tag
        );

        let discourse_api_key = utils::api_key(env)?;

        let topic_id = match utils::topic_id_override(env)? {
            Some(id) => {
                console_warn!("using topic id override: {id}");
                Some(id)
            }
            None => utils::get_topic_id(&discourse_api_key, platform, new_version)
                .await
                .context("could not find topic_id")?,
        };

        match topic_id {
            Some(topic_id) => {
                console_log!("topic_id = {topic_id}");

                let same_release = old_version.major == new_version.major
                    && old_version.minor == new_version.minor;
                console_log!("same_release = {}", same_release);

                let reply_to_post_number = if same_release {
                    state_controller.platform_state(platform).last_post_number
                } else {
                    None
                };
                console_log!("reply_to_post_number = {:?}", reply_to_post_number);

                let comparison =
                    utils::get_github_comparison(platform, &old_tag.name, &new_tag.name)
                        .await
                        .context("could not get build comparison from GitHub")?;

                console_log!("comparison = {:?}", comparison);

                let commits: Vec<post::Commit> = comparison
                    .commits
                    .iter()
                    .map(|github_commit| post::Commit::from_github_commit(platform, github_commit))
                    .collect();

                console_log!("commits.len() = {:?}", commits.len());

                let build_localization_changes = LocalizationChanges::from_comparison(
                    &platform,
                    old_tag,
                    new_tag,
                    &comparison,
                    None,
                );

                let localization_change_codes_complete = build_localization_changes.complete
                    && (!same_release
                        || state_controller
                            .platform_state(platform)
                            .localization_change_codes_complete);

                let release_localization_changes = if !same_release {
                    console_log!("first build of the release, release_localization_changes = None");
                    None
                } else {
                    let last_version_of_previous_release = tags
                        .iter()
                        .find(|(_, version)| {
                            version.minor < new_version.minor || version.major < new_version.major
                        })
                        .ok_or_else(|| {
                            anyhow!("could not determine last version of previous release")
                        })?;

                    console_log!(
                        "last_version_of_previous_release = {:?}",
                        last_version_of_previous_release
                    );

                    let mut changes: Vec<_> = state_controller
                        .platform_state(platform)
                        .localization_change_codes
                        .iter()
                        .map(|code| LocalizationChange {
                            language: Language::from_code(code).unwrap(),
                            filename: platform.filename_for_language_code(code),
                        })
                        .chain(build_localization_changes.changes.iter().cloned())
                        .collect();

                    changes.dedup();
                    changes.sort_unstable();

                    let release_localization_changes = LocalizationChanges {
                        platform,
                        old_tag: &last_version_of_previous_release.0,
                        new_tag,
                        complete: localization_change_codes_complete,
                        changes: Rc::new(changes),
                    };

                    console_log!(
                        "release_localization_changes = {:?}",
                        release_localization_changes
                    );

                    Some(release_localization_changes)
                };

                let localization_change_codes = release_localization_changes
                    .as_ref()
                    .unwrap_or(&build_localization_changes)
                    .changes
                    .iter()
                    .map(|change| change.language.full_code())
                    .collect();

                let post = post::Post::new(
                    platform,
                    old_tag,
                    new_tag,
                    commits,
                    LocalizationChangeCollection {
                        build_changes: build_localization_changes,
                        release_changes: release_localization_changes,
                    },
                );

                let post_number = post
                    .post(&discourse_api_key, topic_id, reply_to_post_number)
                    .await
                    .context("could not post to Discourse")?;

                console_log!("posted post_number = {:?}", post_number);

                state_controller
                    .set_platform_state(
                        platform,
                        state::PlatformState::new(
                            new_tag.clone(),
                            Some(post_number),
                            localization_change_codes,
                            localization_change_codes_complete,
                        ),
                    )
                    .await
                    .context("could not set platform state")?;

                console_log!(
                    "saved platform state to KV: {:?}",
                    state_controller.platform_state(platform)
                );

                Ok(Posted)
            }
            None => Ok(TopicNotFound),
        }
    } else {
        Ok(LatestVersionIsAlreadyPosted)
    }
}
