use anyhow::Result;
use contracts::BarSeries;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeProfileConfig {
    pub num_bins: usize,
    pub value_area_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeProfileReport {
    pub symbol: String,
    pub poc_price: f64,
    pub poc_volume: f64,
    pub value_area_high: f64,
    pub value_area_low: f64,
    pub total_volume: f64,
    pub bins: Vec<VolumeBin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeBin {
    pub price_level: f64,
    pub volume: f64,
}

pub fn analyze_volume_profile(
    series: &BarSeries,
    config: VolumeProfileConfig,
) -> Result<VolumeProfileReport> {
    if series.bars.is_empty() {
        return Err(anyhow::anyhow!("No bars provided"));
    }

    if config.num_bins == 0 {
        return Err(anyhow::anyhow!("num_bins must be greater than 0"));
    }

    let symbol = series.bars[0].symbol.clone();

    // 1. Find price range
    let mut min_price = f64::MAX;
    let mut max_price = f64::MIN;

    for bar in &series.bars {
        if bar.low < min_price {
            min_price = bar.low;
        }
        if bar.high > max_price {
            max_price = bar.high;
        }
    }

    // Handle edge case where max_price == min_price
    if max_price == min_price {
        let total_volume: f64 = series.bars.iter().map(|b| b.volume).sum();
        let mut bins = Vec::new();
        bins.push(VolumeBin {
            price_level: min_price,
            volume: total_volume,
        });

        return Ok(VolumeProfileReport {
            symbol,
            poc_price: min_price,
            poc_volume: total_volume,
            value_area_high: min_price,
            value_area_low: min_price,
            total_volume,
            bins,
        });
    }

    // 2. Initialize Bins
    let bin_size = (max_price - min_price) / config.num_bins as f64;
    let mut bin_volumes = vec![0.0; config.num_bins];
    let mut total_volume = 0.0;

    // 3. Distribute Volume
    for bar in &series.bars {
        let bar_range = bar.high - bar.low;
        let bar_vol = bar.volume;
        total_volume += bar_vol;

        if bar_range > 0.0 {
            // Find which bins overlap with this bar
            let start_bin = ((bar.low - min_price) / bin_size).floor() as usize;
            let mut end_bin = ((bar.high - min_price) / bin_size).floor() as usize;

            let start_bin = start_bin.min(config.num_bins - 1);
            end_bin = end_bin.min(config.num_bins - 1);

            let num_overlapping_bins = (end_bin - start_bin + 1) as f64;
            let vol_per_bin = bar_vol / num_overlapping_bins;

            for i in start_bin..=end_bin {
                bin_volumes[i] += vol_per_bin;
            }
        } else {
            // If bar_range is 0, add all volume to the single bin it falls into
            let bin = ((bar.low - min_price) / bin_size).floor() as usize;
            let bin = bin.min(config.num_bins - 1);
            bin_volumes[bin] += bar_vol;
        }
    }

    // 4. Find Point of Control (POC)
    let mut poc_idx = 0;
    let mut max_vol = 0.0;

    for (i, &vol) in bin_volumes.iter().enumerate() {
        if vol > max_vol {
            max_vol = vol;
            poc_idx = i;
        }
    }

    // 5. Calculate Value Area
    let target_volume = total_volume * config.value_area_pct;
    let mut current_volume = bin_volumes[poc_idx];

    let mut va_low_idx = poc_idx;
    let mut va_high_idx = poc_idx;

    // Expand VA from POC up and down
    while current_volume < target_volume {
        let up_idx = if va_high_idx + 1 < config.num_bins {
            Some(va_high_idx + 1)
        } else {
            None
        };
        let down_idx = if va_low_idx > 0 {
            Some(va_low_idx - 1)
        } else {
            None
        };

        let up_vol = up_idx.map(|idx| bin_volumes[idx]).unwrap_or(-1.0);
        let down_vol = down_idx.map(|idx| bin_volumes[idx]).unwrap_or(-1.0);

        if up_vol == -1.0 && down_vol == -1.0 {
            break; // No more bins to expand into
        }

        if up_vol > down_vol {
            let next_idx = up_idx.unwrap();
            current_volume += bin_volumes[next_idx];
            va_high_idx = next_idx;
        } else {
            let next_idx = down_idx.unwrap();
            current_volume += bin_volumes[next_idx];
            va_low_idx = next_idx;
        }
    }

    // 6. Format Result
    let mut bins = Vec::with_capacity(config.num_bins);
    for (i, &vol) in bin_volumes.iter().enumerate() {
        let price_level = min_price + (i as f64 * bin_size) + (bin_size / 2.0);
        bins.push(VolumeBin {
            price_level,
            volume: vol,
        });
    }

    let poc_price = bins[poc_idx].price_level;
    let va_low_price = min_price + (va_low_idx as f64 * bin_size); // Lower bound of VA
    let va_high_price = min_price + ((va_high_idx + 1) as f64 * bin_size); // Upper bound of VA

    Ok(VolumeProfileReport {
        symbol,
        poc_price,
        poc_volume: max_vol,
        value_area_high: va_high_price,
        value_area_low: va_low_price,
        total_volume,
        bins,
    })
}

#[cfg(feature = "nova")]
pub fn print_ascii_profile(report: &VolumeProfileReport) {
    let max_vol = report.bins.iter().map(|b| b.volume).fold(0.0, f64::max);

    if max_vol == 0.0 {
        println!("No volume data to display.");
        return;
    }

    let term_width = 50; // Max width for the bars

    println!("\nVolume Profile for {}", report.symbol);
    println!("--------------------------------------------------");

    for bin in report.bins.iter().rev() {
        // Start from highest price
        let is_va =
            bin.price_level >= report.value_area_low && bin.price_level <= report.value_area_high;
        let is_poc = (bin.volume - report.poc_volume).abs() < f64::EPSILON;

        let bar_len = ((bin.volume / max_vol) * term_width as f64).round() as usize;
        let bar = "█".repeat(bar_len);

        // Styling based on Value Area and Point of Control
        let (color_start, color_end) = if is_poc {
            ("\x1b[1;31m", "\x1b[0m") // Red for POC
        } else if is_va {
            ("\x1b[1;34m", "\x1b[0m") // Blue for Value Area
        } else {
            ("\x1b[1;30m", "\x1b[0m") // Dark gray outside VA
        };

        // Align price to 10 characters
        println!(
            "{:10.2} | {}{}{}",
            bin.price_level, color_start, bar, color_end
        );
    }

    println!("--------------------------------------------------");
    println!(
        "POC: {:.2} (Vol: {:.2})",
        report.poc_price, report.poc_volume
    );
    println!(
        "Value Area: {:.2} - {:.2}",
        report.value_area_low, report.value_area_high
    );
    println!("Total Volume: {:.2}\n", report.total_volume);
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    #[test]
    fn test_volume_profile_poc() {
        let mut bars = Vec::new();
        let now = 100000;

        // Bar 1: 90 - 100, Vol 100
        bars.push(Bar {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: now,
            open: 95.0,
            high: 100.0,
            low: 90.0,
            close: 95.0,
            volume: 100.0,
        });

        // Bar 2: 95 - 105, Vol 500 (Heaviest volume)
        bars.push(Bar {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: now + 60000,
            open: 100.0,
            high: 105.0,
            low: 95.0,
            close: 100.0,
            volume: 500.0,
        });

        // Bar 3: 100 - 110, Vol 50
        bars.push(Bar {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: now + 120000,
            open: 105.0,
            high: 110.0,
            low: 100.0,
            close: 105.0,
            volume: 50.0,
        });

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let config = VolumeProfileConfig {
            num_bins: 10,
            value_area_pct: 0.70,
        };

        let report = analyze_volume_profile(&series, config).unwrap();

        // POC should be roughly around the middle of Bar 2 (around 100.0)
        assert!(report.poc_price >= 95.0 && report.poc_price <= 105.0);
        assert_eq!(report.total_volume, 650.0);
        assert!(report.value_area_high >= report.value_area_low);
        assert!(report.bins.len() <= 10);
    }

    #[test]
    #[cfg(feature = "nova")]
    fn test_print_ascii_profile() {
        let report = VolumeProfileReport {
            symbol: "TEST".to_string(),
            poc_price: 100.0,
            poc_volume: 500.0,
            value_area_high: 105.0,
            value_area_low: 95.0,
            total_volume: 650.0,
            bins: vec![
                VolumeBin {
                    price_level: 95.0,
                    volume: 100.0,
                },
                VolumeBin {
                    price_level: 100.0,
                    volume: 500.0,
                },
                VolumeBin {
                    price_level: 105.0,
                    volume: 50.0,
                },
            ],
        };

        // This test ensures the function doesn't panic and covers the visualization logic.
        print_ascii_profile(&report);
    }
}
