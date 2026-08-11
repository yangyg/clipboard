//! Minimal WebDAV client (Basic auth): MKCOL / PUT / GET / HEAD / PROPFIND.

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, ETAG, IF_MATCH, IF_NONE_MATCH};
use reqwest::redirect;
use reqwest::{Client, Method, StatusCode};
use std::time::Duration;
use url::Url;

const MAX_REMOTE_RESPONSE_BYTES: usize = 64 * 1024 * 1024;
const MAX_ERROR_RESPONSE_BYTES: usize = 64 * 1024;

pub struct RemoteBytes {
    pub bytes: Vec<u8>,
    pub etag: Option<String>,
}

async fn limited_error_body(mut response: reqwest::Response) -> String {
    let mut bytes = Vec::new();
    while let Ok(Some(chunk)) = response.chunk().await {
        let remaining = MAX_ERROR_RESPONSE_BYTES.saturating_sub(bytes.len());
        if remaining == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

#[derive(Clone)]
pub struct WebDavClient {
    client: Client,
    base_url: String,
    username: String,
    password: String,
}

impl WebDavClient {
    pub fn new(base_url: &str, username: &str, password: &str) -> Result<Self, String> {
        let base = base_url.trim().trim_end_matches('/').to_string();
        if base.is_empty() {
            return Err("WebDAV URL 不能为空".into());
        }
        // HTTPS only: Basic auth + the whole clipboard bundle would otherwise
        // be sent in cleartext and sniffable on the network.
        if !base.starts_with("https://") {
            return Err("WebDAV 必须使用 https://（明文 http 会泄露密码与剪贴板内容）".into());
        }
        // Redirects are the default reqwest behavior (follow up to 10 hops
        // across schemes/hosts). That is unsafe here:
        //  - A 30x to `http://` on the same host would re-send the Basic-auth
        //    header and the whole clipboard bundle over cleartext (TLS downgrade).
        //  - A 307/308 to an internal host (127.0.0.1, cloud metadata, …) would
        //    stream clipboard data to an attacker-influenced endpoint (SSRF).
        // We therefore only ever follow redirects that stay on HTTPS to the
        // exact host the user configured; everything else is treated as a final
        // (non-success) response.
        let base_host = Url::parse(&base)
            .ok()
            .and_then(|u| u.host_str().map(str::to_string));
        let redirect_policy = redirect::Policy::custom(move |attempt| {
            let url = attempt.url();
            if url.scheme() != "https" {
                return attempt.stop();
            }
            match (&base_host, url.host_str()) {
                (Some(host), Some(candidate)) if host == candidate => attempt.follow(),
                _ => attempt.stop(),
            }
        });
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(20))
            .redirect(redirect_policy)
            .build()
            .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))?;
        Ok(Self {
            client,
            base_url: base,
            username: username.to_string(),
            password: password.to_string(),
        })
    }

    fn url(&self, relative: &str) -> Result<String, String> {
        let mut url = Url::parse(self.base_url.trim_end_matches('/'))
            .map_err(|e| format!("WebDAV URL 无效: {e}"))?;
        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| "WebDAV URL 不支持路径追加".to_string())?;
            for part in relative.split(['/', '\\']).filter(|part| !part.is_empty()) {
                segments.push(part);
            }
        }
        Ok(url.to_string())
    }

    fn auth_header(&self) -> String {
        use base64::Engine;
        let token = base64::engine::general_purpose::STANDARD
            .encode(format!("{}:{}", self.username, self.password));
        format!("Basic {token}")
    }

    pub async fn ensure_collection(&self, relative_dir: &str) -> Result<(), String> {
        let parts: Vec<&str> = relative_dir
            .split(['/', '\\'])
            .filter(|s| !s.is_empty())
            .collect();
        let mut acc = String::new();
        for part in parts {
            if !acc.is_empty() {
                acc.push('/');
            }
            acc.push_str(part);
            let url = self.url(&acc)?;
            let res = self
                .client
                .request(Method::from_bytes(b"MKCOL").unwrap(), &url)
                .header(AUTHORIZATION, self.auth_header())
                .send()
                .await
                .map_err(|e| format!("MKCOL {acc}: {e}"))?;
            let status = res.status();
            // 201 created, 405 method not allowed (some servers), 409/301 already exists
            if status.is_success()
                || status == StatusCode::METHOD_NOT_ALLOWED
                || status == StatusCode::CONFLICT
                || status == StatusCode::MOVED_PERMANENTLY
                || status == StatusCode::FOUND
                || status == StatusCode::TEMPORARY_REDIRECT
            {
                continue;
            }
            if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                return Err(format!("WebDAV 鉴权失败 ({status})"));
            }
            // Some providers return 403 for existing folders — try PROPFIND as soft check
            if status.as_u16() >= 400 {
                if self.exists(&acc).await.unwrap_or(false) {
                    continue;
                }
                let body = limited_error_body(res).await;
                return Err(format!("MKCOL {acc} 失败: {status} {body}"));
            }
        }
        Ok(())
    }

    pub async fn put_bytes(
        &self,
        relative: &str,
        bytes: Vec<u8>,
        content_type: &str,
    ) -> Result<(), String> {
        let url = self.url(relative)?;
        let res = self
            .client
            .put(&url)
            .header(AUTHORIZATION, self.auth_header())
            .header(CONTENT_TYPE, content_type)
            .body(bytes)
            .send()
            .await
            .map_err(|e| format!("PUT {relative}: {e}"))?;
        let status = res.status();
        if status.is_success() || status == StatusCode::CREATED || status == StatusCode::NO_CONTENT
        {
            return Ok(());
        }
        let body = limited_error_body(res).await;
        Err(format!("PUT {relative} 失败: {status} {body}"))
    }

    pub async fn put_bytes_if_match(
        &self,
        relative: &str,
        bytes: Vec<u8>,
        content_type: &str,
        etag: Option<&str>,
    ) -> Result<Option<String>, String> {
        let url = self.url(relative)?;
        let request = self
            .client
            .put(&url)
            .header(AUTHORIZATION, self.auth_header())
            .header(CONTENT_TYPE, content_type);
        let request = match etag {
            Some(value) => request.header(IF_MATCH, value),
            None => request.header(IF_NONE_MATCH, "*"),
        };
        let res = request
            .body(bytes)
            .send()
            .await
            .map_err(|e| format!("PUT {relative}: {e}"))?;
        let status = res.status();
        if status.is_success() || status == StatusCode::CREATED || status == StatusCode::NO_CONTENT
        {
            return Ok(res
                .headers()
                .get(ETAG)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string));
        }
        if status == StatusCode::PRECONDITION_FAILED {
            return Err(format!("PUT {relative} 被并发修改，已取消覆盖"));
        }
        let body = limited_error_body(res).await;
        Err(format!("PUT {relative} 失败: {status} {body}"))
    }

    pub async fn delete_bytes_if_match(&self, relative: &str, etag: &str) -> Result<(), String> {
        let url = self.url(relative)?;
        let res = self
            .client
            .delete(&url)
            .header(AUTHORIZATION, self.auth_header())
            .header(IF_MATCH, etag)
            .send()
            .await
            .map_err(|e| format!("DELETE {relative}: {e}"))?;
        let status = res.status();
        if status.is_success() || status == StatusCode::NOT_FOUND {
            return Ok(());
        }
        if status == StatusCode::PRECONDITION_FAILED {
            return Err(format!("DELETE {relative} 被并发修改，未执行回滚"));
        }
        let body = limited_error_body(res).await;
        Err(format!("DELETE {relative} 失败: {status} {body}"))
    }

    pub async fn get_bytes(&self, relative: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(self
            .get_bytes_with_etag(relative)
            .await?
            .map(|remote| remote.bytes))
    }

    pub async fn get_bytes_with_etag(&self, relative: &str) -> Result<Option<RemoteBytes>, String> {
        let url = self.url(relative)?;
        let res = self
            .client
            .get(&url)
            .header(AUTHORIZATION, self.auth_header())
            .send()
            .await
            .map_err(|e| format!("GET {relative}: {e}"))?;
        let status = res.status();
        if status == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !status.is_success() {
            let body = limited_error_body(res).await;
            return Err(format!("GET {relative} 失败: {status} {body}"));
        }
        if res
            .content_length()
            .is_some_and(|length| length > MAX_REMOTE_RESPONSE_BYTES as u64)
        {
            return Err(format!(
                "远端文件过大（上限 {} MB）",
                MAX_REMOTE_RESPONSE_BYTES / (1024 * 1024)
            ));
        }
        let etag = res
            .headers()
            .get(ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let content_length = res.content_length();
        let mut res = res;
        let mut bytes = Vec::with_capacity(
            content_length
                .unwrap_or(0)
                .min(MAX_REMOTE_RESPONSE_BYTES as u64) as usize,
        );
        while let Some(chunk) = res
            .chunk()
            .await
            .map_err(|e| format!("读取 {relative}: {e}"))?
        {
            if bytes.len().saturating_add(chunk.len()) > MAX_REMOTE_RESPONSE_BYTES {
                return Err(format!(
                    "远端文件过大（上限 {} MB）",
                    MAX_REMOTE_RESPONSE_BYTES / (1024 * 1024)
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(Some(RemoteBytes { bytes, etag }))
    }

    pub async fn exists(&self, relative: &str) -> Result<bool, String> {
        let url = self.url(relative)?;
        let res = self
            .client
            .head(&url)
            .header(AUTHORIZATION, self.auth_header())
            .send()
            .await;
        match res {
            Ok(r) => {
                let status = r.status();
                if status.is_success() {
                    return Ok(true);
                }
                if status == StatusCode::NOT_FOUND {
                    return Ok(false);
                }
                if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                    return Err(format!("WebDAV 鉴权失败 ({status})"));
                }
                // Some servers reject HEAD (405/4xx) — fall back to a Depth:0
                // PROPFIND instead of GET: GET would download the entire file
                // just to check existence.
                self.propfind_exists(relative).await
            }
            Err(_) => self.propfind_exists(relative).await,
        }
    }

    /// WebDAV-native existence probe (Depth: 0 PROPFIND). Avoids downloading
    /// file bodies on servers that reject HEAD.
    async fn propfind_exists(&self, relative: &str) -> Result<bool, String> {
        let url = self.url(relative)?;
        let res = self
            .client
            .request(Method::from_bytes(b"PROPFIND").unwrap(), &url)
            .header(AUTHORIZATION, self.auth_header())
            .header("Depth", "0")
            .header(CONTENT_TYPE, "application/xml")
            .body(
                r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:"><d:prop><d:resourcetype/></d:prop></d:propfind>"#,
            )
            .send()
            .await
            .map_err(|e| format!("PROPFIND {relative} 失败: {e}"))?;
        let status = res.status();
        if status.is_success() || status == StatusCode::MULTI_STATUS {
            return Ok(true);
        }
        if status == StatusCode::NOT_FOUND {
            return Ok(false);
        }
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            return Err(format!("WebDAV 鉴权失败 ({status})"));
        }
        Ok(false)
    }

    /// Lightweight auth probe: PROPFIND Depth:0 on base URL (or GET root).
    pub async fn test_connection(&self) -> Result<(), String> {
        let url = self.url("")?;
        let res = self
            .client
            .request(Method::from_bytes(b"PROPFIND").unwrap(), &url)
            .header(AUTHORIZATION, self.auth_header())
            .header("Depth", "0")
            .header(CONTENT_TYPE, "application/xml")
            .body(
                r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:"><d:prop><d:resourcetype/></d:prop></d:propfind>"#,
            )
            .send()
            .await
            .map_err(|e| format!("连接失败: {e}"))?;
        let status = res.status();
        if status.is_success()
            || status == StatusCode::MULTI_STATUS
            || status == StatusCode::METHOD_NOT_ALLOWED
        {
            return Ok(());
        }
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            return Err("用户名或密码错误，或无访问权限".into());
        }
        // Fallback: try GET
        let get = self
            .client
            .get(&url)
            .header(AUTHORIZATION, self.auth_header())
            .send()
            .await
            .map_err(|e| format!("连接失败: {e}"))?;
        if get.status().is_success() || get.status() == StatusCode::NOT_FOUND {
            return Ok(());
        }
        Err(format!("连接失败: HTTP {}", get.status()))
    }
}

#[cfg(test)]
mod tests {
    use super::WebDavClient;

    #[test]
    fn url_encodes_remote_path_segments() {
        let client = WebDavClient::new("https://example.test/base", "user", "pass").unwrap();

        let url = client.url("Clip Vault/#备份").unwrap();

        assert_eq!(
            url,
            "https://example.test/base/Clip%20Vault/%23%E5%A4%87%E4%BB%BD"
        );
    }
}
