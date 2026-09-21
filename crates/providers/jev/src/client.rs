//! Blocking client for `POST /v1/systemone`.

use std::time::Duration;

use reqwest::blocking::Client;
use serde::Serialize;

use crate::answer::SystemOneResponse;
use crate::error::JevError;
use crate::question::Questions;

/// Default API root.
pub const DEFAULT_BASE_URL: &str = "https://api.typesafe.ai";
/// Default model alias, which currently resolves to `jev-1.13.0`.
pub const DEFAULT_MODEL: &str = "jev-latest";
/// Path of the System One endpoint, appended to the base URL.
pub const SYSTEM_ONE_PATH: &str = "/v1/systemone";

const MAX_ERROR_BODY: usize = 512;

/// Connection settings for [`JevClient`].
///
/// # Examples
///
/// ```rust
/// use jev_provider::JevConfig;
///
/// let config = JevConfig::from_env_with(|key| match key {
///     "TYPESAFE_API_KEY" => Some("sk-test".to_string()),
///     _ => None,
/// })
/// .unwrap();
///
/// assert_eq!(config.model, "jev-latest");
/// assert_eq!(config.base_url, "https://api.typesafe.ai");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JevConfig {
    /// Bearer token sent on every request.
    pub api_key: String,
    /// API root, without a trailing slash.
    pub base_url: String,
    /// Model id or alias to request.
    pub model: String,
    /// Total attempts, including the first, before giving up on 429/529.
    pub max_attempts: u32,
    /// Delay before the first retry; doubled on each subsequent retry.
    pub retry_base_delay: Duration,
    /// Per-request timeout.
    pub timeout: Duration,
}

impl JevConfig {
    /// Loads configuration from the process environment.
    ///
    /// # Required Variables
    ///
    /// - `TYPESAFE_API_KEY`
    ///
    /// # Optional Variables
    ///
    /// - `TYPESAFE_BASE_URL` (defaults to `<https://api.typesafe.ai>`)
    /// - `TYPESAFE_MODEL` (defaults to `jev-latest`)
    ///
    /// # Errors
    ///
    /// Returns [`JevError::MissingApiKey`] when the key is absent or blank.
    pub fn from_env() -> Result<Self, JevError> {
        Self::from_env_with(|key| std::env::var(key).ok())
    }

    /// Loads configuration from an arbitrary lookup, for tests and embedding.
    ///
    /// # Errors
    ///
    /// As [`JevConfig::from_env`].
    pub fn from_env_with<F>(get: F) -> Result<Self, JevError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let api_key = get("TYPESAFE_API_KEY")
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
            .ok_or(JevError::MissingApiKey)?;

        let base_url = get("TYPESAFE_BASE_URL")
            .map(|u| u.trim().trim_end_matches('/').to_string())
            .filter(|u| !u.is_empty())
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());

        let model = get("TYPESAFE_MODEL")
            .map(|m| m.trim().to_string())
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| DEFAULT_MODEL.to_string());

        Ok(Self {
            api_key,
            base_url,
            model,
            max_attempts: 4,
            retry_base_delay: Duration::from_secs(2),
            timeout: Duration::from_secs(30),
        })
    }
}

/// The body sent to the System One endpoint.
#[derive(Debug, Clone, Serialize)]
struct SystemOneRequest<'a, S: Serialize> {
    state: &'a S,
    model: &'a str,
    questions: &'a Questions,
}

/// A client for TypeSafe AI's System One endpoint.
///
/// Every call answers the whole question set against one state in a single
/// round trip, so callers should batch related questions rather than looping.
pub struct JevClient {
    config: JevConfig,
    http: Client,
}

impl JevClient {
    /// Builds a client from explicit configuration.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::Http`] when the underlying HTTP client cannot be built.
    pub fn new(config: JevConfig) -> Result<Self, JevError> {
        let http = Client::builder().timeout(config.timeout).build()?;
        Ok(Self { config, http })
    }

    /// Builds a client from the process environment.
    ///
    /// # Errors
    ///
    /// As [`JevConfig::from_env`] and [`JevClient::new`].
    pub fn from_env() -> Result<Self, JevError> {
        Self::new(JevConfig::from_env()?)
    }

    /// The configuration this client was built with.
    pub fn config(&self) -> &JevConfig {
        &self.config
    }

    /// Evaluates `questions` against `state`.
    ///
    /// Every question is validated locally first, so a malformed set fails
    /// without spending a request. Rate limits (429) and overload (529) are
    /// retried with exponential backoff, honouring `Retry-After` when present.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::NoQuestions`] for an empty set,
    /// [`JevError::InvalidQuestion`] when a question breaks an API limit,
    /// [`JevError::Auth`] on 401, [`JevError::Validation`] on 422,
    /// [`JevError::RateLimited`] or [`JevError::Overloaded`] once retries are
    /// exhausted, and [`JevError::Decode`] when the body is not the documented shape.
    pub fn ask<S: Serialize>(
        &self,
        state: &S,
        questions: &Questions,
    ) -> Result<SystemOneResponse, JevError> {
        if questions.is_empty() {
            return Err(JevError::NoQuestions);
        }
        for question in questions.values() {
            question.validate()?;
        }

        let body = SystemOneRequest {
            state,
            model: &self.config.model,
            questions,
        };
        let url = format!("{}{}", self.config.base_url, SYSTEM_ONE_PATH);
        let attempts = self.config.max_attempts.max(1);

        let mut last_retryable = None;
        for attempt in 1..=attempts {
            let response = self
                .http
                .post(&url)
                .bearer_auth(&self.config.api_key)
                .json(&body)
                .send()?;

            let status = response.status().as_u16();
            if status == 429 || status == 529 {
                last_retryable = Some(status);
                if attempt < attempts {
                    std::thread::sleep(self.backoff(attempt, retry_after(&response)));
                    continue;
                }
                break;
            }

            return self.decode(response, status);
        }

        Err(match last_retryable {
            Some(429) => JevError::RateLimited { attempts },
            _ => JevError::Overloaded { attempts },
        })
    }

    fn decode(
        &self,
        response: reqwest::blocking::Response,
        status: u16,
    ) -> Result<SystemOneResponse, JevError> {
        if status == 401 {
            return Err(JevError::Auth);
        }
        if status == 422 {
            return Err(JevError::Validation(read_body(response)));
        }
        if !(200..300).contains(&status) {
            return Err(JevError::UnexpectedStatus {
                status,
                body: read_body(response),
            });
        }

        let text = response.text()?;
        serde_json::from_str(&text).map_err(|e| {
            JevError::Decode(format!("{e} (body: {})", truncate(&text, MAX_ERROR_BODY)))
        })
    }

    fn backoff(&self, attempt: u32, retry_after: Option<Duration>) -> Duration {
        if let Some(server_hint) = retry_after {
            return server_hint;
        }
        let factor = 1u32 << (attempt - 1);
        self.config.retry_base_delay * factor
    }
}

fn retry_after(response: &reqwest::blocking::Response) -> Option<Duration> {
    response
        .headers()
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
        .map(Duration::from_secs)
}

fn read_body(response: reqwest::blocking::Response) -> String {
    let text = response.text().unwrap_or_default();
    truncate(&text, MAX_ERROR_BODY)
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let head: String = text.chars().take(max).collect();
    format!("{head}...")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_defaults_are_applied() {
        let config = JevConfig::from_env_with(|k| match k {
            "TYPESAFE_API_KEY" => Some(" sk-test ".to_string()),
            _ => None,
        })
        .unwrap();
        assert_eq!(config.api_key, "sk-test");
        assert_eq!(config.base_url, DEFAULT_BASE_URL);
        assert_eq!(config.model, DEFAULT_MODEL);
    }

    #[test]
    fn env_overrides_are_honoured_and_normalised() {
        let config = JevConfig::from_env_with(|k| match k {
            "TYPESAFE_API_KEY" => Some("sk".to_string()),
            "TYPESAFE_BASE_URL" => Some("https://proxy.internal/".to_string()),
            "TYPESAFE_MODEL" => Some("jev-preview".to_string()),
            _ => None,
        })
        .unwrap();
        assert_eq!(config.base_url, "https://proxy.internal");
        assert_eq!(config.model, "jev-preview");
    }

    #[test]
    fn blank_api_key_is_missing() {
        let err = JevConfig::from_env_with(|k| match k {
            "TYPESAFE_API_KEY" => Some("   ".to_string()),
            _ => None,
        })
        .unwrap_err();
        assert!(matches!(err, JevError::MissingApiKey));
    }

    #[test]
    fn blank_optional_vars_fall_back_to_defaults() {
        let config = JevConfig::from_env_with(|k| match k {
            "TYPESAFE_API_KEY" => Some("sk".to_string()),
            "TYPESAFE_BASE_URL" => Some("  ".to_string()),
            "TYPESAFE_MODEL" => Some("".to_string()),
            _ => None,
        })
        .unwrap();
        assert_eq!(config.base_url, DEFAULT_BASE_URL);
        assert_eq!(config.model, DEFAULT_MODEL);
    }

    #[test]
    fn truncate_keeps_short_text_intact() {
        assert_eq!(truncate("abc", 10), "abc");
        assert_eq!(truncate("abcdef", 3), "abc...");
    }
}
