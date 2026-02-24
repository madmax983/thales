use crate::analysis;
use crate::rag;
use anyhow::Result;
use contracts::{BarSeries, TradeIntent};
use polars::prelude::*;
use std::path::Path;
use strategies::bollinger_bands::{BollingerBandsConfig, BollingerBandsMeanReversion};
use strategies::strategy::{SignalType, Strategy};

const DEFAULT_RISK_PER_TRADE: f64 = 100.0;

pub async fn generate_signals(
    bars: &BarSeries,
    strategy_name: &str,
    history_path: Option<&Path>,
) -> Result<Vec<TradeIntent>> {
    // 1. Analyze Market
    let market_analysis = analysis::analyze(bars);

    // 2. Prepare Data for Strategy
    let df = bars_to_dataframe(bars)?;

    let latest_timestamp = bars.bars.last().map(|b| b.timestamp_unix_ms).unwrap_or(0);

    // 3. Run Strategy
    // For now, hardcode BollingerBandsMeanReversion. In future, use factory.
    let strategy = if strategy_name == "BollingerBands" || strategy_name == "BollingerBandsMeanReversion" {
        let config = BollingerBandsConfig {
            window_size: 20,
            num_std_dev: 2.0,
            stop_loss_pct: 0.05, // Strategy config default, but we'll override or use for initial filtering
            symbol: market_analysis.symbol.clone(),
        };
        Box::new(BollingerBandsMeanReversion::new(config)) as Box<dyn Strategy>
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
        // Limit to 3 signals per day
        if signals_today < 3 {
             // RAG Step: Check history
            let similar_trades = if let Some(path) = history_path {
                rag::find_similar_trades(&market_analysis, path).unwrap_or_default()
            } else {
                Vec::new()
            };

            let historical_context = if !similar_trades.is_empty() {
                 format!("Found {} similar past trades.", similar_trades.len())
            } else {
                "No similar past trades found.".to_string()
            };

            // Position Sizing and SL/TP (ATR based)
            let last_close = bars.bars.last().map(|b| b.close).unwrap_or(100.0);
            let atr = market_analysis.atr.unwrap_or(last_close * 0.01); // Fallback to 1% if ATR missing

            // SL distance = 2 ATR, TP distance = 4 ATR (2:1 Reward/Risk)
            let sl_dist = 2.0 * atr;
            let tp_dist = 4.0 * atr;

            let (stop_loss, take_profit, size_hint) = match signal.signal_type {
                SignalType::Entry | SignalType::ScaleIn => {
                    let (sl, tp) = if signal.side == "buy" {
                        (
                            Some(last_close - sl_dist),
                            Some(last_close + tp_dist),
                        )
                    } else {
                        (
                            Some(last_close + sl_dist),
                            Some(last_close - tp_dist),
                        )
                    };

                    let size = if sl_dist > 0.0 {
                        (DEFAULT_RISK_PER_TRADE / sl_dist).round().to_string()
                    } else {
                        "0".to_string()
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
                rationale: format!("Strategy: {}. Reason: {}. Market: {}. {}", strategy.name(), signal.reason, market_analysis.regime, historical_context),
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

        let intents = generate_signals(&series, "BollingerBands", None).await?;

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
        // Create bars with predictable range for ATR calculation
        // High - Low = 2.0 (High = Close + 1, Low = Close - 1)
        // Previous Close = Close (so TR is High - Low = 2.0)
        let mut bars = Vec::new();
        let now = 100000;

        // Fill history with constant price 100.0, range 2.0
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

        // Trigger Buy signal: Drop below lower band
        // Mean is 100. Std Dev is 0 (approx). Lower Band is 100.
        // Drop to 95.
        bars.push(Bar {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: now + 20 * 60000,
            open: 100.0,
            high: 101.0,
            low: 94.0, // Low drop
            close: 95.0, // Close below 100
            volume: 1000.0,
        });

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let intents = generate_signals(&series, "BollingerBands", None).await?;

        assert!(!intents.is_empty());
        let intent = &intents[0];
        assert_eq!(intent.side, "buy");

        let close = 95.0;
        let sl = intent.stop_loss.unwrap();
        let tp = intent.take_profit.unwrap();

        assert!(sl < close);
        assert!(tp > close);

        // Check relationships
        let sl_dist = close - sl;
        let tp_dist = tp - close;

        // TP should be roughly 2x SL distance (4 ATR vs 2 ATR)
        assert!((tp_dist - 2.0 * sl_dist).abs() < 0.1);

        // Check size
        let size: f64 = intent.size_hint.parse().unwrap();
        // Size = 100 / sl_dist
        let expected_size = (100.0 / sl_dist).round();
        assert_eq!(size, expected_size);

        // Check signal type
        // With constant price (std_dev = 0), any drop is considered "Deep Value" / ScaleIn by the strategy
        assert_eq!(intent.signal_type, Some("ScaleIn".to_string()));

        Ok(())
    }

    #[test]
    fn test_signal_priority() {
        assert!(signal_priority(&SignalType::Entry) < signal_priority(&SignalType::ScaleIn));
        assert!(signal_priority(&SignalType::ScaleIn) < signal_priority(&SignalType::Exit));
        assert!(signal_priority(&SignalType::Exit) < signal_priority(&SignalType::ScaleOut));
    }
}
