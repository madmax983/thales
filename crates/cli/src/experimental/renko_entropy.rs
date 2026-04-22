//! Renko Entropy Module
//!
//! This module calculates the Shannon entropy of Renko brick transitions
//! (Up-Up, Up-Down, Down-Up, Down-Down). By measuring the randomness of
//! these transitions, it helps determine if a market is trending predictably
//! or moving randomly like pure noise.
//!
//! # Examples
//! ```rust
//! #[cfg(feature = "nova")]
//! # {
//! use thales_cli::experimental::renko_entropy::{analyze_renko_entropy, RenkoEntropyConfig};
//! use contracts::{BarSeries, Bar};
//!
//! let mut bars = Vec::new();
//! for i in 0..10 {
//!     bars.push(Bar { symbol: "TEST".into(), market: "test".into(), timeframe: "1d".into(), timestamp_unix_ms: i * 1000, open: 100.0 + i as f64 * 5.0, high: 105.0 + i as f64 * 5.0, low: 95.0 + i as f64 * 5.0, close: 100.0 + i as f64 * 5.0, volume: 1000.0 });
//! }
//! let series = BarSeries { schema_version: "v0".to_string(), bars };
//! let config = RenkoEntropyConfig { brick_size: 2.0 };
//!
//! if let Ok(report) = analyze_renko_entropy(&series, config) {
//!     println!("Entropy: {}", report.entropy);
//! }
//! # }
//! ```

use anyhow::Result;
use contracts::BarSeries;
use serde::{Deserialize, Serialize};

use crate::renko::{RenkoConfig, analyze_renko};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenkoEntropyConfig {
    pub brick_size: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenkoEntropyReport {
    pub symbol: String,
    pub brick_size: f64,
    pub total_transitions: usize,
    pub up_up_prob: f64,
    pub up_down_prob: f64,
    pub down_up_prob: f64,
    pub down_down_prob: f64,
    pub entropy: f64,
    pub max_entropy: f64,
    pub normalized_entropy: f64,
}

pub fn analyze_renko_entropy(
    series: &BarSeries,
    config: RenkoEntropyConfig,
) -> Result<RenkoEntropyReport> {
    let renko_config = RenkoConfig {
        brick_size: config.brick_size,
    };

    let renko_report = analyze_renko(series, renko_config)?;
    let bricks = renko_report.bricks;

    if bricks.len() < 2 {
        return Err(anyhow::anyhow!(
            "Not enough Renko bricks formed to calculate entropy."
        ));
    }

    let mut up_up = 0;
    let mut up_down = 0;
    let mut down_up = 0;
    let mut down_down = 0;

    for i in 1..bricks.len() {
        let prev_up = bricks[i - 1].is_up;
        let curr_up = bricks[i].is_up;

        match (prev_up, curr_up) {
            (true, true) => up_up += 1,
            (true, false) => up_down += 1,
            (false, true) => down_up += 1,
            (false, false) => down_down += 1,
        }
    }

    let total_transitions = up_up + up_down + down_up + down_down;
    if total_transitions == 0 {
        return Err(anyhow::anyhow!("No transitions found."));
    }

    let t_f64 = total_transitions as f64;
    let p_uu = up_up as f64 / t_f64;
    let p_ud = up_down as f64 / t_f64;
    let p_du = down_up as f64 / t_f64;
    let p_dd = down_down as f64 / t_f64;

    let mut entropy = 0.0;
    let probs = [p_uu, p_ud, p_du, p_dd];
    for p in probs {
        if p > 0.0 {
            entropy -= p * p.log2();
        }
    }

    let max_entropy = 4.0_f64.log2();
    let normalized_entropy = if max_entropy > 0.0 {
        entropy / max_entropy
    } else {
        0.0
    };

    Ok(RenkoEntropyReport {
        symbol: renko_report.symbol,
        brick_size: config.brick_size,
        total_transitions,
        up_up_prob: p_uu,
        up_down_prob: p_ud,
        down_up_prob: p_du,
        down_down_prob: p_dd,
        entropy,
        max_entropy,
        normalized_entropy,
    })
}

pub fn print_ascii_renko_entropy(report: &RenkoEntropyReport) {
    println!(
        "\n=== Renko-Markov Entropy Report for {} ===",
        report.symbol
    );
    println!("Brick Size:           {:.4}", report.brick_size);
    println!("Total Transitions:    {}", report.total_transitions);
    println!("--------------------------------------------------");
    println!("Up -> Up Prob:        {:.2}%", report.up_up_prob * 100.0);
    println!("Up -> Down Prob:      {:.2}%", report.up_down_prob * 100.0);
    println!("Down -> Up Prob:      {:.2}%", report.down_up_prob * 100.0);
    println!(
        "Down -> Down Prob:    {:.2}%",
        report.down_down_prob * 100.0
    );
    println!("--------------------------------------------------");
    println!("Entropy:              {:.4} bits", report.entropy);
    println!("Max Entropy:          {:.4} bits", report.max_entropy);
    println!(
        "Normalized Entropy:   {:.2}%",
        report.normalized_entropy * 100.0
    );
    println!("(100% = Pure Noise, 0% = Perfect Predictability)");
    println!("==================================================\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    fn create_bar(timestamp: i64, close: f64) -> Bar {
        Bar {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            timeframe: "1d".to_string(),
            timestamp_unix_ms: timestamp,
            open: close,
            high: close,
            low: close,
            close,
            volume: 100.0,
        }
    }

    #[test]
    fn test_renko_entropy() {
        let bars = vec![
            create_bar(1000, 10.0),
            create_bar(2000, 11.0),
            create_bar(3000, 12.0),
            create_bar(4000, 13.0),
        ];
        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let config = RenkoEntropyConfig { brick_size: 1.0 };
        let report = analyze_renko_entropy(&series, config).unwrap();

        // 3 bricks created, meaning 2 transitions, both up->up
        assert_eq!(report.total_transitions, 2);
        assert_eq!(report.up_up_prob, 1.0);
        assert_eq!(report.entropy, 0.0);
    }
}
