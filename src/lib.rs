#![feature(array_windows)]

use anyhow::{anyhow, Context};
use semver::Version;
use strum::IntoEnumIterator;
use worker::{console_error, console_log, event, Env, ScheduleContext, ScheduledEvent};

mod language;
mod localization_change;
mod panic_hook;
mod platform;
mod post;
mod state;
mod types;
mod utils;

use localization_change::LocalizationChangeCollection;
use platform::Platform;
use state::StateController;
use utils::GitHubComparisonKind::*;

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
        check_platform(&mut state_controller, env, platform).await?
    }

    Ok(())
}

async fn check_platform(
    state_controller: &mut StateController,
    env: &Env,
    platform: Platform,
) -> anyhow::Result<()> {
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
        .skip_while(|(tag, _)| {
            tag.name != state_controller.platform_state(platform).last_posted_tag
        })
        .cloned()
        .collect();

    console_log!("tags_to_post = {:?}", tags_to_post);

    if tags_to_post.len() <= 1 {
        console_log!("latest version is already posted, finishing");
        return Ok(());
    }

    for [(old_tag, old_version), (new_tag, new_version)] in tags_to_post.array_windows() {
        console_log!(
            "looking at [old_tag: {:?}, new_tag: {:?}]",
            old_tag,
            new_tag
        );

        let discourse_api_key = utils::api_key(env)?;

        let topic_id = match utils::topic_id_override(env)? {
            Some(id) => {
                console_log!("using topic id override: {id}");
                Some(id)
            }
            None => utils::get_topic_id(discourse_api_key.clone(), platform, new_version)
                .await
                .context("could not find topic_id")?,
        };

        match topic_id {
            Some(topic_id) => {
                console_log!("topic_id = {topic_id}");

                let last_posted_version = utils::version_from_tag(
                    &state_controller.platform_state(platform).last_posted_tag,
                )?;

                let reply_to_post_number = if last_posted_version.major == new_version.major
                    && last_posted_version.minor == new_version.minor
                {
                    state_controller.platform_state(platform).last_post_number
                } else {
                    None
                };
                console_log!("reply_to_post_number = {:?}", reply_to_post_number);

                let comparison =
                    utils::get_github_comparison(Full, platform, &old_tag.name, &new_tag.name)
                        .await
                        .context("could not get build comparison from GitHub")?;

                console_log!("comparison = {:?}", comparison);

                let commits: Vec<post::Commit> = comparison
                    .commits
                    .iter()
                    .map(|github_commit| {
                        post::Commit::new(
                            platform,
                            &github_commit.commit.message,
                            &github_commit.sha,
                        )
                    })
                    .collect();

                console_log!("commits.len() = {:?}", commits.len());

                let build_localization_changes =
                    utils::localization_changes_from_comparison(platform, &comparison);

                console_log!(
                    "build_localization_changes.len() = {:?}",
                    build_localization_changes.len()
                );

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

                let mut is_release_complete = true;
                let release_localization_changes = if &last_version_of_previous_release.1
                    == old_version
                {
                    None
                } else {
                    let release_comparison = utils::get_github_comparison(
                        JustAllFiles,
                        platform,
                        &last_version_of_previous_release.0.name,
                        &new_tag.name,
                    )
                    .await
                    .context("could not get release comparison from GitHub")?;

                    console_log!("release_comparison = {:?}", release_comparison);

                    let mut release_localization_changes =
                        utils::localization_changes_from_comparison(platform, &release_comparison);

                    console_log!(
                        "release_localization_changes.len() = {:?}",
                        release_localization_changes.len()
                    );

                    // GitHub API only returns at most 300 files, despite
                    // https://docs.github.com/en/rest/commits/commits#compare-two-commits
                    // saying that it always returns all.
                    let combined_localization_changes = if release_comparison.files.unwrap().len()
                        == 300
                    {
                        console_log!("release_comparison has 300 files, likely incomplete");
                        is_release_complete = false;

                        if !release_localization_changes.is_empty() {
                            console_log!(
                                "merging release_localization_changes and build_localization_changes"
                            );

                            let mut combined_localization_changes =
                                build_localization_changes.clone();
                            combined_localization_changes.append(&mut release_localization_changes);
                            combined_localization_changes.dedup();
                            combined_localization_changes.sort_unstable();
                            combined_localization_changes
                        } else {
                            console_log!(
                                "release_localization_changes is empty, taking build_localization_changes"
                            );

                            build_localization_changes.clone()
                        }
                    } else {
                        console_log!("release_comparison appears to be complete");
                        release_localization_changes
                    };

                    console_log!(
                        "combined_localization_changes.len() = {:?}",
                        combined_localization_changes.len()
                    );

                    Some((
                        last_version_of_previous_release.0.name.clone(),
                        combined_localization_changes,
                    ))
                };

                let localization_change_collection = LocalizationChangeCollection {
                    build_localization_changes,
                    release_localization_changes,
                    is_release_complete,
                };

                let post = post::Post::new(
                    platform,
                    &old_tag.name,
                    &new_tag.name,
                    commits,
                    localization_change_collection,
                );

                let post_number = post
                    .post(discourse_api_key.clone(), topic_id, reply_to_post_number)
                    .await
                    .context("could not post to Discourse")?;

                console_log!("posted post_number = {:?}", post_number);

                state_controller
                    .set_platform_state(
                        platform,
                        state::PlatformState::new(&new_tag.name, Some(post_number)),
                    )
                    .await
                    .context("could not set state")?;

                console_log!(
                    "saved platform state to KV: {:?}",
                    state_controller.platform_state(platform)
                );
            }
            None => {
                console_log!("no topic found, may be not created yet; not trying more tags");
                break;
            }
        }

        if tags_to_post.len() >= 3 {
            console_log!("currently doing only one post per invocation, exiting loop");
            break;
        }
    }

    Ok(())
}
