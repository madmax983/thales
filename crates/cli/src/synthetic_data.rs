use anyhow::Result;
use contracts::{Bar, BarSeries};
use rand_distr::{Distribution, Normal};
use serde::{Deserialize, Serialize};

/// Configuration for synthetic data generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntheticDataConfig {
    pub symbol: String,
    pub initial_price: f64,
    pub drift: f64,
    pub volatility: f64,
    pub num_bars: usize,
    pub timeframe: String,
    pub start_time_ms: i64,
}

pub fn generate_synthetic_data(config: SyntheticDataConfig) -> Result<BarSeries> {
    if config.num_bars == 0 {
        return Err(anyhow::anyhow!("num_bars must be greater than 0"));
    }

    let mut bars = Vec::with_capacity(config.num_bars);
    let mut rng = rand::thread_rng();
    let normal_dist = Normal::new(0.0, 1.0)?;

    let mut current_price = config.initial_price;
    let mut current_time = config.start_time_ms;

    // Parse timeframe to ms, roughly.
    let time_step_ms: i64 = match config.timeframe.as_str() {
        "1m" => 60_000,
        "5m" => 300_000,
        "15m" => 900_000,
        "1h" => 3_600_000,
        "4h" => 14_400_000,
        "1d" => 86_400_000,
        _ => 86_400_000, // default 1d
    };

    let dt = 1.0; // Assume time step of 1 for the GBM formula per bar.

    for _ in 0..config.num_bars {
        let z = normal_dist.sample(&mut rng);
        // Geometric Brownian Motion step
        // dS = S * (mu * dt + sigma * dW)
        // S_new = S_old * exp((mu - 0.5 * sigma^2) * dt + sigma * sqrt(dt) * Z)
        let drift_term = (config.drift - 0.5 * config.volatility.powi(2)) * dt;
        let vol_term = config.volatility * dt.sqrt() * z;
        let next_price = current_price * (drift_term + vol_term).exp();

        // Create OHLC from the step
        // We simulate intraday volatility for high/low
        let open = current_price;
        let close = next_price;

        let intra_z1 = normal_dist.sample(&mut rng).abs();
        let intra_z2 = normal_dist.sample(&mut rng).abs();

        // High is max(open, close) + random noise scaled by volatility
        let high = open.max(close) * (1.0 + config.volatility * 0.5 * intra_z1);
        // Low is min(open, close) - random noise scaled by volatility
        let low = open.min(close) * (1.0 - config.volatility * 0.5 * intra_z2);

        // Volume is somewhat randomized
        let volume_z = normal_dist.sample(&mut rng).abs();
        let volume = 1000.0 * (1.0 + volume_z);

        bars.push(Bar {
            symbol: config.symbol.clone(),
            market: "synthetic".to_string(),
            timeframe: config.timeframe.clone(),
            timestamp_unix_ms: current_time,
            open,
            high,
            low,
            close,
            volume,
        });

        current_price = close;
        current_time += time_step_ms;
    }

    Ok(BarSeries {
        schema_version: "v0".to_string(),
        bars,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_synthetic_data() {
        let config = SyntheticDataConfig {
            symbol: "TEST".to_string(),
            initial_price: 100.0,
            drift: 0.0001,
            volatility: 0.01,
            num_bars: 10,
            timeframe: "1d".to_string(),
            start_time_ms: 100000,
        };

        let result = generate_synthetic_data(config);
        assert!(result.is_ok());
        let series = result.unwrap();
        assert_eq!(series.bars.len(), 10);
        assert_eq!(series.bars[0].open, 100.0);
    }
}
