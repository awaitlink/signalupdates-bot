#![feature(array_windows)]

use anyhow::Context;
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
    Completeness, LocalizationChange, LocalizationChangeCollection, LocalizationChanges,
};
use platform::Platform;
use state::StateController;

const POSTING_DELAY_MILLISECONDS: u64 = 3000;

enum PlatformCheckOutcome {
    LatestVersionIsAlreadyPosted,
    NewTopicNotFound,
    PostedCommits,
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
            NewTopicNotFound => console_warn!("no topic found, may be not created yet"),
            PostedCommits => {
                console_warn!("already posted for {platform} and currently doing only one \"commits\" post per invocation, done");
                break;
            }
        }

        console_log!("----------------------------------------------------------------------");
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
        utils::get_json_from_url(&platform.github_api_tags_url())
            .await
            .context("could not fetch tags from GitHub")?;

    console_log!("all_tags = {:?}", all_tags);

    let mut tags: Vec<(types::github::Tag, Version)> = all_tags
        .iter()
        .filter_map(|tag| tag.try_into().ok().map(|version| (tag.clone(), version)))
        .filter(|(_, version)| platform.should_post_version(version))
        .collect();

    console_log!("tags = {:?}", tags);

    tags.sort_unstable_by(|(_, lhs), (_, rhs)| lhs.cmp(rhs));
    console_log!("after sorting, tags = {:?}", tags);

    // TODO: assumes the last posted tag can be found on this GitHub API page
    let tags_to_post: Vec<(types::github::Tag, Version)> = tags
        .iter()
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

        let new_topic_id =
            utils::get_topic_id_or_override(env, &discourse_api_key, platform, new_version)
                .await
                .context("could not find new_topic_id")?;

        match new_topic_id {
            Some(new_topic_id) => {
                console_log!("new_topic_id = {new_topic_id}");

                let same_release = old_version.major == new_version.major
                    && old_version.minor == new_version.minor;
                console_log!("same_release = {}", same_release);

                // Post archiving message to old topic if necessary and possible
                post_archiving_message_if_necessary(
                    same_release,
                    state_controller,
                    env,
                    platform,
                    &discourse_api_key,
                    old_version,
                    new_topic_id,
                )
                .await?;

                // Post commits to new topic

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

                let unfiltered_commits: Vec<post::Commit> = comparison
                    .commits
                    .iter()
                    .map(|github_commit| post::Commit::from_github_commit(platform, github_commit))
                    .collect();

                let unfiltered_commits_len = unfiltered_commits.len();
                console_log!("unfiltered_commits_len = {:?}", unfiltered_commits_len);

                let commits: Vec<post::Commit> = unfiltered_commits
                    .into_iter()
                    .filter(|commit| platform.should_show_commit(commit.full_message()))
                    .collect();

                let commits_len = commits.len();
                console_log!("commits_len = {:?}", commits_len);

                let mut build_localization_changes =
                    LocalizationChanges::from_comparison(platform, old_tag, new_tag, &comparison);

                if let Completeness::Incomplete = build_localization_changes.completeness {
                    let localization_change_commits: Vec<_> = commits
                        .iter()
                        .filter(|commit| commit.is_likely_localization_change())
                        .collect();

                    console_log!(
                        "localization_change_commits = {:?}",
                        localization_change_commits
                    );

                    if !localization_change_commits.is_empty() {
                        let mut all_complete = true;

                        for commit in localization_change_commits {
                            let with_files =
                                utils::get_github_commit(platform, commit.sha()).await?;

                            console_log!("with_files = {:?}", with_files);

                            let mut changes = LocalizationChange::unsorted_changes_from_files(
                                platform,
                                &with_files.files,
                            );

                            console_log!("changes = {:?}", changes);

                            build_localization_changes.changes.append(&mut changes);
                            build_localization_changes.changes.sort_unstable();
                            build_localization_changes.changes.dedup();

                            let complete = with_files.are_files_likely_complete().unwrap();
                            console_log!(
                                "for commit.sha = {} files.complete = {complete}",
                                commit.sha()
                            );

                            all_complete &= complete;
                        }

                        if all_complete {
                            build_localization_changes.completeness = Completeness::LikelyComplete;

                            console_log!(
                                "got complete files for all localization change commits, build_localization_changes.completeness = {:?}",
                                build_localization_changes.completeness
                            );
                        }
                    }
                }

                let localization_changes_completeness = build_localization_changes
                    .completeness
                    .min(if !same_release {
                        Completeness::Complete
                    } else {
                        state_controller
                            .platform_state(platform)
                            .localization_changes_completeness
                    });

                let (release_localization_changes, last_posted_tag_previous_release) =
                    if !same_release {
                        (None, old_tag)
                    } else {
                        let last_posted_tag_previous_release = &state_controller
                            .platform_state(platform)
                            .last_posted_tag_previous_release;

                        let mut changes: Vec<_> = state_controller
                            .platform_state(platform)
                            .localization_changes
                            .iter()
                            .chain(build_localization_changes.changes.iter())
                            .cloned()
                            .collect();

                        changes.sort_unstable();
                        changes.dedup();

                        let release_localization_changes = LocalizationChanges {
                            platform,
                            old_tag: last_posted_tag_previous_release,
                            new_tag,
                            completeness: localization_changes_completeness,
                            changes,
                        };

                        (
                            Some(release_localization_changes),
                            last_posted_tag_previous_release,
                        )
                    };

                console_log!(
                    "last_posted_tag_previous_release = {:?}, release_localization_changes = {:?}",
                    last_posted_tag_previous_release,
                    release_localization_changes,
                );

                let localization_changes = release_localization_changes
                    .as_ref()
                    .unwrap_or(&build_localization_changes)
                    .changes
                    .clone();

                let post = post::Post::new(
                    platform,
                    old_tag,
                    new_tag,
                    commits,
                    unfiltered_commits_len != commits_len,
                    LocalizationChangeCollection {
                        build_changes: build_localization_changes,
                        release_changes: release_localization_changes,
                    },
                );

                let post_number = post
                    .post(
                        utils::is_dry_run(env)?,
                        &discourse_api_key,
                        new_topic_id,
                        reply_to_post_number,
                    )
                    .await
                    .context("could not post commits to Discourse")?;

                console_log!("posted post_number = {:?}", post_number);

                state_controller
                    .set_platform_state(
                        platform,
                        state::PlatformState {
                            last_posted_tag_previous_release: last_posted_tag_previous_release
                                .clone(),
                            last_posted_tag: new_tag.clone(),
                            last_post_number: Some(post_number),
                            posted_archiving_message: false,
                            localization_changes_completeness,
                            localization_changes,
                        },
                    )
                    .await
                    .context("could not set platform state after posting commits")?;

                console_log!(
                    "saved platform state to KV: {:?}",
                    state_controller.platform_state(platform)
                );

                Ok(PostedCommits)
            }
            None => Ok(NewTopicNotFound),
        }
    } else {
        Ok(LatestVersionIsAlreadyPosted)
    }
}

async fn post_archiving_message_if_necessary(
    same_release: bool,
    state_controller: &mut StateController,
    env: &Env,
    platform: Platform,
    discourse_api_key: &str,
    old_version: &Version,
    new_topic_id: u64,
) -> anyhow::Result<()> {
    if same_release
        || state_controller
            .platform_state(platform)
            .posted_archiving_message
    {
        console_log!("archiving message not necessary");
        return Ok(());
    } else {
        console_log!("attempting to post archiving message");
    }

    let old_topic_id =
        utils::get_topic_id_or_override(env, discourse_api_key, platform, old_version)
            .await
            .context("could not find old_topic_id")?;

    match old_topic_id {
        Some(old_topic_id) => {
            console_log!("old_topic_id = {old_topic_id}");

            let markdown_text = utils::archiving_post_markdown(new_topic_id);
            console_log!("markdown_text.len() = {}", markdown_text.len());

            let result = if !utils::is_dry_run(env)? {
                utils::post_to_discourse(
                    &markdown_text,
                    discourse_api_key,
                    old_topic_id,
                    state_controller.platform_state(platform).last_post_number,
                )
                .await
            } else {
                console_warn!("dry run; not posting to Discourse");
                Ok(0)
            };

            match result {
                Ok(post_number) => {
                    console_log!("posted archiving message, post number = {}", post_number);

                    let mut new_state = state_controller.platform_state(platform).clone();
                    new_state.posted_archiving_message = true;

                    state_controller
                        .set_platform_state(platform, new_state)
                        .await
                        .context("could not set platform state after posting archiving message")?;

                    console_log!(
                        "saved platform state to KV: {:?}",
                        state_controller.platform_state(platform)
                    );

                    utils::delay(POSTING_DELAY_MILLISECONDS).await;
                }
                Err(_) => {
                    console_warn!("could not post archiving message to old topic, it is likely already archived; ignoring, will not post archiving message for this release");
                }
            }
        }
        None => {
            console_warn!("old topic does not exist? ignoring, will not post archiving message for this release");
        }
    }

    console_log!("post_archiving_message_if_necessary done");

    Ok(())
}
