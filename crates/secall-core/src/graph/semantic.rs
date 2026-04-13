use anyhow::Result;
use serde::Deserialize;

use crate::ingest::markdown::SessionFrontmatter;
use crate::store::Database;
use crate::vault::config::GraphConfig;

use super::extract::{extract_semantic_edges, GraphEdge};

// ─── LLM 응답 구조 (공통) ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct SemanticOutput {
    edges: Vec<SemanticEdgeItem>,
}

#[derive(Debug, Deserialize)]
struct SemanticEdgeItem {
    relation: String,
    target_type: String,
    target_label: String,
}

// ─── Anthropic 응답 구조 ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContent {
    text: String,
}

// ─── Ollama 응답 구조 ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    message: OllamaMessage,
}

#[derive(Debug, Deserialize)]
struct OllamaMessage {
    content: String,
}

// ─── 정적 프롬프트 ──────────────────────────────────────────────────────────

const SYSTEM_PROMPT: &str = r#"Extract semantic relationships from this agent session log. Return JSON only, no explanation.

Output schema:
{"edges": [{"relation": "fixes_bug|modifies_file|introduces_tech|discusses_topic", "target_type": "issue|file|tech|topic", "target_label": "<value>"}]}

Rules:
- relation MUST be exactly one of: fixes_bug, modifies_file, introduces_tech, discusses_topic
- fixes_bug: "closes #N" or "fixes #N" → target_type=issue, target_label=number only (e.g. "21")
- modifies_file: edited file paths → target_type=file, target_label=relative path
- introduces_tech: new library/tool → target_type=tech
- discusses_topic: main topic → target_type=topic
- Do not invent relation names
- Do not include trivial relationships
- Return empty edges array if nothing is found"#;

const BODY_LIMIT: usize = 2000;

// ─── user content 생성 (공통) ──────────────────────────────────────────────

fn build_user_content(fm: &SessionFrontmatter, body: &str) -> String {
    let truncated_body = if body.len() > BODY_LIMIT {
        &body[..body
            .char_indices()
            .take_while(|(i, _)| *i < BODY_LIMIT)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(BODY_LIMIT)]
    } else {
        body
    };

    format!(
        "---\nsession_id: {}\nagent: {}\nproject: {}\ndate: {}\nsummary: {}\n---\n\n{}",
        fm.session_id,
        fm.agent,
        fm.project.as_deref().unwrap_or(""),
        fm.date,
        fm.summary.as_deref().unwrap_or(""),
        truncated_body,
    )
}

// ─── Anthropic API 호출 ────────────────────────────────────────────────────

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";

async fn extract_with_anthropic(
    fm: &SessionFrontmatter,
    body: &str,
    model: &str,
) -> Result<Vec<GraphEdge>> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| anyhow::anyhow!("ANTHROPIC_API_KEY not set"))?;

    let user_content = build_user_content(fm, body);

    let request_body = serde_json::json!({
        "model": model,
        "max_tokens": 512,
        "system": SYSTEM_PROMPT,
        "messages": [{"role": "user", "content": user_content}]
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(ANTHROPIC_API_URL)
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Anthropic API error {}: {}", status, text);
    }

    let api_resp: AnthropicResponse = resp.json().await?;
    let text = api_resp
        .content
        .first()
        .map(|c| c.text.as_str())
        .unwrap_or("{}");

    parse_llm_edges(text, &fm.session_id)
}

// ─── Ollama API 호출 ───────────────────────────────────────────────────────

async fn extract_with_ollama(
    fm: &SessionFrontmatter,
    body: &str,
    base_url: &str,
    model: &str,
) -> Result<Vec<GraphEdge>> {
    let user_content = build_user_content(fm, body);

    let request_body = serde_json::json!({
        "model": model,
        "stream": false,
        "options": {"temperature": 0.1},
        "messages": [
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user", "content": user_content}
        ]
    });

    let url = format!("{}/api/chat", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    let resp = client
        .post(&url)
        .header("content-type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Ollama API error {}: {}", status, text);
    }

    let ollama_resp: OllamaResponse = resp.json().await?;
    parse_llm_edges(&ollama_resp.message.content, &fm.session_id)
}

// ─── LLM 응답 파싱 (공통) ──────────────────────────────────────────────────

/// LLM JSON 응답 → GraphEdge 변환
fn parse_llm_edges(json_text: &str, session_id: &str) -> Result<Vec<GraphEdge>> {
    // JSON이 마크다운 코드블록으로 감싸져 있을 수 있음
    let cleaned = json_text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let output: SemanticOutput = serde_json::from_str(cleaned)
        .map_err(|e| anyhow::anyhow!("failed to parse LLM JSON: {}", e))?;

    let session_node_id = format!("session:{}", session_id);

    let weight_for = |relation: &str| -> f64 {
        match relation {
            "fixes_bug" => 0.8,
            "modifies_file" => 0.9,
            "introduces_tech" => 0.6,
            "discusses_topic" => 0.5,
            _ => 0.5,
        }
    };

    // 허용된 relation만 통과
    let allowed = [
        "fixes_bug",
        "modifies_file",
        "introduces_tech",
        "discusses_topic",
    ];

    let edges = output
        .edges
        .into_iter()
        .filter(|item| !item.target_label.is_empty() && allowed.contains(&item.relation.as_str()))
        .map(|item| {
            let target_id = format!("{}:{}", item.target_type, item.target_label);
            GraphEdge {
                source: session_node_id.clone(),
                target: target_id,
                relation: item.relation.clone(),
                confidence: "LLM".to_string(),
                weight: weight_for(&item.relation),
            }
        })
        .collect();

    Ok(edges)
}

// ─── provider 디스패치 ─────────────────────────────────────────────────────

/// 설정에 따라 적절한 LLM backend로 시맨틱 엣지 추출
async fn extract_with_llm(
    config: &GraphConfig,
    fm: &SessionFrontmatter,
    body: &str,
) -> Result<Vec<GraphEdge>> {
    match config.semantic_backend.as_str() {
        "ollama" => {
            let base_url = config
                .ollama_url
                .as_deref()
                .unwrap_or("http://localhost:11434");
            let model = config.ollama_model.as_deref().unwrap_or("gemma4:e4b");
            extract_with_ollama(fm, body, base_url, model).await
        }
        "anthropic" => {
            let model = config
                .anthropic_model
                .as_deref()
                .unwrap_or("claude-haiku-4-5-20251001");
            extract_with_anthropic(fm, body, model).await
        }
        _ => anyhow::bail!("unknown semantic_backend: {}", config.semantic_backend),
    }
}

// ─── 통합 저장 ──────────────────────────────────────────────────────────────

/// 규칙 기반 + (옵션) LLM으로 시맨틱 엣지 추출 후 DB 저장.
///
/// - 항상 규칙 기반 실행
/// - `semantic_backend`가 "disabled"가 아니면 LLM 호출 시도
/// - 실패 시 규칙 결과만 저장
/// - 노드 자동 생성 (issue:N, file:path, tech:X, topic:Y)
/// - 중복은 DB UNIQUE 제약으로 자동 방어
pub async fn extract_and_store(
    db: &Database,
    config: &GraphConfig,
    fm: &SessionFrontmatter,
    body: &str,
) -> Result<usize> {
    // 1. 규칙 기반 — 항상 실행
    let mut all_edges = extract_semantic_edges(fm, body);

    // 2. LLM — backend가 "disabled"가 아닐 때 시도
    if config.semantic_backend != "disabled" {
        match extract_with_llm(config, fm, body).await {
            Ok(llm_edges) => {
                tracing::debug!(
                    session = &fm.session_id[..fm.session_id.len().min(8)],
                    backend = &config.semantic_backend,
                    llm_edges = llm_edges.len(),
                    "LLM edges extracted"
                );
                all_edges.extend(llm_edges);
            }
            Err(e) => {
                tracing::warn!(
                    session = &fm.session_id[..fm.session_id.len().min(8)],
                    backend = &config.semantic_backend,
                    "LLM extraction failed, using rules only: {}",
                    e
                );
            }
        }
    }

    // 3. 중복 제거: (source, target, relation) 기준, 먼저 추출된 엣지 우선
    {
        let mut seen = std::collections::HashSet::new();
        all_edges.retain(|e| seen.insert((e.source.clone(), e.target.clone(), e.relation.clone())));
    }

    // 4. DB 저장
    let mut stored = 0usize;
    for edge in &all_edges {
        let (target_type, target_label) = if let Some(rest) = edge.target.strip_prefix("issue:") {
            ("issue", rest)
        } else if let Some(rest) = edge.target.strip_prefix("file:") {
            ("file", rest)
        } else if let Some(rest) = edge.target.strip_prefix("tech:") {
            ("tech", rest)
        } else if let Some(rest) = edge.target.strip_prefix("topic:") {
            ("topic", rest)
        } else {
            ("unknown", edge.target.as_str())
        };

        let session_node_id = format!("session:{}", fm.session_id);
        let session_label = fm.session_id[..fm.session_id.len().min(8)].to_string();
        db.upsert_graph_node(&session_node_id, "session", &session_label, None)?;

        db.upsert_graph_node(&edge.target, target_type, target_label, None)?;
        let rows = db.upsert_graph_edge(
            &edge.source,
            &edge.target,
            &edge.relation,
            &edge.confidence,
            edge.weight,
        )?;
        stored += rows;
    }

    Ok(stored)
}

// ─── 테스트 ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_fm(id: &str, tools: Option<Vec<&str>>, summary: Option<&str>) -> SessionFrontmatter {
        SessionFrontmatter {
            session_id: id.to_string(),
            agent: "claude-code".to_string(),
            model: None,
            project: Some("test-project".to_string()),
            cwd: None,
            date: "2026-04-13".to_string(),
            start_time: "2026-04-13T00:00:00Z".to_string(),
            end_time: None,
            turns: Some(5),
            tokens_in: None,
            tokens_out: None,
            tools_used: tools.map(|t| t.iter().map(|s| s.to_string()).collect()),
            host: None,
            status: None,
            summary: summary.map(|s| s.to_string()),
            session_type: None,
        }
    }

    fn disabled_config() -> GraphConfig {
        GraphConfig {
            semantic: true,
            semantic_backend: "disabled".to_string(),
            ollama_url: None,
            ollama_model: None,
            anthropic_model: None,
        }
    }

    #[tokio::test]
    async fn test_extract_and_store_rules_only() {
        let db = Database::open_memory().unwrap();
        let config = disabled_config();
        let fm = make_fm("sess001", Some(vec!["Edit"]), Some("closes #42"));
        let body = "> [!tool]- Edit `src/main.rs`\n\nsome content";

        let stored = extract_and_store(&db, &config, &fm, body).await.unwrap();

        assert!(stored >= 2, "expected at least 2 edges, got {}", stored);

        let neighbors = db.get_neighbors("session:sess001").unwrap();
        assert!(
            neighbors
                .iter()
                .any(|(id, rel, _)| id == "issue:42" && rel == "fixes_bug"),
            "fixes_bug edge to issue:42 expected"
        );
        assert!(
            neighbors
                .iter()
                .any(|(id, rel, _)| id == "file:src/main.rs" && rel == "modifies_file"),
            "modifies_file edge to file:src/main.rs expected"
        );
    }

    #[test]
    fn test_llm_response_parsing() {
        let json = r#"{"edges": [
            {"relation": "fixes_bug", "target_type": "issue", "target_label": "15"},
            {"relation": "introduces_tech", "target_type": "tech", "target_label": "ONNX Runtime"}
        ]}"#;

        let edges = parse_llm_edges(json, "test-session").unwrap();
        assert_eq!(edges.len(), 2);

        assert_eq!(edges[0].relation, "fixes_bug");
        assert_eq!(edges[0].target, "issue:15");
        assert_eq!(edges[0].confidence, "LLM");
        assert!((edges[0].weight - 0.8).abs() < f64::EPSILON);

        assert_eq!(edges[1].relation, "introduces_tech");
        assert_eq!(edges[1].target, "tech:ONNX Runtime");
    }

    #[test]
    fn test_llm_response_wrapped_in_codeblock() {
        let json = "```json\n{\"edges\": [{\"relation\": \"fixes_bug\", \"target_type\": \"issue\", \"target_label\": \"7\"}]}\n```";
        let edges = parse_llm_edges(json, "test-session").unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].target, "issue:7");
    }

    #[test]
    fn test_llm_invalid_json_fallback() {
        let bad_json = "not a json at all";
        let result = parse_llm_edges(bad_json, "test-session");
        assert!(result.is_err(), "invalid JSON should return Err");
    }

    #[test]
    fn test_llm_filters_invalid_relations() {
        let json = r#"{"edges": [
            {"relation": "fixes_bug", "target_type": "issue", "target_label": "1"},
            {"relation": "closes", "target_type": "issue", "target_label": "2"},
            {"relation": "invented_relation", "target_type": "tech", "target_label": "foo"}
        ]}"#;
        let edges = parse_llm_edges(json, "test-session").unwrap();
        assert_eq!(edges.len(), 1, "only fixes_bug should pass filter");
        assert_eq!(edges[0].relation, "fixes_bug");
    }

    #[tokio::test]
    async fn test_extract_and_store_fallback_without_llm() {
        let db = Database::open_memory().unwrap();
        let config = disabled_config();
        let fm = make_fm("fallback01", Some(vec!["Edit"]), Some("closes #7"));
        let body = "> [!tool]- Edit `src/lib.rs`\n\nsome code";

        let stored = extract_and_store(&db, &config, &fm, body).await.unwrap();

        assert!(
            stored >= 2,
            "expected at least 2 rule-based edges, got {}",
            stored
        );

        let neighbors = db.get_neighbors("session:fallback01").unwrap();
        assert!(
            neighbors
                .iter()
                .any(|(id, rel, _)| id == "issue:7" && rel == "fixes_bug"),
            "fixes_bug edge expected"
        );
        assert!(
            neighbors
                .iter()
                .any(|(id, rel, _)| id == "file:src/lib.rs" && rel == "modifies_file"),
            "modifies_file edge expected"
        );
    }

    #[tokio::test]
    async fn test_extract_and_store_double_call_returns_zero() {
        let db = Database::open_memory().unwrap();
        let config = disabled_config();
        let fm = make_fm("double01", Some(vec!["Edit"]), Some("closes #99"));
        let body = "> [!tool]- Edit `src/app.rs`\n\nsome content";

        let first = extract_and_store(&db, &config, &fm, body).await.unwrap();
        assert!(
            first >= 2,
            "first call should store at least 2 edges, got {}",
            first
        );

        let second = extract_and_store(&db, &config, &fm, body).await.unwrap();
        assert_eq!(
            second, 0,
            "second call should return 0 (all edges already exist)"
        );
    }

    #[test]
    fn test_dedup_edges_before_store() {
        let edges = vec![
            GraphEdge {
                source: "session:s1".to_string(),
                target: "issue:42".to_string(),
                relation: "fixes_bug".to_string(),
                confidence: "INFERRED".to_string(),
                weight: 0.7,
            },
            GraphEdge {
                source: "session:s1".to_string(),
                target: "issue:42".to_string(),
                relation: "fixes_bug".to_string(),
                confidence: "LLM".to_string(),
                weight: 0.8,
            },
        ];

        let mut deduped = edges;
        {
            let mut seen = std::collections::HashSet::new();
            deduped
                .retain(|e| seen.insert((e.source.clone(), e.target.clone(), e.relation.clone())));
        }
        assert_eq!(deduped.len(), 1, "duplicate edges should be removed");
        assert_eq!(deduped[0].confidence, "INFERRED");
    }
}
