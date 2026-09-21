//! Typed questions posed to a System One model.
//!
//! The TypeSafe API accepts a *state* plus a map of named questions, and answers
//! every question against that state in a single round trip. Three primitives
//! exist, and this module models each one with the constraints the API enforces
//! baked into the type so an invalid question cannot reach the wire.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::JevError;

/// Maximum number of options the API accepts for a [`Question::Choice`].
pub const MAX_CHOICE_OPTIONS: usize = 255;
/// Minimum number of levels the API accepts for a [`Question::Score`].
pub const MIN_SCORE_LEVELS: usize = 2;
/// Maximum number of levels the API accepts for a [`Question::Score`].
pub const MAX_SCORE_LEVELS: usize = 10;

/// A named set of questions to evaluate against a single state.
pub type Questions = BTreeMap<String, Question>;

/// Describes what a `true` and a `false` answer mean for a [`Question::Noul`].
///
/// # Examples
///
/// ```rust
/// use jev_provider::NoulCriteria;
///
/// let criteria = NoulCriteria::new("The trend is intact", "The trend has broken");
/// assert_eq!(criteria.when_true, "The trend is intact");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoulCriteria {
    /// What a `true` answer means.
    #[serde(rename = "true")]
    pub when_true: String,
    /// What a `false` answer means.
    #[serde(rename = "false")]
    pub when_false: String,
}

impl NoulCriteria {
    /// Builds a new pair of noul criteria.
    pub fn new(when_true: impl Into<String>, when_false: impl Into<String>) -> Self {
        Self {
            when_true: when_true.into(),
            when_false: when_false.into(),
        }
    }
}

/// A single typed question.
///
/// Serializes to the `{"type": ..., "instructions": ..., "criteria": ...}` shape
/// the System One endpoint expects.
///
/// # Examples
///
/// ```rust
/// use jev_provider::Question;
///
/// let q = Question::choice(
///     "Should this trade be taken?",
///     [("execute", "Take the trade as proposed"), ("skip", "Do not trade")],
/// );
/// assert!(q.validate().is_ok());
///
/// let json = serde_json::to_value(&q).unwrap();
/// assert_eq!(json["type"], "choice");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Question {
    /// A yes/no question. The answer is a probability in `[0, 1]`.
    Noul {
        /// The question itself.
        instructions: String,
        /// Optional descriptions of what yes and no mean.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        criteria: Option<NoulCriteria>,
    },
    /// A pick-one question over labelled options.
    Choice {
        /// The question itself.
        instructions: String,
        /// Option label to description. At most [`MAX_CHOICE_OPTIONS`] entries.
        criteria: BTreeMap<String, String>,
    },
    /// An ordinal question over ordered levels.
    Score {
        /// The question itself.
        instructions: String,
        /// Ordered level descriptions, lowest first. Between
        /// [`MIN_SCORE_LEVELS`] and [`MAX_SCORE_LEVELS`] entries.
        criteria: Vec<String>,
    },
}

impl Question {
    /// Builds a noul (yes/no) question without explicit criteria.
    pub fn noul(instructions: impl Into<String>) -> Self {
        Self::Noul {
            instructions: instructions.into(),
            criteria: None,
        }
    }

    /// Builds a noul question that spells out what yes and no mean.
    pub fn noul_with(instructions: impl Into<String>, criteria: NoulCriteria) -> Self {
        Self::Noul {
            instructions: instructions.into(),
            criteria: Some(criteria),
        }
    }

    /// Builds a choice question from `(label, description)` pairs.
    pub fn choice<I, K, V>(instructions: impl Into<String>, options: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        Self::Choice {
            instructions: instructions.into(),
            criteria: options
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        }
    }

    /// Builds a score question from ordered level descriptions, lowest first.
    pub fn score<I, S>(instructions: impl Into<String>, levels: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::Score {
            instructions: instructions.into(),
            criteria: levels.into_iter().map(Into::into).collect(),
        }
    }

    /// The instruction text carried by this question.
    pub fn instructions(&self) -> &str {
        match self {
            Self::Noul { instructions, .. }
            | Self::Choice { instructions, .. }
            | Self::Score { instructions, .. } => instructions,
        }
    }

    /// The wire name of this question's primitive, for error messages.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Noul { .. } => "noul",
            Self::Choice { .. } => "choice",
            Self::Score { .. } => "score",
        }
    }

    /// Checks this question against the limits the API enforces.
    ///
    /// Catching these locally turns a 422 round trip into an immediate, precise
    /// error, which matters when a question set is assembled from user input.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::InvalidQuestion`] when the instructions are blank, a
    /// choice has fewer than two or more than [`MAX_CHOICE_OPTIONS`] options, or
    /// a score has a level count outside [`MIN_SCORE_LEVELS`]..=[`MAX_SCORE_LEVELS`].
    pub fn validate(&self) -> Result<(), JevError> {
        if self.instructions().trim().is_empty() {
            return Err(JevError::InvalidQuestion(
                "instructions must not be empty".to_string(),
            ));
        }

        match self {
            Self::Noul { criteria, .. } => {
                if let Some(c) = criteria
                    && (c.when_true.trim().is_empty() || c.when_false.trim().is_empty())
                {
                    return Err(JevError::InvalidQuestion(
                        "noul criteria must describe both true and false".to_string(),
                    ));
                }
            }
            Self::Choice { criteria, .. } => {
                if criteria.len() < 2 {
                    return Err(JevError::InvalidQuestion(format!(
                        "choice needs at least 2 options, got {}",
                        criteria.len()
                    )));
                }
                if criteria.len() > MAX_CHOICE_OPTIONS {
                    return Err(JevError::InvalidQuestion(format!(
                        "choice accepts at most {MAX_CHOICE_OPTIONS} options, got {}",
                        criteria.len()
                    )));
                }
                if criteria.keys().any(|k| k.trim().is_empty()) {
                    return Err(JevError::InvalidQuestion(
                        "choice option labels must not be empty".to_string(),
                    ));
                }
            }
            Self::Score { criteria, .. } => {
                if criteria.len() < MIN_SCORE_LEVELS || criteria.len() > MAX_SCORE_LEVELS {
                    return Err(JevError::InvalidQuestion(format!(
                        "score needs {MIN_SCORE_LEVELS}..={MAX_SCORE_LEVELS} levels, got {}",
                        criteria.len()
                    )));
                }
                if criteria.iter().any(|l| l.trim().is_empty()) {
                    return Err(JevError::InvalidQuestion(
                        "score level descriptions must not be empty".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noul_serializes_without_criteria() {
        let q = Question::noul("Is the trend intact?");
        let json = serde_json::to_value(&q).unwrap();
        assert_eq!(json["type"], "noul");
        assert_eq!(json["instructions"], "Is the trend intact?");
        assert!(json.get("criteria").is_none());
    }

    #[test]
    fn noul_criteria_use_true_false_keys() {
        let q = Question::noul_with("Trend?", NoulCriteria::new("intact", "broken"));
        let json = serde_json::to_value(&q).unwrap();
        assert_eq!(json["criteria"]["true"], "intact");
        assert_eq!(json["criteria"]["false"], "broken");
    }

    #[test]
    fn score_serializes_criteria_as_ordered_array() {
        let q = Question::score("How strong?", ["weak", "ok", "strong"]);
        let json = serde_json::to_value(&q).unwrap();
        assert_eq!(json["type"], "score");
        assert_eq!(json["criteria"][0], "weak");
        assert_eq!(json["criteria"][2], "strong");
    }

    #[test]
    fn blank_instructions_are_rejected() {
        assert!(Question::noul("   ").validate().is_err());
    }

    #[test]
    fn choice_needs_at_least_two_options() {
        let q = Question::choice("Pick", [("only", "the only one")]);
        assert!(q.validate().is_err());
    }

    #[test]
    fn choice_rejects_more_than_max_options() {
        let opts: Vec<(String, String)> = (0..=MAX_CHOICE_OPTIONS)
            .map(|i| (format!("opt{i}"), format!("desc{i}")))
            .collect();
        let q = Question::choice("Pick", opts);
        assert!(q.validate().is_err());
    }

    #[test]
    fn choice_accepts_exactly_max_options() {
        let opts: Vec<(String, String)> = (0..MAX_CHOICE_OPTIONS)
            .map(|i| (format!("opt{i}"), format!("desc{i}")))
            .collect();
        assert!(Question::choice("Pick", opts).validate().is_ok());
    }

    #[test]
    fn score_level_count_is_bounded_on_both_sides() {
        assert!(Question::score("How?", ["one"]).validate().is_err());
        let too_many: Vec<String> = (0..=MAX_SCORE_LEVELS).map(|i| format!("l{i}")).collect();
        assert!(Question::score("How?", too_many).validate().is_err());
        assert!(Question::score("How?", ["lo", "hi"]).validate().is_ok());
    }

    #[test]
    fn questions_round_trip_through_json() {
        let q = Question::choice("Pick", [("a", "first"), ("b", "second")]);
        let text = serde_json::to_string(&q).unwrap();
        let back: Question = serde_json::from_str(&text).unwrap();
        assert_eq!(q, back);
    }
}
