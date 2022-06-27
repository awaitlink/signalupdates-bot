use anyhow::{anyhow, Context};
use semver::Version;
use serde::Serialize;

use super::*;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    pub name: String,
}

impl TryFrom<Tag> for Version {
    type Error = anyhow::Error;

    fn try_from(tag: Tag) -> Result<Self, Self::Error> {
        (&tag).try_into()
    }
}

impl TryFrom<&Tag> for Version {
    type Error = anyhow::Error;

    fn try_from(tag: &Tag) -> Result<Self, Self::Error> {
        lenient_semver::parse(&tag.name)
            .map_err(|e| anyhow!(e.to_string()))
            .context("could not parse version from tag")
    }
}

impl Tag {
    pub fn exact_version_string(&self) -> String {
        self.name.replace('v', "")
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    pub total_commits: usize,
    pub commits: Vec<Commit>,
    pub files: Option<Vec<File>>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub sha: String,
    pub commit: CommitData,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CommitData {
    pub message: String,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct File {
    pub filename: String,
}

#[cfg(test)]
mod tests {
    use test_case::test_case;

    use super::*;

    fn test_version(pre: Option<&str>, build: Option<&str>) -> Version {
        use semver::{BuildMetadata, Prerelease};

        Version {
            major: 1,
            minor: 2,
            patch: 3,
            pre: match pre {
                Some(pre) => Prerelease::new(pre).unwrap(),
                None => Prerelease::EMPTY,
            },
            build: match build {
                Some(build) => BuildMetadata::new(build).unwrap(),
                None => BuildMetadata::EMPTY,
            },
        }
    }

    #[test_case("v1.2.3" => test_version(None, None); "3 digits with v")]
    #[test_case("1.2.3" => test_version(None, None); "3 digits without v")]
    #[test_case("v1.2.3.4" => test_version(None, Some("4")); "4 digits with v")]
    #[test_case("1.2.3.4" => test_version(None, Some("4")); "4 digits without v")]
    #[test_case("v1.2.3-beta.1" => test_version(Some("beta.1"), None); "3 digits beta with v")]
    #[test_case("1.2.3.4-beta" => test_version(Some("beta"), Some("4")); "4 digits beta without v")]
    fn version_from_tag(tag: &str) -> Version {
        Tag {
            name: tag.to_string(),
        }
        .try_into()
        .unwrap()
    }

    #[test]
    fn comparison_deserialization() {
        // Example from https://docs.github.com/en/rest/commits/commits#compare-two-commits
        let input = r#"{
          "url": "https://api.github.com/repos/octocat/Hello-World/compare/master...topic",
          "html_url": "https://github.com/octocat/Hello-World/compare/master...topic",
          "permalink_url": "https://github.com/octocat/Hello-World/compare/octocat:bbcd538c8e72b8c175046e27cc8f907076331401...octocat:0328041d1152db8ae77652d1618a02e57f745f17",
          "diff_url": "https://github.com/octocat/Hello-World/compare/master...topic.diff",
          "patch_url": "https://github.com/octocat/Hello-World/compare/master...topic.patch",
          "base_commit": {
            "url": "https://api.github.com/repos/octocat/Hello-World/commits/6dcb09b5b57875f334f61aebed695e2e4193db5e",
            "sha": "6dcb09b5b57875f334f61aebed695e2e4193db5e",
            "node_id": "MDY6Q29tbWl0NmRjYjA5YjViNTc4NzVmMzM0ZjYxYWViZWQ2OTVlMmU0MTkzZGI1ZQ==",
            "html_url": "https://github.com/octocat/Hello-World/commit/6dcb09b5b57875f334f61aebed695e2e4193db5e",
            "comments_url": "https://api.github.com/repos/octocat/Hello-World/commits/6dcb09b5b57875f334f61aebed695e2e4193db5e/comments",
            "commit": {
              "url": "https://api.github.com/repos/octocat/Hello-World/git/commits/6dcb09b5b57875f334f61aebed695e2e4193db5e",
              "author": {
                "name": "Monalisa Octocat",
                "email": "mona@github.com",
                "date": "2011-04-14T16:00:49Z"
              },
              "committer": {
                "name": "Monalisa Octocat",
                "email": "mona@github.com",
                "date": "2011-04-14T16:00:49Z"
              },
              "message": "Fix all the bugs",
              "tree": {
                "url": "https://api.github.com/repos/octocat/Hello-World/tree/6dcb09b5b57875f334f61aebed695e2e4193db5e",
                "sha": "6dcb09b5b57875f334f61aebed695e2e4193db5e"
              },
              "comment_count": 0,
              "verification": {
                "verified": false,
                "reason": "unsigned",
                "signature": null,
                "payload": null
              }
            },
            "author": {
              "login": "octocat",
              "id": 1,
              "node_id": "MDQ6VXNlcjE=",
              "avatar_url": "https://github.com/images/error/octocat_happy.gif",
              "gravatar_id": "",
              "url": "https://api.github.com/users/octocat",
              "html_url": "https://github.com/octocat",
              "followers_url": "https://api.github.com/users/octocat/followers",
              "following_url": "https://api.github.com/users/octocat/following{/other_user}",
              "gists_url": "https://api.github.com/users/octocat/gists{/gist_id}",
              "starred_url": "https://api.github.com/users/octocat/starred{/owner}{/repo}",
              "subscriptions_url": "https://api.github.com/users/octocat/subscriptions",
              "organizations_url": "https://api.github.com/users/octocat/orgs",
              "repos_url": "https://api.github.com/users/octocat/repos",
              "events_url": "https://api.github.com/users/octocat/events{/privacy}",
              "received_events_url": "https://api.github.com/users/octocat/received_events",
              "type": "User",
              "site_admin": false
            },
            "committer": {
              "login": "octocat",
              "id": 1,
              "node_id": "MDQ6VXNlcjE=",
              "avatar_url": "https://github.com/images/error/octocat_happy.gif",
              "gravatar_id": "",
              "url": "https://api.github.com/users/octocat",
              "html_url": "https://github.com/octocat",
              "followers_url": "https://api.github.com/users/octocat/followers",
              "following_url": "https://api.github.com/users/octocat/following{/other_user}",
              "gists_url": "https://api.github.com/users/octocat/gists{/gist_id}",
              "starred_url": "https://api.github.com/users/octocat/starred{/owner}{/repo}",
              "subscriptions_url": "https://api.github.com/users/octocat/subscriptions",
              "organizations_url": "https://api.github.com/users/octocat/orgs",
              "repos_url": "https://api.github.com/users/octocat/repos",
              "events_url": "https://api.github.com/users/octocat/events{/privacy}",
              "received_events_url": "https://api.github.com/users/octocat/received_events",
              "type": "User",
              "site_admin": false
            },
            "parents": [
              {
                "url": "https://api.github.com/repos/octocat/Hello-World/commits/6dcb09b5b57875f334f61aebed695e2e4193db5e",
                "sha": "6dcb09b5b57875f334f61aebed695e2e4193db5e"
              }
            ]
          },
          "merge_base_commit": {
            "url": "https://api.github.com/repos/octocat/Hello-World/commits/6dcb09b5b57875f334f61aebed695e2e4193db5e",
            "sha": "6dcb09b5b57875f334f61aebed695e2e4193db5e",
            "node_id": "MDY6Q29tbWl0NmRjYjA5YjViNTc4NzVmMzM0ZjYxYWViZWQ2OTVlMmU0MTkzZGI1ZQ==",
            "html_url": "https://github.com/octocat/Hello-World/commit/6dcb09b5b57875f334f61aebed695e2e4193db5e",
            "comments_url": "https://api.github.com/repos/octocat/Hello-World/commits/6dcb09b5b57875f334f61aebed695e2e4193db5e/comments",
            "commit": {
              "url": "https://api.github.com/repos/octocat/Hello-World/git/commits/6dcb09b5b57875f334f61aebed695e2e4193db5e",
              "author": {
                "name": "Monalisa Octocat",
                "email": "mona@github.com",
                "date": "2011-04-14T16:00:49Z"
              },
              "committer": {
                "name": "Monalisa Octocat",
                "email": "mona@github.com",
                "date": "2011-04-14T16:00:49Z"
              },
              "message": "Fix all the bugs",
              "tree": {
                "url": "https://api.github.com/repos/octocat/Hello-World/tree/6dcb09b5b57875f334f61aebed695e2e4193db5e",
                "sha": "6dcb09b5b57875f334f61aebed695e2e4193db5e"
              },
              "comment_count": 0,
              "verification": {
                "verified": false,
                "reason": "unsigned",
                "signature": null,
                "payload": null
              }
            },
            "author": {
              "login": "octocat",
              "id": 1,
              "node_id": "MDQ6VXNlcjE=",
              "avatar_url": "https://github.com/images/error/octocat_happy.gif",
              "gravatar_id": "",
              "url": "https://api.github.com/users/octocat",
              "html_url": "https://github.com/octocat",
              "followers_url": "https://api.github.com/users/octocat/followers",
              "following_url": "https://api.github.com/users/octocat/following{/other_user}",
              "gists_url": "https://api.github.com/users/octocat/gists{/gist_id}",
              "starred_url": "https://api.github.com/users/octocat/starred{/owner}{/repo}",
              "subscriptions_url": "https://api.github.com/users/octocat/subscriptions",
              "organizations_url": "https://api.github.com/users/octocat/orgs",
              "repos_url": "https://api.github.com/users/octocat/repos",
              "events_url": "https://api.github.com/users/octocat/events{/privacy}",
              "received_events_url": "https://api.github.com/users/octocat/received_events",
              "type": "User",
              "site_admin": false
            },
            "committer": {
              "login": "octocat",
              "id": 1,
              "node_id": "MDQ6VXNlcjE=",
              "avatar_url": "https://github.com/images/error/octocat_happy.gif",
              "gravatar_id": "",
              "url": "https://api.github.com/users/octocat",
              "html_url": "https://github.com/octocat",
              "followers_url": "https://api.github.com/users/octocat/followers",
              "following_url": "https://api.github.com/users/octocat/following{/other_user}",
              "gists_url": "https://api.github.com/users/octocat/gists{/gist_id}",
              "starred_url": "https://api.github.com/users/octocat/starred{/owner}{/repo}",
              "subscriptions_url": "https://api.github.com/users/octocat/subscriptions",
              "organizations_url": "https://api.github.com/users/octocat/orgs",
              "repos_url": "https://api.github.com/users/octocat/repos",
              "events_url": "https://api.github.com/users/octocat/events{/privacy}",
              "received_events_url": "https://api.github.com/users/octocat/received_events",
              "type": "User",
              "site_admin": false
            },
            "parents": [
              {
                "url": "https://api.github.com/repos/octocat/Hello-World/commits/6dcb09b5b57875f334f61aebed695e2e4193db5e",
                "sha": "6dcb09b5b57875f334f61aebed695e2e4193db5e"
              }
            ]
          },
          "status": "behind",
          "ahead_by": 1,
          "behind_by": 2,
          "total_commits": 1,
          "commits": [
            {
              "url": "https://api.github.com/repos/octocat/Hello-World/commits/6dcb09b5b57875f334f61aebed695e2e4193db5e",
              "sha": "6dcb09b5b57875f334f61aebed695e2e4193db5e",
              "node_id": "MDY6Q29tbWl0NmRjYjA5YjViNTc4NzVmMzM0ZjYxYWViZWQ2OTVlMmU0MTkzZGI1ZQ==",
              "html_url": "https://github.com/octocat/Hello-World/commit/6dcb09b5b57875f334f61aebed695e2e4193db5e",
              "comments_url": "https://api.github.com/repos/octocat/Hello-World/commits/6dcb09b5b57875f334f61aebed695e2e4193db5e/comments",
              "commit": {
                "url": "https://api.github.com/repos/octocat/Hello-World/git/commits/6dcb09b5b57875f334f61aebed695e2e4193db5e",
                "author": {
                  "name": "Monalisa Octocat",
                  "email": "mona@github.com",
                  "date": "2011-04-14T16:00:49Z"
                },
                "committer": {
                  "name": "Monalisa Octocat",
                  "email": "mona@github.com",
                  "date": "2011-04-14T16:00:49Z"
                },
                "message": "Fix all the bugs",
                "tree": {
                  "url": "https://api.github.com/repos/octocat/Hello-World/tree/6dcb09b5b57875f334f61aebed695e2e4193db5e",
                  "sha": "6dcb09b5b57875f334f61aebed695e2e4193db5e"
                },
                "comment_count": 0,
                "verification": {
                  "verified": false,
                  "reason": "unsigned",
                  "signature": null,
                  "payload": null
                }
              },
              "author": {
                "login": "octocat",
                "id": 1,
                "node_id": "MDQ6VXNlcjE=",
                "avatar_url": "https://github.com/images/error/octocat_happy.gif",
                "gravatar_id": "",
                "url": "https://api.github.com/users/octocat",
                "html_url": "https://github.com/octocat",
                "followers_url": "https://api.github.com/users/octocat/followers",
                "following_url": "https://api.github.com/users/octocat/following{/other_user}",
                "gists_url": "https://api.github.com/users/octocat/gists{/gist_id}",
                "starred_url": "https://api.github.com/users/octocat/starred{/owner}{/repo}",
                "subscriptions_url": "https://api.github.com/users/octocat/subscriptions",
                "organizations_url": "https://api.github.com/users/octocat/orgs",
                "repos_url": "https://api.github.com/users/octocat/repos",
                "events_url": "https://api.github.com/users/octocat/events{/privacy}",
                "received_events_url": "https://api.github.com/users/octocat/received_events",
                "type": "User",
                "site_admin": false
              },
              "committer": {
                "login": "octocat",
                "id": 1,
                "node_id": "MDQ6VXNlcjE=",
                "avatar_url": "https://github.com/images/error/octocat_happy.gif",
                "gravatar_id": "",
                "url": "https://api.github.com/users/octocat",
                "html_url": "https://github.com/octocat",
                "followers_url": "https://api.github.com/users/octocat/followers",
                "following_url": "https://api.github.com/users/octocat/following{/other_user}",
                "gists_url": "https://api.github.com/users/octocat/gists{/gist_id}",
                "starred_url": "https://api.github.com/users/octocat/starred{/owner}{/repo}",
                "subscriptions_url": "https://api.github.com/users/octocat/subscriptions",
                "organizations_url": "https://api.github.com/users/octocat/orgs",
                "repos_url": "https://api.github.com/users/octocat/repos",
                "events_url": "https://api.github.com/users/octocat/events{/privacy}",
                "received_events_url": "https://api.github.com/users/octocat/received_events",
                "type": "User",
                "site_admin": false
              },
              "parents": [
                {
                  "url": "https://api.github.com/repos/octocat/Hello-World/commits/6dcb09b5b57875f334f61aebed695e2e4193db5e",
                  "sha": "6dcb09b5b57875f334f61aebed695e2e4193db5e"
                }
              ]
            }
          ],
          "files": [
            {
              "sha": "bbcd538c8e72b8c175046e27cc8f907076331401",
              "filename": "file1.txt",
              "status": "added",
              "additions": 103,
              "deletions": 21,
              "changes": 124,
              "blob_url": "https://github.com/octocat/Hello-World/blob/6dcb09b5b57875f334f61aebed695e2e4193db5e/file1.txt",
              "raw_url": "https://github.com/octocat/Hello-World/raw/6dcb09b5b57875f334f61aebed695e2e4193db5e/file1.txt",
              "contents_url": "https://api.github.com/repos/octocat/Hello-World/contents/file1.txt?ref=6dcb09b5b57875f334f61aebed695e2e4193db5e",
              "patch": "@@ -132,7 +132,7 @@ module Test @@ -1000,7 +1000,7 @@ module Test"
            }
          ]
        }"#;

        assert_eq!(
            serde_json::from_str::<Comparison>(input).unwrap(),
            Comparison {
                total_commits: 1,
                commits: vec![Commit {
                    sha: "6dcb09b5b57875f334f61aebed695e2e4193db5e".to_string(),
                    commit: CommitData {
                        message: "Fix all the bugs".to_string()
                    }
                }],
                files: Some(vec![File {
                    filename: "file1.txt".to_string()
                }])
            }
        );
    }
}
