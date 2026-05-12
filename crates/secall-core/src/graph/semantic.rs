use anyhow::Result;
use serde::Deserialize;

use crate::ingest::markdown::SessionFrontmatter;
use crate::llm::defaults::{
    warn_using_default, GRAPH_ANTHROPIC_DEFAULT, GRAPH_LMSTUDIO_DEFAULT,
    GRAPH_OLLAMA_CLOUD_DEFAULT, GRAPH_OLLAMA_DEFAULT,
};
use crate::store::Database;
use crate::vault::config::GraphConfig;

use super::extract::{extract_semantic_edges, GraphEdge};
use super::llm::{
    AnthropicGraphBackend, LlmBackend, OllamaCloudGraphBackend, OllamaGraphBackend,
    OpenAiCompatGraphBackend,
};

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

// P50-B: 백엔드별 응답 struct (AnthropicResponse / OllamaResponse / OpenAIResponse)
// 는 graph/llm.rs 로 이동했다.

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

const BODY_LIMIT: usize = 8000;

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

// P50-B: 백엔드별 HTTP 호출 함수 (extract_with_anthropic / extract_with_ollama /
// extract_with_ollama_cloud / extract_with_openai_compat) 는 graph/llm.rs 의
// `LlmBackend` trait + 4 struct impl 로 이동했다. 디스패치는 아래
// `extract_with_llm` 한 곳으로 집중.

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
/// Internal helper exposed for crate-local tests.
pub(crate) async fn extract_with_llm(
    config: &GraphConfig,
    fm: &SessionFrontmatter,
    body: &str,
) -> Result<Vec<GraphEdge>> {
    let backend: Box<dyn LlmBackend> = build_backend(config)?;
    let user_content = build_user_content(fm, body);
    tracing::debug!(backend = backend.name(), session = %fm.session_id, "graph semantic LLM 호출");
    let text = backend.generate(SYSTEM_PROMPT, &user_content).await?;
    parse_llm_edges(&text, &fm.session_id)
}

/// `config.semantic_backend` 에 맞는 `LlmBackend` 구현체를 만들어 반환한다.
/// 모델/URL 디폴트 적용과 cloud API key 검증을 한 자리에서 처리.
fn build_backend(config: &GraphConfig) -> Result<Box<dyn LlmBackend>> {
    match config.semantic_backend.as_str() {
        "ollama" => {
            let base_url = config
                .ollama_url
                .as_deref()
                .unwrap_or("http://localhost:11434");
            let model = config.ollama_model.as_deref().unwrap_or_else(|| {
                warn_using_default("graph.ollama_model", GRAPH_OLLAMA_DEFAULT);
                GRAPH_OLLAMA_DEFAULT
            });
            Ok(Box::new(OllamaGraphBackend {
                base_url: base_url.to_string(),
                model: model.to_string(),
            }))
        }
        "anthropic" => {
            let api_key = std::env::var("ANTHROPIC_API_KEY")
                .map_err(|_| anyhow::anyhow!("ANTHROPIC_API_KEY not set"))?;
            let model = config.anthropic_model.as_deref().unwrap_or_else(|| {
                warn_using_default("graph.anthropic_model", GRAPH_ANTHROPIC_DEFAULT);
                GRAPH_ANTHROPIC_DEFAULT
            });
            Ok(Box::new(AnthropicGraphBackend {
                api_key,
                model: model.to_string(),
            }))
        }
        "lmstudio" => {
            let base_url = config
                .ollama_url
                .as_deref()
                .unwrap_or("http://localhost:1234");
            let model = config.ollama_model.as_deref().unwrap_or_else(|| {
                warn_using_default("graph.ollama_model", GRAPH_LMSTUDIO_DEFAULT);
                GRAPH_LMSTUDIO_DEFAULT
            });
            Ok(Box::new(OpenAiCompatGraphBackend {
                base_url: base_url.to_string(),
                model: model.to_string(),
            }))
        }
        "ollama_cloud" => {
            let base_url = config
                .cloud_host
                .as_deref()
                .unwrap_or("https://ollama.com")
                .to_string();
            let model = config.cloud_model.as_deref().unwrap_or_else(|| {
                warn_using_default("graph.cloud_model", GRAPH_OLLAMA_CLOUD_DEFAULT);
                GRAPH_OLLAMA_CLOUD_DEFAULT
            });
            let api_key = config
                .cloud_api_key
                .as_deref()
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "ollama cloud api key not set \
                         (set `OLLAMA_CLOUD_API_KEY` env or \
                         `[graph].cloud_api_key` in config.toml)"
                    )
                })?
                .to_string();
            Ok(Box::new(OllamaCloudGraphBackend {
                base_url,
                model: model.to_string(),
                api_key,
            }))
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
    use mockito::{Matcher, Server};

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
            archived: None,
            archived_at: None,
        }
    }

    fn disabled_config() -> GraphConfig {
        GraphConfig {
            semantic: true,
            semantic_backend: "disabled".to_string(),
            ..GraphConfig::default()
        }
    }

    fn ollama_response() -> String {
        serde_json::json!({
            "message": {
                "content": r#"{"edges":[]}"#
            }
        })
        .to_string()
    }

    fn openai_compat_response() -> String {
        serde_json::json!({
            "choices": [{
                "message": {
                    "content": r#"{"edges":[]}"#
                }
            }]
        })
        .to_string()
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

    #[tokio::test]
    async fn test_extract_with_llm_ollama_uses_config_model() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/api/chat")
            .match_header("content-type", "application/json")
            .match_body(Matcher::Regex(r#""model":"custom-model""#.to_string()))
            .with_status(200)
            .with_body(ollama_response())
            .create_async()
            .await;

        let config = GraphConfig {
            semantic_backend: "ollama".to_string(),
            ollama_url: Some(server.url()),
            ollama_model: Some("custom-model".to_string()),
            ..GraphConfig::default()
        };

        let edges = extract_with_llm(&config, &make_fm("sess-semantic", None, None), "body")
            .await
            .expect("ollama extract");
        assert!(edges.is_empty());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_extract_with_llm_ollama_falls_back_to_default_model() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/api/chat")
            .match_body(Matcher::Regex(format!(
                r#""model":"{}""#,
                GRAPH_OLLAMA_DEFAULT
            )))
            .with_status(200)
            .with_body(ollama_response())
            .create_async()
            .await;

        let config = GraphConfig {
            semantic_backend: "ollama".to_string(),
            ollama_url: Some(server.url()),
            ollama_model: None,
            ..GraphConfig::default()
        };

        let edges = extract_with_llm(&config, &make_fm("sess-semantic", None, None), "body")
            .await
            .expect("ollama default extract");
        assert!(edges.is_empty());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_extract_with_llm_lmstudio_uses_lmstudio_default() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/chat/completions")
            .match_body(Matcher::Regex(format!(
                r#""model":"{}""#,
                GRAPH_LMSTUDIO_DEFAULT
            )))
            .with_status(200)
            .with_body(openai_compat_response())
            .create_async()
            .await;

        let config = GraphConfig {
            semantic_backend: "lmstudio".to_string(),
            ollama_url: Some(server.url()),
            ollama_model: None,
            ..GraphConfig::default()
        };

        let edges = extract_with_llm(&config, &make_fm("sess-semantic", None, None), "body")
            .await
            .expect("lmstudio extract");
        assert!(edges.is_empty());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_extract_with_llm_unknown_backend_errors() {
        let config = GraphConfig {
            semantic_backend: "nonsense".to_string(),
            ..GraphConfig::default()
        };

        let err = extract_with_llm(&config, &make_fm("sess-semantic", None, None), "body")
            .await
            .expect_err("unknown backend should fail");
        assert!(err.to_string().contains("unknown semantic_backend"));
    }

    #[tokio::test]
    async fn test_extract_with_llm_ollama_cloud_requires_api_key() {
        let config = GraphConfig {
            semantic_backend: "ollama_cloud".to_string(),
            cloud_api_key: None, // no key set
            ..GraphConfig::default()
        };

        let err = extract_with_llm(&config, &make_fm("sess-cloud", None, None), "body")
            .await
            .expect_err("ollama_cloud without api key should fail");
        assert!(
            err.to_string().contains("ollama cloud api key not set"),
            "unexpected error: {err}"
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

    /// 실제 Ollama 서버가 실행 중일 때만 수행하는 통합 테스트.
    /// `cargo test -- --ignored ollama` 로 실행.
    #[tokio::test]
    #[ignore = "requires running Ollama with gemma4:e4b model"]
    async fn test_ollama_live_extract() {
        let db = Database::open_memory().unwrap();
        let config = GraphConfig {
            semantic: true,
            semantic_backend: "ollama".to_string(),
            ollama_url: Some("http://localhost:11434".to_string()),
            ollama_model: Some("gemma4:e4b".to_string()),
            ..GraphConfig::default()
        };
        // introduces_tech 와 discusses_topic 을 유도하는 세션
        let fm = make_fm(
            "ollama-live-01",
            Some(vec!["Bash", "Edit"]),
            Some("Add tokio async runtime for HTTP server"),
        );
        let body = r#"## Turn 1 — User
Add an async HTTP server using tokio and hyper.

## Turn 2 — Assistant
Added tokio and hyper dependencies. Created src/server.rs with async handler."#;

        let result = extract_and_store(&db, &config, &fm, body).await;
        assert!(
            result.is_ok(),
            "extract_and_store should succeed: {:?}",
            result
        );

        let stored = result.unwrap();
        println!("Stored {} edges", stored);

        let neighbors = db.get_neighbors("session:ollama-live-01").unwrap();
        println!("Neighbors: {:?}", neighbors);

        // 최소한 LLM이 tech/topic 엣지를 하나 이상 추출했거나
        // 아니면 규칙 기반만으로도 0 이상이어야 함
        let _ = stored; // tautology check 제거 — panic 없이 여기 도달하면 성공
    }

    // P50-B: 백엔드 직접 호출 단위 테스트 (bearer auth, endpoint matching) 는
    // graph/llm.rs 의 `cfg(test) mod tests` 로 이동했다. 여기 남은 테스트는
    // dispatcher (`extract_with_llm`) + parse_llm_edges 통합 검증.

    #[test]
    fn test_make_fm_with_summary_includes_summary_field() {
        let fm = make_fm("sess-summary", None, Some("closes #99"));
        assert_eq!(
            fm.summary.as_deref(),
            Some("closes #99"),
            "summary field should be set from make_fm third arg"
        );
        let fm_no_summary = make_fm("sess-no-summary", None, None);
        assert!(fm_no_summary.summary.is_none());
    }
}
