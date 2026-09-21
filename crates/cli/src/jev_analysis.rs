//! System One classification of market state.
//!
//! [`crate::analysis::analyze`] labels the regime, sentiment and volatility from
//! technical heuristics. Those labels are useful but uncalibrated: "Trending Up"
//! means the same thing whether the trend is unmistakable or barely there, so
//! anything downstream has to treat a marginal read and an obvious one alike.
//!
//! This module re-labels the same three fields with a System One model, which
//! returns the probability mass behind each one. The heuristic analysis is still
//! computed first and passed in as part of the state, so the model is refining a
//! reading rather than starting from nothing.

use contracts::{BarSeries, ClassifiedField, JevClassification, MarketAnalysis};
use jev_provider::{ChoiceAnswer, JevClient, JevError, Question, Questions, SystemOneResponse};
use serde_json::json;

use crate::jev_gate::{PRICE_SUMMARY_BARS, PriceSummary};

/// Question id for the regime classification.
pub const Q_REGIME: &str = "regime";
/// Question id for the sentiment classification.
pub const Q_SENTIMENT: &str = "sentiment";
/// Question id for the volatility classification.
pub const Q_VOLATILITY: &str = "volatility";

/// The regime labels, matching the vocabulary `AGENTS.md` already uses.
pub const REGIMES: [(&str, &str); 5] = [
    (
        "Trending Up",
        "A sustained advance with higher highs and higher lows.",
    ),
    (
        "Trending Down",
        "A sustained decline with lower highs and lower lows.",
    ),
    (
        "Ranging",
        "Price is oscillating within a band with no net direction.",
    ),
    (
        "Volatile",
        "Large, erratic swings without a stable direction.",
    ),
    ("Calm", "Quiet, low-range drift with little participation."),
];

/// The sentiment labels.
pub const SENTIMENTS: [(&str, &str); 3] = [
    ("Bullish", "Buyers are in control; dips are being bought."),
    ("Bearish", "Sellers are in control; rallies are being sold."),
    ("Neutral", "Neither side is in control."),
];

/// The volatility labels.
pub const VOLATILITIES: [(&str, &str); 3] = [
    ("High", "Ranges are wide relative to recent history."),
    ("Normal", "Ranges are typical for this instrument."),
    ("Low", "Ranges are compressed relative to recent history."),
];

/// The question set used to classify market state.
///
/// # Examples
///
/// ```rust
/// use thales_cli::jev_analysis::{classification_questions, Q_REGIME};
///
/// let questions = classification_questions();
/// assert_eq!(questions.len(), 3);
/// assert!(questions[Q_REGIME].validate().is_ok());
/// ```
pub fn classification_questions() -> Questions {
    let mut questions = Questions::new();
    questions.insert(
        Q_REGIME.to_string(),
        Question::choice("Which regime is this market in?", REGIMES),
    );
    questions.insert(
        Q_SENTIMENT.to_string(),
        Question::choice("What is the prevailing sentiment?", SENTIMENTS),
    );
    questions.insert(
        Q_VOLATILITY.to_string(),
        Question::choice(
            "How would you characterise current volatility?",
            VOLATILITIES,
        ),
    );
    questions
}

/// Builds the state describing the market to be classified.
pub fn build_state(analysis: &MarketAnalysis, price: Option<&PriceSummary>) -> serde_json::Value {
    json!({
        "symbol": analysis.symbol,
        "market": analysis.market,
        "recent_price_action": price,
        "technical_indicators": {
            "atr": analysis.atr,
            "key_levels": analysis.key_levels,
            "detected_patterns": analysis.patterns,
        },
        "heuristic_reading": {
            "regime": analysis.regime,
            "sentiment": analysis.sentiment,
            "volatility": analysis.volatility,
            "note": "Produced by technical rules. Treat it as one input, not as ground truth.",
        },
        "research": analysis.research_summary,
        "news": analysis.news_summary,
    })
}

fn field(answer: &ChoiceAnswer) -> ClassifiedField {
    ClassifiedField {
        value: answer.choice.clone(),
        confidence: answer.confidence,
        probabilities: answer.probabilities.clone(),
    }
}

/// Folds a model response into `analysis`, replacing the heuristic labels.
///
/// The overall `confidence` becomes the probability behind the regime call,
/// since the regime is what most downstream logic keys off.
///
/// # Errors
///
/// Returns [`JevError::MissingAnswer`] or [`JevError::AnswerKindMismatch`] when
/// the response does not carry all three choice answers.
pub fn apply(
    analysis: &MarketAnalysis,
    response: &SystemOneResponse,
) -> Result<MarketAnalysis, JevError> {
    let regime = response.choice(Q_REGIME)?;
    let sentiment = response.choice(Q_SENTIMENT)?;
    let volatility = response.choice(Q_VOLATILITY)?;

    let mut out = analysis.clone();
    out.regime = regime.choice.clone();
    out.sentiment = sentiment.choice.clone();
    out.volatility = volatility.choice.clone();
    out.confidence = regime.probability(&regime.choice).clamp(0.0, 1.0);
    out.jev = Some(JevClassification {
        model: response.model.clone(),
        regime: field(regime),
        sentiment: field(sentiment),
        volatility: field(volatility),
    });
    Ok(out)
}

/// Classifies `analysis` with the model, using `series` for price context.
///
/// # Errors
///
/// Propagates client and decoding errors from the model call.
pub fn classify(
    client: &JevClient,
    analysis: &MarketAnalysis,
    series: &BarSeries,
) -> Result<MarketAnalysis, JevError> {
    let price = PriceSummary::from_series(series, PRICE_SUMMARY_BARS);
    let state = build_state(analysis, price.as_ref());
    let response = client.ask(&state, &classification_questions())?;
    apply(analysis, &response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn heuristic() -> MarketAnalysis {
        MarketAnalysis {
            symbol: "BTCUSD".to_string(),
            market: "crypto".to_string(),
            regime: "Ranging".to_string(),
            sentiment: "Neutral".to_string(),
            patterns: vec!["Breakout".to_string()],
            key_levels: vec![60000.0],
            volatility: "Low".to_string(),
            atr: Some(1200.0),
            research_summary: None,
            news_summary: None,
            recommendation: None,
            confidence: 0.5,
            timestamp_unix_ms: 42,
            jev: None,
        }
    }

    fn choice(id: &str, pick: &str, p: f64, confidence: f64) -> (String, serde_json::Value) {
        let mut probabilities = BTreeMap::new();
        probabilities.insert(pick.to_string(), p);
        probabilities.insert("__other".to_string(), 1.0 - p);
        (
            id.to_string(),
            json!({
                "type": "choice",
                "choice": pick,
                "confidence": confidence,
                "probabilities": probabilities,
            }),
        )
    }

    fn response() -> SystemOneResponse {
        let answers: serde_json::Map<String, serde_json::Value> = [
            choice(Q_REGIME, "Trending Up", 0.79, 0.9),
            choice(Q_SENTIMENT, "Bullish", 0.71, 0.85),
            choice(Q_VOLATILITY, "High", 0.66, 0.8),
        ]
        .into_iter()
        .collect();

        serde_json::from_value(json!({
            "model": "jev-1.13.0",
            "answers": answers,
            "usage": {"input_tokens": 200, "output_tokens": 9},
        }))
        .expect("valid fixture")
    }

    #[test]
    fn classification_replaces_every_heuristic_label() {
        let out = apply(&heuristic(), &response()).unwrap();
        assert_eq!(out.regime, "Trending Up");
        assert_eq!(out.sentiment, "Bullish");
        assert_eq!(out.volatility, "High");
    }

    #[test]
    fn confidence_becomes_the_regime_probability() {
        let out = apply(&heuristic(), &response()).unwrap();
        assert_eq!(out.confidence, 0.79);
    }

    #[test]
    fn the_distribution_is_preserved_for_audit() {
        let out = apply(&heuristic(), &response()).unwrap();
        let jev = out.jev.expect("classification is attached");
        assert_eq!(jev.model, "jev-1.13.0");
        assert_eq!(jev.regime.value, "Trending Up");
        assert_eq!(jev.regime.confidence, 0.9);
        assert_eq!(jev.sentiment.probabilities["Bullish"], 0.71);
        assert_eq!(jev.volatility.value, "High");
    }

    #[test]
    fn untouched_fields_survive_classification() {
        let before = heuristic();
        let after = apply(&before, &response()).unwrap();
        assert_eq!(after.symbol, before.symbol);
        assert_eq!(after.atr, before.atr);
        assert_eq!(after.key_levels, before.key_levels);
        assert_eq!(after.patterns, before.patterns);
        assert_eq!(after.timestamp_unix_ms, before.timestamp_unix_ms);
    }

    #[test]
    fn an_incomplete_response_is_an_error_rather_than_a_partial_write() {
        let answers: serde_json::Map<String, serde_json::Value> =
            [choice(Q_REGIME, "Trending Up", 0.9, 0.9)]
                .into_iter()
                .collect();
        let partial: SystemOneResponse =
            serde_json::from_value(json!({"model": "jev-1.13.0", "answers": answers})).unwrap();
        let err = apply(&heuristic(), &partial).unwrap_err();
        assert!(matches!(err, JevError::MissingAnswer(id) if id == Q_SENTIMENT));
    }

    #[test]
    fn every_classification_question_satisfies_the_api_limits() {
        for (id, q) in &classification_questions() {
            q.validate().unwrap_or_else(|e| panic!("{id}: {e}"));
        }
    }

    #[test]
    fn state_shows_the_model_the_heuristic_reading_without_asserting_it() {
        let state = build_state(&heuristic(), None);
        assert_eq!(state["heuristic_reading"]["regime"], "Ranging");
        assert_eq!(state["symbol"], "BTCUSD");
        assert!(
            state["heuristic_reading"]["note"]
                .as_str()
                .unwrap()
                .contains("not as ground truth")
        );
    }

    #[test]
    fn an_analysis_without_a_classification_still_serializes_cleanly() {
        let json = serde_json::to_value(heuristic()).unwrap();
        assert!(json.get("jev").is_none(), "absent field is skipped");
    }

    #[test]
    fn reports_written_before_this_field_existed_still_parse() {
        let legacy = json!({
            "symbol": "AAPL", "market": "equities", "regime": "Trending",
            "sentiment": "Bullish", "patterns": [], "key_levels": [],
            "volatility": "High", "confidence": 0.8, "timestamp_unix_ms": 0
        });
        let parsed: MarketAnalysis = serde_json::from_value(legacy).unwrap();
        assert!(parsed.jev.is_none());
    }
}
