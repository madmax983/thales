//! Jev decision gate: a System One judgement between signal generation and execution.
//!
//! [`crate::signals`] answers "did the strategy trigger?". That is a mechanical
//! question, and a strategy will happily trigger into a regime it has no edge
//! in. This module answers the separate question "should we actually take it?"
//! by handing the proposed trade, the market analysis and recent price action to
//! a System One model as a typed question set, then applying a fixed, documented
//! gate to the calibrated answers.
//!
//! The gate is deliberately boring. All of the judgement lives in the model's
//! probabilities; the code below only thresholds them, so every rejection can be
//! explained by pointing at one rule and one number.
//!
//! # Gate rules
//!
//! A signal is approved only if, in order:
//!
//! 1. the instrument clears `min_instrument_quality`;
//! 2. the model's confidence in its own verdict is at least `min_confidence`;
//! 3. the verdict is not [`DECISION_SKIP`];
//! 4. the probability of the chosen action is at least `min_probability`.
//!
//! Rule 1 is a standing veto. A momentum strategy will fire just as readily on a
//! novelty token with no depth as on a major pair, and the mechanical signal
//! looks identical in both cases; only something that knows what the symbol *is*
//! can tell them apart.
//!
//! On approval the intent's `confidence` is replaced by the calibrated
//! probability of [`DECISION_EXECUTE`], and a [`DECISION_REDUCE`] verdict scales
//! `size_hint` by `reduce_factor`.

use std::collections::BTreeMap;

use contracts::{Bar, BarSeries, MarketAnalysis, Position, TradeIntent};
use jev_provider::{
    JevClient, JevError, NoulCriteria, Question, Questions, SystemOneResponse, Usage,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Question id for the pick-one verdict.
pub const Q_VERDICT: &str = "verdict";
/// Question id for the regime-fit check.
pub const Q_REGIME_FIT: &str = "regime_fit";
/// Question id for the conviction rating.
pub const Q_CONVICTION: &str = "conviction";
/// Question id for the instrument-quality veto.
pub const Q_INSTRUMENT_QUALITY: &str = "instrument_quality";

/// Verdict option: take the trade exactly as proposed.
pub const DECISION_EXECUTE: &str = "execute";
/// Verdict option: take the trade, but smaller.
pub const DECISION_REDUCE: &str = "reduce_size";
/// Verdict option: stand aside.
pub const DECISION_SKIP: &str = "skip";

/// How many trailing bars the state summary describes.
pub const PRICE_SUMMARY_BARS: usize = 20;

/// Thresholds applied to a model verdict.
///
/// # Examples
///
/// ```rust
/// use thales_cli::jev_gate::GateThresholds;
///
/// let defaults = GateThresholds::default();
/// assert_eq!(defaults.min_probability, 0.55);
/// assert_eq!(defaults.reduce_factor, 0.5);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GateThresholds {
    /// Minimum probability the chosen action must carry.
    pub min_probability: f64,
    /// Minimum confidence the model must have in its own verdict.
    pub min_confidence: f64,
    /// Multiplier applied to `size_hint` on a [`DECISION_REDUCE`] verdict.
    pub reduce_factor: f64,
    /// Minimum probability that the instrument is worth trading at all.
    ///
    /// Vetoes thin, obscure or novelty listings whose price action is mostly
    /// noise, before the verdict is even considered.
    pub min_instrument_quality: f64,
}

impl Default for GateThresholds {
    fn default() -> Self {
        Self {
            min_probability: 0.55,
            min_confidence: 0.60,
            reduce_factor: 0.5,
            min_instrument_quality: 0.5,
        }
    }
}

/// A compact description of recent price action, built to fit the state budget.
///
/// Jev's state window is finite, so bars are summarised rather than dumped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriceSummary {
    /// How many bars the summary covers.
    pub bars: usize,
    /// The timeframe of those bars.
    pub timeframe: String,
    /// The most recent close.
    pub last_close: f64,
    /// Percent change across the window.
    pub change_pct: f64,
    /// Highest high in the window.
    pub high: f64,
    /// Lowest low in the window.
    pub low: f64,
    /// Where the last close sits in the window's range, from 0.0 (low) to 1.0 (high).
    pub range_position: f64,
    /// Mean volume across the window.
    pub avg_volume: f64,
    /// Most recent volume as a multiple of `avg_volume`.
    pub volume_ratio: f64,
}

impl PriceSummary {
    /// Summarises the trailing `window` bars of `series`.
    ///
    /// Returns `None` when the series is empty.
    pub fn from_series(series: &BarSeries, window: usize) -> Option<Self> {
        let bars: &[Bar] = &series.bars;
        if bars.is_empty() {
            return None;
        }
        let window = window.max(1).min(bars.len());
        let slice = &bars[bars.len() - window..];

        let first_open = slice[0].open;
        let last = slice.last()?;
        let high = slice.iter().map(|b| b.high).fold(f64::MIN, f64::max);
        let low = slice.iter().map(|b| b.low).fold(f64::MAX, f64::min);
        let avg_volume = slice.iter().map(|b| b.volume).sum::<f64>() / window as f64;

        let change_pct = if first_open.abs() > f64::EPSILON {
            (last.close - first_open) / first_open * 100.0
        } else {
            0.0
        };
        let span = high - low;
        let range_position = if span.abs() > f64::EPSILON {
            ((last.close - low) / span).clamp(0.0, 1.0)
        } else {
            0.5
        };
        let volume_ratio = if avg_volume.abs() > f64::EPSILON {
            last.volume / avg_volume
        } else {
            0.0
        };

        Some(Self {
            bars: window,
            timeframe: last.timeframe.clone(),
            last_close: last.close,
            change_pct,
            high,
            low,
            range_position,
            avg_volume,
            volume_ratio,
        })
    }
}

/// Everything the model is shown alongside the proposed trade.
#[derive(Debug, Clone, Default)]
pub struct JudgeContext {
    /// Output of `analyze-market`, if available.
    pub analysis: Option<MarketAnalysis>,
    /// Recent price action, if bars were supplied.
    pub price: Option<PriceSummary>,
    /// Currently open positions, so the model can see concentration.
    pub positions: Vec<Position>,
}

/// The conviction rating, flattened for the audit record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConvictionDetail {
    /// The selected level index.
    pub score: f64,
    /// The description of that level, if the legend carried one.
    pub label: Option<String>,
    /// The level rescaled to `[0, 1]`.
    pub normalized: f64,
    /// The model's confidence in the rating.
    pub confidence: f64,
}

/// The full, auditable record of one signal's adjudication.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JevVerdict {
    /// The intent this verdict applies to.
    pub intent_id: String,
    /// The symbol being traded.
    pub symbol: String,
    /// The action the model chose.
    pub decision: String,
    /// Whether the gate let the signal through.
    pub approved: bool,
    /// Why, in the gate's own terms. Always at least one entry.
    pub reasons: Vec<String>,
    /// The confidence the strategy originally assigned.
    pub strategy_confidence: f64,
    /// The calibrated probability of [`DECISION_EXECUTE`].
    pub execute_probability: f64,
    /// The probability of the action the model actually chose.
    pub decision_probability: f64,
    /// The model's confidence in its own verdict.
    pub verdict_confidence: f64,
    /// Probability mass over every verdict option.
    pub probabilities: BTreeMap<String, f64>,
    /// Probability that the strategy suits the current regime, if asked.
    pub regime_fit: Option<f64>,
    /// Probability that the instrument is worth trading at all, if asked.
    pub instrument_quality: Option<f64>,
    /// The conviction rating, if asked.
    pub conviction: Option<ConvictionDetail>,
    /// The `size_hint` the signal arrived with.
    pub original_size_hint: String,
    /// The `size_hint` after any reduction.
    pub size_hint: String,
    /// The concrete model version that answered.
    pub model: String,
    /// Token accounting for the call.
    pub usage: Usage,
}

/// The outcome of judging a batch of signals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JudgeReport {
    /// Signals the gate approved, with confidence and size already adjusted.
    pub approved: Vec<TradeIntent>,
    /// Signals the gate turned away, unmodified.
    pub rejected: Vec<TradeIntent>,
    /// One verdict per signal, in input order.
    pub verdicts: Vec<JevVerdict>,
    /// Thresholds the gate ran with.
    pub thresholds: GateThresholds,
    /// Total tokens consumed across every call.
    pub usage: Usage,
}

/// The default question set: a verdict, an instrument veto, a regime check and a
/// conviction rating.
///
/// All three are answered in a single round trip.
///
/// # Examples
///
/// ```rust
/// use thales_cli::jev_gate::{default_questions, Q_VERDICT};
///
/// let questions = default_questions();
/// assert_eq!(questions.len(), 4);
/// assert!(questions[Q_VERDICT].validate().is_ok());
/// ```
pub fn default_questions() -> Questions {
    let mut questions = Questions::new();

    questions.insert(
        Q_VERDICT.to_string(),
        Question::choice(
            "A trading strategy has produced the proposed trade below. Given the \
             market state, should the trade be taken as proposed, taken at a \
             reduced size, or skipped?",
            [
                (
                    DECISION_EXECUTE,
                    "The setup is sound and the market state supports it. Take it at full size.",
                ),
                (
                    DECISION_REDUCE,
                    "The setup is plausible but the market state adds risk. Take it at reduced size.",
                ),
                (
                    DECISION_SKIP,
                    "The setup conflicts with the market state, or the edge is absent. Stand aside.",
                ),
            ],
        ),
    );

    questions.insert(
        Q_REGIME_FIT.to_string(),
        Question::noul_with(
            "Is the strategy that produced this signal well suited to the current market regime?",
            NoulCriteria::new(
                "The strategy's edge depends on conditions that are present right now.",
                "The strategy's edge depends on conditions that are absent, or the regime works against it.",
            ),
        ),
    );

    questions.insert(
        Q_INSTRUMENT_QUALITY.to_string(),
        Question::noul_with(
            "Is this instrument liquid and established enough that a technical signal on it \
             is meaningful?",
            NoulCriteria::new(
                "A major, liquid instrument with real depth and a genuine participant base, \
                 where price action reflects supply and demand.",
                "A thin, obscure, novelty or meme listing where price action is dominated by \
                 noise, manipulation or a handful of participants, and a technical signal \
                 carries no edge.",
            ),
        ),
    );

    questions.insert(
        Q_CONVICTION.to_string(),
        Question::score(
            "How strong is this setup, judged on the market state alone?",
            [
                "No edge. The signal looks like noise.",
                "Weak. Marginally better than random.",
                "Fair. A reasonable setup with obvious counterarguments.",
                "Strong. Multiple factors line up behind it.",
                "Exceptional. A high-quality setup with little working against it.",
            ],
        ),
    );

    questions
}

/// Builds the state the model reasons over for a single signal.
///
/// Bars are summarised rather than embedded, so the state stays well inside the
/// model's window regardless of how long the input series is.
pub fn build_state(intent: &TradeIntent, context: &JudgeContext) -> serde_json::Value {
    let mut state = json!({
        "proposed_trade": {
            "symbol": intent.symbol,
            "market": intent.market,
            "side": intent.side,
            "size_hint": intent.size_hint,
            "order_type": intent.order_type,
            "horizon": intent.horizon,
            "strategy": intent.strategy,
            "signal_type": intent.signal_type,
            "strategy_confidence": intent.confidence,
            "rationale": intent.rationale,
            "invalidation": intent.invalidation,
            "stop_loss": intent.stop_loss,
            "take_profit": intent.take_profit,
        }
    });

    let map = state
        .as_object_mut()
        .expect("state is constructed as a JSON object");

    if let Some(analysis) = &context.analysis {
        map.insert(
            "market_analysis".to_string(),
            json!({
                "symbol": analysis.symbol,
                "regime": analysis.regime,
                "sentiment": analysis.sentiment,
                "volatility": analysis.volatility,
                "atr": analysis.atr,
                "patterns": analysis.patterns,
                "key_levels": analysis.key_levels,
                "recommendation": analysis.recommendation,
                "research": analysis.research_summary,
                "news": analysis.news_summary,
                "analysis_confidence": analysis.confidence,
            }),
        );
    }

    if let Some(price) = &context.price {
        map.insert(
            "recent_price_action".to_string(),
            serde_json::to_value(price).unwrap_or(serde_json::Value::Null),
        );
    }

    if !context.positions.is_empty() {
        let same_symbol = context
            .positions
            .iter()
            .filter(|p| p.symbol.eq_ignore_ascii_case(&intent.symbol))
            .count();
        map.insert(
            "portfolio".to_string(),
            json!({
                "open_positions": context.positions,
                "existing_positions_in_this_symbol": same_symbol,
            }),
        );
    }

    state
}

/// Applies the gate rules to a model response.
///
/// This is pure: given the same response and thresholds it always produces the
/// same verdict, which is what makes a rejection defensible after the fact.
///
/// # Errors
///
/// Returns [`JevError::MissingAnswer`] or [`JevError::AnswerKindMismatch`] when
/// the response does not carry a usable [`Q_VERDICT`] answer. The optional
/// [`Q_REGIME_FIT`] and [`Q_CONVICTION`] answers are recorded when present and
/// silently skipped when not, so a custom question set need not supply them.
pub fn adjudicate(
    intent: &TradeIntent,
    response: &SystemOneResponse,
    thresholds: &GateThresholds,
) -> Result<JevVerdict, JevError> {
    let verdict = response.choice(Q_VERDICT)?;
    let decision = verdict.choice.clone();
    let decision_probability = verdict.probability(&decision);
    let execute_probability = verdict.probability(DECISION_EXECUTE);

    let mut reasons = Vec::new();
    let mut approved = true;

    let instrument_quality = response.noul(Q_INSTRUMENT_QUALITY).ok().map(|a| a.noul);
    if let Some(quality) = instrument_quality {
        if quality < thresholds.min_instrument_quality {
            approved = false;
            reasons.push(format!(
                "instrument quality {quality:.2} is below the {:.2} floor; \
                 a technical signal here is not meaningful",
                thresholds.min_instrument_quality
            ));
        } else {
            reasons.push(format!("instrument quality {quality:.2}"));
        }
    }

    if verdict.confidence < thresholds.min_confidence {
        approved = false;
        reasons.push(format!(
            "verdict confidence {:.2} is below the {:.2} floor",
            verdict.confidence, thresholds.min_confidence
        ));
    }

    if decision == DECISION_SKIP {
        approved = false;
        reasons.push(format!("model chose to skip (p={decision_probability:.2})"));
    } else if decision_probability < thresholds.min_probability {
        approved = false;
        reasons.push(format!(
            "'{decision}' carries only p={decision_probability:.2}, below the {:.2} floor",
            thresholds.min_probability
        ));
    }

    if approved {
        reasons.push(format!(
            "model chose '{decision}' at p={decision_probability:.2} with confidence {:.2}",
            verdict.confidence
        ));
    }

    let regime_fit = response.noul(Q_REGIME_FIT).ok().map(|a| a.noul);
    if let Some(fit) = regime_fit {
        reasons.push(format!("regime fit {fit:.2}"));
    }

    let conviction = response.score(Q_CONVICTION).ok().map(|s| ConvictionDetail {
        score: s.score,
        label: s.label().map(str::to_string),
        normalized: s.normalized(),
        confidence: s.confidence,
    });
    if let Some(c) = &conviction {
        reasons.push(match &c.label {
            Some(label) => format!("conviction {:.0} ({label})", c.score),
            None => format!("conviction {:.0}", c.score),
        });
    }

    let original_size_hint = intent.size_hint.clone();
    let mut size_hint = original_size_hint.clone();
    if approved && decision == DECISION_REDUCE {
        match scale_size(&original_size_hint, thresholds.reduce_factor) {
            Some(scaled) => {
                reasons.push(format!(
                    "size reduced from {original_size_hint} to {scaled}"
                ));
                size_hint = scaled;
            }
            None => reasons.push(format!(
                "size '{original_size_hint}' is not numeric, left unchanged despite reduce_size"
            )),
        }
    }

    Ok(JevVerdict {
        intent_id: intent.intent_id.clone(),
        symbol: intent.symbol.clone(),
        decision,
        approved,
        reasons,
        strategy_confidence: intent.confidence,
        execute_probability,
        decision_probability,
        verdict_confidence: verdict.confidence,
        probabilities: verdict.probabilities.clone(),
        regime_fit,
        instrument_quality,
        conviction,
        original_size_hint,
        size_hint,
        model: response.model.clone(),
        usage: response.usage,
    })
}

/// Produces the intent to execute, with the verdict folded in.
///
/// `confidence` becomes the calibrated probability of [`DECISION_EXECUTE`] and
/// the verdict is appended to `rationale`, so the reasoning survives all the way
/// into the execution record.
pub fn apply(intent: &TradeIntent, verdict: &JevVerdict) -> TradeIntent {
    let mut out = intent.clone();
    out.confidence = verdict.execute_probability.clamp(0.0, 1.0);
    out.size_hint = verdict.size_hint.clone();
    out.rationale = format!(
        "{} | jev[{}]: {}",
        intent.rationale,
        verdict.model,
        verdict.reasons.join("; ")
    );
    out
}

/// Multiplies a numeric `size_hint` by `factor`, preserving a readable format.
///
/// Returns `None` for non-numeric hints such as `"max"`, which the caller
/// surfaces as a warning rather than silently ignoring.
fn scale_size(size_hint: &str, factor: f64) -> Option<String> {
    let parsed: f64 = size_hint.trim().parse().ok()?;
    if !parsed.is_finite() {
        return None;
    }
    let scaled = parsed * factor;
    Some(format!("{scaled}"))
}

/// Judges every signal in `intents` against the model.
///
/// One request per signal, each answering the whole question set. Signals are
/// judged independently so one bad signal cannot drag down the batch.
///
/// # Errors
///
/// Propagates the first [`JevError`] from the client or from [`adjudicate`].
pub fn judge(
    client: &JevClient,
    intents: &[TradeIntent],
    context: &JudgeContext,
    thresholds: &GateThresholds,
    questions: &Questions,
) -> Result<JudgeReport, JevError> {
    let mut report = JudgeReport {
        approved: Vec::new(),
        rejected: Vec::new(),
        verdicts: Vec::new(),
        thresholds: *thresholds,
        usage: Usage::default(),
    };

    for intent in intents {
        let state = build_state(intent, context);
        let response = client.ask(&state, questions)?;
        let verdict = adjudicate(intent, &response, thresholds)?;

        report.usage.input_tokens += response.usage.input_tokens;
        report.usage.output_tokens += response.usage.output_tokens;

        if verdict.approved {
            report.approved.push(apply(intent, &verdict));
        } else {
            report.rejected.push(intent.clone());
        }
        report.verdicts.push(verdict);
    }

    Ok(report)
}

/// Renders rejected signals as the audit rows `AGENTS.md` specifies for `portfolio.md`.
///
/// Returns an empty string when nothing was rejected, so callers can append
/// unconditionally without writing empty sections.
pub fn rejection_log(report: &JudgeReport, timestamp: &str) -> String {
    let rejected: Vec<&JevVerdict> = report.verdicts.iter().filter(|v| !v.approved).collect();
    if rejected.is_empty() {
        return String::new();
    }

    let mut out = String::from("\n| Date/Time | Symbol | Signal Ref | Rejection Reason |\n");
    out.push_str("| --- | --- | --- | --- |\n");
    for verdict in rejected {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            timestamp,
            verdict.symbol,
            verdict.intent_id,
            verdict.reasons.join("; ")
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn intent() -> TradeIntent {
        TradeIntent {
            intent_id: "crypto:BTCUSD:buy:v0".to_string(),
            market: "crypto".to_string(),
            symbol: "BTCUSD".to_string(),
            side: "buy".to_string(),
            size_hint: "2".to_string(),
            confidence: 0.7,
            rationale: "Bollinger lower band touch".to_string(),
            strategy: "BollingerBands".to_string(),
            ..Default::default()
        }
    }

    /// Builds a response whose verdict picks `decision` at `p`, spreading the
    /// remaining mass over the other two options.
    fn response(decision: &str, p: f64, confidence: f64) -> SystemOneResponse {
        response_for("BTCUSD", decision, p, confidence, 0.95)
    }

    /// As [`response`], but lets a test set the instrument-quality answer.
    fn response_for(
        _symbol: &str,
        decision: &str,
        p: f64,
        confidence: f64,
        instrument_quality: f64,
    ) -> SystemOneResponse {
        let others: Vec<&str> = [DECISION_EXECUTE, DECISION_REDUCE, DECISION_SKIP]
            .into_iter()
            .filter(|o| *o != decision)
            .collect();
        let rest = ((1.0 - p) / others.len() as f64).max(0.0);
        let mut probabilities = BTreeMap::new();
        probabilities.insert(decision.to_string(), p);
        for o in others {
            probabilities.insert(o.to_string(), rest);
        }

        serde_json::from_value(json!({
            "model": "jev-1.13.0",
            "answers": {
                Q_VERDICT: {
                    "type": "choice",
                    "choice": decision,
                    "confidence": confidence,
                    "probabilities": probabilities,
                },
                Q_REGIME_FIT: {"type": "noul", "noul": 0.8},
                Q_INSTRUMENT_QUALITY: {"type": "noul", "noul": instrument_quality},
                Q_CONVICTION: {
                    "type": "score",
                    "score": 3.0,
                    "confidence": 0.9,
                    "legend": {"0": "No edge", "1": "Weak", "2": "Fair", "3": "Strong", "4": "Exceptional"},
                    "probabilities": {"0": 0.0, "1": 0.1, "2": 0.2, "3": 0.6, "4": 0.1},
                },
            },
            "usage": {"input_tokens": 300, "output_tokens": 12},
        }))
        .expect("test fixture is a valid response")
    }

    #[test]
    fn execute_above_both_floors_is_approved() {
        let v = adjudicate(
            &intent(),
            &response(DECISION_EXECUTE, 0.8, 0.9),
            &GateThresholds::default(),
        )
        .unwrap();
        assert!(v.approved);
        assert_eq!(v.decision, DECISION_EXECUTE);
        assert_eq!(v.execute_probability, 0.8);
        assert_eq!(v.size_hint, "2");
    }

    #[test]
    fn skip_verdict_is_rejected_even_when_certain() {
        let v = adjudicate(
            &intent(),
            &response(DECISION_SKIP, 0.99, 1.0),
            &GateThresholds::default(),
        )
        .unwrap();
        assert!(!v.approved);
        assert!(v.reasons.iter().any(|r| r.contains("chose to skip")));
    }

    #[test]
    fn low_verdict_confidence_is_rejected() {
        let v = adjudicate(
            &intent(),
            &response(DECISION_EXECUTE, 0.95, 0.3),
            &GateThresholds::default(),
        )
        .unwrap();
        assert!(!v.approved);
        assert!(v.reasons.iter().any(|r| r.contains("below the 0.60 floor")));
    }

    #[test]
    fn indecisive_probability_is_rejected() {
        let v = adjudicate(
            &intent(),
            &response(DECISION_EXECUTE, 0.40, 0.9),
            &GateThresholds::default(),
        )
        .unwrap();
        assert!(!v.approved);
        assert!(v.reasons.iter().any(|r| r.contains("p=0.40")));
    }

    #[test]
    fn threshold_boundaries_are_inclusive() {
        let thresholds = GateThresholds::default();
        let at_floor = adjudicate(
            &intent(),
            &response(
                DECISION_EXECUTE,
                thresholds.min_probability,
                thresholds.min_confidence,
            ),
            &thresholds,
        )
        .unwrap();
        assert!(at_floor.approved, "a verdict exactly at the floor passes");
    }

    #[test]
    fn reduce_size_scales_a_numeric_hint() {
        let v = adjudicate(
            &intent(),
            &response(DECISION_REDUCE, 0.7, 0.8),
            &GateThresholds::default(),
        )
        .unwrap();
        assert!(v.approved);
        assert_eq!(v.original_size_hint, "2");
        assert_eq!(v.size_hint, "1");
    }

    #[test]
    fn reduce_size_leaves_a_non_numeric_hint_alone_and_says_so() {
        let mut i = intent();
        i.size_hint = "max".to_string();
        let v = adjudicate(
            &i,
            &response(DECISION_REDUCE, 0.7, 0.8),
            &GateThresholds::default(),
        )
        .unwrap();
        assert!(v.approved);
        assert_eq!(v.size_hint, "max");
        assert!(v.reasons.iter().any(|r| r.contains("not numeric")));
    }

    #[test]
    fn a_rejected_signal_is_never_size_adjusted() {
        let v = adjudicate(
            &intent(),
            &response(DECISION_REDUCE, 0.2, 0.9),
            &GateThresholds::default(),
        )
        .unwrap();
        assert!(!v.approved);
        assert_eq!(v.size_hint, v.original_size_hint);
    }

    #[test]
    fn apply_replaces_confidence_with_the_calibrated_probability() {
        let i = intent();
        let v = adjudicate(
            &i,
            &response(DECISION_EXECUTE, 0.83, 0.9),
            &GateThresholds::default(),
        )
        .unwrap();
        let out = apply(&i, &v);
        assert_eq!(out.confidence, 0.83);
        assert_ne!(out.confidence, i.confidence);
        assert!(
            out.rationale
                .starts_with("Bollinger lower band touch | jev[jev-1.13.0]:")
        );
        // Everything else survives untouched.
        assert_eq!(out.symbol, i.symbol);
        assert_eq!(out.side, i.side);
        assert_eq!(out.intent_id, i.intent_id);
    }

    #[test]
    fn apply_carries_the_reduced_size_through() {
        let i = intent();
        let v = adjudicate(
            &i,
            &response(DECISION_REDUCE, 0.7, 0.8),
            &GateThresholds::default(),
        )
        .unwrap();
        assert_eq!(apply(&i, &v).size_hint, "1");
    }

    #[test]
    fn optional_questions_are_recorded_when_present() {
        let v = adjudicate(
            &intent(),
            &response(DECISION_EXECUTE, 0.8, 0.9),
            &GateThresholds::default(),
        )
        .unwrap();
        assert_eq!(v.regime_fit, Some(0.8));
        let conviction = v.conviction.expect("conviction was asked and answered");
        assert_eq!(conviction.label.as_deref(), Some("Strong"));
        assert_eq!(conviction.normalized, 0.75);
    }

    #[test]
    fn a_verdict_only_response_still_adjudicates() {
        let response: SystemOneResponse = serde_json::from_value(json!({
            "model": "jev-1.13.0",
            "answers": {
                Q_VERDICT: {
                    "type": "choice",
                    "choice": DECISION_EXECUTE,
                    "confidence": 0.9,
                    "probabilities": {DECISION_EXECUTE: 0.9, DECISION_SKIP: 0.1},
                }
            }
        }))
        .unwrap();
        let v = adjudicate(&intent(), &response, &GateThresholds::default()).unwrap();
        assert!(v.approved);
        assert_eq!(v.regime_fit, None);
        assert!(v.conviction.is_none());
    }

    #[test]
    fn a_response_without_a_verdict_is_an_error() {
        let response: SystemOneResponse = serde_json::from_value(json!({
            "model": "jev-1.13.0",
            "answers": {"something_else": {"type": "noul", "noul": 0.5}}
        }))
        .unwrap();
        let err = adjudicate(&intent(), &response, &GateThresholds::default()).unwrap_err();
        assert!(matches!(err, JevError::MissingAnswer(id) if id == Q_VERDICT));
    }

    #[test]
    fn every_verdict_carries_at_least_one_reason() {
        for decision in [DECISION_EXECUTE, DECISION_REDUCE, DECISION_SKIP] {
            for p in [0.1, 0.5, 0.9] {
                for confidence in [0.1, 0.9] {
                    let v = adjudicate(
                        &intent(),
                        &response(decision, p, confidence),
                        &GateThresholds::default(),
                    )
                    .unwrap();
                    assert!(!v.reasons.is_empty(), "{decision} p={p} c={confidence}");
                }
            }
        }
    }

    #[test]
    fn default_questions_all_satisfy_the_api_limits() {
        let questions = default_questions();
        assert_eq!(questions.len(), 4);
        for (id, q) in &questions {
            q.validate().unwrap_or_else(|e| panic!("{id}: {e}"));
        }
    }

    // --- state construction -------------------------------------------------

    fn bar(ts: i64, open: f64, high: f64, low: f64, close: f64, volume: f64) -> Bar {
        Bar {
            symbol: "BTCUSD".to_string(),
            market: "crypto".to_string(),
            timeframe: "1h".to_string(),
            timestamp_unix_ms: ts,
            open,
            high,
            low,
            close,
            volume,
            adjusted_close: None,
        }
    }

    #[test]
    fn price_summary_describes_the_trailing_window() {
        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars: vec![
                bar(1, 100.0, 110.0, 90.0, 105.0, 10.0),
                bar(2, 105.0, 120.0, 100.0, 115.0, 20.0),
                bar(3, 115.0, 130.0, 110.0, 130.0, 60.0),
            ],
        };
        let s = PriceSummary::from_series(&series, 20).unwrap();
        assert_eq!(s.bars, 3);
        assert_eq!(s.timeframe, "1h");
        assert_eq!(s.last_close, 130.0);
        assert_eq!(s.high, 130.0);
        assert_eq!(s.low, 90.0);
        assert_eq!(s.change_pct, 30.0);
        assert_eq!(s.range_position, 1.0);
        assert_eq!(s.avg_volume, 30.0);
        assert_eq!(s.volume_ratio, 2.0);
    }

    #[test]
    fn price_summary_honours_a_window_shorter_than_the_series() {
        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars: (0..50)
                .map(|i| bar(i, 100.0, 101.0, 99.0, 100.0, 5.0))
                .collect(),
        };
        assert_eq!(PriceSummary::from_series(&series, 20).unwrap().bars, 20);
    }

    #[test]
    fn price_summary_is_none_for_an_empty_series() {
        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars: vec![],
        };
        assert!(PriceSummary::from_series(&series, 20).is_none());
    }

    #[test]
    fn price_summary_survives_a_flat_series_without_dividing_by_zero() {
        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars: vec![bar(1, 100.0, 100.0, 100.0, 100.0, 0.0)],
        };
        let s = PriceSummary::from_series(&series, 20).unwrap();
        assert_eq!(s.range_position, 0.5);
        assert_eq!(s.volume_ratio, 0.0);
        assert!(s.change_pct.is_finite());
    }

    #[test]
    fn state_always_carries_the_proposed_trade() {
        let state = build_state(&intent(), &JudgeContext::default());
        assert_eq!(state["proposed_trade"]["symbol"], "BTCUSD");
        assert_eq!(state["proposed_trade"]["strategy"], "BollingerBands");
        assert_eq!(state["proposed_trade"]["strategy_confidence"], 0.7);
        assert!(state.get("market_analysis").is_none());
        assert!(state.get("portfolio").is_none());
    }

    #[test]
    fn state_includes_context_when_supplied() {
        let context = JudgeContext {
            analysis: Some(MarketAnalysis {
                symbol: "BTCUSD".to_string(),
                market: "crypto".to_string(),
                regime: "Trending Up".to_string(),
                sentiment: "Bullish".to_string(),
                patterns: vec!["Breakout".to_string()],
                key_levels: vec![60000.0],
                volatility: "High".to_string(),
                atr: Some(1200.0),
                research_summary: None,
                news_summary: None,
                recommendation: None,
                confidence: 0.8,
                timestamp_unix_ms: 0,
                jev: None,
            }),
            price: None,
            positions: vec![
                Position {
                    symbol: "BTCUSD".to_string(),
                    side: "long".to_string(),
                    qty: 1.0,
                    entry_price: Some(59000.0),
                },
                Position {
                    symbol: "ETHUSD".to_string(),
                    side: "long".to_string(),
                    qty: 5.0,
                    entry_price: None,
                },
            ],
        };
        let state = build_state(&intent(), &context);
        assert_eq!(state["market_analysis"]["regime"], "Trending Up");
        assert_eq!(state["portfolio"]["existing_positions_in_this_symbol"], 1);
    }

    #[test]
    fn state_summarises_bars_rather_than_embedding_them() {
        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars: (0..5000)
                .map(|i| bar(i, 100.0, 101.0, 99.0, 100.5, 7.0))
                .collect(),
        };
        let context = JudgeContext {
            price: PriceSummary::from_series(&series, PRICE_SUMMARY_BARS),
            ..Default::default()
        };
        let state = build_state(&intent(), &context);
        assert_eq!(state["recent_price_action"]["bars"], PRICE_SUMMARY_BARS);
        // A 5000-bar series must not balloon the state sent to the model.
        assert!(
            state.to_string().len() < 4096,
            "state grew to {} bytes",
            state.to_string().len()
        );
    }

    // --- audit log ----------------------------------------------------------

    fn report_with(verdicts: Vec<JevVerdict>) -> JudgeReport {
        JudgeReport {
            approved: vec![],
            rejected: vec![],
            verdicts,
            thresholds: GateThresholds::default(),
            usage: Usage::default(),
        }
    }

    #[test]
    fn rejection_log_is_empty_when_nothing_was_rejected() {
        let v = adjudicate(
            &intent(),
            &response(DECISION_EXECUTE, 0.9, 0.9),
            &GateThresholds::default(),
        )
        .unwrap();
        assert_eq!(
            rejection_log(&report_with(vec![v]), "2026-09-21T00:00:00Z"),
            ""
        );
    }

    #[test]
    fn rejection_log_emits_the_documented_portfolio_columns() {
        let v = adjudicate(
            &intent(),
            &response(DECISION_SKIP, 0.9, 0.9),
            &GateThresholds::default(),
        )
        .unwrap();
        let log = rejection_log(&report_with(vec![v]), "2026-09-21T00:00:00Z");
        assert!(log.contains("| Date/Time | Symbol | Signal Ref | Rejection Reason |"));
        assert!(log.contains("| 2026-09-21T00:00:00Z | BTCUSD | crypto:BTCUSD:buy:v0 |"));
    }
    #[test]
    fn a_thin_novelty_listing_is_vetoed_however_good_the_signal_looks() {
        // The failure this gate exists for: a strategy firing on an obscure
        // token with a textbook-looking setup.
        let mut i = intent();
        i.symbol = "APENFT".to_string();
        i.intent_id = "crypto:APENFT:buy:v0".to_string();

        let v = adjudicate(
            &i,
            &response_for("APENFT", DECISION_EXECUTE, 0.97, 0.99, 0.05),
            &GateThresholds::default(),
        )
        .unwrap();

        assert!(!v.approved);
        assert_eq!(v.instrument_quality, Some(0.05));
        assert!(
            v.reasons
                .iter()
                .any(|r| r.contains("instrument quality 0.05"))
        );
    }

    #[test]
    fn the_instrument_veto_is_independent_of_the_verdict() {
        // Even a maximally confident execute verdict cannot override it.
        for (p, confidence) in [(0.99, 0.99), (0.60, 0.70)] {
            let v = adjudicate(
                &intent(),
                &response_for("X", DECISION_EXECUTE, p, confidence, 0.1),
                &GateThresholds::default(),
            )
            .unwrap();
            assert!(!v.approved, "p={p} c={confidence}");
        }
    }

    #[test]
    fn a_liquid_instrument_passes_the_veto_and_is_recorded() {
        let v = adjudicate(
            &intent(),
            &response_for("BTCUSD", DECISION_EXECUTE, 0.8, 0.9, 0.98),
            &GateThresholds::default(),
        )
        .unwrap();
        assert!(v.approved);
        assert_eq!(v.instrument_quality, Some(0.98));
    }

    #[test]
    fn the_instrument_veto_boundary_is_inclusive() {
        let thresholds = GateThresholds::default();
        let v = adjudicate(
            &intent(),
            &response_for(
                "X",
                DECISION_EXECUTE,
                0.8,
                0.9,
                thresholds.min_instrument_quality,
            ),
            &thresholds,
        )
        .unwrap();
        assert!(v.approved);
    }

    #[test]
    fn the_instrument_veto_can_be_disabled_by_lowering_the_floor() {
        let thresholds = GateThresholds {
            min_instrument_quality: 0.0,
            ..GateThresholds::default()
        };
        let v = adjudicate(
            &intent(),
            &response_for("X", DECISION_EXECUTE, 0.8, 0.9, 0.01),
            &thresholds,
        )
        .unwrap();
        assert!(v.approved);
    }

    #[test]
    fn default_question_set_includes_the_instrument_veto() {
        let questions = default_questions();
        assert_eq!(questions.len(), 4);
        assert!(questions.contains_key(Q_INSTRUMENT_QUALITY));
    }
}
