use serde_derive::Deserialize;

pub mod github {
    use super::*;

    #[derive(Deserialize, Debug, Clone)]
    pub struct Tag {
        pub name: String,
    }

    #[derive(Deserialize, Debug, Clone)]
    pub struct Comparison {
        pub commits: Vec<Commit>,
    }

    #[derive(Deserialize, Debug, Clone)]
    pub struct Commit {
        pub sha: String,
        pub commit: CommitData,
    }

    #[derive(Deserialize, Debug, Clone)]
    pub struct CommitData {
        pub message: String,
    }
}

pub mod discourse {
    use serde_json::Value;
    use std::collections::HashMap;

    use super::*;

    #[derive(Deserialize, Debug)]
    pub struct PostApiResponse {
        pub post_number: Option<u64>,

        #[serde(flatten)]
        pub other_fields: HashMap<String, Value>,
    }

    #[derive(Deserialize, Debug)]
    pub struct TopicResponse {
        pub post_stream: PostStream,
    }

    #[derive(Deserialize, Debug)]
    pub struct PostStream {
        pub posts: Vec<Post>,
    }

    #[derive(Deserialize, Debug)]
    pub struct Post {
        pub topic_id: u64,
    }
}
