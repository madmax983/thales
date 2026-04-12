#![cfg(feature = "nova")]

use contracts::BarSeries;
use serde::{Deserialize, Serialize};

/// 🌟 Nova: Market Ecosystem
/// Translates market conditions into ecological terms (animals/biomes).
///
/// - High Volume & Low Volatility = Whale Feeding Ground (Accumulation)
/// - High Volume & Strong Up Trend = Bull Stampede
/// - High Volume & Strong Down Trend = Bear Hibernation (or Bear Attack)
/// - Low Volume & Sideways = Crab Tide Pool (Chop)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketEcosystem {
    pub volume_trend: f64, // Volume relative to past
    pub price_trend: f64,  // Price trend
    pub volatility: f64,   // Volatility
    pub ecosystem: String, // The final ecosystem state
}

pub fn analyze_ecosystem(series: &BarSeries) -> Option<MarketEcosystem> {
    if series.bars.len() < 14 {
        return None;
    }

    let n = series.bars.len();
    let recent = &series.bars[n - 14..n];

    // Price Trend (Temperature/Direction)
    let start_price = recent[0].close;
    let end_price = recent[13].close;
    let price_trend = if start_price > 0.0 {
        ((end_price - start_price) / start_price) * 100.0 // percentage
    } else {
        0.0
    };

    // Volatility
    let mut total_volatility = 0.0;
    let mut total_volume = 0.0;

    for bar in recent.iter().take(14) {
        if bar.low > 0.0 {
            total_volatility += (bar.high - bar.low) / bar.low;
        }
        total_volume += bar.volume;
    }
    let volatility = (total_volatility / 14.0) * 100.0;

    // Volume Trend
    let avg_volume = total_volume / 14.0;
    let latest_volume = recent[13].volume;
    let volume_trend = if avg_volume > 0.0 {
        latest_volume / avg_volume
    } else {
        0.0
    };

    // Determine Ecosystem Condition
    let ecosystem = if volume_trend > 1.5 && volatility < 2.0 {
        "Whale Feeding Ground 🐋".to_string()
    } else if volume_trend > 1.2 && price_trend > 5.0 {
        "Bull Stampede 🐂".to_string()
    } else if volume_trend > 1.2 && price_trend < -5.0 {
        "Bear Attack 🐻".to_string()
    } else if volatility < 1.5 && price_trend.abs() < 2.0 {
        "Crab Tide Pool 🦀".to_string()
    } else if volatility > 5.0 {
        "Shark Infested Waters 🦈".to_string()
    } else {
        "Plankton Drift 🦠".to_string()
    };

    Some(MarketEcosystem {
        volume_trend,
        price_trend,
        volatility,
        ecosystem,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    fn create_mock_bar(high: f64, low: f64, close: f64, volume: f64) -> Bar {
        Bar {
            symbol: "TEST".to_string(),
            market: "test".to_string(),
            timeframe: "1d".to_string(),
            timestamp_unix_ms: 0,
            open: close,
            high,
            low,
            close,
            volume,
        }
    }

    #[test]
    fn test_market_ecosystem_whale() {
        let mut bars = Vec::new();
        // create low volatility
        for _ in 0..13 {
            bars.push(create_mock_bar(100.5, 99.5, 100.0, 100.0));
        }
        // Last bar: huge volume, low volatility
        bars.push(create_mock_bar(100.5, 99.5, 100.0, 500.0));

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let eco = analyze_ecosystem(&series).unwrap();
        assert_eq!(eco.ecosystem, "Whale Feeding Ground 🐋");
    }

    #[test]
    fn test_market_ecosystem_bull() {
        let mut bars = Vec::new();
        let mut price = 100.0;
        for _ in 0..13 {
            bars.push(create_mock_bar(price + 2.0, price - 2.0, price, 100.0));
            price += 1.0;
        }
        // Last bar: high volume, high price (bull)
        bars.push(create_mock_bar(
            price + 5.0,
            price - 1.0,
            price + 5.0,
            500.0,
        ));

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let eco = analyze_ecosystem(&series).unwrap();
        assert_eq!(eco.ecosystem, "Bull Stampede 🐂");
    }
}
