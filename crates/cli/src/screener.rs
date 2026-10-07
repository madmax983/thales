//! # Universe screener
//!
//! Cheap, deterministic ranking pass over the full audited universe (124
//! symbols). Each scan deep-dives only a bounded shortlist (fetch +
//! strategies + evidence + Jev gate per symbol is expensive), so this module
//! answers "which symbols deserve the deep dive *today*?" instead of always
//! taking the first N names in static manifest order.
//!
//! The screen is deliberately cheap: one daily-bars fetch per symbol and four
//! scale-free metrics computed locally — no Jev calls, no evidence pulls.
//! Each metric is converted to a percentile rank across the scored universe
//! and combined with fixed weights, so no single metric's units can dominate.
//!
//! Symbols with currently open positions are always shortlisted (in addition
//! to the top-N ranked picks): exits and scale-outs can only fire for symbols
//! the deep scan actually evaluates, so the book must stay in view.

use contracts::Bar;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// How many daily bars the screen needs: 21 for the 20-day windows plus a
/// couple of spare for the ATR seed.
const MIN_BARS: usize = 26;

/// Fixed composite weights: momentum 30%, range expansion 25%,
/// volume spike 25%, breakout proximity 20%.
const W_MOMENTUM: f64 = 0.30;
const W_RANGE: f64 = 0.25;
const W_VOLUME: f64 = 0.25;
const W_BREAKOUT: f64 = 0.20;

/// The four raw screen metrics for one symbol, all computed from daily bars.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ScreenMetrics {
    /// |close_last / close_20d_ago - 1|: magnitude of the 20-day move.
    /// Absolute (direction-agnostic): the ensemble holds both breakout and
    /// mean-reversion strategies, so movers in either direction are interesting.
    pub momentum_20d: f64,
    /// Last bar's (high - low) / ATR(14): is today moving unusually?
    pub range_expansion: f64,
    /// Last bar's volume / 20-day average volume: unusual activity.
    pub volume_spike: f64,
    /// 1 / (1 + distance_to_nearest_20d_edge_in_atr): near a Donchian edge,
    /// which is what the breakout strategy trades. In (0, 1], higher = closer.
    pub breakout_proximity: f64,
}

/// One ranked universe entry.
#[derive(Debug, Clone, Serialize)]
pub struct RankedSymbol {
    pub canonical: String,
    pub yahoo_alias: String,
    pub asset_class: String,
    /// Composite percentile-rank score in [0, 1]. Higher = more interesting.
    pub score: f64,
    /// True when the symbol is held open (always shortlisted, not ranked out).
    pub held: bool,
    pub metrics: ScreenMetrics,
}

fn true_range(high: f64, low: f64, prev_close: f64) -> f64 {
    (high - low)
        .max((high - prev_close).abs())
        .max((low - prev_close).abs())
}

/// Compute the four screen metrics from daily bars (oldest first).
/// Returns `None` when there are too few bars or the data is degenerate
/// (non-positive ATR, non-finite values) — the symbol is skipped, never
/// fabricated.
pub fn score_bars(bars: &[Bar]) -> Option<ScreenMetrics> {
    if bars.len() < MIN_BARS {
        return None;
    }
    let n = bars.len();
    let close = |i: usize| bars[i].close;
    let high = |i: usize| bars[i].high;
    let low = |i: usize| bars[i].low;
    let vol = |i: usize| bars[i].volume;

    // Everything must be finite and positive where it matters.
    if !(1..n).all(|i| {
        close(i).is_finite()
            && high(i).is_finite()
            && low(i).is_finite()
            && close(i) > 0.0
            && high(i) >= low(i)
    }) {
        return None;
    }

    // ATR(14): mean of the last 14 true ranges. Simple mean (not Wilder
    // smoothing) is plenty for a screening pass.
    let atr: f64 = (n - 14..n)
        .map(|i| true_range(high(i), low(i), close(i - 1)))
        .sum::<f64>()
        / 14.0;
    if !atr.is_finite() || atr <= 0.0 {
        return None;
    }

    let last_close = close(n - 1);
    let momentum_20d = (last_close / close(n - 21) - 1.0).abs();

    let range_expansion = (high(n - 1) - low(n - 1)) / atr;

    let avg_vol: f64 = (n - 21..n - 1).map(vol).sum::<f64>() / 20.0;
    if !avg_vol.is_finite() || avg_vol <= 0.0 {
        return None;
    }
    let volume_spike = vol(n - 1) / avg_vol;

    let hi20 = (n - 20..n).map(high).fold(f64::NEG_INFINITY, f64::max);
    let lo20 = (n - 20..n).map(low).fold(f64::INFINITY, f64::min);
    let dist_atr = ((hi20 - last_close).min(last_close - lo20) / atr).max(0.0);
    let breakout_proximity = 1.0 / (1.0 + dist_atr);

    let m = ScreenMetrics {
        momentum_20d,
        range_expansion,
        volume_spike,
        breakout_proximity,
    };
    if [
        m.momentum_20d,
        m.range_expansion,
        m.volume_spike,
        m.breakout_proximity,
    ]
    .iter()
    .all(|v| v.is_finite())
    {
        Some(m)
    } else {
        None
    }
}

/// Fraction of `values` strictly below `v`. With all-equal values this is 0
/// for every entry, which correctly contributes nothing to the composite.
fn percentile_rank(v: f64, values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let below = values.iter().filter(|&&x| x < v).count() as f64;
    below / (values.len() as f64 - 1.0)
}

/// Rank pre-scored `(canonical, metrics)` pairs by the fixed-weight composite
/// of percentile ranks. Deterministic: ties break on canonical symbol.
pub fn rank_by_score(scored: Vec<(String, ScreenMetrics)>) -> Vec<(String, f64)> {
    let moms: Vec<f64> = scored.iter().map(|(_, m)| m.momentum_20d).collect();
    let ranges: Vec<f64> = scored.iter().map(|(_, m)| m.range_expansion).collect();
    let vols: Vec<f64> = scored.iter().map(|(_, m)| m.volume_spike).collect();
    let proxs: Vec<f64> = scored.iter().map(|(_, m)| m.breakout_proximity).collect();

    let mut ranked: Vec<(String, f64)> = scored
        .into_iter()
        .map(|(sym, m)| {
            let score = W_MOMENTUM * percentile_rank(m.momentum_20d, &moms)
                + W_RANGE * percentile_rank(m.range_expansion, &ranges)
                + W_VOLUME * percentile_rank(m.volume_spike, &vols)
                + W_BREAKOUT * percentile_rank(m.breakout_proximity, &proxs);
            (sym, score)
        })
        .collect();
    ranked.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    ranked
}

/// One entry of the audited universe manifest.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct UniverseEntry {
    pub canonical: String,
    pub asset_class: String,
    pub aliases: HashMap<String, String>,
    #[allow(dead_code)]
    pub provenance: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
struct UniverseManifest {
    entries: Vec<UniverseEntry>,
}

const EMBEDDED_UNIVERSE_JSON: &str = include_str!("../universe/universe.json");

/// Load the audited universe manifest (env override or embedded), failing
/// closed on unreadable/invalid/empty manifests or duplicate canonicals.
pub fn load_manifest() -> Result<Vec<UniverseEntry>, String> {
    let json = match std::env::var("THALES_UNIVERSE_PATH") {
        Ok(path) if !path.trim().is_empty() => std::fs::read_to_string(&path)
            .map_err(|err| format!("universe manifest unreadable at {path}: {err}"))?,
        _ => EMBEDDED_UNIVERSE_JSON.to_string(),
    };
    let manifest: UniverseManifest =
        serde_json::from_str(&json).map_err(|err| format!("universe manifest invalid: {err}"))?;
    if manifest.entries.is_empty() {
        return Err("universe manifest has no entries".to_string());
    }
    let mut seen = HashSet::new();
    for entry in &manifest.entries {
        if entry.canonical.trim().is_empty() {
            return Err("universe manifest has a blank canonical symbol".to_string());
        }
        if !seen.insert(entry.canonical.clone()) {
            return Err(format!(
                "duplicate canonical symbol in universe manifest: {}",
                entry.canonical
            ));
        }
        if entry.asset_class.trim().is_empty() {
            return Err(format!(
                "universe manifest: {} is missing its asset class",
                entry.canonical
            ));
        }
        if entry.provenance.is_empty() {
            return Err(format!(
                "universe manifest: {} is missing provenance",
                entry.canonical
            ));
        }
    }
    Ok(manifest.entries)
}

/// The screen result: full ranking plus the deep-scan shortlist.
#[derive(Debug, Serialize)]
pub struct ScreenResult {
    /// Every scored symbol, best first, with its composite score and metrics.
    pub ranked: Vec<RankedSymbol>,
    /// Deep-scan shortlist: held symbols (manifest order) followed by the
    /// top-N ranked non-held symbols.
    pub shortlist: Vec<String>,
    /// Canonical symbols force-included because they are held open.
    pub held: Vec<String>,
    pub scored: usize,
    pub skipped: Vec<String>,
    pub top_n: usize,
}

/// Run the cheap ranking pass over the whole manifest.
///
/// `fetch` supplies daily bars for a Yahoo alias (injected so tests can run
/// without network). Per-symbol fetch/score failures are collected as
/// warnings and the symbol is skipped — never fabricated. Returns the result
/// plus warnings. Fails closed only when the manifest is bad or nothing
/// scored at all.
pub fn screen_universe(
    top_n: usize,
    held_symbols: &[String],
    fetch: &dyn Fn(&str) -> Result<Vec<Bar>, String>,
) -> Result<(ScreenResult, Vec<String>), String> {
    let manifest = load_manifest()?;
    let held: HashSet<String> = held_symbols
        .iter()
        .map(|s| s.trim().to_uppercase())
        .filter(|s| !s.is_empty())
        .collect();

    let mut scored_inputs: Vec<(UniverseEntry, ScreenMetrics)> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    for (i, entry) in manifest.iter().enumerate() {
        let alias = entry
            .aliases
            .get("yahoo")
            .map(String::as_str)
            .unwrap_or(entry.canonical.as_str());
        match fetch(alias) {
            Ok(bars) => match score_bars(&bars) {
                Some(metrics) => scored_inputs.push((entry.clone(), metrics)),
                None => {
                    skipped.push(entry.canonical.clone());
                    warnings.push(format!(
                        "{}: insufficient or degenerate bars ({} fetched) — skipped",
                        entry.canonical,
                        bars.len()
                    ));
                }
            },
            Err(e) => {
                skipped.push(entry.canonical.clone());
                warnings.push(format!(
                    "{}: fetch failed — skipped ({})",
                    entry.canonical, e
                ));
            }
        }
        if (i + 1) % 25 == 0 || i + 1 == manifest.len() {
            eprintln!(
                "screen-universe: {}/{} symbols screened",
                i + 1,
                manifest.len()
            );
        }
    }

    if scored_inputs.is_empty() {
        return Err(format!(
            "screen-universe: no symbols scored ({} skipped) — nothing to rank",
            skipped.len()
        ));
    }

    let ranked_scores = rank_by_score(
        scored_inputs
            .iter()
            .map(|(e, m)| (e.canonical.clone(), *m))
            .collect(),
    );
    let score_by_symbol: HashMap<&str, f64> = ranked_scores
        .iter()
        .map(|(s, score)| (s.as_str(), *score))
        .collect();

    let mut ranked: Vec<RankedSymbol> = scored_inputs
        .into_iter()
        .map(|(entry, metrics)| {
            let is_held = held.contains(&entry.canonical.to_uppercase());
            RankedSymbol {
                canonical: entry.canonical.clone(),
                yahoo_alias: entry
                    .aliases
                    .get("yahoo")
                    .cloned()
                    .unwrap_or_else(|| entry.canonical.clone()),
                asset_class: entry.asset_class.clone(),
                score: score_by_symbol
                    .get(entry.canonical.as_str())
                    .copied()
                    .unwrap_or(0.0),
                held: is_held,
                metrics,
            }
        })
        .collect();
    ranked.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.canonical.cmp(&b.canonical))
    });

    // Shortlist: held symbols first (manifest order — the book stays in
    // view so exits can fire), then the top-N ranked non-held symbols.
    let mut shortlist: Vec<String> = manifest
        .iter()
        .filter(|e| held.contains(&e.canonical.to_uppercase()))
        .filter(|e| ranked.iter().any(|r| r.canonical == e.canonical))
        .map(|e| e.canonical.clone())
        .collect();
    let held_list = shortlist.clone();
    for r in &ranked {
        if shortlist.len() >= held_list.len() + top_n {
            break;
        }
        if !r.held {
            shortlist.push(r.canonical.clone());
        }
    }

    Ok((
        ScreenResult {
            scored: ranked.len(),
            skipped,
            shortlist,
            held: held_list,
            top_n,
            ranked,
        },
        warnings,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds `n` synthetic daily bars ending at `last_close`, walking up
    /// `drift` per bar with a fixed 2% daily range and flat volume `vol`.
    fn synth_bars(n: usize, last_close: f64, drift: f64, vol: f64) -> Vec<Bar> {
        (0..n)
            .map(|i| {
                let close = last_close - drift * (n - 1 - i) as f64;
                Bar {
                    symbol: "TEST".to_string(),
                    market: "equities".to_string(),
                    timestamp_unix_ms: i as i64,
                    open: close,
                    high: close * 1.01,
                    low: close * 0.99,
                    close,
                    volume: vol,
                    timeframe: "1d".to_string(),
                    adjusted_close: None,
                }
            })
            .collect()
    }

    #[test]
    fn test_score_bars_needs_min_bars() {
        assert!(score_bars(&synth_bars(25, 100.0, 0.1, 1000.0)).is_none());
        assert!(score_bars(&synth_bars(26, 100.0, 0.1, 1000.0)).is_some());
    }

    #[test]
    fn test_score_bars_momentum_direction_agnostic() {
        let up = score_bars(&synth_bars(30, 120.0, 1.0, 1000.0)).unwrap();
        let down = score_bars(&synth_bars(30, 80.0, -1.0, 1000.0)).unwrap();
        // |120/100 - 1| == |80/100 - 1|: same magnitude either way.
        assert!((up.momentum_20d - down.momentum_20d).abs() < 1e-9);
        assert!(up.momentum_20d > 0.15);
    }

    #[test]
    fn test_score_bars_volume_spike_detected() {
        let mut bars = synth_bars(30, 100.0, 0.0, 1000.0);
        bars.last_mut().unwrap().volume = 5000.0;
        let m = score_bars(&bars).unwrap();
        assert!((m.volume_spike - 5.0).abs() < 1e-9);
    }

    #[test]
    fn test_score_bars_breakout_proximity_at_edge() {
        // Bars pinned to the top of their range: close == 20d high -> proximity 1.
        let bars: Vec<Bar> = (0..30)
            .map(|i| Bar {
                symbol: "TEST".to_string(),
                market: "equities".to_string(),
                timestamp_unix_ms: i as i64,
                open: 99.0,
                high: 101.0,
                low: 99.0,
                close: 101.0,
                volume: 1000.0,
                timeframe: "1d".to_string(),
                adjusted_close: None,
            })
            .collect();
        let m = score_bars(&bars).unwrap();
        assert!((m.breakout_proximity - 1.0).abs() < 1e-9);
        // Range is 2.0, ATR is 2.0 (TR = max(2, |101-101|, |99-101|) = 2).
        assert!((m.range_expansion - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_score_bars_rejects_degenerate() {
        let mut bars = synth_bars(30, 100.0, 0.1, 1000.0);
        bars[10].close = f64::NAN;
        assert!(score_bars(&bars).is_none());
        let mut flat = synth_bars(30, 100.0, 0.0, 1000.0);
        for b in flat.iter_mut() {
            b.high = 100.0;
            b.low = 100.0;
        }
        // Zero ATR -> degenerate.
        assert!(score_bars(&flat).is_none());
    }

    #[test]
    fn test_rank_by_score_orders_and_ties_break_deterministically() {
        let mk = |mom: f64| ScreenMetrics {
            momentum_20d: mom,
            range_expansion: 1.0,
            volume_spike: 1.0,
            breakout_proximity: 0.5,
        };
        let ranked = rank_by_score(vec![
            ("AAA".to_string(), mk(0.01)),
            ("BBB".to_string(), mk(0.05)),
            ("CCC".to_string(), mk(0.03)),
        ]);
        let order: Vec<&str> = ranked.iter().map(|(s, _)| s.as_str()).collect();
        assert_eq!(order, vec!["BBB", "CCC", "AAA"]);
        assert!(ranked[0].1 > ranked[1].1 && ranked[1].1 > ranked[2].1);

        // All-equal metrics -> all scores 0, canonical tiebreak.
        let tied = rank_by_score(vec![
            ("ZZZ".to_string(), mk(0.02)),
            ("AAA".to_string(), mk(0.02)),
        ]);
        assert_eq!(tied[0].0, "AAA");
        assert_eq!(tied[1].0, "ZZZ");
    }
}
