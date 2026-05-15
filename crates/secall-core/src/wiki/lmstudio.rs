use async_trait::async_trait;
use futures_util::StreamExt as _;

use super::WikiBackend;

pub struct LmStudioBackend {
    pub api_url: String,
    pub model: String,
    pub max_tokens: u32,
}

#[async_trait]
impl WikiBackend for LmStudioBackend {
    fn name(&self) -> &'static str {
        "lmstudio"
    }

    async fn generate(&self, prompt: &str) -> anyhow::Result<String> {
        // P52/P59: LM Studio server hang 회피. wiki 생성은 출력이 길어 1800s
        // 한도 (P59 에서 5분 → 30분 상향).
        // P60: stream=true (OpenAI-compatible SSE) 로 전환. delta.content 누적.
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(1800))
            .build()?;
        let resp = client
            .post(format!("{}/v1/chat/completions", self.api_url))
            .json(&serde_json::json!({
                "model": self.model,
                "messages": [{"role": "user", "content": prompt}],
                "max_tokens": self.max_tokens,
                "stream": true
            }))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("LM Studio request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("LM Studio API error ({}): {}", status, body);
        }

        consume_sse_stream(resp).await
    }
}

/// LM Studio (OpenAI-compatible) `/v1/chat/completions` 의 SSE 응답을 파싱.
///
/// 각 이벤트는 `data: <json>\n` 한 줄 (보통 `\n\n` 으로 구분되지만 라인 단위로
/// 처리해도 된다). `data: [DONE]` 으로 종료. JSON 의 `choices[0].delta.content`
/// 가 매 토큰의 증분이며 누적해 본문을 만든다. 라인 경계 cross-chunk 대응을
/// 위해 buffer 에 누적.
async fn consume_sse_stream(resp: reqwest::Response) -> anyhow::Result<String> {
    let mut stream = resp.bytes_stream();
    let mut line_buf = String::new();
    let mut full = String::new();
    let mut echo_pending = String::new();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| anyhow::anyhow!("LM Studio stream chunk error: {}", e))?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|e| anyhow::anyhow!("LM Studio stream UTF-8 error: {}", e))?;
        line_buf.push_str(text);
        while let Some(idx) = line_buf.find('\n') {
            let line = line_buf[..idx].trim_end_matches('\r').to_string();
            line_buf.drain(..=idx);
            let payload = match line.strip_prefix("data: ") {
                Some(p) => p,
                None => continue, // empty / comment / heartbeat line
            };
            if payload == "[DONE]" {
                flush_echo(&mut echo_pending, true);
                return Ok(full);
            }
            let value: serde_json::Value = serde_json::from_str(payload).map_err(|e| {
                anyhow::anyhow!("LM Studio SSE parse error: {} (data: {})", e, payload)
            })?;
            if let Some(piece) = value
                .pointer("/choices/0/delta/content")
                .and_then(|v| v.as_str())
            {
                full.push_str(piece);
                echo_pending.push_str(piece);
                flush_echo(&mut echo_pending, false);
            }
        }
    }
    flush_echo(&mut echo_pending, true);
    if full.is_empty() {
        anyhow::bail!("LM Studio stream ended without any content");
    }
    Ok(full)
}

/// stderr echo 정책 (ollama.rs 와 동일 톤): 80자 누적 또는 `\n` 시 flush.
fn flush_echo(pending: &mut String, force: bool) {
    loop {
        if let Some(idx) = pending.find('\n') {
            let line = pending[..idx].to_string();
            pending.drain(..=idx);
            if !line.trim().is_empty() {
                eprintln!("  [lmstudio] {}", line);
            }
            continue;
        }
        if pending.chars().count() >= 80 {
            let line = std::mem::take(pending);
            eprintln!("  [lmstudio] {}", line);
            continue;
        }
        break;
    }
    if force && !pending.trim().is_empty() {
        eprintln!("  [lmstudio] {}", pending);
        pending.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    /// SSE body — 3 delta 합쳐 "wiki content" + [DONE].
    fn lmstudio_sse_body() -> String {
        let line1 = r#"data: {"choices":[{"delta":{"content":"wiki "}}]}"#;
        let line2 = r#"data: {"choices":[{"delta":{"content":"content"}}]}"#;
        let done = "data: [DONE]";
        format!("{line1}\n\n{line2}\n\n{done}\n\n")
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_lmstudio_backend_generate_streams_delta_content() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/chat/completions")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(lmstudio_sse_body())
            .create_async()
            .await;

        let backend = LmStudioBackend {
            api_url: server.url(),
            model: "qwen3-coder-32b".to_string(),
            max_tokens: 4096,
        };

        let result = backend.generate("test prompt").await;
        assert!(result.is_ok(), "generate should succeed: {:?}", result.err());
        assert_eq!(result.unwrap(), "wiki content");
        mock.assert_async().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_lmstudio_backend_generate_propagates_4xx_error() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("POST", "/v1/chat/completions")
            .with_status(400)
            .with_body(r#"{"error":"bad request"}"#)
            .create_async()
            .await;

        let backend = LmStudioBackend {
            api_url: server.url(),
            model: "qwen3-coder-32b".to_string(),
            max_tokens: 4096,
        };

        let result = backend.generate("test prompt").await;
        assert!(result.is_err(), "4xx response should return Err");
    }

    /// SSE 라인이 chunk 중간에 잘려도 누적되는지 (mockito 는 chunked 직접 제어
    /// 안 되지만 단일 body 안에 cross-line 형식을 그대로 둬도 line buffer 가
    /// 처리해야 함).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_lmstudio_stream_accumulates_across_deltas() {
        let body = format!(
            "{a}\n\n{b}\n\n{c}\n\ndata: [DONE]\n\n",
            a = r#"data: {"choices":[{"delta":{"content":"hello "}}]}"#,
            b = r#"data: {"choices":[{"delta":{"content":"world"}}]}"#,
            c = r#"data: {"choices":[{"delta":{"content":"!"}}]}"#,
        );

        let mut server = Server::new_async().await;
        let _mock = server
            .mock("POST", "/v1/chat/completions")
            .with_status(200)
            .with_body(body)
            .create_async()
            .await;

        let backend = LmStudioBackend {
            api_url: server.url(),
            model: "qwen3-coder-32b".to_string(),
            max_tokens: 4096,
        };

        let result = backend.generate("test prompt").await.unwrap();
        assert_eq!(result, "hello world!");
    }
}
