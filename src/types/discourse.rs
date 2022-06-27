use std::collections::HashMap;

use serde_json::Value;

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
