use async_trait::async_trait;
use futures_util::StreamExt as _;

use super::WikiBackend;

pub struct OllamaBackend {
    pub api_url: String,
    pub model: String,
    pub max_tokens: u32,
    pub api_key: Option<String>,
    /// P85 (issue #87): config.wiki.generation_timeout_secs.
    pub timeout_secs: u64,
}

#[async_trait]
impl WikiBackend for OllamaBackend {
    fn name(&self) -> &'static str {
        "ollama"
    }

    async fn generate(&self, prompt: &str) -> anyhow::Result<String> {
        // P52/P59: ollama server hang 회피. wiki 생성은 출력이 길어 1800s 한도
        // (P59 에서 5분 → 30분 상향 — 정상 케이스도 5분 넘는 사례 관측).
        // P60: stream=true 로 전환 + bytes_stream() 로 NDJSON 라인 파싱.
        // claude.rs 의 line-stream 과 동일 톤으로 stderr 에 `[ollama] ...` echo
        // 하여 사용자가 진행 상황을 본다.
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(self.timeout_secs))
            .build()?;
        let mut req =
            client
                .post(format!("{}/api/generate", self.api_url))
                .json(&serde_json::json!({
                    "model": self.model,
                    "prompt": prompt,
                    "stream": true,
                    "options": { "num_predict": self.max_tokens }
                }));
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Ollama request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Ollama API error ({}): {}", status, body);
        }

        consume_ndjson_stream(resp).await
    }
}

/// Ollama `/api/generate` 의 stream 응답 (NDJSON) 을 파싱한다.
///
/// 각 라인은 `{"response": "<chunk>", "done": false}` 형식. 마지막 라인이
/// `{"done": true, ...}`. chunk 경계가 라인 중간을 가를 수 있어 buffer 에 누적
/// 후 `\n` split. 누적된 응답 텍스트가 80자 이상 쌓이거나 라인 단위 break 가
/// 나타나면 stderr 에 echo 한다.
async fn consume_ndjson_stream(resp: reqwest::Response) -> anyhow::Result<String> {
    let mut stream = resp.bytes_stream();
    let mut line_buf = String::new();
    let mut full = String::new();
    let mut echo_pending = String::new();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| anyhow::anyhow!("Ollama stream chunk error: {}", e))?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|e| anyhow::anyhow!("Ollama stream UTF-8 error: {}", e))?;
        line_buf.push_str(text);
        while let Some(idx) = line_buf.find('\n') {
            let line = line_buf[..idx].to_string();
            line_buf.drain(..=idx);
            if line.trim().is_empty() {
                continue;
            }
            let value: serde_json::Value = serde_json::from_str(&line).map_err(|e| {
                anyhow::anyhow!("Ollama NDJSON parse error: {} (line: {})", e, line)
            })?;
            if let Some(piece) = value.get("response").and_then(|v| v.as_str()) {
                full.push_str(piece);
                echo_pending.push_str(piece);
                flush_echo(&mut echo_pending, false);
            }
            if value.get("done").and_then(|v| v.as_bool()).unwrap_or(false) {
                flush_echo(&mut echo_pending, true);
                return Ok(full);
            }
        }
    }
    // EOF. line_buf 에 newline 없는 잔여 라인이 남아있을 수 있다 (일부 서버가
    // 마지막 라인에 \n 미부착). non-stream 응답 (단일 JSON object) 으로 응답하는
    // mock / proxy 환경 호환을 위해 처리.
    let trailing = line_buf.trim();
    if !trailing.is_empty() {
        let value: serde_json::Value = serde_json::from_str(trailing).map_err(|e| {
            anyhow::anyhow!(
                "Ollama trailing line parse error: {} (line: {})",
                e,
                trailing
            )
        })?;
        if let Some(piece) = value.get("response").and_then(|v| v.as_str()) {
            full.push_str(piece);
            echo_pending.push_str(piece);
        }
    }
    flush_echo(&mut echo_pending, true);
    if full.is_empty() {
        anyhow::bail!("Ollama stream ended without any response");
    }
    Ok(full)
}

/// stderr echo 정책: 80자 누적되거나 `\n` 포함 시 flush. `force=true` 면 잔여 모두.
fn flush_echo(pending: &mut String, force: bool) {
    loop {
        if let Some(idx) = pending.find('\n') {
            let line = pending[..idx].to_string();
            pending.drain(..=idx);
            if !line.trim().is_empty() {
                eprintln!("  [ollama] {}", line);
            }
            continue;
        }
        if pending.chars().count() >= 80 {
            let line = std::mem::take(pending);
            eprintln!("  [ollama] {}", line);
            continue;
        }
        break;
    }
    if force && !pending.trim().is_empty() {
        eprintln!("  [ollama] {}", pending);
        pending.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::{Matcher, Server};

    /// NDJSON stream 응답 — 3 chunks 합쳐 "wiki content".
    fn ollama_stream_body() -> String {
        [
            r#"{"response":"wiki ","done":false}"#,
            r#"{"response":"content","done":false}"#,
            r#"{"response":"","done":true}"#,
        ]
        .join("\n")
            + "\n"
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_ollama_backend_generate_includes_bearer_auth_when_api_key_set() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/api/generate")
            .match_header("Authorization", "Bearer cloud-key")
            .with_status(200)
            .with_body(ollama_stream_body())
            .create_async()
            .await;

        let backend = OllamaBackend {
            api_url: server.url(),
            model: "gemma4:31b-cloud".to_string(),
            max_tokens: 4096,
            api_key: Some("cloud-key".to_string()),
            timeout_secs: 60,
        };

        let result = backend.generate("test prompt").await;
        assert!(
            result.is_ok(),
            "generate should succeed: {:?}",
            result.err()
        );
        assert_eq!(result.unwrap(), "wiki content");
        mock.assert_async().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_ollama_backend_generate_omits_auth_header_when_api_key_none() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/api/generate")
            .match_header("Authorization", Matcher::Missing)
            .with_status(200)
            .with_body(ollama_stream_body())
            .create_async()
            .await;

        let backend = OllamaBackend {
            api_url: server.url(),
            model: "local-model".to_string(),
            max_tokens: 4096,
            api_key: None,
            timeout_secs: 60,
        };

        let result = backend.generate("test prompt").await;
        assert!(
            result.is_ok(),
            "generate without api_key should succeed: {:?}",
            result.err()
        );
        mock.assert_async().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_ollama_backend_generate_propagates_4xx_error() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("POST", "/api/generate")
            .with_status(401)
            .with_body(r#"{"error":"unauthorized"}"#)
            .create_async()
            .await;

        let backend = OllamaBackend {
            api_url: server.url(),
            model: "gemma4:31b-cloud".to_string(),
            max_tokens: 4096,
            api_key: Some("bad-key".to_string()),
            timeout_secs: 60,
        };

        let result = backend.generate("test prompt").await;
        assert!(result.is_err(), "4xx response should return Err");
    }

    /// stream 의 chunk 경계가 JSON 라인 중간을 잘라도 정확히 누적되는지.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_ollama_stream_accumulates_across_chunks() {
        // 의도적으로 한 라인을 여러 토큰으로 쪼개 done=true 로 종료.
        let body = [
            r#"{"response":"hello ","done":false}"#,
            r#"{"response":"world","done":false}"#,
            r#"{"response":"!","done":true}"#,
        ]
        .join("\n")
            + "\n";

        let mut server = Server::new_async().await;
        let _mock = server
            .mock("POST", "/api/generate")
            .with_status(200)
            .with_body(body)
            .create_async()
            .await;

        let backend = OllamaBackend {
            api_url: server.url(),
            model: "local-model".to_string(),
            max_tokens: 4096,
            api_key: None,
            timeout_secs: 60,
        };

        let result = backend.generate("test prompt").await.unwrap();
        assert_eq!(result, "hello world!");
    }
}
