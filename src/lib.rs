#![feature(array_windows)]

use anyhow::Context;
use strum::IntoEnumIterator;
use worker::{console_error, console_log, event, Env, ScheduleContext, ScheduledEvent};

mod panic_hook;
mod platform;
mod post;
mod state;
mod types;
mod utils;

use platform::Platform;
use state::StateController;

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
    let tags: Vec<types::github::Tag> = utils::get_json_from_url(platform.github_api_tags_url())
        .await
        .context("could not fetch tags from GitHub")?;
    console_log!("tags = {:?}", tags);

    // TODO: assumes the last posted tag can be found on this GitHub API page
    let tags_to_post = tags
        .iter()
        .rev()
        .skip_while(|tag| tag.name != state_controller.platform_state(platform).last_posted_tag)
        .collect::<Vec<_>>();

    console_log!("tags_to_post = {:?}", tags_to_post);

    if tags_to_post.len() <= 1 {
        console_log!("latest version is already posted, finishing");
        return Ok(());
    }

    for [previous_tag, new_tag] in tags_to_post.array_windows() {
        console_log!(
            "looking at [previous_tag: {:?}, new_tag: {:?}]",
            previous_tag,
            new_tag
        );

        let new_version =
            utils::version_from_tag(&new_tag.name).context("could not parse new_version")?;

        if new_version.patch == 0 {
            console_log!("skipping because new_version.patch is 0");
            continue;
        }

        let discourse_api_key = utils::api_key(env);

        let topic_id = utils::get_topic_id(discourse_api_key.clone(), platform, &new_version)
            .await
            .context("could not find topic_id")?;

        match topic_id {
            Some(topic_id) => {
                console_log!("topic_id = {topic_id}");

                let last_posted_version = utils::version_from_tag(
                    &state_controller.platform_state(platform).last_posted_tag,
                )?;

                let reply_to_post_number = if last_posted_version.major == new_version.major
                    && last_posted_version.minor == new_version.minor
                {
                    Some(state_controller.platform_state(platform).last_post_number)
                } else {
                    None
                };
                console_log!("reply_to_post_number = {:?}", reply_to_post_number);

                let comparison: types::github::Comparison = utils::get_json_from_url(
                    platform.github_api_comparison_url(&previous_tag.name, &new_tag.name),
                )
                .await
                .context("could not fetch comparison from GitHub")?;

                console_log!("comparison = {:?}", comparison);

                let commits = comparison
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

                console_log!("commits = {:?}", commits);

                let post = post::Post::new(platform, &previous_tag.name, &new_tag.name, commits);

                console_log!("post.markdown_text() = {:?}", post.markdown_text());

                let post_number = post
                    .post(discourse_api_key.clone(), topic_id, reply_to_post_number)
                    .await
                    .context("could not post to Discourse")?;

                console_log!("posted post_number = {:?}", post_number);

                state_controller
                    .set_platform_state(
                        platform,
                        state::PlatformState::new(&new_tag.name, post_number),
                    )
                    .await
                    .context("could not set state")?;

                console_log!(
                    "saved state to KV: {:?}",
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
