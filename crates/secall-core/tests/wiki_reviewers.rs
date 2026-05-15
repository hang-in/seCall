use secall_core::wiki::{LmStudioReviewer, OllamaReviewer, WikiReviewer};

#[tokio::test]
async fn ollama_reviewer_parses_valid_response() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/api/chat")
        .with_status(200)
        .with_body(
            serde_json::json!({
                "message": { "content": r#"{"approved":true,"issues":[]}"# }
            })
            .to_string(),
        )
        .create_async()
        .await;

    let result = OllamaReviewer {
        api_url: server.url(),
        model: "gemma4".into(),
        api_key: None,
    }
    .review("page", "summary")
    .await
    .unwrap();

    assert!(result.approved);
    mock.assert_async().await;
}

#[tokio::test]
async fn ollama_reviewer_retries_on_parse_failure() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/api/chat")
        .with_status(200)
        .with_body_from_request(|req| {
            let body = String::from_utf8_lossy(req.body().expect("request body"));
            if body.contains("재시도 지시") {
                serde_json::json!({
                    "message": { "content": r#"{"approved":true,"issues":[]}"# }
                })
                .to_string()
                .into()
            } else {
                serde_json::json!({
                    "message": { "content": "not-json" }
                })
                .to_string()
                .into()
            }
        })
        .expect(2)
        .create_async()
        .await;

    let result = OllamaReviewer {
        api_url: server.url(),
        model: "gemma4".into(),
        api_key: None,
    }
    .review("page", "summary")
    .await
    .unwrap();

    assert!(result.approved);
    mock.assert_async().await;
}

#[tokio::test]
async fn lmstudio_reviewer_parses_response_format_json_object() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/chat/completions")
        .match_body(mockito::Matcher::Regex("json_object".into()))
        .with_status(200)
        .with_body(
            serde_json::json!({
                "choices": [{
                    "message": { "content": r#"{"approved":false,"issues":[{"severity":"error","description":"bad","suggestion":null}]}"# }
                }]
            })
            .to_string(),
        )
        .create_async()
        .await;

    let result = LmStudioReviewer {
        api_url: server.url(),
        model: "local-model".into(),
    }
    .review("page", "summary")
    .await
    .unwrap();

    assert!(!result.approved);
    assert_eq!(result.issues.len(), 1);
    mock.assert_async().await;
}

#[tokio::test]
async fn ollama_reviewer_fails_after_two_parse_failures() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/api/chat")
        .with_status(200)
        .with_body(
            serde_json::json!({
                "message": { "content": "still-not-json" }
            })
            .to_string(),
        )
        .expect(2)
        .create_async()
        .await;

    let err = OllamaReviewer {
        api_url: server.url(),
        model: "gemma4".into(),
        api_key: None,
    }
    .review("page", "summary")
    .await
    .unwrap_err();

    assert!(err.to_string().contains("parse failed after retry"));
    mock.assert_async().await;
}
