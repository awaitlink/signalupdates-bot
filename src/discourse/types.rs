use std::collections::HashMap;

use serde_derive::Deserialize;
use serde_json::Value;

#[derive(Deserialize, Debug)]
pub struct PostApiResponse {
    pub post_number: Option<u64>,

    pub action: Option<String>,
    pub pending_post: Option<Post>,
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
    pub id: u64,

    pub topic_id: u64,
    pub post_number: u64,
}

#[derive(Deserialize, Debug)]
pub struct Error {
    pub error_type: String,

    #[serde(flatten)]
    pub other_fields: HashMap<String, Value>,
}
