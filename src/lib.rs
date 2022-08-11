#![feature(array_windows)]

use anyhow::Context;
use chrono::prelude::*;
use semver::Version;
use strum::IntoEnumIterator;
use worker::{
    console_error, console_log, console_warn, event, Env, ScheduleContext, ScheduledEvent,
};

mod discourse;
mod github;
mod localization;
mod markdown;
mod panic_hook;
mod platform;
mod state;
mod utils;

use discourse::PostingOutcome;
use localization::{
    Completeness, LocalizationChange, LocalizationChangeCollection, LocalizationChanges,
};
use platform::Platform;
use state::{PendingState, StateController};

const POSTING_DELAY_MILLISECONDS: u64 = 3000;

enum PlatformCheckOutcome {
    WaitingForApproval,
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
    let now = DateTime::from(utils::now());
    console_log!(
        "now = {} (nanos: {})",
        now.to_rfc3339(),
        now.timestamp_nanos()
    );

    let platforms = Platform::iter().collect::<Vec<_>>();
    let index = (now.minute() / 10)
        .try_into()
        .context("should be able to convert to usize")?;
    let platforms = permute::permutations_of(&platforms)
        .nth(index)
        .context("there should be >= 6 permutations")?;

    let mut state_controller = state::StateController::from_kv(env).await?;

    for &platform in platforms {
        let outcome = check_platform(&mut state_controller, env, platform).await?;

        match outcome {
            WaitingForApproval => console_warn!("outcome: waiting for post approval"),
            LatestVersionIsAlreadyPosted => {
                console_log!("outcome: latest version is already posted")
            }
            NewTopicNotFound => console_warn!("outcome: no topic found, may be not created yet"),
            PostedCommits => {
                console_warn!("outcome: already posted for {platform} and currently doing only one \"commits\" post per invocation, done");
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

    // Check if a pending post exists and has been approved
    if let Some(PendingState {
        post_id,
        platform_state,
    }) = state_controller
        .platform_state(platform)
        .pending_state
        .as_deref()
    {
        console_log!(
            "post id = {post_id} is waiting for approval, checking if there is a post number"
        );

        if let Some(number) = discourse::get_post_number(*post_id).await? {
            console_log!("post number = {number}, approval confirmed");

            let mut new_state = platform_state.clone();
            new_state.last_post_number = Some(number);

            state_controller
                .set_platform_state(platform, new_state)
                .await
                .context("could not set platform state after confirming post approval")?;

            console_log!("continuing main logic");
        } else {
            return Ok(WaitingForApproval);
        }
    } else {
        console_log!("no post waiting for approval, continuing main logic");
    }

    let all_tags: Vec<github::Tag> = utils::get_json_from_url(&platform.github_api_tags_url())
        .await
        .context("could not fetch tags from GitHub")?;

    console_log!("all_tags = {:?}", all_tags);

    let mut tags: Vec<(github::Tag, Version)> = all_tags
        .iter()
        .filter_map(|tag| tag.to_version().ok().map(|version| (tag.clone(), version)))
        .filter(|(_, version)| platform.should_post_version(version))
        .collect();

    console_log!("tags = {:?}", tags);

    tags.sort_unstable_by(|(_, lhs), (_, rhs)| lhs.cmp(rhs));
    console_log!("after sorting, tags = {:?}", tags);

    // TODO: assumes the last posted tag can be found on this GitHub API page
    let tags_to_post: Vec<(github::Tag, Version)> = tags
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
            discourse::get_topic_id_or_override(env, &discourse_api_key, platform, new_version)
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

                let comparison = github::get_comparison(platform, &old_tag.name, &new_tag.name)
                    .await
                    .context("could not get build comparison from GitHub")?;

                console_log!("comparison = {:?}", comparison);

                let unfiltered_commits: Vec<markdown::Commit> = comparison
                    .commits
                    .iter()
                    .map(|github_commit| {
                        markdown::Commit::from_github_commit(platform, github_commit)
                    })
                    .collect();

                let unfiltered_commits_len = unfiltered_commits.len();
                console_log!("unfiltered_commits_len = {:?}", unfiltered_commits_len);

                let commits: Vec<markdown::Commit> = unfiltered_commits
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
                            let with_files = github::get_commit(platform, commit.sha()).await?;

                            console_log!("with_files = {:?}", with_files);

                            let mut changes = LocalizationChange::unsorted_changes_from_files(
                                platform,
                                &with_files.files,
                            );

                            console_log!("changes = {:?}", changes);

                            build_localization_changes.add_unsorted_changes(&mut changes);

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

                console_log!(
                    "last_posted_tag_previous_release = {:?}, release_localization_changes = {:?}",
                    last_posted_tag_previous_release,
                    release_localization_changes,
                );

                let localization_changes = release_localization_changes
                    .as_ref()
                    .unwrap_or(&build_localization_changes)
                    .unsorted_changes
                    .clone();

                let post = markdown::Post::new(
                    platform,
                    old_tag,
                    new_tag,
                    commits,
                    unfiltered_commits_len,
                    LocalizationChangeCollection {
                        build_changes: build_localization_changes,
                        release_changes: release_localization_changes,
                    },
                );

                let outcome = post
                    .post(
                        utils::is_dry_run(env)?,
                        &discourse_api_key,
                        new_topic_id,
                        reply_to_post_number,
                    )
                    .await
                    .context("could not post commits to Discourse")?;

                console_log!("posting outcome = {outcome:?}");

                let final_state = {
                    let mut new_state = state::PlatformState {
                        last_posted_tag_previous_release: last_posted_tag_previous_release.clone(),
                        last_posted_tag: new_tag.clone(),
                        last_post_number: None,
                        posted_archiving_message: false,
                        localization_changes_completeness,
                        localization_changes,
                        pending_state: None,
                    };

                    match outcome {
                        PostingOutcome::Posted { number } => {
                            new_state.last_post_number = Some(number);
                            new_state
                        }
                        PostingOutcome::Enqueued { id } => {
                            let mut final_state = state_controller.platform_state(platform).clone();

                            final_state.pending_state = Some(Box::new(PendingState {
                                post_id: id,
                                platform_state: new_state,
                            }));

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
        console_log!("archiving message not necessary");
        return Ok(());
    } else {
        console_log!("attempting to post archiving message");
    }

    let old_topic_id =
        discourse::get_topic_id_or_override(env, discourse_api_key, platform, old_version)
            .await
            .context("could not find old_topic_id")?;

    match old_topic_id {
        Some(old_topic_id) => {
            console_log!("old_topic_id = {old_topic_id}");

            let markdown_text = discourse::archiving_post_markdown(new_topic_id);
            console_log!("markdown_text.len() = {}", markdown_text.len());

            let result = if !utils::is_dry_run(env)? {
                discourse::post(
                    &markdown_text,
                    discourse_api_key,
                    old_topic_id,
                    state_controller.platform_state(platform).last_post_number,
                )
                .await
            } else {
                console_warn!("dry run; not posting to Discourse");
                Ok(PostingOutcome::Posted { number: 0 })
            };

            match result {
                Ok(outcome) => {
                    console_log!("posted archiving message, outcome = {outcome:?}");

                    let mut new_state = state_controller.platform_state(platform).clone();
                    new_state.posted_archiving_message = true;

                    state_controller
                        .set_platform_state(platform, new_state)
                        .await
                        .context("could not set platform state after posting archiving message")?;

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
