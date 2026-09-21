//! HTTP-level behaviour of [`JevClient`], exercised against a mock server.

use std::time::Duration;

use jev_provider::{JevClient, JevConfig, JevError, Question, Questions};

fn config(base_url: String) -> JevConfig {
    JevConfig {
        api_key: "sk-test".to_string(),
        base_url,
        model: "jev-latest".to_string(),
        max_attempts: 3,
        // Keep retries instant so the suite stays fast.
        retry_base_delay: Duration::from_millis(1),
        timeout: Duration::from_secs(5),
    }
}

fn questions() -> Questions {
    let mut q = Questions::new();
    q.insert(
        "verdict".to_string(),
        Question::choice(
            "Take the trade?",
            [("execute", "Take it"), ("skip", "Stand aside")],
        ),
    );
    q.insert(
        "urgent".to_string(),
        Question::noul("Is this time sensitive?"),
    );
    q
}

const OK_BODY: &str = r#"{
  "model": "jev-1.13.0",
  "answers": {
    "verdict": {
      "type": "choice",
      "choice": "execute",
      "confidence": 0.81,
      "probabilities": {"execute": 0.74, "skip": 0.26}
    },
    "urgent": {"type": "noul", "noul": 0.9}
  },
  "usage": {"input_tokens": 120, "output_tokens": 8}
}"#;

#[test]
fn posts_to_system_one_with_bearer_auth_and_documented_body() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("POST", "/v1/systemone")
        .match_header("authorization", "Bearer sk-test")
        .match_header("content-type", "application/json")
        .match_body(mockito::Matcher::PartialJson(serde_json::json!({
            "model": "jev-latest",
            "state": {"symbol": "BTCUSD"},
            "questions": {
                "verdict": {
                    "type": "choice",
                    "instructions": "Take the trade?",
                    "criteria": {"execute": "Take it", "skip": "Stand aside"}
                },
                "urgent": {"type": "noul", "instructions": "Is this time sensitive?"}
            }
        })))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(OK_BODY)
        .create();

    let client = JevClient::new(config(server.url())).unwrap();
    let state = serde_json::json!({"symbol": "BTCUSD"});
    let response = client.ask(&state, &questions()).unwrap();

    mock.assert();
    assert_eq!(response.model, "jev-1.13.0");
    let verdict = response.choice("verdict").unwrap();
    assert_eq!(verdict.choice, "execute");
    assert_eq!(verdict.probability("execute"), 0.74);
    assert_eq!(response.noul("urgent").unwrap().noul, 0.9);
    assert_eq!(response.usage.input_tokens, 120);
}

#[test]
fn unauthorized_maps_to_auth_error() {
    let mut server = mockito::Server::new();
    let _m = server
        .mock("POST", "/v1/systemone")
        .with_status(401)
        .with_body(r#"{"error":"bad key"}"#)
        .create();

    let client = JevClient::new(config(server.url())).unwrap();
    let err = client
        .ask(&serde_json::json!("state"), &questions())
        .unwrap_err();
    assert!(matches!(err, JevError::Auth));
    assert!(!err.is_transient());
}

#[test]
fn unprocessable_surfaces_the_server_message() {
    let mut server = mockito::Server::new();
    let _m = server
        .mock("POST", "/v1/systemone")
        .with_status(422)
        .with_body(r#"{"error":"criteria too long"}"#)
        .create();

    let client = JevClient::new(config(server.url())).unwrap();
    let err = client
        .ask(&serde_json::json!("state"), &questions())
        .unwrap_err();
    match err {
        JevError::Validation(body) => assert!(body.contains("criteria too long")),
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn rate_limit_is_retried_then_succeeds() {
    let mut server = mockito::Server::new();
    let throttled = server
        .mock("POST", "/v1/systemone")
        .with_status(429)
        .with_body("slow down")
        .expect(1)
        .create();
    let ok = server
        .mock("POST", "/v1/systemone")
        .with_status(200)
        .with_body(OK_BODY)
        .create();

    let client = JevClient::new(config(server.url())).unwrap();
    let response = client
        .ask(&serde_json::json!("state"), &questions())
        .unwrap();

    throttled.assert();
    ok.assert();
    assert_eq!(response.choice("verdict").unwrap().choice, "execute");
}

#[test]
fn rate_limit_gives_up_after_max_attempts() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("POST", "/v1/systemone")
        .with_status(429)
        .expect(3)
        .create();

    let client = JevClient::new(config(server.url())).unwrap();
    let err = client
        .ask(&serde_json::json!("state"), &questions())
        .unwrap_err();

    mock.assert();
    assert!(matches!(err, JevError::RateLimited { attempts: 3 }));
    assert!(err.is_transient());
}

#[test]
fn overload_gives_up_after_max_attempts() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("POST", "/v1/systemone")
        .with_status(529)
        .expect(3)
        .create();

    let client = JevClient::new(config(server.url())).unwrap();
    let err = client
        .ask(&serde_json::json!("state"), &questions())
        .unwrap_err();

    mock.assert();
    assert!(matches!(err, JevError::Overloaded { attempts: 3 }));
}

#[test]
fn retry_after_header_is_honoured() {
    let mut server = mockito::Server::new();
    let throttled = server
        .mock("POST", "/v1/systemone")
        .with_status(429)
        .with_header("retry-after", "0")
        .expect(1)
        .create();
    let ok = server
        .mock("POST", "/v1/systemone")
        .with_status(200)
        .with_body(OK_BODY)
        .create();

    let mut cfg = config(server.url());
    // A base delay this long would stall the test if the header were ignored.
    cfg.retry_base_delay = Duration::from_secs(30);
    let client = JevClient::new(cfg).unwrap();

    let started = std::time::Instant::now();
    let response = client
        .ask(&serde_json::json!("state"), &questions())
        .unwrap();

    throttled.assert();
    ok.assert();
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(response.model, "jev-1.13.0");
}

#[test]
fn unexpected_status_carries_code_and_body() {
    let mut server = mockito::Server::new();
    let _m = server
        .mock("POST", "/v1/systemone")
        .with_status(500)
        .with_body("boom")
        .create();

    let client = JevClient::new(config(server.url())).unwrap();
    let err = client
        .ask(&serde_json::json!("state"), &questions())
        .unwrap_err();
    match err {
        JevError::UnexpectedStatus { status, body } => {
            assert_eq!(status, 500);
            assert_eq!(body, "boom");
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn malformed_body_is_a_decode_error() {
    let mut server = mockito::Server::new();
    let _m = server
        .mock("POST", "/v1/systemone")
        .with_status(200)
        .with_body("not json")
        .create();

    let client = JevClient::new(config(server.url())).unwrap();
    let err = client
        .ask(&serde_json::json!("state"), &questions())
        .unwrap_err();
    assert!(matches!(err, JevError::Decode(_)));
}

#[test]
fn empty_question_set_never_reaches_the_wire() {
    let mut server = mockito::Server::new();
    let mock = server.mock("POST", "/v1/systemone").expect(0).create();

    let client = JevClient::new(config(server.url())).unwrap();
    let err = client
        .ask(&serde_json::json!("state"), &Questions::new())
        .unwrap_err();

    mock.assert();
    assert!(matches!(err, JevError::NoQuestions));
}

#[test]
fn invalid_question_never_reaches_the_wire() {
    let mut server = mockito::Server::new();
    let mock = server.mock("POST", "/v1/systemone").expect(0).create();

    let mut q = Questions::new();
    q.insert(
        "bad".to_string(),
        Question::score("How strong?", ["only one level"]),
    );

    let client = JevClient::new(config(server.url())).unwrap();
    let err = client.ask(&serde_json::json!("state"), &q).unwrap_err();

    mock.assert();
    assert!(matches!(err, JevError::InvalidQuestion(_)));
}

#[test]
fn a_plain_string_state_is_accepted() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("POST", "/v1/systemone")
        .match_body(mockito::Matcher::PartialJson(serde_json::json!({
            "state": "BTCUSD broke resistance on rising volume"
        })))
        .with_status(200)
        .with_body(OK_BODY)
        .create();

    let client = JevClient::new(config(server.url())).unwrap();
    client
        .ask(&"BTCUSD broke resistance on rising volume", &questions())
        .unwrap();
    mock.assert();
}
