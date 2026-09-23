//! Deterministic ensemble dedup for the four-strategy scan.
//!
//! The morning scan evaluates `BollingerBands`, `RsiMeanReversion`,
//! `Supertrend`, and `DonchianBreakout` independently per symbol. Each
//! strategy file is gated through `judge-signals` — but the gate must never
//! see two live candidates for the same symbol, or one symbol could take
//! multiple positions from a single scan.
//!
//! Rules (deterministic, no judgement calls):
//! - **Conflict:** if the ensemble fires both `buy` and `sell` on a symbol
//!   (any two distinct sides), emit nothing for it and record the conflict.
//!   Neither side is gated.
//! - **Same-direction corroboration:** multiple same-side candidates collapse
//!   to exactly one intent — the highest `confidence`; ties break on
//!   `strategy` ascending, then `intent_id` ascending. The dropped candidates
//!   are recorded in the disposition, never gated, never executed.
//! - **Single:** one candidate passes through untouched.
//!
//! The kept intent is never mutated: confidence is not inflated for
//! corroboration. The disposition record is where the corroboration lives.

use std::collections::BTreeMap;

use contracts::TradeIntent;
use serde::{Deserialize, Serialize};

/// How one symbol's ensemble candidates were resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Disposition {
    /// No strategy fired on this symbol — a valid no-setup.
    None,
    /// Exactly one candidate — passed through untouched.
    Single,
    /// Multiple same-direction candidates — kept one, dropped the rest.
    Deduped,
    /// Buy and sell both fired — emitted nothing, gated nothing.
    Conflict,
}

impl Disposition {
    /// Short audit label for this disposition.
    pub fn label(self) -> &'static str {
        match self {
            Disposition::None => "no-signal",
            Disposition::Single => "single",
            Disposition::Deduped => "deduped",
            Disposition::Conflict => "CONFLICT",
        }
    }
}

/// Per-symbol record of the dedup decision. Preserved in the run's audit
/// trail alongside the four raw strategy files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolDisposition {
    pub symbol: String,
    pub disposition: Disposition,
    /// Distinct sides seen across the ensemble (sorted).
    pub sides: Vec<String>,
    /// Distinct strategies that fired on this symbol (sorted).
    pub strategies: Vec<String>,
    /// The surviving intent (`Single` / `Deduped` only).
    pub kept_intent_id: Option<String>,
    pub kept_strategy: Option<String>,
    /// Candidates that were not emitted: dropped corroborators (`Deduped`)
    /// or every candidate (`Conflict`).
    pub dropped_intent_ids: Vec<String>,
}

/// Full result of one `dedupe-signals` run.
#[derive(Debug)]
pub struct DedupeResult {
    /// Surviving intents, one per symbol at most, sorted by symbol.
    pub kept: Vec<TradeIntent>,
    /// One record per symbol that had at least one candidate, sorted by symbol.
    pub dispositions: Vec<SymbolDisposition>,
    /// Input labels (usually file paths) that held zero intents.
    pub empty_inputs: Vec<String>,
    /// Human-readable notes: conflicts and dedup summaries.
    pub warnings: Vec<String>,
}

/// Deduplicate ensemble candidates.
///
/// `inputs` pairs each input label (file path) with the intents parsed from
/// it. Labels are only used for the empty-input record and warnings; the
/// dedup itself keys on `TradeIntent` fields, never on labels.
pub fn dedupe_files(inputs: Vec<(String, Vec<TradeIntent>)>) -> DedupeResult {
    let mut empty_inputs = Vec::new();
    let mut by_symbol: BTreeMap<String, Vec<TradeIntent>> = BTreeMap::new();
    for (label, intents) in &inputs {
        if intents.is_empty() {
            empty_inputs.push(label.clone());
        }
        for intent in intents {
            by_symbol
                .entry(intent.symbol.clone())
                .or_default()
                .push(intent.clone());
        }
    }

    let mut kept = Vec::new();
    let mut dispositions = Vec::new();
    let mut warnings = Vec::new();

    for (symbol, mut candidates) in by_symbol {
        let mut sides: Vec<String> = candidates.iter().map(|c| c.side.clone()).collect();
        sides.sort();
        sides.dedup();
        let mut strategies: Vec<String> = candidates
            .iter()
            .map(|c| {
                if c.strategy.is_empty() {
                    "unknown".to_string()
                } else {
                    c.strategy.clone()
                }
            })
            .collect();
        strategies.sort();
        strategies.dedup();

        if sides.len() > 1 {
            // Conflict: opposite directions — emit nothing, gate nothing.
            let ids: Vec<String> = candidates.iter().map(|c| c.intent_id.clone()).collect();
            warnings.push(format!(
                "CONFLICT on {symbol}: ensemble fired {} ({}) — no signal emitted, nothing gated",
                sides.join("/"),
                strategies.join(", ")
            ));
            dispositions.push(SymbolDisposition {
                symbol,
                disposition: Disposition::Conflict,
                sides,
                strategies,
                kept_intent_id: None,
                kept_strategy: None,
                dropped_intent_ids: ids,
            });
            continue;
        }

        if candidates.len() == 1 {
            let only = candidates.pop().expect("len checked");
            dispositions.push(SymbolDisposition {
                symbol: symbol.clone(),
                disposition: Disposition::Single,
                sides,
                strategies,
                kept_intent_id: Some(only.intent_id.clone()),
                kept_strategy: Some(only.strategy.clone()),
                dropped_intent_ids: Vec::new(),
            });
            kept.push(only);
            continue;
        }

        // Same-direction corroboration: keep exactly one. Highest confidence
        // wins; ties break deterministically on strategy, then intent_id.
        // (confidence descending via total_cmp on the reversed value.)
        candidates.sort_by(|a, b| {
            b.confidence
                .total_cmp(&a.confidence)
                .then_with(|| a.strategy.cmp(&b.strategy))
                .then_with(|| a.intent_id.cmp(&b.intent_id))
        });
        let winner = candidates.remove(0);
        let dropped: Vec<String> = candidates.iter().map(|c| c.intent_id.clone()).collect();
        warnings.push(format!(
            "deduped {symbol}: kept {} (conf {:.2}); dropped {} corroborator(s)",
            winner.intent_id,
            winner.confidence,
            dropped.len()
        ));
        dispositions.push(SymbolDisposition {
            symbol: symbol.clone(),
            disposition: Disposition::Deduped,
            sides,
            strategies,
            kept_intent_id: Some(winner.intent_id.clone()),
            kept_strategy: Some(winner.strategy.clone()),
            dropped_intent_ids: dropped,
        });
        kept.push(winner);
    }

    DedupeResult {
        kept,
        dispositions,
        empty_inputs,
        warnings,
    }
}

/// Renders dedup dispositions as audit rows in the same four-column shape
/// `judge-signals --log` uses, so one `audit.md` reads end to end.
///
/// Returns an empty string when there is nothing to record.
pub fn dedup_log(result: &DedupeResult, timestamp: &str) -> String {
    if result.dispositions.is_empty() {
        return String::new();
    }
    let mut out = String::from("\n| Date/Time | Symbol | Signal Ref | Disposition |\n");
    out.push_str("| --- | --- | --- | --- |\n");
    for d in &result.dispositions {
        let signal_ref = d
            .kept_intent_id
            .clone()
            .unwrap_or_else(|| d.dropped_intent_ids.join(", "));
        let detail = match d.disposition {
            Disposition::Single => format!(
                "single: {} fired alone — passed through to the gate",
                d.kept_strategy.clone().unwrap_or_default()
            ),
            Disposition::Deduped => format!(
                "deduped: kept {} ({}); dropped {} corroborator(s): {}",
                d.kept_strategy.clone().unwrap_or_default(),
                d.kept_intent_id.clone().unwrap_or_default(),
                d.dropped_intent_ids.len(),
                d.dropped_intent_ids.join(", ")
            ),
            Disposition::Conflict => format!(
                "CONFLICT: ensemble fired {} ({}) — no signal emitted, nothing gated",
                d.sides.join("/"),
                d.strategies.join(", ")
            ),
            Disposition::None => "no-signal".to_string(),
        };
        out.push_str(&format!(
            "| {timestamp} | {} | {signal_ref} | {detail} |\n",
            d.symbol
        ));
    }
    if !result.empty_inputs.is_empty() {
        out.push_str(&format!(
            "| {timestamp} | — | — | no-signal inputs (empty strategy files): {} |\n",
            result.empty_inputs.join(", ")
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn intent(id: &str, symbol: &str, strategy: &str, side: &str, conf: f64) -> TradeIntent {
        TradeIntent {
            intent_id: id.to_string(),
            market: "crypto".to_string(),
            symbol: symbol.to_string(),
            side: side.to_string(),
            size_hint: "1".to_string(),
            confidence: conf,
            rationale: "test".to_string(),
            strategy: strategy.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn single_candidate_passes_through_untouched() {
        let only = intent("a", "BTCUSD", "BollingerBands", "buy", 0.6);
        let r = dedupe_files(vec![("f1".into(), vec![only.clone()])]);
        assert_eq!(r.kept, vec![only]);
        assert_eq!(r.dispositions.len(), 1);
        assert_eq!(r.dispositions[0].disposition, Disposition::Single);
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn same_direction_dedups_to_highest_confidence() {
        let lo = intent("lo", "BTCUSD", "Supertrend", "buy", 0.55);
        let hi = intent("hi", "BTCUSD", "BollingerBands", "buy", 0.8);
        let r = dedupe_files(vec![
            ("f1".into(), vec![lo]),
            ("f2".into(), vec![hi.clone()]),
        ]);
        assert_eq!(r.kept, vec![hi]);
        let d = &r.dispositions[0];
        assert_eq!(d.disposition, Disposition::Deduped);
        assert_eq!(d.kept_intent_id.as_deref(), Some("hi"));
        assert_eq!(d.dropped_intent_ids, vec!["lo".to_string()]);
        assert_eq!(r.warnings.len(), 1);
    }

    #[test]
    fn confidence_tie_breaks_on_strategy_then_intent_id() {
        let b = intent("b", "BTCUSD", "Supertrend", "buy", 0.7);
        let a = intent("a", "BTCUSD", "BollingerBands", "buy", 0.7);
        let r = dedupe_files(vec![("f".into(), vec![b, a.clone()])]);
        assert_eq!(r.kept, vec![a]);
        assert_eq!(r.dispositions[0].dropped_intent_ids, vec!["b".to_string()]);
    }

    #[test]
    fn opposite_directions_conflict_and_emit_nothing() {
        let buy = intent("buy1", "BTCUSD", "BollingerBands", "buy", 0.8);
        let sell = intent("sell1", "BTCUSD", "Supertrend", "sell", 0.9);
        let r = dedupe_files(vec![("f".into(), vec![buy, sell])]);
        assert!(r.kept.is_empty());
        let d = &r.dispositions[0];
        assert_eq!(d.disposition, Disposition::Conflict);
        assert!(d.kept_intent_id.is_none());
        assert_eq!(d.dropped_intent_ids.len(), 2);
        assert!(r.warnings.iter().any(|w| w.starts_with("CONFLICT")));
    }

    #[test]
    fn symbols_are_independent_and_sorted() {
        let eth = intent("e", "ETHUSD", "Supertrend", "sell", 0.5);
        let btc = intent("b", "BTCUSD", "BollingerBands", "buy", 0.6);
        let r = dedupe_files(vec![("f".into(), vec![eth, btc])]);
        assert_eq!(r.kept.len(), 2);
        assert_eq!(r.kept[0].symbol, "BTCUSD");
        assert_eq!(r.kept[1].symbol, "ETHUSD");
        assert_eq!(r.dispositions[0].symbol, "BTCUSD");
    }

    #[test]
    fn empty_inputs_are_recorded() {
        let r = dedupe_files(vec![
            ("empty-a".into(), vec![]),
            (
                "f".into(),
                vec![intent("a", "BTCUSD", "BollingerBands", "buy", 0.6)],
            ),
        ]);
        assert_eq!(r.empty_inputs, vec!["empty-a".to_string()]);
        assert_eq!(r.kept.len(), 1);
    }

    #[test]
    fn intent_ids_carry_strategy_so_strategies_cannot_collide() {
        // Two strategies firing on the same symbol/side/timestamp must be
        // two distinct candidates, not one id seen twice.
        let a = intent(
            "crypto:BTCUSD:BollingerBands:buy:1",
            "BTCUSD",
            "BollingerBands",
            "buy",
            0.6,
        );
        let b = intent(
            "crypto:BTCUSD:Supertrend:buy:1",
            "BTCUSD",
            "Supertrend",
            "buy",
            0.6,
        );
        assert_ne!(a.intent_id, b.intent_id);
        let r = dedupe_files(vec![("f".into(), vec![a, b])]);
        assert_eq!(r.dispositions[0].disposition, Disposition::Deduped);
        assert_eq!(r.dispositions[0].dropped_intent_ids.len(), 1);
    }

    #[test]
    fn dedup_log_renders_conflict_and_dedup_rows() {
        let buy = intent("buy1", "BTCUSD", "BollingerBands", "buy", 0.8);
        let buy2 = intent("buy2", "BTCUSD", "Supertrend", "buy", 0.7);
        let sell = intent("sell1", "ETHUSD", "Supertrend", "sell", 0.9);
        let sell2 = intent("sell2", "ETHUSD", "DonchianBreakout", "buy", 0.9);
        let r = dedupe_files(vec![("f".into(), vec![buy, buy2, sell, sell2])]);
        let log = dedup_log(&r, "2026-09-23T00:00:00Z");
        assert!(log.contains("| Date/Time | Symbol | Signal Ref | Disposition |"));
        assert!(log.contains("CONFLICT"));
        assert!(log.contains("deduped: kept"));
    }

    #[test]
    fn dedup_log_is_empty_when_nothing_happened() {
        let r = dedupe_files(vec![("f".into(), vec![])]);
        assert!(dedup_log(&r, "t").is_empty());
    }
}
