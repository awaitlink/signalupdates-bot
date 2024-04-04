use serde_derive::Deserialize;
use serde_json::Value;

#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(untagged)]
pub enum ApiResponse<T> {
    Ok(T),
    Err(Error),
    Unknown(Value),
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(tag = "error_type")]
pub enum Error {
    #[serde(rename = "not_found")]
    NotFound,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct Topic {
    pub post_stream: PostStream,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct PostStream {
    pub posts: Vec<Post>,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct WrappedPost {
    pub post: Post,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct Post {
    pub id: u64,

    pub topic_id: u64,
    pub post_number: u64,

    pub user_id: u64,

    pub raw: String,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(untagged)]
pub enum CreatePostResponse {
    Posted(Post),
    Action(PostAction),
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(tag = "action")]
pub enum PostAction {
    #[serde(rename = "enqueued")]
    Enqueued { pending_post: PendingPost },
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct PendingPost {
    #[serde(rename = "id")]
    pub reviewable_id: u64,
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::*;

    #[test]
    fn api_response_unknown_error_deserialization() {
        let input = json! {{
            "errors": ["Something went wrong."],
            "error_type": "oops"
        }};

        assert_eq!(
            serde_json::from_str::<ApiResponse<CreatePostResponse>>(&input.to_string()).unwrap(),
            ApiResponse::Unknown(Value::Object({
                let mut map = serde_json::Map::new();

                map.insert(
                    "errors".to_string(),
                    Value::Array(vec![Value::String("Something went wrong.".to_string())]),
                );
                map.insert("error_type".to_string(), Value::String("oops".to_string()));

                map
            }))
        );
    }

    #[test]
    fn api_response_error_deserialization() {
        let input = json! {{
            "errors": ["The requested URL or resource could not be found."],
            "error_type": "not_found"
        }};

        assert_eq!(
            serde_json::from_str::<ApiResponse<CreatePostResponse>>(&input.to_string()).unwrap(),
            ApiResponse::Err(Error::NotFound)
        );
    }

    #[test]
    fn api_response_ok_post_deserialization() {
        let input = json! {{
            "id": 0,
            "topic_id": 0,
            "post_number": 0,
            "user_id": 0,
            "raw": "content",
        }};

        assert_eq!(
            serde_json::from_str::<ApiResponse<CreatePostResponse>>(&input.to_string()).unwrap(),
            ApiResponse::Ok(CreatePostResponse::Posted(Post {
                id: 0,
                topic_id: 0,
                post_number: 0,
                user_id: 0,
                raw: String::from("content"),
            }))
        );
    }

    #[test]
    fn api_response_ok_action_deserialization() {
        let input = json! {{
            "action": "enqueued",
            "pending_post": {
                "id": 0,
            },
        }};

        assert_eq!(
            serde_json::from_str::<ApiResponse<CreatePostResponse>>(&input.to_string()).unwrap(),
            ApiResponse::Ok(CreatePostResponse::Action(PostAction::Enqueued {
                pending_post: PendingPost { reviewable_id: 0 }
            }))
        );
    }
}
