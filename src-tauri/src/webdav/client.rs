//! Minimal WebDAV client (Basic auth): MKCOL / PUT / GET / HEAD / PROPFIND.

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use reqwest::{Client, Method, StatusCode};
use std::time::Duration;

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
        if !(base.starts_with("https://") || base.starts_with("http://")) {
            return Err("WebDAV URL 必须以 http:// 或 https:// 开头".into());
        }
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(20))
            .build()
            .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))?;
        Ok(Self {
            client,
            base_url: base,
            username: username.to_string(),
            password: password.to_string(),
        })
    }

    fn url(&self, relative: &str) -> String {
        let rel = relative.trim_start_matches('/');
        format!("{}/{}", self.base_url, rel)
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
            let url = self.url(&acc);
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
                let body = res.text().await.unwrap_or_default();
                return Err(format!("MKCOL {acc} 失败: {status} {body}"));
            }
        }
        Ok(())
    }

    pub async fn put_bytes(&self, relative: &str, bytes: Vec<u8>, content_type: &str) -> Result<(), String> {
        let url = self.url(relative);
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
        if status.is_success() || status == StatusCode::CREATED || status == StatusCode::NO_CONTENT {
            return Ok(());
        }
        let body = res.text().await.unwrap_or_default();
        Err(format!("PUT {relative} 失败: {status} {body}"))
    }

    pub async fn get_bytes(&self, relative: &str) -> Result<Option<Vec<u8>>, String> {
        let url = self.url(relative);
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
            let body = res.text().await.unwrap_or_default();
            return Err(format!("GET {relative} 失败: {status} {body}"));
        }
        let bytes = res
            .bytes()
            .await
            .map_err(|e| format!("读取 {relative}: {e}"))?;
        Ok(Some(bytes.to_vec()))
    }

    pub async fn exists(&self, relative: &str) -> Result<bool, String> {
        let url = self.url(relative);
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
                if status == StatusCode::NOT_FOUND || status == StatusCode::METHOD_NOT_ALLOWED {
                    // Fallback GET for servers without HEAD
                    if status == StatusCode::METHOD_NOT_ALLOWED {
                        return Ok(self.get_bytes(relative).await?.is_some());
                    }
                    return Ok(false);
                }
                if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                    return Err(format!("WebDAV 鉴权失败 ({status})"));
                }
                // Some servers reject HEAD — try GET
                Ok(self.get_bytes(relative).await?.is_some())
            }
            Err(_) => Ok(self.get_bytes(relative).await?.is_some()),
        }
    }

    /// Lightweight auth probe: PROPFIND Depth:0 on base URL (or GET root).
    pub async fn test_connection(&self) -> Result<(), String> {
        let url = format!("{}/", self.base_url.trim_end_matches('/'));
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
