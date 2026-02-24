use crate::analysis;
use crate::rag;
use anyhow::Result;
use contracts::{BarSeries, TradeIntent};
use polars::prelude::*;
use std::path::Path;
use strategies::bollinger_bands::{BollingerBandsConfig, BollingerBandsMeanReversion};
use strategies::ema_crossover::{EmaCrossover, EmaCrossoverConfig};
use strategies::rsi_mean_reversion::{RsiMeanReversion, RsiMeanReversionConfig};
use strategies::strategy::{SignalType, Strategy};

pub async fn generate_signals(
    bars: &BarSeries,
    strategy_name: &str,
    history_path: Option<&Path>,
    risk_per_trade: f64,
) -> Result<Vec<TradeIntent>> {
    // 1. Analyze Market
    let market_analysis = analysis::analyze(bars);

    // 2. Prepare Data for Strategy
    let df = bars_to_dataframe(bars)?;

    let latest_timestamp = bars.bars.last().map(|b| b.timestamp_unix_ms).unwrap_or(0);

    // 3. Run Strategy
    // For now, hardcode BollingerBandsMeanReversion. In future, use factory.
    let strategy: Box<dyn Strategy> = if strategy_name == "BollingerBands" || strategy_name == "BollingerBandsMeanReversion" {
        let config = BollingerBandsConfig {
            window_size: 20,
            num_std_dev: 2.0,
            stop_loss_pct: 0.05, // Strategy config default, but we'll override or use for initial filtering
            symbol: market_analysis.symbol.clone(),
        };
        Box::new(BollingerBandsMeanReversion::new(config))
    } else if strategy_name == "EmaCrossover" {
        let config = EmaCrossoverConfig {
            short_window: 9,
            long_window: 21,
            stop_loss_pct: 0.05,
            symbol: market_analysis.symbol.clone(),
        };
        Box::new(EmaCrossover::new(config))
    } else if strategy_name == "RsiMeanReversion" {
        let config = RsiMeanReversionConfig {
            period: 14,
            oversold_threshold: 30.0,
            overbought_threshold: 70.0,
            stop_loss_pct: 0.05,
            symbol: market_analysis.symbol.clone(),
        };
        Box::new(RsiMeanReversion::new(config))
    } else {
        // Fallback or Error
        return Err(anyhow::anyhow!("Unknown strategy: {}", strategy_name));
    };

    let raw_signals = strategy.generate_signals(&df).await?;

    // 4. Enrich and Filter Signals
    let mut intents = Vec::new();

    // Check existing signals count from history
    let existing_signals_count = if let Some(path) = history_path {
        rag::count_todays_signals(&market_analysis.symbol, path, latest_timestamp)?
    } else {
        0
    };

    let signals_today = existing_signals_count;

    // Filter for latest signals only
    // Filter, Sort and Deduplicate signals
    // 1. Filter by timestamp
    let mut valid_signals: Vec<_> = raw_signals
        .into_iter()
        .filter(|s| s.timestamp_ms == latest_timestamp)
        .collect();

    // 2. Sort by Priority (Entry > ScaleIn > Exit > ScaleOut) and then Confidence
    valid_signals.sort_by(|a, b| {
        let p_a = signal_priority(&a.signal_type);
        let p_b = signal_priority(&b.signal_type);
        if p_a != p_b {
            p_a.cmp(&p_b)
        } else {
            b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal)
        }
    });

    // 3. Take the best signal (if any)
    // We assume the strategy output is for the single symbol we analyzed.
    // If there are multiple signals (e.g. conflicting or redundant), the top one wins.
    if let Some(signal) = valid_signals.first() {
        // SIGNAL FILTERING: Limit to 3 signals per symbol per day.
        // We check `signals_today` count from history. If we have 0, 1, or 2, we allow a new one.
        // If we have 3 or more, we skip.
        if signals_today < 3 {
             // RAG Step: Check history
            let similar_trades = if let Some(path) = history_path {
                rag::find_similar_trades(&market_analysis, path).unwrap_or_default()
            } else {
                Vec::new()
            };

            let historical_context = rag::summarize_history(&similar_trades);

            // Position Sizing and SL/TP
            let last_close = bars.bars.last().map(|b| b.close).unwrap_or(100.0);
            let atr = market_analysis.atr.unwrap_or(last_close * 0.01); // Fallback to 1% if ATR missing

            // Calculate SL/TP
            let (stop_loss, take_profit, size_hint) = match signal.signal_type {
                SignalType::Entry | SignalType::ScaleIn => {
                    // Use Strategy SL if provided, else ATR fallback (2.0 ATR)
                    let sl = if let Some(s) = signal.stop_loss {
                        Some(s)
                    } else {
                        if signal.side == "buy" {
                            Some(last_close - (2.0 * atr))
                        } else {
                            Some(last_close + (2.0 * atr))
                        }
                    };

                    // Use Strategy TP if provided, else ATR fallback (4.0 ATR)
                    let tp = if let Some(t) = signal.take_profit {
                        Some(t)
                    } else {
                        if signal.side == "buy" {
                            Some(last_close + (4.0 * atr))
                        } else {
                            Some(last_close - (4.0 * atr))
                        }
                    };

                    // Calculate Size based on Risk and SL Distance
                    // Size = Risk / |Entry - SL|
                    let size = if let Some(s) = sl {
                        let dist = (last_close - s).abs();
                        if dist > 0.0 {
                            let calc_size = risk_per_trade / dist;
                            // Safety check: Avoid Infinite or NaN sizes
                            // Also cap max size if needed, but for now just ensure finite
                            if calc_size.is_finite() {
                                format!("{:.6}", calc_size)
                            } else {
                                "0".to_string()
                            }
                        } else {
                            // If SL distance is 0 (e.g. no volatility), return 0 to indicate invalid sizing
                            "0".to_string()
                        }
                    } else {
                         signal.size_hint.clone()
                    };

                    (sl, tp, size)
                },
                SignalType::Exit | SignalType::ScaleOut => {
                    (None, None, signal.size_hint.clone())
                }
            };

            let time_in_force = if market_analysis.market == "crypto" {
                "GTC".to_string()
            } else {
                "day".to_string()
            };

            // Map SignalType
            let signal_type_str = format!("{:?}", signal.signal_type);

            let intent = TradeIntent {
                intent_id: format!("{}:{}:{}:{}", market_analysis.market, signal.symbol, signal.side, signal.timestamp_ms),
                market: market_analysis.market.clone(),
                symbol: signal.symbol.clone(),
                side: signal.side.clone(),
                size_hint,
                confidence: signal.confidence,
                horizon: "1d".to_string(),
                rationale: format!("Strategy: {}. Reason: {}. Market Context: {} ({} Volatility). {}", strategy.name(), signal.reason, market_analysis.regime, market_analysis.volatility, historical_context),
                invalidation: "Price hits Stop Loss".to_string(),
                schema_version: "v0".to_string(),
                signal_type: Some(signal_type_str),
                stop_loss,
                take_profit,
                order_type: "market".to_string(),
                limit_price: None,
                stop_price: None,
                time_in_force,
            };

            intents.push(intent);
        }
    }

    Ok(intents)
}

fn signal_priority(signal_type: &SignalType) -> u8 {
    match signal_type {
        SignalType::Entry => 1,
        SignalType::ScaleIn => 2,
        SignalType::Exit => 3,
        SignalType::ScaleOut => 4,
    }
}

fn bars_to_dataframe(series: &BarSeries) -> Result<DataFrame> {
    let closes: Vec<f64> = series.bars.iter().map(|b| b.close).collect();
    let times: Vec<i64> = series.bars.iter().map(|b| b.timestamp_unix_ms).collect();

    let df = df!(
        "close" => closes,
        "timestamp_unix_ms" => times
    )?;
    Ok(df)
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    #[tokio::test]
    async fn test_generate_signals_e2e() -> Result<()> {
        // Create a dummy BarSeries that triggers a Bollinger Band signal
        let mut bars = Vec::new();
        let now = 100000;

        // Generate stable price
        for i in 0..20 {
            bars.push(Bar {
                symbol: "AAPL".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: 100.0,
                high: 101.0,
                low: 99.0,
                close: 100.0,
                volume: 1000.0,
            });
        }

        // Generate spike to trigger Sell signal (Upper Band breakout)
        // This should trigger Entry (Sell)
        bars.push(Bar {
            symbol: "AAPL".to_string(),
            market: "equities".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: now + 20 * 60000,
            open: 100.0,
            high: 110.0,
            low: 100.0,
            close: 110.0, // Big jump
            volume: 5000.0,
        });

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let intents = generate_signals(&series, "BollingerBands", None, 100.0).await?;

        assert!(!intents.is_empty());
        let intent = &intents[0];
        assert_eq!(intent.symbol, "AAPL");
        assert_eq!(intent.side, "sell"); // Should be sell
        assert!(intent.stop_loss.is_some());
        assert!(intent.take_profit.is_some());

        // Verify SL/TP logic for Sell
        let close = 110.0;
        // Volatility might be calculated as High or Medium depending on the implementation in analysis.rs
        // But let's check basic direction
        assert!(intent.stop_loss.unwrap() > close); // SL above entry for short
        assert!(intent.take_profit.unwrap() < close); // TP below entry for short

        Ok(())
    }

    #[tokio::test]
    async fn test_generate_signals_atr_sizing() -> Result<()> {
        // Create bars with predictable range for ATR calculation but varying close for StdDev
        let mut bars = Vec::new();
        let now = 100000;

        // Fill history with oscillating price to ensure StdDev > 0
        for i in 0..20 {
            let close = if i % 2 == 0 { 100.0 } else { 102.0 };
            bars.push(Bar {
                symbol: "TEST".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: close,
                high: close + 1.0,
                low: close - 1.0,
                close: close,
                volume: 1000.0,
            });
        }

        // Trigger Buy signal: Drop below lower band
        bars.push(Bar {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: now + 20 * 60000,
            open: 100.0,
            high: 101.0,
            low: 90.0,
            close: 90.0, // Drop
            volume: 1000.0,
        });

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let intents = generate_signals(&series, "BollingerBands", None, 100.0).await?;

        assert!(!intents.is_empty());
        let intent = &intents[0];
        assert_eq!(intent.side, "buy");

        let close = 90.0;
        let sl = intent.stop_loss.unwrap();
        let tp = intent.take_profit.unwrap();

        assert!(sl < close);
        assert!(tp > close);

        // Check size is calculated
        let size: f64 = intent.size_hint.parse().unwrap();
        assert!(size > 0.0);

        // Check signal type
        // Should be Entry or ScaleIn
        assert!(intent.signal_type.is_some());

        Ok(())
    }

    #[test]
    fn test_signal_priority() {
        assert!(signal_priority(&SignalType::Entry) < signal_priority(&SignalType::ScaleIn));
        assert!(signal_priority(&SignalType::ScaleIn) < signal_priority(&SignalType::Exit));
        assert!(signal_priority(&SignalType::Exit) < signal_priority(&SignalType::ScaleOut));
    }

    #[tokio::test]
    async fn test_generate_signals_crypto_sizing() -> Result<()> {
        // High priced asset (BTC), small risk
        let mut bars = Vec::new();
        let now = 100000;

        // Close 50000. Oscillation for StdDev.
        for i in 0..20 {
            let close = if i % 2 == 0 { 50000.0 } else { 50100.0 };
            bars.push(Bar {
                symbol: "BTCUSD".to_string(),
                market: "crypto".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: close,
                high: close + 500.0,
                low: close - 500.0,
                close: close,
                volume: 1.0,
            });
        }

        // Trigger Buy: Drop
        bars.push(Bar {
            symbol: "BTCUSD".to_string(),
            market: "crypto".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: now + 20 * 60000,
            open: 50000.0,
            high: 50000.0,
            low: 45000.0,
            close: 45000.0,
            volume: 1.0,
        });

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let intents = generate_signals(&series, "BollingerBands", None, 100.0).await?;
        assert!(!intents.is_empty());
        let intent = &intents[0];

        let size: f64 = intent.size_hint.parse().unwrap();
        assert!(size > 0.0, "Size should be greater than 0");
        assert!(size < 1.0, "Size should be fractional");

        Ok(())
    }

    #[tokio::test]
    async fn test_generate_signals_rationale_format() -> Result<()> {
        let mut bars = Vec::new();
        let now = 100000;
        for i in 0..20 {
            bars.push(Bar {
                symbol: "TEST".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: 100.0,
                high: 101.0,
                low: 99.0,
                close: 100.0,
                volume: 1000.0,
            });
        }
        // Trigger signal
        bars.push(Bar {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: now + 20 * 60000,
            open: 100.0,
            high: 110.0,
            low: 100.0,
            close: 110.0,
            volume: 5000.0,
        });

        let series = BarSeries { schema_version: "v0".to_string(), bars };
        let intents = generate_signals(&series, "BollingerBands", None, 100.0).await?;
        let intent = &intents[0];

        // "Strategy: {}. Reason: {}. Market Context: {} ({} Volatility). {}"
        assert!(intent.rationale.contains("Strategy: BollingerBandsMeanReversion"));
        assert!(intent.rationale.contains("Market Context:"));
        assert!(intent.rationale.contains("Volatility"));
        assert!(intent.rationale.contains("No similar past trades found")); // Default history context

        Ok(())
    }
}
