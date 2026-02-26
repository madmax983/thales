use crate::indicators::{atr, stochastic};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StochasticOscillatorConfig {
    pub k_period: usize,
    pub k_smoothing: usize,
    pub d_period: usize,
    pub oversold_threshold: f64,
    pub overbought_threshold: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for StochasticOscillatorConfig {}

pub struct StochasticOscillator {
    config: StochasticOscillatorConfig,
}

impl StochasticOscillator {
    pub fn new(config: StochasticOscillatorConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for StochasticOscillator {
    fn name(&self) -> &str {
        "StochasticOscillator"
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        // Calculate Stochastic
        let (k_series, d_series) = stochastic::calculate(
            data,
            self.config.k_period,
            self.config.k_smoothing,
            self.config.d_period,
        )?;
        let k_arr = k_series.f64()?;
        let d_arr = d_series.f64()?;

        // Calculate ATR for Stop Loss
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();

        // Iterate through data
        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let k_curr = k_arr.get(i);
            let d_curr = d_arr.get(i);
            let k_prev = k_arr.get(i - 1);
            let d_prev = d_arr.get(i - 1);

            let price_opt = close_arr.get(i);
            let atr_opt = atr_arr.get(i);

            if let (Some(k), Some(d), Some(pk), Some(pd), Some(price)) =
                (k_curr, d_curr, k_prev, d_prev, price_opt)
            {
                // Entry Condition: Bullish Crossover in Oversold Zone
                // Cross: Previous K < Previous D AND Current K > Current D
                // Zone: Current K < Oversold Threshold
                let bullish_cross = pk < pd && k > d;
                let oversold = k < self.config.oversold_threshold;

                if bullish_cross && oversold {
                    // Calculate Stop Loss
                    let sl_dist = if let Some(atr_val) = atr_opt {
                        atr_val * self.config.stop_loss_atr_mult
                    } else {
                        price * 0.05 // Fallback 5%
                    };

                    let stop_loss = price - sl_dist;
                    // Take Profit could be fixed multiple or dynamic.
                    // Let's use 2x risk (2 * ATR)
                    let take_profit = price + (sl_dist * 2.0);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(stop_loss),
                        take_profit: Some(take_profit),
                        reason: format!(
                            "Stochastic Buy: K({:.2}) crossed above D({:.2}) in oversold zone",
                            k, d
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Exit Condition: Bearish Crossover in Overbought Zone
                // Cross: Previous K > Previous D AND Current K < Current D
                // Zone: Current K > Overbought Threshold
                let bearish_cross = pk > pd && k < d;
                let overbought = k > self.config.overbought_threshold;

                if bearish_cross && overbought {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Exit Long
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "Stochastic Sell: K({:.2}) crossed below D({:.2}) in overbought zone",
                            k, d
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: StochasticOscillatorConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_stochastic_signals() -> Result<()> {
        let config = StochasticOscillatorConfig {
            k_period: 3,
            k_smoothing: 1,
            d_period: 2,
            oversold_threshold: 20.0,
            overbought_threshold: 80.0,
            stop_loss_atr_mult: 1.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = StochasticOscillator::new(config);

        // Data construction
        // Need ATR(2) -> Need 3 bars min to start.
        // Need Stochastic(3,1,2).

        // Let's craft data to force K and D values.
        // Price:
        // 0: 10
        // 1: 10
        // 2: 10
        // 3: 10
        // ... Stable price = K is 50? No, if H=L=C, K is handled (50).

        // We want Oversold Buy.
        // K must go below 20. C near Low.
        // Then K crosses D up.

        // Let's use a sinusoidal pattern or explicit highs/lows.

        // i=0,1: Setup
        // i=2: C=91. Range [90, 100]. K = 100 * (1/10) = 10.
        // i=3: C=91. K=10. D (SMA 2 of K) = (10+10)/2 = 10. No cross.
        // i=4: C=91.5. K=15. D = (10+15)/2 = 12.5.
        //      Prev (i=3): K=10, D=10. (Equal).
        //      Curr (i=4): K=15, D=12.5. K > D.
        //      Is Prev K < Prev D? 10 < 10 is False. It is equal.
        //      My logic uses pk < pd. So strictly less.

        // Let's adjust i=3 slightly to ensure pk < pd.
        // We want K to dip then rise.
        // K sequence: 10, 8, 12.
        // D sequence: ?, 9, 10.
        // 8 < 9 (True). 12 > 10 (True).

        // Adjusted Close:
        // 2: 91.0 -> K=10.
        // 3: 90.5 -> K=5. D=(10+5)/2 = 7.5. (K < D) -> Bearish.
        // 4: 92.0 -> K=20. D=(5+20)/2 = 12.5. (K > D). Cross!
        // Oversold? K=20 <= 20? 20 < 20 is False. config is < 20.
        // Need K < 20.

        // Let's make i=4 K=15. C=91.5.
        // 4: 91.5 -> K=15. D=(5+15)/2 = 10.
        // Cross: Prev(5 < 7.5), Curr(15 > 10). Oversold(15 < 20).
        // SIGNAL!

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000],
            "high" =>  &[100.0, 100.0, 100.0, 100.0, 100.0],
            "low" =>   &[ 90.0,  90.0,  90.0,  90.0,  90.0],
            "close" => &[ 95.0,  95.0,  91.0,  90.5,  91.5]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Expect Entry at i=4 (ts=5000)
        let entries: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry)
            .collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].timestamp_ms, 5000);
        assert!(entries[0].reason.contains("Stochastic Buy"));

        Ok(())
    }
}
