//! Errors raised by the Jev client.

use thiserror::Error;

/// Everything that can go wrong talking to the System One endpoint.
#[derive(Debug, Error)]
pub enum JevError {
    /// `TYPESAFE_API_KEY` was absent or blank.
    #[error("missing TYPESAFE_API_KEY")]
    MissingApiKey,

    /// A question failed local validation and was never sent.
    #[error("invalid question: {0}")]
    InvalidQuestion(String),

    /// No questions were supplied, so the request would be pointless.
    #[error("no questions supplied")]
    NoQuestions,

    /// HTTP 401. The API key was rejected.
    #[error("invalid TypeSafe API key (HTTP 401)")]
    Auth,

    /// HTTP 422. The server rejected the request body.
    #[error("TypeSafe rejected the request (HTTP 422): {0}")]
    Validation(String),

    /// HTTP 429, still failing after every retry.
    #[error("TypeSafe rate limit exceeded (HTTP 429) after {attempts} attempts")]
    RateLimited {
        /// How many attempts were made in total.
        attempts: u32,
    },

    /// HTTP 529, still failing after every retry.
    #[error("TypeSafe service overloaded (HTTP 529) after {attempts} attempts")]
    Overloaded {
        /// How many attempts were made in total.
        attempts: u32,
    },

    /// Any other non-success status.
    #[error("TypeSafe returned HTTP {status}: {body}")]
    UnexpectedStatus {
        /// The status code returned.
        status: u16,
        /// The response body, truncated for readability.
        body: String,
    },

    /// The request could not be completed at the transport level.
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    /// The response body was not the documented shape.
    #[error("could not decode TypeSafe response: {0}")]
    Decode(String),

    /// The response omitted an answer that was asked for.
    #[error("response is missing an answer for question '{0}'")]
    MissingAnswer(String),

    /// The response answered with a different primitive than was asked for.
    #[error("question '{id}' expected a {expected} answer but got {actual}")]
    AnswerKindMismatch {
        /// The question id.
        id: String,
        /// The primitive that was asked for.
        expected: &'static str,
        /// The primitive that came back.
        actual: &'static str,
    },
}

impl JevError {
    /// Whether retrying the same request could plausibly succeed.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use jev_provider::JevError;
    ///
    /// assert!(JevError::Overloaded { attempts: 3 }.is_transient());
    /// assert!(!JevError::Auth.is_transient());
    /// ```
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::RateLimited { .. } | Self::Overloaded { .. } | Self::Http(_)
        )
    }
}
