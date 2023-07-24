#![feature(array_windows)]

mod discord;
mod discourse;
mod env;
mod github;
mod localization;
mod logging;
mod markdown;
mod network;
mod panic_hook;
mod platform;
mod state;
mod utils;

use anyhow::Context;
use chrono::prelude::*;
use github::Tag;
use semver::Version;
use worker::{event, Env, ScheduleContext, ScheduledEvent};

use crate::{
    discourse::PostingOutcome,
    env::EnvExt,
    localization::{
        Completeness, LocalizationChange, LocalizationChangeCollection, LocalizationChanges,
    },
    logging::Logger,
    platform::{android::BuildConfiguration, Platform},
    state::{PostInformation, StateController},
};

const POSTING_DELAY_MILLISECONDS: u64 = 3000;

enum PlatformCheckOutcome {
    WaitingForApproval,
    LatestVersionIsAlreadyPosted,
    NewTopicNotFound,
    PostedCommits,
    NotifyForDebugging(String),
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

    let logger = Logger::new();

    match check_all_platforms(env, &logger).await {
        Err(error) => {
            tracing::error!(?error);

            let log = logger.collect_log();

            match discord::send_error_message(env, &error, &log)
                .await
                .context("could not send error message to Discord")
            {
                Ok(_) => tracing::info!("sent error message to Discord"),
                Err(error) => tracing::warn!(?error),
            };
        }
        Ok(_) => tracing::info!("finished successfully"),
    }
}

async fn check_all_platforms(env: &Env, logger: &Logger) -> anyhow::Result<()> {
    let now = DateTime::from(utils::now());
    tracing::debug!("now = {} (seconds: {})", now.to_rfc3339(), now.timestamp());

    let platforms = utils::platforms_order(&env.enabled_platforms()?, now.time())?;
    tracing::debug!(?platforms);

    let mut state_controller = state::StateController::from_kv(env).await?;

    for platform in platforms {
        let outcome = check_platform(&mut state_controller, env, platform).await?;

        match outcome {
            WaitingForApproval => tracing::warn!("outcome: waiting for post approval"),
            LatestVersionIsAlreadyPosted => {
                tracing::info!("outcome: latest version is already posted")
            }
            NewTopicNotFound => tracing::warn!("outcome: no topic found, may be not created yet"),
            PostedCommits => {
                tracing::warn!(%platform, "outcome: already posted for platform and currently doing only one \"commits\" post per invocation, done");
                break;
            }
            NotifyForDebugging(content) => {
                tracing::info!("outcome: notify for debugging");

                let log = logger.collect_log();

                match discord::send_misc_message(env, &content, &log)
                    .await
                    .context("could not send misc message to Discord")
                {
                    Ok(_) => tracing::info!("sent misc message to Discord"),
                    Err(error) => tracing::warn!(?error),
                };
            }
        }

        logging::separator();
    }

    Ok(())
}

async fn check_platform(
    state_controller: &mut StateController,
    env: &Env,
    platform: Platform,
) -> anyhow::Result<PlatformCheckOutcome> {
    tracing::debug!(%platform, "checking platform");

    // Check if a pending post exists and has been approved
    if let Some(pending_state) = state_controller
        .platform_state(platform)
        .pending_state
        .as_deref()
    {
        tracing::debug!("a post is waiting for approval");

        match state_controller
            .platform_state(platform)
            .last_post
            .as_ref()
            .map(|post| post.id)
        {
            Some(last_post_id) => {
                tracing::debug!("checking if there is a reply by the bot to the latest known post");

                let user_id = env.user_id().context("couldn't get user id from env")?;
                let posts = discourse::get_replies_to_post(last_post_id)
                    .await
                    .context("couldn't get replies to latest known post")?;

                tracing::trace!(?posts);

                let first_reply_by_bot = posts.iter().find(|post| post.user_id == user_id);

                if let Some(post) = first_reply_by_bot {
                    tracing::info!(?post, "approval confirmed");

                    let mut new_state = pending_state.clone();
                    new_state.last_post = Some(PostInformation {
                        id: post.id,
                        number: post.post_number,
                    });

                    state_controller
                        .set_platform_state(platform, new_state)
                        .await
                        .context("could not set platform state after confirming post approval")?;

                    tracing::trace!("continuing main logic");

                    // Submit log for potentially helping debug incorrect `reply_to_post_number` on post after an approved post
                    // TODO: remove once issue is resolved
                    return Ok(NotifyForDebugging(String::from(
                        "confirmed approval of post",
                    )));
                } else {
                    return Ok(WaitingForApproval);
                }
            }
            None => {
                tracing::warn!("there is no last_post in state; confirming post approval isn't implemented in this state; assuming it is already approved, but with an unknown post ID and number");

                state_controller
                    .set_platform_state(platform, pending_state.clone())
                    .await
                    .context("could not set platform state after assumed post approval")?;

                tracing::trace!("continuing main logic");

                // Submit log for potentially helping debug incorrect `reply_to_post_number` on post after an approved post
                // TODO: remove once issue is resolved
                return Ok(NotifyForDebugging(String::from(
                    "confirmed approval of post",
                )));
            }
        }
    } else {
        tracing::trace!("no post waiting for approval, continuing main logic");
    }

    let tags_to_post = github::get_tags_to_post(
        state_controller
            .platform_state(platform)
            .last_posted_tag
            .clone(),
        platform,
    )
    .await
    .context("could not obtain tags to post")?;

    if let Some([(old_tag, old_version), (new_tag, new_version)]) =
        tags_to_post.array_windows().next()
    {
        tracing::debug!(?old_tag, ?new_tag, "looking at [old_tag, new_tag]");

        let discourse_api_key = env.discourse_api_key()?;

        let new_topic_id =
            discourse::get_topic_id_or_override(env, &discourse_api_key, platform, new_version)
                .await
                .context("could not find new_topic_id")?;

        match new_topic_id {
            Some(new_topic_id) => {
                tracing::debug!(new_topic_id);

                let same_release = old_version.major == new_version.major
                    && old_version.minor == new_version.minor;
                tracing::debug!(same_release);

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
                    state_controller
                        .platform_state(platform)
                        .last_post
                        .as_ref()
                        .map(|post| post.number)
                } else {
                    None
                };
                tracing::debug!(reply_to_post_number);

                let comparison = github::get_comparison(platform, &old_tag.name, &new_tag.name)
                    .await
                    .context("could not get build comparison from GitHub")?;

                tracing::trace!(
                    comparison.total_commits,
                    comparison.commits.len = comparison.commits.len(),
                    comparison.files.len = comparison.files.as_ref().map(|files| files.len()),
                );

                let unfiltered_commits: Vec<markdown::Commit> = comparison
                    .commits
                    .iter()
                    .map(|github_commit| {
                        markdown::Commit::from_github_commit(platform, github_commit)
                    })
                    .collect();

                let unfiltered_commits_len = unfiltered_commits.len();
                tracing::trace!(unfiltered_commits.len = unfiltered_commits_len);

                let commits: Vec<markdown::Commit> = unfiltered_commits
                    .into_iter()
                    .filter(|commit| platform.should_show_commit(commit.full_message()))
                    .collect();

                let commits_len = commits.len();
                tracing::trace!(commits.len = commits_len);

                let mut build_localization_changes =
                    LocalizationChanges::from_comparison(platform, old_tag, new_tag, &comparison);

                if let Completeness::Incomplete = build_localization_changes.completeness {
                    let localization_change_commits: Vec<_> = commits
                        .iter()
                        .filter(|commit| commit.is_likely_localization_change())
                        .collect();

                    tracing::trace!(?localization_change_commits);

                    if !localization_change_commits.is_empty() {
                        let mut all_complete = true;

                        for commit in localization_change_commits {
                            let with_files = github::get_commit(platform, commit.sha()).await?;

                            let mut changes = LocalizationChange::unsorted_changes_from_files(
                                platform,
                                &with_files.files,
                            );

                            tracing::trace!(changes.len = changes.len());

                            build_localization_changes.add_unsorted_changes(&mut changes);

                            let complete = with_files.are_files_likely_complete().unwrap();
                            tracing::trace!(commit.sha = commit.sha(), files.complete = complete);

                            all_complete &= complete;
                        }

                        if all_complete {
                            build_localization_changes.completeness = Completeness::LikelyComplete;

                            tracing::debug!(
                                ?build_localization_changes.completeness,
                                "got complete files for all localization change commits"
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

                        let changes = state_controller
                            .platform_state(platform)
                            .localization_changes
                            .clone();

                        let mut release_localization_changes = LocalizationChanges {
                            platform,
                            old_tag: last_posted_tag_previous_release,
                            new_tag,
                            completeness: localization_changes_completeness,
                            unsorted_changes: changes,
                        };

                        release_localization_changes.add_unsorted_changes(
                            &mut build_localization_changes.unsorted_changes.clone(),
                        );

                        (
                            Some(release_localization_changes),
                            last_posted_tag_previous_release,
                        )
                    };

                tracing::debug!(
                    ?last_posted_tag_previous_release,
                    ?release_localization_changes,
                );

                let localization_changes = release_localization_changes
                    .as_ref()
                    .unwrap_or(&build_localization_changes)
                    .unsorted_changes
                    .clone();

                let new_build_configuration = if platform == Platform::Android {
                    match get_android_build_configuration(new_tag).await {
                        Ok(config) => Some(config),
                        Err(e) => {
                            tracing::error!("couldn't get new build configuration: {e:?}");
                            None
                        }
                    }
                } else {
                    None
                };

                let post = markdown::Post::new(
                    platform,
                    old_tag,
                    new_tag,
                    new_build_configuration,
                    commits,
                    unfiltered_commits_len,
                    LocalizationChangeCollection {
                        build_changes: build_localization_changes,
                        release_changes: release_localization_changes,
                    },
                );

                let outcome = post
                    .post(
                        env.is_dry_run()?,
                        &discourse_api_key,
                        new_topic_id,
                        reply_to_post_number,
                    )
                    .await
                    .context("could not post commits to Discourse")?;

                tracing::info!(?outcome, "posted to Discourse");

                let final_state = {
                    let mut new_state = state::PlatformState {
                        last_posted_tag_previous_release: last_posted_tag_previous_release.clone(),
                        last_posted_tag: new_tag.clone(),
                        last_post: None,
                        posted_archiving_message: false,
                        localization_changes_completeness,
                        localization_changes,
                        pending_state: None,
                    };

                    match outcome {
                        PostingOutcome::Posted { id, number } => {
                            new_state.last_post = Some(PostInformation { id, number });
                            new_state
                        }
                        PostingOutcome::Enqueued if !same_release => {
                            tracing::warn!("verifying post approval when the post is in a new topic is not implemented; assuming it is already approved, but with an unknown post ID and number");
                            new_state
                        }
                        PostingOutcome::Enqueued => {
                            let mut final_state = state_controller.platform_state(platform).clone();
                            final_state.pending_state = Some(Box::new(new_state));
                            final_state
                        }
                    }
                };

                state_controller
                    .set_platform_state(platform, final_state)
                    .await
                    .context("could not set platform state after posting commits")?;

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
        tracing::trace!("archiving message not necessary");
        return Ok(());
    } else {
        tracing::trace!("attempting to post archiving message");
    }

    let old_topic_id =
        discourse::get_topic_id_or_override(env, discourse_api_key, platform, old_version)
            .await
            .context("could not find old_topic_id")?;

    match old_topic_id {
        Some(old_topic_id) => {
            tracing::debug!(old_topic_id);

            let markdown_text = discourse::archiving_post_markdown(new_topic_id);
            tracing::debug!(markdown_text.len = markdown_text.len());

            let result = if !env.is_dry_run()? {
                discourse::post(
                    &markdown_text,
                    discourse_api_key,
                    old_topic_id,
                    state_controller
                        .platform_state(platform)
                        .last_post
                        .as_ref()
                        .map(|post| post.number),
                )
                .await
            } else {
                tracing::warn!("dry run; not posting to Discourse");
                Ok(PostingOutcome::Posted { id: 0, number: 0 })
            };

            match result {
                Ok(outcome) => {
                    tracing::info!(?outcome, "posted archiving message");

                    let mut new_state = state_controller.platform_state(platform).clone();
                    new_state.posted_archiving_message = true;

                    state_controller
                        .set_platform_state(platform, new_state)
                        .await
                        .context("could not set platform state after posting archiving message")?;

                    utils::delay(POSTING_DELAY_MILLISECONDS).await;
                }
                Err(_) => {
                    tracing::warn!("could not post archiving message to old topic, it is likely already archived; ignoring, will not post archiving message for this release");
                }
            }
        }
        None => {
            tracing::warn!("old topic does not exist? ignoring, will not post archiving message for this release");
        }
    }

    Ok(())
}

async fn get_android_build_configuration(new_tag: &Tag) -> anyhow::Result<BuildConfiguration> {
    let file = github::get_file_content(Platform::Android, &new_tag.name, "app/build.gradle")
        .await
        .context("couldn't get app/build.gradle file content")?;

    BuildConfiguration::from_app_build_gradle(&file).context("couldn't parse build configuration")
}
