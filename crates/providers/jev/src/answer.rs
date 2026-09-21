//! Typed answers returned by a System One model.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::JevError;

/// The answer to a [`crate::Question::Noul`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoulAnswer {
    /// Probability in `[0, 1]` that the answer is yes.
    pub noul: f64,
}

/// The answer to a [`crate::Question::Choice`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChoiceAnswer {
    /// The selected option label.
    pub choice: String,
    /// Probability mass over every option offered.
    pub probabilities: BTreeMap<String, f64>,
    /// How sure the model is of the distribution itself, in `[0, 1]`.
    pub confidence: f64,
}

impl ChoiceAnswer {
    /// The probability assigned to `option`, or `0.0` if it was not scored.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use jev_provider::ChoiceAnswer;
    ///
    /// let answer: ChoiceAnswer = serde_json::from_str(
    ///     r#"{"choice":"execute","probabilities":{"execute":0.9,"skip":0.1},"confidence":0.8}"#,
    /// )
    /// .unwrap();
    ///
    /// assert_eq!(answer.probability("execute"), 0.9);
    /// assert_eq!(answer.probability("never_offered"), 0.0);
    /// ```
    pub fn probability(&self, option: &str) -> f64 {
        self.probabilities.get(option).copied().unwrap_or(0.0)
    }
}

/// The answer to a [`crate::Question::Score`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoreAnswer {
    /// The selected level index, as a number.
    pub score: f64,
    /// Level index (stringified) to the description that was supplied.
    #[serde(default)]
    pub legend: BTreeMap<String, String>,
    /// Probability mass over every level.
    #[serde(default)]
    pub probabilities: BTreeMap<String, f64>,
    /// How sure the model is of the distribution itself, in `[0, 1]`.
    pub confidence: f64,
}

impl ScoreAnswer {
    /// The description of the selected level, if the legend carries one.
    pub fn label(&self) -> Option<&str> {
        self.legend
            .get(&format!("{}", self.score as i64))
            .map(String::as_str)
    }

    /// The selected level rescaled to `[0, 1]` across the legend's levels.
    ///
    /// Returns `0.0` when the legend has fewer than two levels, since there is
    /// then no range to normalise against.
    pub fn normalized(&self) -> f64 {
        let levels = self.legend.len();
        if levels < 2 {
            return 0.0;
        }
        (self.score / (levels - 1) as f64).clamp(0.0, 1.0)
    }
}

/// One answer, tagged by the primitive that produced it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Answer {
    /// A yes/no answer.
    Noul(NoulAnswer),
    /// A pick-one answer.
    Choice(ChoiceAnswer),
    /// An ordinal answer.
    Score(ScoreAnswer),
}

impl Answer {
    /// The wire name of this answer's primitive, for error messages.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Noul(_) => "noul",
            Self::Choice(_) => "choice",
            Self::Score(_) => "score",
        }
    }
}

/// Token accounting for a single request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Usage {
    /// Tokens consumed by the state and questions.
    #[serde(default)]
    pub input_tokens: u64,
    /// Tokens produced. Complimentary on current Jev pricing.
    #[serde(default)]
    pub output_tokens: u64,
}

/// A full response from `POST /v1/systemone`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemOneResponse {
    /// The concrete model version that served the request.
    pub model: String,
    /// One answer per question id that was sent.
    pub answers: BTreeMap<String, Answer>,
    /// Token accounting for the request.
    #[serde(default)]
    pub usage: Usage,
}

impl SystemOneResponse {
    /// Looks up the noul answer named `id`.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::MissingAnswer`] when no answer carries that id, or
    /// [`JevError::AnswerKindMismatch`] when the answer is a different primitive.
    pub fn noul(&self, id: &str) -> Result<&NoulAnswer, JevError> {
        match self.require(id)? {
            Answer::Noul(a) => Ok(a),
            other => Err(JevError::AnswerKindMismatch {
                id: id.to_string(),
                expected: "noul",
                actual: other.kind(),
            }),
        }
    }

    /// Looks up the choice answer named `id`.
    ///
    /// # Errors
    ///
    /// As [`SystemOneResponse::noul`], for the choice primitive.
    pub fn choice(&self, id: &str) -> Result<&ChoiceAnswer, JevError> {
        match self.require(id)? {
            Answer::Choice(a) => Ok(a),
            other => Err(JevError::AnswerKindMismatch {
                id: id.to_string(),
                expected: "choice",
                actual: other.kind(),
            }),
        }
    }

    /// Looks up the score answer named `id`.
    ///
    /// # Errors
    ///
    /// As [`SystemOneResponse::noul`], for the score primitive.
    pub fn score(&self, id: &str) -> Result<&ScoreAnswer, JevError> {
        match self.require(id)? {
            Answer::Score(a) => Ok(a),
            other => Err(JevError::AnswerKindMismatch {
                id: id.to_string(),
                expected: "score",
                actual: other.kind(),
            }),
        }
    }

    fn require(&self, id: &str) -> Result<&Answer, JevError> {
        self.answers
            .get(id)
            .ok_or_else(|| JevError::MissingAnswer(id.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
      "model": "jev-1.13.0",
      "answers": {
        "department": {
          "type": "choice",
          "choice": "technical",
          "confidence": 0.78,
          "probabilities": {"technical": 0.85, "sales": 0.0, "billing": 0.15}
        },
        "frustration": {
          "type": "score",
          "score": 1.0,
          "confidence": 1.0,
          "legend": {"0": "Calm", "1": "Frustrated", "2": "Very angry"},
          "probabilities": {"0": 0.0, "1": 1.0, "2": 0.0}
        },
        "is_urgent": {"type": "noul", "noul": 1.0}
      },
      "usage": {"input_tokens": 392, "output_tokens": 65}
    }"#;

    fn sample() -> SystemOneResponse {
        serde_json::from_str(SAMPLE).unwrap()
    }

    #[test]
    fn parses_the_documented_response_shape() {
        let r = sample();
        assert_eq!(r.model, "jev-1.13.0");
        assert_eq!(r.usage.input_tokens, 392);
        assert_eq!(r.answers.len(), 3);
    }

    #[test]
    fn typed_accessors_return_each_primitive() {
        let r = sample();
        assert_eq!(r.noul("is_urgent").unwrap().noul, 1.0);
        assert_eq!(r.choice("department").unwrap().choice, "technical");
        assert_eq!(r.score("frustration").unwrap().score, 1.0);
    }

    #[test]
    fn probability_defaults_to_zero_for_unknown_option() {
        let r = sample();
        let c = r.choice("department").unwrap();
        assert_eq!(c.probability("technical"), 0.85);
        assert_eq!(c.probability("nonexistent"), 0.0);
    }

    #[test]
    fn missing_answer_is_an_error() {
        let err = sample().noul("absent").unwrap_err();
        assert!(matches!(err, JevError::MissingAnswer(id) if id == "absent"));
    }

    #[test]
    fn wrong_primitive_is_an_error_naming_both_kinds() {
        let err = sample().noul("department").unwrap_err();
        match err {
            JevError::AnswerKindMismatch {
                expected, actual, ..
            } => {
                assert_eq!(expected, "noul");
                assert_eq!(actual, "choice");
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn score_label_and_normalization_use_the_legend() {
        let r = sample();
        let s = r.score("frustration").unwrap();
        assert_eq!(s.label(), Some("Frustrated"));
        assert_eq!(s.normalized(), 0.5);
    }

    #[test]
    fn normalization_is_zero_when_legend_is_degenerate() {
        let s = ScoreAnswer {
            score: 3.0,
            legend: BTreeMap::new(),
            probabilities: BTreeMap::new(),
            confidence: 1.0,
        };
        assert_eq!(s.normalized(), 0.0);
    }

    #[test]
    fn usage_defaults_when_absent() {
        let r: SystemOneResponse = serde_json::from_str(
            r#"{"model":"jev-1.13.0","answers":{"a":{"type":"noul","noul":0.5}}}"#,
        )
        .unwrap();
        assert_eq!(r.usage, Usage::default());
    }
}
