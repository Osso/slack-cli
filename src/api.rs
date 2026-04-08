use crate::cache;
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;

pub struct Client {
    http: reqwest::Client,
    token: String,
    base_url: String,
}

impl Client {
    async fn get_users_cached(&self) -> Result<Value> {
        let mut c = cache::load_cache();
        if let Some(users) = cache::get_users(&c) {
            return Ok(users);
        }
        let users = self.list_users().await?;
        cache::set_users(&mut c, users.clone());
        let _ = cache::save_cache(&c);
        Ok(users)
    }

    pub async fn get_channels_cached(&self) -> Result<Value> {
        let mut c = cache::load_cache();
        if let Some(channels) = cache::get_channels(&c) {
            return Ok(channels);
        }
        let channels = self.list_channels().await?;
        cache::set_channels(&mut c, channels.clone());
        let _ = cache::save_cache(&c);
        Ok(channels)
    }
}

impl Client {
    pub fn new(token: &str) -> Result<Self> {
        Self::with_base_url(token, "https://slack.com/api")
    }

    pub fn with_base_url(token: &str, base_url: &str) -> Result<Self> {
        let http = reqwest::Client::builder().build()?;
        Ok(Self {
            http,
            token: token.to_string(),
            base_url: base_url.to_string(),
        })
    }

    async fn get(&self, method: &str, params: &[(&str, &str)]) -> Result<Value> {
        let url = format!("{}/{}", self.base_url, method);
        let resp = self
            .http
            .get(&url)
            .bearer_auth(&self.token)
            .query(params)
            .send()
            .await
            .context("Failed to send request")?;

        let body: Value = resp.json().await.context("Failed to parse response")?;

        if body.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            let error = body
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            anyhow::bail!("Slack API error: {}", error);
        }

        Ok(body)
    }

    async fn post(&self, method: &str, payload: &HashMap<&str, &str>) -> Result<Value> {
        let url = format!("{}/{}", self.base_url, method);
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.token)
            .json(payload)
            .send()
            .await
            .context("Failed to send request")?;

        let body: Value = resp.json().await.context("Failed to parse response")?;

        if body.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            let error = body
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            anyhow::bail!("Slack API error: {}", error);
        }

        Ok(body)
    }

    pub async fn list_channels(&self) -> Result<Value> {
        let mut all_channels = Vec::new();
        let mut cursor = String::new();

        loop {
            let mut params = vec![("types", "public_channel,private_channel")];
            if !cursor.is_empty() {
                params.push(("cursor", &cursor));
            }

            let resp = self.get("conversations.list", &params).await?;

            if let Some(channels) = resp.get("channels").and_then(|c| c.as_array()) {
                all_channels.extend(channels.clone());
            }

            cursor = resp
                .get("response_metadata")
                .and_then(|m| m.get("next_cursor"))
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string();

            if cursor.is_empty() {
                break;
            }
        }

        Ok(serde_json::json!({
            "ok": true,
            "channels": all_channels
        }))
    }

    pub async fn get_channel(&self, channel: &str) -> Result<Value> {
        self.get("conversations.info", &[("channel", channel)])
            .await
    }

    pub async fn get_messages(&self, channel: &str, limit: u32) -> Result<Value> {
        self.get(
            "conversations.history",
            &[("channel", channel), ("limit", &limit.to_string())],
        )
        .await
    }

    pub async fn get_thread(&self, channel: &str, ts: &str, limit: u32) -> Result<Value> {
        self.get(
            "conversations.replies",
            &[
                ("channel", channel),
                ("ts", ts),
                ("limit", &limit.to_string()),
            ],
        )
        .await
    }

    pub async fn send_message(
        &self,
        channel: &str,
        text: &str,
        thread_ts: Option<&str>,
    ) -> Result<Value> {
        let mut payload = HashMap::new();
        payload.insert("channel", channel);
        payload.insert("text", text);
        if let Some(ts) = thread_ts {
            payload.insert("thread_ts", ts);
        }
        self.post("chat.postMessage", &payload).await
    }

    pub async fn list_dms(&self) -> Result<Value> {
        self.get("conversations.list", &[("types", "im,mpim")])
            .await
    }

    pub async fn list_users(&self) -> Result<Value> {
        self.get("users.list", &[]).await
    }

    pub async fn search_messages(&self, query: &str, count: u32) -> Result<Value> {
        let count_str = count.to_string();
        self.get(
            "search.messages",
            &[("query", query), ("count", &count_str)],
        )
        .await
    }

    fn find_member_id<'a>(users: &'a Value, name: &str) -> Option<&'a str> {
        users.get("members")?.as_array()?.iter().find_map(|member| {
            let username = member.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let display_name = member
                .get("profile")
                .and_then(|p| p.get("display_name"))
                .and_then(|d| d.as_str())
                .unwrap_or("");
            if username.eq_ignore_ascii_case(name) || display_name.eq_ignore_ascii_case(name) {
                member.get("id").and_then(|i| i.as_str())
            } else {
                None
            }
        })
    }

    fn find_channel_id<'a>(channels: &'a Value, name: &str) -> Option<&'a str> {
        channels
            .get("channels")?
            .as_array()?
            .iter()
            .find_map(|chan| {
                let chan_name = chan.get("name").and_then(|n| n.as_str()).unwrap_or("");
                if chan_name.eq_ignore_ascii_case(name) {
                    chan.get("id").and_then(|i| i.as_str())
                } else {
                    None
                }
            })
    }

    pub async fn resolve_target(&self, target: &str) -> Result<String> {
        if target.starts_with('U')
            || target.starts_with('C')
            || target.starts_with('D')
            || target.starts_with('G')
        {
            return Ok(target.to_string());
        }

        let name = target.trim_start_matches('@').trim_start_matches('#');

        let users = self.get_users_cached().await?;
        if let Some(id) = Self::find_member_id(&users, name) {
            return Ok(id.to_string());
        }

        let channels = self.get_channels_cached().await?;
        if let Some(id) = Self::find_channel_id(&channels, name) {
            return Ok(id.to_string());
        }

        anyhow::bail!("Could not resolve '{}' to a user or channel", target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{bearer_token, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_list_channels_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/conversations.list"))
            .and(bearer_token("test-token"))
            .and(query_param("types", "public_channel,private_channel"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "ok": true,
                "channels": [
                    {"id": "C123", "name": "general"},
                    {"id": "C456", "name": "random"}
                ]
            })))
            .mount(&mock_server)
            .await;

        let client = Client::with_base_url("test-token", &mock_server.uri()).unwrap();
        let result = client.list_channels().await.unwrap();

        assert_eq!(result["ok"], true);
        assert_eq!(result["channels"][0]["name"], "general");
    }

    #[tokio::test]
    async fn test_get_messages_with_limit() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/conversations.history"))
            .and(query_param("channel", "C123"))
            .and(query_param("limit", "10"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "ok": true,
                "messages": [
                    {"text": "Hello", "ts": "1234567890.000001"},
                    {"text": "World", "ts": "1234567890.000002"}
                ]
            })))
            .mount(&mock_server)
            .await;

        let client = Client::with_base_url("test-token", &mock_server.uri()).unwrap();
        let result = client.get_messages("C123", 10).await.unwrap();

        assert_eq!(result["messages"][0]["text"], "Hello");
    }

    #[tokio::test]
    async fn test_send_message_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/chat.postMessage"))
            .and(bearer_token("test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "ok": true,
                "channel": "C123",
                "ts": "1234567890.000001",
                "message": {"text": "Hello!"}
            })))
            .mount(&mock_server)
            .await;

        let client = Client::with_base_url("test-token", &mock_server.uri()).unwrap();
        let result = client.send_message("C123", "Hello!", None).await.unwrap();

        assert_eq!(result["ok"], true);
        assert_eq!(result["message"]["text"], "Hello!");
    }

    #[tokio::test]
    async fn test_api_error_response() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/users.list"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "ok": false,
                "error": "invalid_auth"
            })))
            .mount(&mock_server)
            .await;

        let client = Client::with_base_url("bad-token", &mock_server.uri()).unwrap();
        let result = client.list_users().await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid_auth"));
    }

    #[tokio::test]
    async fn test_list_dms() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/conversations.list"))
            .and(query_param("types", "im,mpim"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "ok": true,
                "channels": [{"id": "D123", "user": "U456"}]
            })))
            .mount(&mock_server)
            .await;

        let client = Client::with_base_url("test-token", &mock_server.uri()).unwrap();
        let result = client.list_dms().await.unwrap();

        assert_eq!(result["channels"][0]["id"], "D123");
    }

    #[tokio::test]
    async fn test_search_messages() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/search.messages"))
            .and(query_param("query", "hello world"))
            .and(query_param("count", "30"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "ok": true,
                "messages": {
                    "matches": [{"text": "hello world from channel"}]
                }
            })))
            .mount(&mock_server)
            .await;

        let client = Client::with_base_url("test-token", &mock_server.uri()).unwrap();
        let result = client.search_messages("hello world", 30).await.unwrap();

        assert_eq!(result["ok"], true);
    }

    #[tokio::test]
    async fn test_get_channel_info() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/conversations.info"))
            .and(query_param("channel", "C789"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "ok": true,
                "channel": {
                    "id": "C789",
                    "name": "engineering",
                    "is_private": false
                }
            })))
            .mount(&mock_server)
            .await;

        let client = Client::with_base_url("test-token", &mock_server.uri()).unwrap();
        let result = client.get_channel("C789").await.unwrap();

        assert_eq!(result["channel"]["name"], "engineering");
    }
}
