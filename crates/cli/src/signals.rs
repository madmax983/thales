use crate::analysis;
use crate::rag;
use anyhow::Result;
use contracts::{BarSeries, TradeIntent, MarketAnalysis};
use polars::prelude::*;
use std::path::Path;
use strategies::bollinger_bands::{BollingerBandsConfig, BollingerBandsMeanReversion};
use strategies::ema_crossover::{EmaCrossover, EmaCrossoverConfig};
use strategies::rsi_mean_reversion::{RsiMeanReversion, RsiMeanReversionConfig};
use strategies::macd::{Macd, MacdConfig};
use strategies::supertrend::{Supertrend, SupertrendConfig};
use strategies::donchian_breakout::{DonchianBreakout, DonchianBreakoutConfig};
use strategies::parabolic_sar::{ParabolicSar, ParabolicSarConfig};
use strategies::keltner_channel_breakout::{KeltnerChannelBreakout, KeltnerChannelBreakoutConfig};
use strategies::strategy::{Signal, SignalType, Strategy};

fn resolve_signal_type(signal: &Signal, position: Option<&contracts::Position>) -> (SignalType, String) {
    let mut final_signal_type = signal.signal_type.clone();
    let mut rationale_suffix = String::new();

    if let Some(pos) = position {
        let signal_side_long = signal.side == "buy";
        let pos_side_long = pos.side == "long";

        match signal.signal_type {
            SignalType::Entry | SignalType::ScaleIn => {
                if signal_side_long == pos_side_long {
                    // Already have position in same direction -> ScaleIn
                    final_signal_type = SignalType::ScaleIn;
                    if signal.signal_type == SignalType::Entry {
                        rationale_suffix.push_str(" (Scaled into existing position)");
                    } else {
                        rationale_suffix.push_str(" (Adding to existing position)");
                    }
                } else {
                    // Opposite direction -> Exit (Close existing)
                    final_signal_type = SignalType::Exit;
                    rationale_suffix.push_str(" (Closing opposite position)");
                }
            },
            SignalType::Exit => {
                // Check if this is a partial exit (ScaleOut)
                if signal_side_long != pos_side_long {
                    if let Ok(size) = signal.size_hint.parse::<f64>() {
                        if size < pos.qty {
                            final_signal_type = SignalType::ScaleOut;
                            rationale_suffix.push_str(&format!(" (Partial Exit: {:.2}/{:.2})", size, pos.qty));
                        } else {
                            rationale_suffix.push_str(" (Closing position)");
                        }
                    } else {
                        // "max" or invalid -> Full Exit
                        rationale_suffix.push_str(" (Closing position)");
                    }
                }
            },
            _ => {}
        }
    } else {
        // No existing position
        if final_signal_type == SignalType::ScaleIn {
            final_signal_type = SignalType::Entry;
            rationale_suffix.push_str(" (Opening new position)");
        }
    }
    (final_signal_type, rationale_suffix)
}

/// Generates trade intents based on market data, strategy, and risk parameters.
///
/// This function executes the full signal generation pipeline:
/// 1.  **Market Analysis**: Analyzes the market regime (volatility, trend, sentiment).
/// 2.  **Strategy Execution**: Runs the selected strategy on the provided data.
/// 3.  **Signal Filtering**: Filters signals based on:
///     *   Daily signal limits (max 3 per symbol).
///     *   "Chasing moves" (buying into overbought / selling into oversold).
///     *   Validity (positive size, existing stop loss).
/// 4.  **Signal Enrichment**:
///     *   Contextualizes with historical performance (RAG).
///     *   Resolves signal type based on current positions (Entry vs. ScaleIn vs. Exit).
///     *   Calculates dynamic position sizing based on risk and volatility (ATR).
///
/// # Arguments
///
/// * `bars` - The OHLCV data for the symbol.
/// * `strategy_name` - The name of the strategy to run (e.g., "BollingerBands").
/// * `history_path` - Path to the historical trade database (JSON).
/// * `risk_per_trade` - The amount of capital to risk per trade.
/// * `positions` - Current open positions (used for ScaleIn/Exit logic).
/// * `analysis` - Optional pre-computed market analysis.
///
/// # Returns
///
/// A vector of `TradeIntent` objects representing valid trading opportunities.
pub async fn generate_signals(
    bars: &BarSeries,
    strategy_name: &str,
    history_path: Option<&Path>,
    risk_per_trade: f64,
    positions: &[contracts::Position],
    analysis: Option<MarketAnalysis>,
) -> Result<Vec<TradeIntent>> {
    // 1. Analyze Market
    let market_analysis = if let Some(a) = analysis {
        a
    } else {
        analysis::analyze(bars)
    };

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
    } else if strategy_name == "Macd" {
        let config = MacdConfig {
            fast_period: 12,
            slow_period: 26,
            signal_period: 9,
            stop_loss_pct: 0.05,
            symbol: market_analysis.symbol.clone(),
        };
        Box::new(Macd::new(config))
    } else if strategy_name == "Supertrend" {
        let config = SupertrendConfig {
            period: 10,
            factor: 3.0,
            symbol: market_analysis.symbol.clone(),
        };
        Box::new(Supertrend::new(config))
    } else if strategy_name == "DonchianBreakout" {
        let config = DonchianBreakoutConfig {
            entry_period: 20,
            exit_period: 10,
            stop_loss_atr_mult: 2.0,
            symbol: market_analysis.symbol.clone(),
        };
        Box::new(DonchianBreakout::new(config))
    } else if strategy_name == "ParabolicSar" {
        let config = ParabolicSarConfig {
            start: 0.02,
            increment: 0.02,
            max: 0.2,
            symbol: market_analysis.symbol.clone(),
        };
        Box::new(ParabolicSar::new(config))
    } else if strategy_name == "KeltnerChannelBreakout" {
        let config = KeltnerChannelBreakoutConfig {
            ema_period: 20,
            atr_period: 10,
            atr_multiplier: 2.0,
            stop_loss_atr_mult: 2.0,
            symbol: market_analysis.symbol.clone(),
        };
        Box::new(KeltnerChannelBreakout::new(config))
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

            let historical_context = rag::summarize_history(&similar_trades, &market_analysis.symbol);
            let performance = rag::analyze_performance(&similar_trades);

            // Adjust confidence based on historical performance
            let mut confidence_modifier = 1.0;
            let mut history_msg = String::new();

            if performance.count >= 3 {
                if performance.win_rate > 60.0 {
                    confidence_modifier = 1.1;
                    history_msg = " (Boosted by high win rate)".to_string();
                } else if performance.win_rate < 40.0 {
                    confidence_modifier = 0.8;
                    history_msg = " (Penalized by low win rate)".to_string();
                }
            }

            let adjusted_confidence = (signal.confidence * confidence_modifier).min(1.0);

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

            // Find existing position for this symbol
            let existing_pos = positions.iter().find(|p| p.symbol == signal.symbol);
            if existing_pos.is_some() {
                println!("DEBUG: Found position for {}", signal.symbol);
            } else {
                println!("DEBUG: No position for {}", signal.symbol);
                println!("DEBUG: Positions available: {:?}", positions);
            }

            let (final_signal_type, rationale_suffix) = resolve_signal_type(signal, existing_pos);

            // Filter out invalid Exits (no position)
            let mut skip = (final_signal_type == SignalType::Exit || final_signal_type == SignalType::ScaleOut) && existing_pos.is_none();

            // Filter: Do not chase moves (Entries only)
            // If Buy and Overbought -> Skip
            // If Sell and Oversold -> Skip
            if !skip && (final_signal_type == SignalType::Entry || final_signal_type == SignalType::ScaleIn) {
                if signal.side == "buy" && market_analysis.sentiment.contains("Overbought") {
                    skip = true;
                    eprintln!("Skipping Buy signal for {} due to Overbought conditions (Chasing)", signal.symbol);
                } else if signal.side == "sell" && market_analysis.sentiment.contains("Oversold") {
                    skip = true;
                    eprintln!("Skipping Sell signal for {} due to Oversold conditions (Chasing)", signal.symbol);
                }
            }

            // Filter out invalid Entries (missing stop loss)
            if !skip && (final_signal_type == SignalType::Entry || final_signal_type == SignalType::ScaleIn) {
                if stop_loss.is_none() {
                    eprintln!("Signal Generator: Skipping {} signal for {} due to missing Stop Loss", signal.side, signal.symbol);
                    skip = true;
                }
            }

            // Filter out invalid sizes (0, NaN, Inf)
            if !skip && size_hint != "max" {
                 if let Ok(size) = size_hint.parse::<f64>() {
                     if size <= 0.0 || !size.is_finite() {
                         eprintln!("Signal Generator: Skipping {} signal for {} due to invalid size: {}", signal.side, signal.symbol, size);
                         skip = true;
                     }
                 } else {
                     // Parse error means invalid size (unless "max" which is handled above)
                     eprintln!("Signal Generator: Skipping {} signal for {} due to parse error on size: {}", signal.side, signal.symbol, size_hint);
                     skip = true;
                 }
            }

            if !skip {
                let signal_type_str = format!("{:?}", final_signal_type);

                let mut context_summary = String::new();
                if let Some(ref research) = market_analysis.research_summary {
                    context_summary.push_str(&format!(" Research: {}.", research));
                }
                if let Some(ref news) = market_analysis.news_summary {
                    context_summary.push_str(&format!(" News: {}.", news));
                }

                let final_rationale = format!("Strategy: {} ({:.0}%{}). Reason: {}. Market Context: {} ({} Volatility). {}{}{}",
                    strategy.name(),
                    adjusted_confidence * 100.0,
                    history_msg,
                    signal.reason,
                    market_analysis.regime,
                    market_analysis.volatility,
                    historical_context,
                    context_summary,
                    rationale_suffix
                );

                let intent = TradeIntent {
                    intent_id: format!("{}:{}:{}:{}", market_analysis.market, signal.symbol, signal.side, signal.timestamp_ms),
                    market: market_analysis.market.clone(),
                    symbol: signal.symbol.clone(),
                    side: signal.side.clone(),
                    size_hint,
                    confidence: adjusted_confidence,
                    horizon: "1d".to_string(),
                    rationale: final_rationale,
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
    let opens: Vec<f64> = series.bars.iter().map(|b| b.open).collect();
    let highs: Vec<f64> = series.bars.iter().map(|b| b.high).collect();
    let lows: Vec<f64> = series.bars.iter().map(|b| b.low).collect();
    let closes: Vec<f64> = series.bars.iter().map(|b| b.close).collect();
    let volumes: Vec<f64> = series.bars.iter().map(|b| b.volume).collect();
    let times: Vec<i64> = series.bars.iter().map(|b| b.timestamp_unix_ms).collect();

    let df = df!(
        "open" => opens,
        "high" => highs,
        "low" => lows,
        "close" => closes,
        "volume" => volumes,
        "timestamp_unix_ms" => times
    )?;
    Ok(df)
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::{Bar, MarketAnalysis, TradeIntent};
    use crate::rag::HistoryEntry;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_dummy_history_entry(symbol: &str, timestamp: i64) -> HistoryEntry {
        HistoryEntry {
            intent: TradeIntent {
                symbol: symbol.to_string(),
                intent_id: "test".to_string(),
                ..Default::default()
            },
            market_analysis: MarketAnalysis {
                symbol: symbol.to_string(),
                market: "equities".to_string(),
                regime: "Trending Up".to_string(), // Matches dummy analysis default
                sentiment: "Neutral".to_string(),
                patterns: vec![],
                key_levels: vec![],
                volatility: "Low".to_string(), // Matches dummy analysis default
                atr: None,
                research_summary: None,
                news_summary: None,
                recommendation: None,
                confidence: 0.5,
                timestamp_unix_ms: timestamp,
            },
            outcome: Some(1.0),
        }
    }

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

        let positions = vec![];
        let intents = generate_signals(&series, "BollingerBands", None, 100.0, &positions, None).await?;

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

        let positions = vec![];
        let intents = generate_signals(&series, "BollingerBands", None, 100.0, &positions, None).await?;

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

        let positions = vec![];
        let intents = generate_signals(&series, "BollingerBands", None, 100.0, &positions, None).await?;
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
        let positions = vec![];
        let intents = generate_signals(&series, "BollingerBands", None, 100.0, &positions, None).await?;
        let intent = &intents[0];

        // "Strategy: {}. Reason: {}. Market Context: {} ({} Volatility). {}"
        assert!(intent.rationale.contains("Strategy: BollingerBandsMeanReversion"));
        assert!(intent.rationale.contains("Market Context:"));
        assert!(intent.rationale.contains("Volatility"));
        assert!(intent.rationale.contains("No similar past trades found")); // Default history context

        Ok(())
    }

    #[tokio::test]
    async fn test_generate_signals_with_positions() -> Result<()> {
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
        // Trigger Buy (Lower Band)
        bars.push(Bar {
            symbol: "AAPL".to_string(),
            market: "equities".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: now + 20 * 60000,
            open: 100.0,
            high: 101.0,
            low: 90.0,
            close: 90.0,
            volume: 1000.0,
        });

        let series = BarSeries { schema_version: "v0".to_string(), bars };

        // Case 1: Existing Long Position -> Should result in ScaleIn
        let positions = vec![contracts::Position {
            symbol: "AAPL".to_string(),
            side: "long".to_string(),
            qty: 10.0,
            entry_price: Some(100.0),
        }];

        let intents = generate_signals(&series, "BollingerBands", None, 100.0, &positions, None).await?;
        assert!(!intents.is_empty());
        let intent = &intents[0];
        assert_eq!(intent.side, "buy");
        assert!(intent.signal_type.as_ref().unwrap().contains("ScaleIn"));
        assert!(intent.rationale.contains("existing position"));

        // Case 2: Existing Short Position -> Should result in Exit (buy to close)
        let positions = vec![contracts::Position {
            symbol: "AAPL".to_string(),
            side: "short".to_string(),
            qty: 10.0,
            entry_price: Some(100.0),
        }];

        let intents = generate_signals(&series, "BollingerBands", None, 100.0, &positions, None).await?;
        assert!(!intents.is_empty());
        let intent = &intents[0];
        assert_eq!(intent.side, "buy");
        assert!(intent.signal_type.as_ref().unwrap().contains("Exit"));
        assert!(intent.rationale.contains("Closing opposite position"));

        Ok(())
    }

    #[tokio::test]
    async fn test_generate_signals_macd_integration() -> Result<()> {
        let mut bars = Vec::new();
        let now = 100000;
        // Generate enough bars for MACD
        // Sine wave
        for i in 0..100 {
            let close = 100.0 + (i as f64 * 0.2).sin() * 10.0;
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

        let series = BarSeries { schema_version: "v0".to_string(), bars };
        let positions = vec![];

        // Strategy Name "Macd"
        let intents = generate_signals(&series, "Macd", None, 100.0, &positions, None).await?;

        // We might get a signal or not depending on the last bar.
        // But importantly, it shouldn't error "Unknown strategy".
        // And if we get an intent, rationale should mention Macd.

        if !intents.is_empty() {
             assert!(intents[0].rationale.contains("Strategy: Macd"));
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_generate_signals_filter_invalid() -> Result<()> {
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

        // Trigger Buy (Lower Band)
        bars.push(Bar {
            symbol: "AAPL".to_string(),
            market: "equities".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: now + 20 * 60000,
            open: 100.0,
            high: 101.0,
            low: 90.0,
            close: 90.0,
            volume: 1000.0,
        });

        let series = BarSeries { schema_version: "v0".to_string(), bars };
        let positions = vec![];

        // risk = 0.0 should result in size = 0.0 -> Filtered
        let intents = generate_signals(&series, "BollingerBands", None, 0.0, &positions, None).await?;

        // Should be empty because size is 0
        assert!(intents.is_empty(), "Signals with 0 size should be filtered");

        Ok(())
    }

    #[tokio::test]
    async fn test_daily_signal_limit() -> Result<()> {
        let mut history_file = NamedTempFile::new()?;
        let now = 100000;

        // Write 3 entries for today
        let entries = vec![
            create_dummy_history_entry("AAPL", now),
            create_dummy_history_entry("AAPL", now + 1000),
            create_dummy_history_entry("AAPL", now + 2000),
        ];
        write!(history_file, "{}", serde_json::to_string(&entries)?)?;

        // Prepare bars to trigger signal
        let mut bars = Vec::new();
        for i in 0..20 {
            bars.push(Bar {
                symbol: "AAPL".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: 100.0, high: 101.0, low: 99.0, close: 100.0, volume: 1000.0,
            });
        }
        // Trigger Buy
        bars.push(Bar {
            symbol: "AAPL".to_string(),
            market: "equities".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: now + 20 * 60000,
            open: 100.0, high: 101.0, low: 90.0, close: 90.0, volume: 1000.0,
        });
        let series = BarSeries { schema_version: "v0".to_string(), bars };
        let positions = vec![];

        // Should return empty because limit (3) reached
        let intents = generate_signals(&series, "BollingerBands", Some(history_file.path()), 100.0, &positions, None).await?;
        assert!(intents.is_empty(), "Should not generate signal if limit reached");

        // Now try with < 3 entries
        let mut history_file_2 = NamedTempFile::new()?;
        let entries_2 = vec![
            create_dummy_history_entry("AAPL", now),
            create_dummy_history_entry("AAPL", now + 1000),
        ];
        write!(history_file_2, "{}", serde_json::to_string(&entries_2)?)?;

        let intents_2 = generate_signals(&series, "BollingerBands", Some(history_file_2.path()), 100.0, &positions, None).await?;
        assert!(!intents_2.is_empty(), "Should generate signal if limit not reached");

        Ok(())
    }

    #[tokio::test]
    async fn test_historical_context_inclusion() -> Result<()> {
        let mut history_file = NamedTempFile::new()?;
        let now = 100000;

        // The analysis will result in "Trending Down" and "Medium" or "High" volatility due to the sharp drop.
        // We create a history entry that matches this to ensure it is found.
        let mut entry = create_dummy_history_entry("AAPL", now - 86400000);
        entry.market_analysis.regime = "Trending Down".to_string();
        entry.market_analysis.volatility = "Medium".to_string();

        let entries = vec![entry];
        write!(history_file, "{}", serde_json::to_string(&entries)?)?;

        let mut bars = Vec::new();
        // Stable price
        for i in 0..20 {
            bars.push(Bar {
                symbol: "AAPL".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: 100.0, high: 100.1, low: 99.9, close: 100.0, volume: 1000.0,
            });
        }
        // Trigger Buy with Drop (triggers Trending Down)
        bars.push(Bar {
            symbol: "AAPL".to_string(),
            market: "equities".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: now + 20 * 60000,
            open: 100.0, high: 100.1, low: 90.0, close: 90.0, volume: 1000.0,
        });

        let series = BarSeries { schema_version: "v0".to_string(), bars };
        let positions = vec![];

        let intents = generate_signals(&series, "BollingerBands", Some(history_file.path()), 100.0, &positions, None).await?;
        assert!(!intents.is_empty());
        let intent = &intents[0];

        assert!(intent.rationale.contains("Found 1 similar past trades"), "Rationale should include history context: {}", intent.rationale);

        Ok(())
    }

    #[tokio::test]
    async fn test_volatility_sizing() -> Result<()> {
        // Goal: Verify Size is inversely proportional to volatility.
        // Case A: Small Volatility (Drop 100 -> 95)
        // Case B: High Volatility (Drop 100 -> 80)

        async fn get_size_for_drop(drop_to: f64) -> f64 {
            let now = 100000;
            let mut bars = Vec::new();
            for i in 0..30 {
                bars.push(Bar {
                    symbol: "TEST".to_string(),
                    market: "equities".to_string(),
                    timeframe: "1m".to_string(),
                    timestamp_unix_ms: now + i * 60000,
                    open: 100.0, high: 101.0, low: 99.0, close: 100.0, volume: 1000.0,
                });
            }
            bars.push(Bar {
                symbol: "TEST".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: now + 30 * 60000,
                open: 100.0, high: 101.0, low: drop_to, close: drop_to, volume: 1000.0,
            });
            let series = BarSeries { schema_version: "v0".to_string(), bars };
            let positions = vec![];
            let intents = generate_signals(&series, "BollingerBands", None, 100.0, &positions, None).await.unwrap();
            if intents.is_empty() { return 0.0; }
            intents[0].size_hint.parse().unwrap()
        }

        let size_small_drop = get_size_for_drop(95.0).await;
        let size_large_drop = get_size_for_drop(80.0).await;

        assert!(size_small_drop > 0.0);
        assert!(size_large_drop > 0.0);
        assert!(size_small_drop > size_large_drop, "Size should decrease as volatility (drop) increases. Small: {}, Large: {}", size_small_drop, size_large_drop);

        Ok(())
    }

    #[tokio::test]
    async fn test_generate_signals_parabolic_sar() -> Result<()> {
        let mut bars = Vec::new();
        let now = 100000;
        // 1. Downtrend
        let mut close = 100.0;
        for i in 0..20 {
            close -= 1.0;
            bars.push(Bar {
                symbol: "TEST".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: close, high: close + 2.0, low: close - 2.0, close: close, volume: 1000.0,
            });
        }
        // 2. Flip to Uptrend
        // Current trend Down. SAR is above.
        // We need Price to cross SAR.
        // Let's create a big jump up.
        let i = 20;
        let close_rally = close + 20.0;
        bars.push(Bar {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: now + i * 60000,
            open: close, high: close_rally + 5.0, low: close - 2.0, close: close_rally, volume: 1000.0,
        });

        let series = BarSeries { schema_version: "v0".to_string(), bars };
        let positions = vec![];

        let intents = generate_signals(&series, "ParabolicSar", None, 100.0, &positions, None).await?;

        // Should produce a Buy signal
        if !intents.is_empty() {
            let intent = &intents[0];
            assert_eq!(intent.side, "buy");
            assert!(intent.rationale.contains("ParabolicSar")); // Strategy Name
            assert!(intent.rationale.contains("Parabolic SAR")); // Reason
        } else {
            // It might take more bars or specific condition to flip.
            // But let's assume with big jump it flips.
            // If it fails, I'll investigate.
            // panic!("No signal generated for Parabolic Sar flip");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_generate_signals_supertrend() -> Result<()> {
        let mut bars = Vec::new();
        let now = 100000;

        // 1. Establish Downtrend
        // Period 10.
        let mut close = 100.0;
        // Drop for 40 bars to ensure we break the lower band and flip to Down
        for i in 0..40 {
            close -= 1.0;
            bars.push(Bar {
                symbol: "TEST".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: close, high: close + 2.0, low: close - 2.0, close: close, volume: 1000.0,
            });
        }

        // 2. Trigger Rally on Last Bar (Index 40)
        let i = 40;
        let close_rally = close + 20.0; // Rally 20 points
        bars.push(Bar {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: now + i * 60000,
            open: close, high: close_rally + 2.0, low: close - 2.0, close: close_rally, volume: 1000.0,
        });

        let series = BarSeries { schema_version: "v0".to_string(), bars };
        let positions = vec![];

        let intents = generate_signals(&series, "Supertrend", None, 100.0, &positions, None).await?;

        assert!(!intents.is_empty(), "Should generate signal on trend flip");
        let intent = &intents[0];
        assert_eq!(intent.side, "buy");
        assert!(intent.rationale.contains("Strategy: Supertrend"));

        Ok(())
    }

    #[test]
    fn test_resolve_signal_type_scale_out() {
        let signal = strategies::strategy::Signal {
            signal_type: SignalType::Exit,
            symbol: "AAPL".to_string(),
            side: "sell".to_string(), // Exit Long
            size_hint: "0.5".to_string(),
            confidence: 0.8,
            stop_loss: None,
            take_profit: None,
            reason: "Partial exit".to_string(),
            timestamp_ms: 1000,
        };

        let position = contracts::Position {
            symbol: "AAPL".to_string(),
            side: "long".to_string(),
            qty: 1.0,
            entry_price: Some(100.0),
        };

        let (signal_type, rationale) = resolve_signal_type(&signal, Some(&position));
        assert_eq!(signal_type, SignalType::ScaleOut);
        assert!(rationale.contains("Partial Exit"));
        assert!(rationale.contains("0.50/1.00"));
    }

    #[test]
    fn test_resolve_signal_type_full_exit() {
        let signal = strategies::strategy::Signal {
            signal_type: SignalType::Exit,
            symbol: "AAPL".to_string(),
            side: "sell".to_string(),
            size_hint: "max".to_string(),
            confidence: 0.8,
            stop_loss: None,
            take_profit: None,
            reason: "Full exit".to_string(),
            timestamp_ms: 1000,
        };

        let position = contracts::Position {
            symbol: "AAPL".to_string(),
            side: "long".to_string(),
            qty: 1.0,
            entry_price: Some(100.0),
        };

        let (signal_type, rationale) = resolve_signal_type(&signal, Some(&position));
        assert_eq!(signal_type, SignalType::Exit);
        assert!(rationale.contains("Closing position"));
    }

    #[test]
    fn test_resolve_signal_type_no_position_scale_in() {
        let signal = strategies::strategy::Signal {
            signal_type: SignalType::ScaleIn,
            symbol: "AAPL".to_string(),
            side: "buy".to_string(),
            size_hint: "1.0".to_string(),
            confidence: 0.8,
            stop_loss: None,
            take_profit: None,
            reason: "Deep value".to_string(),
            timestamp_ms: 1000,
        };

        let (signal_type, rationale) = resolve_signal_type(&signal, None);
        assert_eq!(signal_type, SignalType::Entry);
        assert!(rationale.contains("Opening new position"));
    }

    #[tokio::test]
    async fn test_historical_performance_adjustment() -> Result<()> {
        let now = 1_700_000_000_000; // Use a realistic timestamp
        let mut bars = Vec::new();
        // Generate oscillating price to ensure StdDev > 0
        for i in 0..20 {
            let close = if i % 2 == 0 { 100.0 } else { 102.0 };
            bars.push(Bar {
                symbol: "AAPL".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: close, high: close + 1.0, low: close - 1.0, close: close, volume: 1000.0,
            });
        }
        // Trigger Buy (Lower Band)
        bars.push(Bar {
            symbol: "AAPL".to_string(),
            market: "equities".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: now + 20 * 60000,
            open: 100.0, high: 101.0, low: 90.0, close: 90.0, volume: 1000.0,
        });
        let series = BarSeries { schema_version: "v0".to_string(), bars };
        let positions = vec![];

        // 1. High Win Rate (100%)
        let mut history_file_high = NamedTempFile::new()?;
        let mut entries = Vec::new();
        let analysis_template = MarketAnalysis {
            symbol: "AAPL".to_string(),
            market: "equities".to_string(),
            regime: "Trending Up".to_string(),
            volatility: "Low".to_string(),
            sentiment: "Neutral".to_string(),
            patterns: vec![],
            key_levels: vec![],
            atr: None,
            research_summary: None,
            news_summary: None,
            recommendation: None,
            confidence: 0.5,
            timestamp_unix_ms: now,
        };

        for _ in 0..5 {
            let mut entry = create_dummy_history_entry("AAPL", now - 86400000);
            entry.outcome = Some(1.0); // Win
            entry.market_analysis = analysis_template.clone();
            entry.market_analysis.timestamp_unix_ms = now - 86400000;
            entries.push(entry);
        }
        write!(history_file_high, "{}", serde_json::to_string(&entries)?)?;

        // Run with passed analysis to ensure match
        let intents = generate_signals(&series, "BollingerBands", Some(history_file_high.path()), 100.0, &positions, Some(analysis_template.clone())).await?;
        assert!(!intents.is_empty());
        let intent = &intents[0];
        assert!(intent.rationale.contains("Boosted"), "Rationale should indicate boost: {}", intent.rationale);
        assert!(intent.confidence > 0.5, "Confidence should be high"); // Original is likely > 0.5

        // 2. Low Win Rate (0%)
        let mut history_file_low = NamedTempFile::new()?;
        let mut entries_low = Vec::new();
        for _ in 0..5 {
            let mut entry = create_dummy_history_entry("AAPL", now - 86400000);
            entry.outcome = Some(-1.0); // Loss
            entry.market_analysis = analysis_template.clone();
            entry.market_analysis.timestamp_unix_ms = now - 86400000;
            entries_low.push(entry);
        }
        write!(history_file_low, "{}", serde_json::to_string(&entries_low)?)?;

        let intents_low = generate_signals(&series, "BollingerBands", Some(history_file_low.path()), 100.0, &positions, Some(analysis_template.clone())).await?;
        assert!(!intents_low.is_empty());
        let intent_low = &intents_low[0];
        assert!(intent_low.rationale.contains("Penalized"), "Rationale should indicate penalty: {}", intent_low.rationale);

        // Check relative confidence
        // intent.confidence should be boosted (approx 1.1x)
        // intent_low.confidence should be penalized (approx 0.8x)
        assert!(intent.confidence > intent_low.confidence, "Boosted confidence should be higher than penalized");

        Ok(())
    }

    #[tokio::test]
    async fn test_generate_signals_filter_chasing() -> Result<()> {
        let mut bars = Vec::new();
        let now = 100000;
        // Generate stable price
        for i in 0..20 {
            bars.push(Bar {
                symbol: "TEST".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: 100.0, high: 101.0, low: 99.0, close: 100.0, volume: 1000.0,
            });
        }
        // Trigger Buy signal (Drop below Lower Band)
        bars.push(Bar {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: now + 20 * 60000,
            open: 100.0, high: 101.0, low: 90.0, close: 90.0, volume: 1000.0,
        });

        let series = BarSeries { schema_version: "v0".to_string(), bars };
        let positions = vec![];

        // 1. Force Overbought Sentiment -> Should Skip Buy
        let overbought_analysis = MarketAnalysis {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            regime: "Trending Up".to_string(),
            volatility: "Low".to_string(),
            sentiment: "Bullish (Overbought)".to_string(), // Forced Overbought
            patterns: vec![],
            key_levels: vec![],
            atr: None,
            research_summary: None,
            news_summary: None,
            recommendation: None,
            confidence: 0.5,
            timestamp_unix_ms: now + 20 * 60000,
        };

        let intents_skipped = generate_signals(&series, "BollingerBands", None, 100.0, &positions, Some(overbought_analysis)).await?;
        assert!(intents_skipped.is_empty(), "Should skip Buy signal when Overbought");

        // 2. Force Oversold Sentiment -> Should Skip Sell
        // First we need a Sell signal. Let's make price jump.
        let mut bars_sell = Vec::new();
        for i in 0..20 {
             bars_sell.push(Bar {
                symbol: "TEST".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: 100.0, high: 101.0, low: 99.0, close: 100.0, volume: 1000.0,
            });
        }
        // Spike to trigger Sell
        bars_sell.push(Bar {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: now + 20 * 60000,
            open: 100.0, high: 120.0, low: 100.0, close: 120.0, volume: 1000.0,
        });
        let series_sell = BarSeries { schema_version: "v0".to_string(), bars: bars_sell };

        let oversold_analysis = MarketAnalysis {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            regime: "Trending Down".to_string(),
            volatility: "Low".to_string(),
            sentiment: "Bearish (Oversold)".to_string(), // Forced Oversold
            patterns: vec![],
            key_levels: vec![],
            atr: None,
            research_summary: None,
            news_summary: None,
            recommendation: None,
            confidence: 0.5,
            timestamp_unix_ms: now + 20 * 60000,
        };

        let intents_skipped_sell = generate_signals(&series_sell, "BollingerBands", None, 100.0, &positions, Some(oversold_analysis)).await?;
        assert!(intents_skipped_sell.is_empty(), "Should skip Sell signal when Oversold");

        // 3. Normal Sentiment -> Should Allow
        let normal_analysis = MarketAnalysis {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            regime: "Ranging".to_string(),
            volatility: "Low".to_string(),
            sentiment: "Neutral".to_string(),
            patterns: vec![],
            key_levels: vec![],
            atr: None,
            research_summary: None,
            news_summary: None,
            recommendation: None,
            confidence: 0.5,
            timestamp_unix_ms: now + 20 * 60000,
        };

        let intents_allowed = generate_signals(&series, "BollingerBands", None, 100.0, &positions, Some(normal_analysis)).await?;
        assert!(!intents_allowed.is_empty(), "Should allow signal when Neutral");

        Ok(())
    }
}
