#![cfg(feature = "nova")]

//! 🌟 Nova: Market Weather Module
//!
//! This module gamifies market data by translating abstract financial conditions
//! into relatable meteorological terms.
//!
//! Why explain market dynamics using dry statistics when you can warn traders of
//! an incoming "Hurricane" (high volatility and volume) or advise them to wait out
//! the "Foggy / Stagnant" conditions (low momentum)?
//!
//! This approach provides an alternative, intuitive way to quickly assess a financial
//! instrument's current state based on temperature (trend), wind speed (momentum),
//! and precipitation (volume).

use contracts::BarSeries;
use serde::{Deserialize, Serialize};

/// Represents the meteorological state derived from market price action.
///
/// - High Volatility = Stormy
/// - High Momentum = Windy
/// - High Volume = Heavy Precipitation
/// - Sideways = Foggy
///
/// # Examples
///
/// ```rust
/// use thales_cli::experimental::market_weather::MarketWeather;
///
/// let weather = MarketWeather {
///     temperature: 15.0,
///     wind_speed: 25.5,
///     precipitation: 1.8,
///     condition: "Hurricane".to_string(),
/// };
///
/// assert_eq!(weather.condition, "Hurricane");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketWeather {
    pub temperature: f64,   // Trend (positive = hot, negative = cold)
    pub wind_speed: f64,    // Momentum
    pub precipitation: f64, // Volume relative to recent
    pub condition: String,  // The final weather state
}

/// Calculates `MarketWeather` by evaluating the trend, momentum, and volume over the last 14 bars.
///
/// # Examples
///
/// ```rust
/// use contracts::{Bar, BarSeries};
/// use thales_cli::experimental::market_weather::calculate_weather;
///
/// // Create a dummy series with at least 14 bars to satisfy the requirement
/// let mut bars = Vec::new();
/// for i in 0..14 {
///     bars.push(Bar {
///         symbol: "AAPL".to_string(),
///         market: "equities".to_string(),
///         timeframe: "1d".to_string(),
///         timestamp_unix_ms: i * 86400000,
///         open: 100.0, high: 100.0, low: 100.0, close: 100.0, volume: 100.0,
///     });
/// }
/// let series = BarSeries { schema_version: "v1".to_string(), bars };
///
/// let weather = calculate_weather(&series).unwrap();
/// assert_eq!(weather.condition, "Foggy / Stagnant");
/// ```
pub fn calculate_weather(series: &BarSeries) -> Option<MarketWeather> {
    if series.bars.len() < 14 {
        return None;
    }

    let n = series.bars.len();
    let recent = &series.bars[n - 14..n];

    // Temperature (Trend over last 14 bars)
    let start_price = recent[0].close;
    let end_price = recent[13].close;
    let temperature = ((end_price - start_price) / start_price) * 100.0; // percentage

    // Wind Speed (Momentum: average of absolute bar changes)
    let mut total_change = 0.0;
    let mut total_volume = 0.0;

    for i in 1..14 {
        let change = ((recent[i].close - recent[i - 1].close) / recent[i - 1].close).abs();
        total_change += change;
        total_volume += recent[i].volume;
    }
    let wind_speed = (total_change / 13.0) * 1000.0; // Scaled for readability

    // Precipitation (Volume comparison)
    let avg_volume = total_volume / 13.0;
    let latest_volume = recent[13].volume;
    let precipitation = if avg_volume > 0.0 {
        latest_volume / avg_volume
    } else {
        0.0
    };

    // Determine Condition
    let condition = if wind_speed > 20.0 && precipitation > 1.5 {
        "Hurricane".to_string()
    } else if wind_speed > 10.0 {
        if temperature > 0.0 {
            "Hot & Windy".to_string()
        } else {
            "Blizzard".to_string()
        }
    } else if wind_speed < 2.0 {
        "Foggy / Stagnant".to_string()
    } else if temperature > 5.0 {
        "Heatwave".to_string()
    } else if temperature < -5.0 {
        "Freezing".to_string()
    } else if precipitation > 2.0 {
        "Heavy Rain".to_string()
    } else {
        "Clear Skies".to_string()
    };

    Some(MarketWeather {
        temperature,
        wind_speed,
        precipitation,
        condition,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    fn create_mock_bar(close: f64, volume: f64) -> Bar {
        Bar {
            symbol: "TEST".to_string(),
            market: "test".to_string(),
            timeframe: "1d".to_string(),
            timestamp_unix_ms: 0,
            open: close,
            high: close,
            low: close,
            close,
            volume,
        }
    }

    #[test]
    fn test_market_weather_hurricane() {
        let mut bars = Vec::new();
        let mut price = 100.0;
        for i in 0..13 {
            bars.push(create_mock_bar(price, 100.0));
            if i % 2 == 0 {
                price += 5.0;
            } else {
                price -= 4.0;
            }
        }
        // Last bar: huge move, huge volume
        bars.push(create_mock_bar(150.0, 500.0));

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let weather = calculate_weather(&series).unwrap();
        assert!(weather.wind_speed > 20.0);
        assert!(weather.precipitation > 1.5);
        assert_eq!(weather.condition, "Hurricane");
    }

    #[test]
    fn test_market_weather_foggy() {
        let mut bars = Vec::new();
        for _ in 0..14 {
            bars.push(create_mock_bar(100.0, 100.0));
        }

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let weather = calculate_weather(&series).unwrap();
        assert_eq!(weather.temperature, 0.0);
        assert_eq!(weather.wind_speed, 0.0);
        assert_eq!(weather.condition, "Foggy / Stagnant");
    }
}
