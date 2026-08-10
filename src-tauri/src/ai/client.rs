//! OpenAI-compatible `/chat/completions` client used for record enrichment.
//! Mirrors the WebDAV client's safety posture: HTTPS except loopback hosts,
//! no cross-host/downgrade redirects, tight connect/read timeouts.

use reqwest::redirect;
use reqwest::Client;
use std::time::Duration;
use url::Url;

use crate::db::ALIAS_MAX_CHARS;
use crate::types::AiResult;

const MAX_TAGS: usize = 5;
const MAX_TAG_CHARS: usize = 20;
const REQUEST_TIMEOUT_SECS: u64 = 60;
const CONNECT_TIMEOUT_SECS: u64 = 20;
const MAX_ERROR_BODY_BYTES: usize = 64 * 1024;

fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || host == "::1"
}

/// Trim a success payload down to the pieces we trust before handing it to the
/// DB: summary capped to the alias column width; tags deduped/trimmed/capped.
fn sanitize_result(mut r: AiResult) -> AiResult {
    r.summary = r.summary.trim().to_string();
    if r.summary.chars().count() > ALIAS_MAX_CHARS {
        r.summary = r.summary.chars().take(ALIAS_MAX_CHARS).collect();
    }
    let mut seen: Vec<String> = Vec::new();
    for tag in r.tags {
        let tag = tag.trim().to_string();
        if tag.is_empty() || tag.chars().count() > MAX_TAG_CHARS {
            continue;
        }
        let key = tag.to_lowercase();
        if seen.iter().any(|t| t.to_lowercase() == key) {
            continue;
        }
        seen.push(tag);
        if seen.len() >= MAX_TAGS {
            break;
        }
    }
    r.tags = seen;
    r
}

/// Best-effort JSON extraction from the model's text reply: tolerates a
/// leading "```json" fence or prose around the object.
fn extract_json_object(text: &str) -> Option<serde_json::Value> {
    let trimmed = text.trim();
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if v.is_object() {
            return Some(v);
        }
    }
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str(&trimmed[start..=end]).ok()
}

/// Build the system prompt that instructs the model to return strict JSON with
/// a short summary (<=80 chars) plus up to 5 tags.
pub(crate) fn build_system_prompt(language: &str) -> String {
    let lang = if language.eq_ignore_ascii_case("en-US") {
        "English"
    } else {
        "简体中文"
    };
    format!(
        "You organize clipboard content. Reply in {output_lang}. Output ONLY strict JSON with \
         this exact shape, and nothing outside it: \
         {{\"summary\":\"...\",\"tags\":[\"...\",\"...\"]}}.\n\
         Rules:\n\
         - \"summary\" is one sentence (prefer {output_lang}, max 80 characters) capturing the \
         core information, suitable as a list title/alias. Empty string when the content has no \
         real meaning (bare link, garbled text, a single symbol).\n\
         - \"tags\" holds 1 to 5 short tags (2-8 characters each) covering the topic; empty array \
         when nothing can be classified.\n\
         - Never invent facts that are not in the content. Do not repeat the whole content.",
        output_lang = lang
    )
}

#[derive(Clone)]
pub struct AiClient {
    client: Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl AiClient {
    /// Validate + build. `base_url` must be `https://` unless the host is
    /// loopback (local Ollama) — mirroring WebDAV's cleartext policy.
    pub fn new(base_url: &str, api_key: &str, model: &str) -> Result<Self, String> {
        let base = base_url.trim().trim_end_matches('/').to_string();
        if base.is_empty() || model.trim().is_empty() {
            return Err("AI 地址与模型名不能为空".into());
        }
        let parsed = Url::parse(&base).map_err(|e| format!("AI 地址无效: {e}"))?;
        let scheme = parsed.scheme();
        let host = parsed.host_str().unwrap_or_default().to_string();
        if scheme != "https" && !is_loopback_host(&host) {
            return Err(
                "AI 地址必须使用 https://（仅允许 http://localhost 之类的本机模型）".into(),
            );
        }
        // Only follow redirects that keep HTTPS + the exact configured host, so
        // API keys / clipboard content are never forwarded to a downgraded or
        // attacker-influenced endpoint.
        let redirect_policy = redirect::Policy::custom(move |attempt| {
            let url = attempt.url();
            if url.scheme() != "https" || url.host_str() != Some(host.as_str()) {
                return attempt.stop();
            }
            attempt.follow()
        });
        let client = Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
            .redirect(redirect_policy)
            .build()
            .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))?;
        Ok(Self {
            client,
            base_url: base,
            api_key: api_key.trim().to_string(),
            model: model.trim().to_string(),
        })
    }

    fn chat_endpoint(&self) -> String {
        format!("{}/chat/completions", self.base_url)
    }

    /// Lightweight auth/endpoint probe: a single-token request that only needs
    /// a 2xx response to pass — content/format parsing is not the point here.
    pub async fn test_connection(&self) -> Result<(), String> {
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 1,
            "messages": [{ "role": "user", "content": "ping" }]
        });
        let mut req = self.client.post(self.chat_endpoint()).json(&body);
        if !self.api_key.is_empty() {
            req = req.bearer_auth(&self.api_key);
        }
        let res = req.send().await.map_err(|e| format!("AI 请求失败: {e}"))?;
        let status = res.status();
        if status.is_success() {
            return Ok(());
        }
        let text = res.text().await.unwrap_or_default();
        let truncated: String = text.chars().take(MAX_ERROR_BODY_BYTES).collect();
        Err(format!("AI 接口返回 {status}: {truncated}"))
    }

    /// Call the model once on the given (already-truncated) content and parse
    /// the structured result. All provider errors surface as `Err` strings.
    pub async fn chat_json(&self, content: &str, language: &str) -> Result<AiResult, String> {
        let body = serde_json::json!({
            "model": self.model,
            "temperature": 0,
            "messages": [
                { "role": "system", "content": build_system_prompt(language) },
                { "role": "user", "content": content }
            ]
        });
        let mut req = self.client.post(self.chat_endpoint()).json(&body);
        if !self.api_key.is_empty() {
            req = req.bearer_auth(&self.api_key);
        }
        let res = req.send().await.map_err(|e| format!("AI 请求失败: {e}"))?;
        let status = res.status();
        if !status.is_success() {
            let text = res.text().await.unwrap_or_default();
            let truncated: String = text.chars().take(MAX_ERROR_BODY_BYTES).collect();
            return Err(format!("AI 接口返回 {status}: {truncated}"));
        }
        let payload: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("无法解析模型响应: {e}"))?;
        let text = payload
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|c| c.first())
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|s| s.as_str())
            .ok_or("模型响应缺少 choices[0].message.content")?;

        let obj = extract_json_object(text).ok_or("模型未返回可解析的 JSON 指令格式")?;
        let parsed = serde_json::from_value::<AiResult>(obj)
            .map_err(|e| format!("模型 JSON 字段不合法: {e}"))?;
        Ok(sanitize_result(parsed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_loopback_allowed_others_rejected() {
        assert!(AiClient::new("http://localhost:11434/v1", "x", "llama3").is_ok());
        assert!(AiClient::new("http://127.0.0.1:8000/v1", "", "llama3").is_ok());
        assert!(AiClient::new("http://evil.example/v1", "x", "m").is_err());
        assert!(AiClient::new("https://ok.example/v1", "x", "m").is_ok());
        assert!(AiClient::new("https://ok.example/v1", "x", "").is_err());
    }

    #[test]
    fn parses_fenced_json_and_caps_result() {
        let text = "```json\n{\"summary\":\" 一个很长的摘要...\",\"tags\":[\"a\",\"b\",\"b\",\"  c  \",\"\",\"标签标签标签标签标签标签标签标签标签标签标签标签\"]}\n```";
        let obj = extract_json_object(text).unwrap();
        let parsed = serde_json::from_value::<AiResult>(obj).unwrap();
        let out = sanitize_result(parsed);
        assert_eq!(
            out.tags,
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
        assert!(out.summary.chars().count() <= ALIAS_MAX_CHARS);
    }

    #[test]
    fn empty_content_yields_no_tags() {
        let text = "{\"summary\":\"\",\"tags\":[]}";
        let out = sanitize_result(serde_json::from_str(text).unwrap());
        assert!(out.summary.is_empty());
        assert!(out.tags.is_empty());
    }
}
