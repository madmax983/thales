//! Client for TypeSafe AI's System One API, whose current model is **Jev**.
//!
//! A System One model is not autoregressive. Instead of emitting tokens, it
//! evaluates *typed questions* against a *state* and returns the answers
//! directly, each with a probability distribution and a confidence score. That
//! makes it a good fit for the places a trading pipeline needs a judgement
//! rather than prose: is this signal worth taking, does the strategy suit the
//! regime, what regime is this.
//!
//! Three primitives exist, and all three can be mixed in one request:
//!
//! - [`Question::noul`] — yes/no, answered with a probability in `[0, 1]`.
//! - [`Question::choice`] — pick one of up to 255 labelled options.
//! - [`Question::score`] — an ordinal rating over 2 to 10 ordered levels.
//!
//! # Examples
//!
//! ```rust,no_run
//! use jev_provider::{JevClient, Question, Questions};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let client = JevClient::from_env()?;
//!
//! let mut questions = Questions::new();
//! questions.insert(
//!     "verdict".to_string(),
//!     Question::choice(
//!         "Should this trade be executed?",
//!         [("execute", "Take it as proposed"), ("skip", "Stand aside")],
//!     ),
//! );
//!
//! let state = serde_json::json!({ "symbol": "BTCUSD", "regime": "Trending Up" });
//! let response = client.ask(&state, &questions)?;
//!
//! let verdict = response.choice("verdict")?;
//! println!("{} at p={:.2}", verdict.choice, verdict.probability(&verdict.choice));
//! # Ok(())
//! # }
//! ```
//!
//! # Configuration
//!
//! - `TYPESAFE_API_KEY` (required)
//! - `TYPESAFE_BASE_URL` (optional, defaults to `<https://api.typesafe.ai>`)
//! - `TYPESAFE_MODEL` (optional, defaults to `jev-latest`)

pub mod answer;
pub mod client;
pub mod error;
pub mod question;

pub use answer::{Answer, ChoiceAnswer, NoulAnswer, ScoreAnswer, SystemOneResponse, Usage};
pub use client::{DEFAULT_BASE_URL, DEFAULT_MODEL, JevClient, JevConfig, SYSTEM_ONE_PATH};
pub use error::JevError;
pub use question::{
    MAX_CHOICE_OPTIONS, MAX_SCORE_LEVELS, MIN_SCORE_LEVELS, NoulCriteria, Question, Questions,
};
