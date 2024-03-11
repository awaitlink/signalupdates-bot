use anyhow::{anyhow, bail, Context};
use semver::Version;
use serde::de::DeserializeOwned;
use worker::{Fetch, Method, Url};

use crate::{
    network::{self, ContentType},
    platform::Platform,
};

mod types;
pub use types::*;

pub async fn get_tags_to_post(
    last_posted_tag: Tag,
    platform: Platform,
) -> anyhow::Result<Vec<(Tag, Version)>> {
    tracing::debug!(?last_posted_tag, %platform, "getting tags for platform from GitHub until last_posted_tag is found");

    let enough_tags = get_paginated_response(
        &platform.github_api_tags_url(),
        vec![],
        |target, source| target.append(source),
        |result| result.contains(&last_posted_tag),
    )
    .await
    .context("could not fetch tags from GitHub")?;

    tracing::trace!(?enough_tags);

    let mut tags: Vec<(Tag, Version)> = enough_tags
        .iter()
        .filter_map(|tag| tag.to_version().ok().map(|version| (tag.clone(), version)))
        .filter(|(_, version)| platform.should_post_version(version))
        .collect();

    tracing::trace!(?tags);

    tags.sort_unstable_by(|(_, lhs), (_, rhs)| lhs.cmp(rhs));
    tracing::trace!(?tags, "after sorting");

    let tags_to_post: Vec<(Tag, Version)> = tags
        .iter()
        .skip_while(|(tag, _)| *tag != last_posted_tag)
        .cloned()
        .collect();

    tracing::debug!(?tags_to_post);
    Ok(tags_to_post)
}

pub async fn get_comparison(
    platform: Platform,
    old_tag: &str,
    new_tag: &str,
) -> anyhow::Result<Comparison> {
    tracing::debug!(
        old_tag, new_tag, %platform,
        "getting comparison between old_tag and new_tag for platform from GitHub"
    );

    let initial_url = platform.github_api_comparison_url(old_tag, new_tag);

    let comparison = get_paginated_response(
        &initial_url,
        Comparison {
            total_commits: 0,
            commits: Vec::new(),
            files: Some(Vec::new()),
        },
        |target, source| {
            target.total_commits = source.total_commits; // always the total number of commits
            target.commits.append(&mut source.commits);
            if let Some(part_files) = &mut source.files {
                target.files.as_mut().unwrap().append(part_files);
            }
        },
        |_| false,
    )
    .await?;

    if comparison.total_commits != comparison.commits.len() {
        bail!(
            "incomplete full comparison: total_commits = {} but commits.len() = {}, commits = {:?}",
            comparison.total_commits,
            comparison.commits.len(),
            comparison.commits
        )
    };

    Ok(comparison)
}

pub async fn get_commit(platform: Platform, sha: &str) -> anyhow::Result<Commit> {
    tracing::debug!(commit.sha = sha, %platform, "getting commit for platform from GitHub");

    let initial_url = platform.github_api_commit_url(sha);

    let commit = get_paginated_response(
        &initial_url,
        Commit {
            sha: sha.to_string(),
            commit: CommitData {
                message: String::new(),
            },
            files: Some(Vec::new()),
        },
        |target, source| {
            target.sha.clone_from(&source.sha);
            target.commit = source.commit.clone();
            if let Some(part_files) = &mut source.files {
                target.files.as_mut().unwrap().append(part_files);
            }
        },
        |_| false,
    )
    .await?;

    Ok(commit)
}

/// `merge`: `Fn(&mut target, &mut source)`
async fn get_paginated_response<T, F, P>(
    initial_url: &str,
    initial_result: T,
    merge: F,
    stop_if_result: P,
) -> anyhow::Result<T>
where
    T: DeserializeOwned,
    F: Fn(&mut T, &mut T),
    P: Fn(&T) -> bool,
{
    tracing::trace!("getting paginated response from GitHub");

    let mut page = 1;
    let per_page = 100;

    let mut url_string = format!("{initial_url}?page={page}&per_page={per_page}");

    let mut result: T = initial_result;

    loop {
        tracing::trace!(page, url = url_string, "getting page");

        let url = Url::parse(&url_string).context("could not parse URL")?;
        let request = network::create_request(
            url,
            Method::Get,
            ContentType::ApplicationJson,
            ContentType::ApplicationJson,
            None,
            None,
        )?;

        let mut response = network::fetch(Fetch::Request(request))
            .await
            .context("could not fetch from GitHub")?;

        let mut part: T = network::json_from_response(&mut response)
            .await
            .context("could not get JSON for part")?;

        merge(&mut result, &mut part);

        if stop_if_result(&result) {
            tracing::debug!(
                "provided stop condition matched, stopping getting paginating response"
            );
            break;
        }

        let link_header_string = match response.headers().get("Link").unwrap() {
            Some(header_string) => header_string,
            None => {
                tracing::warn!(
                    "no `Link` header in GitHub's response, likely done getting paginated response"
                );
                break;
            }
        };

        let link_header = parse_link_header::parse_with_rel(&link_header_string)
            .context("could not parse `Link` header")?;

        match link_header.get("next") {
            Some(link) => {
                url_string.clone_from(&link.raw_uri);
                page += 1;
            }
            None => {
                tracing::debug!("no `next` link, done getting full response");
                break;
            }
        }
    }

    Ok(result)
}

pub async fn get_file_content(
    platform: Platform,
    revision: &str,
    path: &str,
) -> anyhow::Result<String> {
    tracing::trace!(?platform, revision, path, "getting file content");

    let url = platform.github_raw_url(revision) + "/" + path;
    let url = Url::parse(&url).context("could not parse URL")?;

    let request = network::create_request(
        url,
        Method::Get,
        ContentType::ApplicationJson,
        ContentType::TextPlain,
        None,
        None,
    )?;

    let mut response = network::fetch(Fetch::Request(request))
        .await
        .context("could not fetch from GitHub")?;

    response
        .text()
        .await
        .map_err(|e| anyhow!(e.to_string()))
        .context("couldn't get text")
}
