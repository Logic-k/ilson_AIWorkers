//! 사장님이 넣어 준 공개 웹 페이지를 읽어 본문 텍스트로 바꾼다.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// 한 페이지에서 AI에게 넘기는 최대 글자 수. 넘으면 앞부분만 쓰고 그 사실을 기록에 남긴다.
pub const MAX_CHARS_PER_PAGE: usize = 12_000;
const MAX_BYTES: usize = 3 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub url: String,
    pub title: String,
    pub text: String,
    pub truncated: bool,
    pub original_chars: usize,
}

pub fn parse_urls(raw: &str) -> Vec<String> {
    raw.split(|c: char| c.is_whitespace() || c == ',')
        .map(|s| s.trim())
        .filter(|s| s.starts_with("http://") || s.starts_with("https://"))
        .map(String::from)
        .collect()
}

fn extract_title(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<title")?;
    let open_end = lower[start..].find('>')? + start + 1;
    let end = lower[open_end..].find("</title>")? + open_end;
    let title = html[open_end..end].split_whitespace().collect::<Vec<_>>().join(" ");
    (!title.is_empty()).then_some(title)
}

pub async fn fetch(client: &reqwest::Client, url: &str) -> Result<Page> {
    let resp = client
        .get(url)
        .header(
            "user-agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0 Safari/537.36 Ilson/0.1",
        )
        .header("accept-language", "ko-KR,ko;q=0.9,en;q=0.6")
        .timeout(Duration::from_secs(25))
        .send()
        .await
        .map_err(|e| anyhow!("페이지를 열지 못했습니다: {e}"))?;
    if !resp.status().is_success() {
        return Err(anyhow!("페이지가 {} 응답을 보냈습니다", resp.status().as_u16()));
    }
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !(content_type.is_empty() || content_type.contains("html") || content_type.contains("text")) {
        return Err(anyhow!("글로 읽을 수 없는 형식입니다 ({content_type})"));
    }
    let bytes = resp.bytes().await?;
    let bytes = &bytes[..bytes.len().min(MAX_BYTES)];
    let html = String::from_utf8_lossy(bytes);
    let title = extract_title(&html).unwrap_or_else(|| url.to_string());
    let text = if content_type.contains("html") || html.trim_start().starts_with('<') {
        html2text::from_read(bytes, 120).unwrap_or_else(|_| html.to_string())
    } else {
        html.to_string()
    };
    // 빈 줄과 공백을 정리해 토큰을 아낀다.
    let cleaned: String = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let original_chars = cleaned.chars().count();
    let truncated = original_chars > MAX_CHARS_PER_PAGE;
    let text = if truncated {
        cleaned.chars().take(MAX_CHARS_PER_PAGE).collect()
    } else {
        cleaned
    };
    Ok(Page { url: url.to_string(), title, text, truncated, original_chars })
}
