use anyhow::Result;
use contracts::BarSeries;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntropyConfig {
    pub num_bins: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntropyReport {
    pub symbol: String,
    pub entropy: f64,
    pub max_entropy: f64,
    pub normalized_entropy: f64, // entropy / max_entropy, 0.0 to 1.0
    pub num_bins: usize,
    pub bins: Vec<EntropyBin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntropyBin {
    pub center_return: f64,
    pub probability: f64,
    pub count: usize,
}

pub fn analyze_entropy(series: &BarSeries, config: EntropyConfig) -> Result<EntropyReport> {
    if series.bars.len() < 2 {
        return Err(anyhow::anyhow!(
            "At least two bars are required for Entropy analysis"
        ));
    }

    if config.num_bins == 0 {
        return Err(anyhow::anyhow!("num_bins must be greater than 0"));
    }

    let symbol = series.bars[0].symbol.clone();

    // 1. Calculate returns
    let mut returns = Vec::with_capacity(series.bars.len() - 1);
    for i in 1..series.bars.len() {
        let prev = &series.bars[i - 1];
        let curr = &series.bars[i];
        if prev.close == 0.0 {
            continue;
        }
        // Log return: ln(P_t / P_{t-1})
        let ret = (curr.close / prev.close).ln();
        returns.push(ret);
    }

    if returns.is_empty() {
        return Err(anyhow::anyhow!("No valid returns calculated"));
    }

    // 2. Find range of returns
    let mut min_ret = f64::MAX;
    let mut max_ret = f64::MIN;

    for &r in &returns {
        if r < min_ret {
            min_ret = r;
        }
        if r > max_ret {
            max_ret = r;
        }
    }

    // Handle edge case of identical returns
    if max_ret == min_ret {
        let bins = vec![EntropyBin {
            center_return: min_ret,
            probability: 1.0,
            count: returns.len(),
        }];

        return Ok(EntropyReport {
            symbol,
            entropy: 0.0,
            max_entropy: 0.0,
            normalized_entropy: 0.0,
            num_bins: 1,
            bins,
        });
    }

    // 3. Create histogram bins
    let bin_size = (max_ret - min_ret) / config.num_bins as f64;
    let mut bin_counts = vec![0; config.num_bins];

    for &r in &returns {
        let mut bin_idx = ((r - min_ret) / bin_size).floor() as usize;
        if bin_idx >= config.num_bins {
            bin_idx = config.num_bins - 1; // inclusive upper bound
        }
        bin_counts[bin_idx] += 1;
    }

    // 4. Calculate probabilities and Shannon Entropy
    let total_returns = returns.len() as f64;
    let mut entropy = 0.0;
    let mut bins = Vec::with_capacity(config.num_bins);

    for (i, &count) in bin_counts.iter().enumerate() {
        let prob = count as f64 / total_returns;
        if prob > 0.0 {
            // H = -sum(p * log2(p))
            entropy -= prob * prob.log2();
        }

        let center_return = min_ret + (i as f64 * bin_size) + (bin_size / 2.0);
        bins.push(EntropyBin {
            center_return,
            probability: prob,
            count,
        });
    }

    // Max entropy is log2(N) where N is number of bins
    let max_entropy = (config.num_bins as f64).log2();
    let normalized_entropy = if max_entropy > 0.0 {
        entropy / max_entropy
    } else {
        0.0
    };

    Ok(EntropyReport {
        symbol,
        entropy,
        max_entropy,
        normalized_entropy,
        num_bins: config.num_bins,
        bins,
    })
}

#[cfg(feature = "nova")]
pub fn print_ascii_entropy(report: &EntropyReport) {
    if report.bins.is_empty() {
        println!("No entropy data to display.");
        return;
    }

    let max_count = report.bins.iter().map(|b| b.count).max().unwrap_or(0);
    if max_count == 0 {
        println!("No return counts to display.");
        return;
    }

    let term_width = 50; // Max width for the bars

    println!("\nShannon Entropy Analysis for {}", report.symbol);
    println!("--------------------------------------------------");
    println!("Entropy:            {:.4} bits", report.entropy);
    println!("Max Entropy:        {:.4} bits", report.max_entropy);
    println!(
        "Normalized Entropy: {:.2}%",
        report.normalized_entropy * 100.0
    );
    println!("(100% = Pure Noise/Randomness, 0% = Perfect Predictability)");
    println!("--------------------------------------------------");

    println!("{:>10} | {:>6} | Histogram", "Ret Center", "Prob");
    for bin in &report.bins {
        let bar_len = ((bin.count as f64 / max_count as f64) * term_width as f64).round() as usize;
        let bar = "█".repeat(bar_len);

        // Color intensity based on probability
        let (color_start, color_end) = if bin.probability > 0.2 {
            ("\x1b[1;31m", "\x1b[0m") // Red
        } else if bin.probability > 0.05 {
            ("\x1b[1;33m", "\x1b[0m") // Yellow
        } else {
            ("\x1b[1;30m", "\x1b[0m") // Dark gray
        };

        println!(
            "{:9.4}% | {:5.1}% | {}{}{}",
            bin.center_return * 100.0,
            bin.probability * 100.0,
            color_start,
            bar,
            color_end
        );
    }
    println!("--------------------------------------------------\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    fn create_bar(close: f64) -> Bar {
        Bar {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            timeframe: "1d".to_string(),
            timestamp_unix_ms: 100000,
            open: 100.0,
            high: 100.0,
            low: 100.0,
            close,
            volume: 100.0,
        }
    }

    #[test]
    fn test_entropy_zero_randomness() {
        let bars = vec![
            create_bar(100.0),
            create_bar(105.0),    // +~4.8% log return
            create_bar(110.25),   // +~4.8% log return
            create_bar(115.7625), // +~4.8% log return
        ];

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let config = EntropyConfig { num_bins: 10 };
        let report = analyze_entropy(&series, config).unwrap();

        // All returns are exactly the same, min_ret == max_ret, entropy should be 0
        assert_eq!(report.entropy, 0.0);
        assert_eq!(report.normalized_entropy, 0.0);
    }

    #[test]
    fn test_entropy_calculation() {
        let bars = vec![
            create_bar(100.0),
            create_bar(105.0), // ret1 > 0
            create_bar(100.0), // ret2 < 0
            create_bar(105.0), // ret3 > 0
            create_bar(100.0), // ret4 < 0
        ];

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let config = EntropyConfig { num_bins: 2 }; // Just positive vs negative
        let report = analyze_entropy(&series, config).unwrap();

        // We have 2 positive and 2 negative returns
        // Probability = 0.5 for each bin
        // H = -(0.5*log2(0.5) + 0.5*log2(0.5)) = 1.0
        assert!((report.entropy - 1.0).abs() < 1e-6);
        assert_eq!(report.normalized_entropy, 1.0); // 1.0 / log2(2) = 1.0
    }

    #[test]
    #[cfg(feature = "nova")]
    fn test_print_ascii_entropy() {
        let report = EntropyReport {
            symbol: "TEST".to_string(),
            entropy: 1.0,
            max_entropy: 2.0,
            normalized_entropy: 0.5,
            num_bins: 4,
            bins: vec![
                EntropyBin {
                    center_return: -0.05,
                    probability: 0.25,
                    count: 1,
                },
                EntropyBin {
                    center_return: 0.0,
                    probability: 0.5,
                    count: 2,
                },
                EntropyBin {
                    center_return: 0.05,
                    probability: 0.25,
                    count: 1,
                },
            ],
        };
        print_ascii_entropy(&report);
    }
}
