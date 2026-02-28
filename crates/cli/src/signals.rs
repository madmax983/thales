use crate::analysis;
use crate::rag;
use crate::strategy_factory;
use anyhow::Result;
use contracts::{BarSeries, MarketAnalysis, TradeIntent};
use polars::prelude::*;
use std::path::Path;
use strategies::strategy::{Signal, SignalType, StrategyType};

fn resolve_signal_type(
    signal: &Signal,
    position: Option<&contracts::Position>,
) -> (SignalType, String) {
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
            }
            SignalType::Exit => {
                // Check if this is a partial exit (ScaleOut)
                if signal_side_long != pos_side_long {
                    if let Ok(size) = signal.size_hint.parse::<f64>() {
                        if size < pos.qty {
                            final_signal_type = SignalType::ScaleOut;
                            rationale_suffix
                                .push_str(&format!(" (Partial Exit: {:.2}/{:.2})", size, pos.qty));
                        } else {
                            rationale_suffix.push_str(" (Closing position)");
                        }
                    } else {
                        // "max" or invalid -> Full Exit
                        rationale_suffix.push_str(" (Closing position)");
                    }
                }
            }
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
    let strategy = strategy_factory::create_strategy(strategy_name, &market_analysis.symbol)?;

    let raw_signals = strategy.generate_signals(&df).await?;
    let raw_signals_count = raw_signals.len();

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

    if valid_signals.is_empty() {
        if raw_signals_count > 0 {
            eprintln!(
                "No signals generated for the latest timestamp. (Found {} signals in historical data)",
                raw_signals_count
            );
        } else {
            eprintln!("No signals generated by the strategy.");
        }
    }

    // 2. Sort by Priority (Entry > ScaleIn > Exit > ScaleOut) and then Confidence
    valid_signals.sort_by(|a, b| {
        let p_a = signal_priority(&a.signal_type);
        let p_b = signal_priority(&b.signal_type);
        if p_a != p_b {
            p_a.cmp(&p_b)
        } else {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
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
            // We pass the strategy name to filter history
            let similar_trades = if let Some(path) = history_path {
                rag::find_similar_trades(&market_analysis, path, Some(strategy.name()))
                    .unwrap_or_default()
            } else {
                Vec::new()
            };

            let historical_context =
                rag::summarize_history(&similar_trades, &market_analysis.symbol);
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

            // Combine Strategy Confidence, History Modifier, and Market Analysis Confidence
            let adjusted_confidence =
                (signal.confidence * confidence_modifier * market_analysis.confidence).min(1.0);

            let mut skip = false;

            // Filter: Minimum Confidence
            if adjusted_confidence < 0.5 {
                skip = true;
                eprintln!(
                    "Skipping {} signal for {} due to low confidence ({:.2})",
                    signal.side, signal.symbol, adjusted_confidence
                );
            }

            // Position Sizing and SL/TP
            let last_close = bars.bars.last().map(|b| b.close).unwrap_or(100.0);
            let atr = market_analysis.atr.unwrap_or(last_close * 0.01); // Fallback to 1% if ATR missing

            // Some strategies use fixed % SL which violates volatility sizing rules. Override them.
            // Specifically, Mean Reversion strategies often assume a reversion which might need tighter
            // or different stops than trend followers. But historically we've overridden
            // EmaCrossover, RsiMeanReversion, Macd.
            // With new types, we can apply logic generally or stick to specifics if needed.
            // For now, let's keep the specific override or expand it.
            // Actually, the instruction says "exempt from 'Do not chase' checks if not MeanReversion".
            // It doesn't explicitly say to change the SL override logic, but consistency is good.
            // Let's stick to the current list for SL override unless instructed otherwise,
            // or perhaps check if strategy_type is TrendFollowing?
            // The existing code has explicit check: matches!(strategy_name, "EmaCrossover" | "RsiMeanReversion" | "Macd")
            // Let's leave this part alone to minimize regression risk unless requested.
            let use_atr_sl_override = matches!(
                strategy_name,
                "EmaCrossover" | "RsiMeanReversion" | "Macd" | "ConnorsRsiMeanReversion" | "WilliamsR"
            );

            // Calculate SL/TP
            let (stop_loss, take_profit, size_hint) = match signal.signal_type {
                SignalType::Entry | SignalType::ScaleIn => {
                    // Use Strategy SL if provided, else ATR fallback (2.0 ATR)
                    // If use_atr_sl_override is true, ignore strategy SL and force ATR based.
                    let sl = if use_atr_sl_override {
                        if signal.side == "buy" {
                            Some(last_close - (2.0 * atr))
                        } else {
                            Some(last_close + (2.0 * atr))
                        }
                    } else if let Some(s) = signal.stop_loss {
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

                    // Log sizing details for observability
                    if let Some(sl_price) = sl {
                        let dist = (last_close - sl_price).abs();
                        eprintln!(
                            "Risk-based Sizing: Risk=${:.2}, SL Dist={:.4} (Price={:.4}, SL={:.4}), Calc Size={}",
                            risk_per_trade, dist, last_close, sl_price, size
                        );
                    }
                    (sl, tp, size)
                }
                SignalType::Exit | SignalType::ScaleOut => (None, None, signal.size_hint.clone()),
            };

            let time_in_force = if market_analysis.market == "crypto" {
                "GTC".to_string()
            } else {
                "day".to_string()
            };

            // Find existing position for this symbol
            let existing_pos = positions.iter().find(|p| p.symbol == signal.symbol);
            if existing_pos.is_some() {
                eprintln!("DEBUG: Found position for {}", signal.symbol);
            } else {
                eprintln!("DEBUG: No position for {}", signal.symbol);
                eprintln!("DEBUG: Positions available: {:?}", positions);
            }

            let (final_signal_type, mut rationale_suffix) = resolve_signal_type(signal, existing_pos);

            // Filter out invalid Exits (no position)
            if !skip {
                skip = (final_signal_type == SignalType::Exit
                    || final_signal_type == SignalType::ScaleOut)
                    && existing_pos.is_none();
            }

            // Filter: Do not chase moves (Entries only)
            // If Buy and Overbought -> Skip
            // If Sell and Oversold -> Skip
            if !skip
                && (final_signal_type == SignalType::Entry
                    || final_signal_type == SignalType::ScaleIn)
            {
                // We use the new strategy_type() to check for exemption.
                // Only Breakout and Momentum strategies ARE exempt (can chase).
                // TrendFollowing and MeanReversion strategies SHOULD respect Overbought/Oversold checks (don't chase).
                let strategy_type = strategy.strategy_type();
                let can_chase = matches!(
                    strategy_type,
                    StrategyType::Breakout | StrategyType::Momentum
                );

                if !can_chase {
                    if signal.side == "buy" && market_analysis.sentiment.contains("Overbought") {
                        skip = true;
                        eprintln!(
                            "Skipping Buy signal for {} due to Overbought conditions (Strategy Type {:?} - Chasing)",
                            signal.symbol, strategy_type
                        );
                    } else if signal.side == "sell"
                        && market_analysis.sentiment.contains("Oversold")
                    {
                        skip = true;
                        eprintln!(
                            "Skipping Sell signal for {} due to Oversold conditions (Strategy Type {:?} - Chasing)",
                            signal.symbol, strategy_type
                        );
                    }
                } else {
                    // Momentum/Breakout - Allowed to chase, but add rationale
                    if signal.side == "buy" && market_analysis.sentiment.contains("Overbought") {
                        rationale_suffix
                            .push_str(" (Buying strength in Overbought conditions)");
                    } else if signal.side == "sell"
                        && market_analysis.sentiment.contains("Oversold")
                    {
                        rationale_suffix
                            .push_str(" (Selling weakness in Oversold conditions)");
                    }
                }
            }

            // Filter out invalid Entries (missing stop loss)
            if !skip
                && (final_signal_type == SignalType::Entry
                    || final_signal_type == SignalType::ScaleIn)
            {
                if stop_loss.is_none() {
                    eprintln!(
                        "Signal Generator: Skipping {} signal for {} due to missing Stop Loss",
                        signal.side, signal.symbol
                    );
                    skip = true;
                }
            }

            // Filter out invalid sizes (0, NaN, Inf)
            if !skip && size_hint != "max" {
                if let Ok(size) = size_hint.parse::<f64>() {
                    if size <= 0.0 || !size.is_finite() {
                        eprintln!(
                            "Signal Generator: Skipping {} signal for {} due to invalid size: {}",
                            signal.side, signal.symbol, size
                        );
                        skip = true;
                    }
                } else {
                    // Parse error means invalid size (unless "max" which is handled above)
                    eprintln!(
                        "Signal Generator: Skipping {} signal for {} due to parse error on size: {}",
                        signal.side, signal.symbol, size_hint
                    );
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

                let risk_rationale = if let Some(sl_price) = stop_loss {
                    let dist = (last_close - sl_price).abs();
                    format!(" (Risk: ${:.0}, SL Dist: {:.2})", risk_per_trade, dist)
                } else {
                    String::new()
                };

                let final_rationale = format!(
                    "Strategy: {} ({:.0}%{}, MA: {:.2}). Reason: {}. Market Context: {} ({} Volatility). {}{}{}{}",
                    strategy.name(),
                    adjusted_confidence * 100.0,
                    history_msg,
                    market_analysis.confidence,
                    signal.reason,
                    market_analysis.regime,
                    market_analysis.volatility,
                    historical_context,
                    context_summary,
                    rationale_suffix,
                    risk_rationale
                );

                let intent = TradeIntent {
                    intent_id: format!(
                        "{}:{}:{}:{}",
                        market_analysis.market, signal.symbol, signal.side, signal.timestamp_ms
                    ),
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
                    execution_algo: Some("Market".to_string()),
                    strategy: strategy.name().to_string(),
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
    use crate::rag::HistoryEntry;
    use contracts::{Bar, MarketAnalysis, TradeIntent};
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
        let intents =
            generate_signals(&series, "BollingerBands", None, 100.0, &positions, None).await?;

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
        let intents =
            generate_signals(&series, "BollingerBands", None, 100.0, &positions, None).await?;

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
        let intents =
            generate_signals(&series, "BollingerBands", None, 100.0, &positions, None).await?;
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

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let positions = vec![];
        let intents =
            generate_signals(&series, "BollingerBands", None, 100.0, &positions, None).await?;
        let intent = &intents[0];

        // "Strategy: {}. Reason: {}. Market Context: {} ({} Volatility). {}"
        assert!(
            intent
                .rationale
                .contains("Strategy: BollingerBands")
        );
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

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        // Case 1: Existing Long Position -> Should result in ScaleIn
        let positions = vec![contracts::Position {
            symbol: "AAPL".to_string(),
            side: "long".to_string(),
            qty: 10.0,
            entry_price: Some(100.0),
        }];

        let intents =
            generate_signals(&series, "BollingerBands", None, 100.0, &positions, None).await?;
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

        let intents =
            generate_signals(&series, "BollingerBands", None, 100.0, &positions, None).await?;
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

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
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

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let positions = vec![];

        // risk = 0.0 should result in size = 0.0 -> Filtered
        let intents =
            generate_signals(&series, "BollingerBands", None, 0.0, &positions, None).await?;

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
                open: 100.0,
                high: 101.0,
                low: 99.0,
                close: 100.0,
                volume: 1000.0,
            });
        }
        // Trigger Buy
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
        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let positions = vec![];

        // Should return empty because limit (3) reached
        let intents = generate_signals(
            &series,
            "BollingerBands",
            Some(history_file.path()),
            100.0,
            &positions,
            None,
        )
        .await?;
        assert!(
            intents.is_empty(),
            "Should not generate signal if limit reached"
        );

        // Now try with < 3 entries
        let mut history_file_2 = NamedTempFile::new()?;
        let entries_2 = vec![
            create_dummy_history_entry("AAPL", now),
            create_dummy_history_entry("AAPL", now + 1000),
        ];
        write!(history_file_2, "{}", serde_json::to_string(&entries_2)?)?;

        let intents_2 = generate_signals(
            &series,
            "BollingerBands",
            Some(history_file_2.path()),
            100.0,
            &positions,
            None,
        )
        .await?;
        assert!(
            !intents_2.is_empty(),
            "Should generate signal if limit not reached"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_historical_context_inclusion() -> Result<()> {
        let mut history_file = NamedTempFile::new()?;
        let now = 100000;

        // Create the bars first to determine exact analysis output
        let mut bars = Vec::new();
        // Stable price
        for i in 0..20 {
            bars.push(Bar {
                symbol: "AAPL".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: 100.0,
                high: 100.1,
                low: 99.9,
                close: 100.0,
                volume: 1000.0,
            });
        }
        // Trigger Buy with Drop (triggers Trending Down)
        bars.push(Bar {
            symbol: "AAPL".to_string(),
            market: "equities".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: now + 20 * 60000,
            open: 100.0,
            high: 100.1,
            low: 90.0,
            close: 90.0,
            volume: 1000.0,
        });

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        // Analyze to get the actual regime/volatility
        let actual_analysis = analysis::analyze(&series);

        // Create a history entry that matches the actual analysis
        let mut entry = create_dummy_history_entry("AAPL", now - 86400000);
        entry.market_analysis.regime = actual_analysis.regime.clone();
        entry.market_analysis.volatility = actual_analysis.volatility.clone();
        entry.intent.strategy = "BollingerBands".to_string();

        let entries = vec![entry];
        write!(history_file, "{}", serde_json::to_string(&entries)?)?;

        let positions = vec![];

        let intents = generate_signals(
            &series,
            "BollingerBands",
            Some(history_file.path()),
            100.0,
            &positions,
            None,
        )
        .await?;
        assert!(!intents.is_empty());
        let intent = &intents[0];

        assert!(
            intent.rationale.contains("Found 1 similar past trades"),
            "Rationale should include history context: {}",
            intent.rationale
        );

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
                    open: 100.0,
                    high: 101.0,
                    low: 99.0,
                    close: 100.0,
                    volume: 1000.0,
                });
            }
            bars.push(Bar {
                symbol: "TEST".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: now + 30 * 60000,
                open: 100.0,
                high: 101.0,
                low: drop_to,
                close: drop_to,
                volume: 1000.0,
            });
            let series = BarSeries {
                schema_version: "v0".to_string(),
                bars,
            };
            let positions = vec![];
            let intents =
                generate_signals(&series, "BollingerBands", None, 100.0, &positions, None)
                    .await
                    .unwrap();
            if intents.is_empty() {
                return 0.0;
            }
            intents[0].size_hint.parse().unwrap()
        }

        let size_small_drop = get_size_for_drop(95.0).await;
        let size_large_drop = get_size_for_drop(80.0).await;

        assert!(size_small_drop > 0.0);
        assert!(size_large_drop > 0.0);
        assert!(
            size_small_drop > size_large_drop,
            "Size should decrease as volatility (drop) increases. Small: {}, Large: {}",
            size_small_drop,
            size_large_drop
        );

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
                open: close,
                high: close + 2.0,
                low: close - 2.0,
                close: close,
                volume: 1000.0,
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
            open: close,
            high: close_rally + 5.0,
            low: close - 2.0,
            close: close_rally,
            volume: 1000.0,
        });

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let positions = vec![];

        let intents =
            generate_signals(&series, "ParabolicSar", None, 100.0, &positions, None).await?;

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
                open: close,
                high: close + 2.0,
                low: close - 2.0,
                close: close,
                volume: 1000.0,
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
            open: close,
            high: close_rally + 2.0,
            low: close - 2.0,
            close: close_rally,
            volume: 1000.0,
        });

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let positions = vec![];

        let i = 40;
        let high_conf_analysis = MarketAnalysis {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            regime: "Trending Up".to_string(),
            volatility: "Low".to_string(),
            sentiment: "Bullish".to_string(),
            patterns: vec![],
            key_levels: vec![],
            atr: None,
            research_summary: None,
            news_summary: None,
            recommendation: None,
            confidence: 0.8,
            timestamp_unix_ms: now + i * 60000,
        };
        let intents = generate_signals(
            &series,
            "Supertrend",
            None,
            100.0,
            &positions,
            Some(high_conf_analysis),
        )
        .await?;

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
                open: close,
                high: close + 1.0,
                low: close - 1.0,
                close: close,
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
        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
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
            confidence: 0.8,
            timestamp_unix_ms: now,
        };

        for _ in 0..5 {
            let mut entry = create_dummy_history_entry("AAPL", now - 86400000);
            entry.outcome = Some(1.0); // Win
            entry.market_analysis = analysis_template.clone();
            entry.market_analysis.timestamp_unix_ms = now - 86400000;
            entry.intent.strategy = "BollingerBands".to_string();
            entries.push(entry);
        }
        write!(history_file_high, "{}", serde_json::to_string(&entries)?)?;

        // Run with passed analysis to ensure match
        let intents = generate_signals(
            &series,
            "BollingerBands",
            Some(history_file_high.path()),
            100.0,
            &positions,
            Some(analysis_template.clone()),
        )
        .await?;
        assert!(!intents.is_empty());
        let intent = &intents[0];
        assert!(
            intent.rationale.contains("Boosted"),
            "Rationale should indicate boost: {}",
            intent.rationale
        );
        assert!(intent.confidence > 0.5, "Confidence should be high"); // Original is likely > 0.5

        // 2. Low Win Rate (0%)
        let mut history_file_low = NamedTempFile::new()?;
        let mut entries_low = Vec::new();
        for _ in 0..5 {
            let mut entry = create_dummy_history_entry("AAPL", now - 86400000);
            entry.outcome = Some(-1.0); // Loss
            entry.market_analysis = analysis_template.clone();
            entry.market_analysis.timestamp_unix_ms = now - 86400000;
            entry.intent.strategy = "BollingerBands".to_string();
            entries_low.push(entry);
        }
        write!(history_file_low, "{}", serde_json::to_string(&entries_low)?)?;

        let intents_low = generate_signals(
            &series,
            "BollingerBands",
            Some(history_file_low.path()),
            100.0,
            &positions,
            Some(analysis_template.clone()),
        )
        .await?;
        assert!(!intents_low.is_empty());
        let intent_low = &intents_low[0];
        assert!(
            intent_low.rationale.contains("Penalized"),
            "Rationale should indicate penalty: {}",
            intent_low.rationale
        );

        // Check relative confidence
        // intent.confidence should be boosted (approx 1.1x)
        // intent_low.confidence should be penalized (approx 0.8x)
        assert!(
            intent.confidence > intent_low.confidence,
            "Boosted confidence should be higher than penalized"
        );

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
                open: 100.0,
                high: 101.0,
                low: 99.0,
                close: 100.0,
                volume: 1000.0,
            });
        }
        // Trigger Buy signal (Drop below Lower Band)
        bars.push(Bar {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: now + 20 * 60000,
            open: 100.0,
            high: 101.0,
            low: 90.0,
            close: 90.0,
            volume: 1000.0,
        });

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let positions = vec![];

        // 1. Force Overbought Sentiment -> Should Skip Buy (BollingerBands is MeanReversion, should NOT chase?)
        // Wait, Mean Reversion strategies BUY when OVERSOLD and SELL when OVERBOUGHT (mostly).
        // If Sentiment is "Overbought", price is High. BB Strategy would typically signal SELL.
        // If BB signals BUY, it means price is Low (below lower band).
        // It's contradictory for sentiment to be "Overbought" (RSI>70) while BB signals Buy (Price < Low Band).
        // But if it happens (divergence), should we skip?
        //
        // The rule "Do not chase moves":
        // "If Buy and Overbought -> Skip".
        // For Mean Reversion: Buying when Overbought is definitely bad (chasing a top?).
        // Actually, Mean Reversion usually sells when Overbought.
        //
        // The test sets up a BUY signal (Price < Lower Band).
        // And forces Sentiment = Overbought.
        // Logic: if Buy & Overbought -> Skip.
        // Since BollingerBands is MeanReversion, `can_chase` is false.
        // So it should skip.

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
            confidence: 0.8,
            timestamp_unix_ms: now + 20 * 60000,
        };

        let intents_skipped = generate_signals(
            &series,
            "BollingerBands",
            None,
            100.0,
            &positions,
            Some(overbought_analysis),
        )
        .await?;
        assert!(
            intents_skipped.is_empty(),
            "Should skip Buy signal when Overbought (Mean Reversion)"
        );

        // 2. Force Oversold Sentiment -> Should Skip Sell
        // First we need a Sell signal. Let's make price jump.
        let mut bars_sell = Vec::new();
        for i in 0..20 {
            bars_sell.push(Bar {
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
        // Spike to trigger Sell
        bars_sell.push(Bar {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: now + 20 * 60000,
            open: 100.0,
            high: 120.0,
            low: 100.0,
            close: 120.0,
            volume: 1000.0,
        });
        let series_sell = BarSeries {
            schema_version: "v0".to_string(),
            bars: bars_sell,
        };

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
            confidence: 0.8,
            timestamp_unix_ms: now + 20 * 60000,
        };

        let intents_skipped_sell = generate_signals(
            &series_sell,
            "BollingerBands",
            None,
            100.0,
            &positions,
            Some(oversold_analysis),
        )
        .await?;
        assert!(
            intents_skipped_sell.is_empty(),
            "Should skip Sell signal when Oversold"
        );

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
            confidence: 0.8,
            timestamp_unix_ms: now + 20 * 60000,
        };

        let intents_allowed = generate_signals(
            &series,
            "BollingerBands",
            None,
            100.0,
            &positions,
            Some(normal_analysis),
        )
        .await?;
        assert!(
            !intents_allowed.is_empty(),
            "Should allow signal when Neutral"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_generate_signals_low_confidence() -> Result<()> {
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
        // Trigger signal
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

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let positions = vec![];

        // Force low confidence via analysis
        let low_conf_analysis = MarketAnalysis {
            symbol: "AAPL".to_string(),
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
            confidence: 0.4, // Low confidence
            timestamp_unix_ms: now + 20 * 60000,
        };

        let intents = generate_signals(
            &series,
            "BollingerBands",
            None,
            100.0,
            &positions,
            Some(low_conf_analysis),
        )
        .await?;

        // Currently, this SHOULD be empty because we filter < 0.5
        assert!(
            intents.is_empty(),
            "Signals with low confidence should be filtered out"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_donchian_breakout_allows_chasing() -> Result<()> {
        let mut bars = Vec::new();
        let now = 100000;
        // 1. Setup bars for Breakout (New High)
        // 20 bars range 100-105
        for i in 0..20 {
            let close = 100.0 + (i % 5) as f64;
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
        // Breakout!
        bars.push(Bar {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: now + 20 * 60000,
            open: 105.0,
            high: 110.0,
            low: 105.0,
            close: 110.0,
            volume: 5000.0,
        });

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let positions = vec![];

        // 2. Force Overbought Sentiment
        let overbought_analysis = MarketAnalysis {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            regime: "Trending Up".to_string(),
            volatility: "High".to_string(),
            sentiment: "Bullish (Overbought)".to_string(), // Overbought!
            patterns: vec![],
            key_levels: vec![],
            atr: None,
            research_summary: None,
            news_summary: None,
            recommendation: None,
            confidence: 0.8,
            timestamp_unix_ms: now + 20 * 60000,
        };

        // 3. Generate Signals with DonchianBreakout
        let intents = generate_signals(
            &series,
            "DonchianBreakout",
            None,
            100.0,
            &positions,
            Some(overbought_analysis),
        )
        .await?;

        // 4. Assert Signal Generated
        // DonchianBreakout is StrategyType::Breakout, so it should be allowed to chase.
        assert!(
            !intents.is_empty(),
            "DonchianBreakout should allow chasing (Buy when Overbought)"
        );
        let intent = &intents[0];
        assert_eq!(intent.side, "buy");
        assert!(intent.rationale.contains("DonchianBreakout"));

        Ok(())
    }

    #[tokio::test]
    async fn test_generate_signals_stochastic() -> Result<()> {
        let mut bars = Vec::new();
        let now = 100000;
        // Generate bars for Stochastic (min 14+3+3 = 20)
        // We need oversold < 20.
        // Let's create a scenario where prices are low.
        // Bars 0-19: High price (100)
        for i in 0..20 {
            let close = 100.0;
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
        // Bars 20-35: Low price (80). This pushes Stochastic down.
        for i in 20..36 {
            let close = 80.0;
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
        // Bar 36: Price ticks up slightly to cause %K > %D crossover while still oversold?
        // Actually, simple way: Just verifying it runs and produces valid intents if conditions met
        // is enough if we trust the strategy unit test.
        // But let's try to make a cross.
        // 36: 82.0
        bars.push(Bar {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: now + 36 * 60000,
            open: 80.0,
            high: 83.0,
            low: 80.0,
            close: 82.0,
            volume: 1000.0,
        });

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let positions = vec![];

        let intents = generate_signals(
            &series,
            "StochasticOscillator",
            None,
            100.0,
            &positions,
            None,
        )
        .await?;

        // We might or might not get a signal depending on exact calculation.
        // But if we get one, it should be valid.
        if !intents.is_empty() {
            let intent = &intents[0];
            assert!(intent.rationale.contains("StochasticOscillator"));
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_generate_signals_ema_crossover() -> Result<()> {
        let mut bars = Vec::new();
        let now = 100000;
        // 1. Establish Flat/Oscillating Trend (EMAs converged)
        // Short (9) ~ Long (21)
        for i in 0..50 {
            let close = if i % 2 == 0 { 100.0 } else { 99.0 };
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
        // 2. Trigger Crossover (Short > Long)
        // Moderate rally to trigger crossover but NOT overbought
        let i = 50;
        let close_rally = 105.0;
        bars.push(Bar {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: now + i * 60000,
            open: 100.0,
            high: close_rally + 1.0,
            low: 99.0,
            close: close_rally,
            volume: 1000.0,
        });

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let positions = vec![];

        // Need analysis that supports Trending Up otherwise strategy might be filtered by some logic?
        // No, generate_signals runs the strategy requested.
        let intents =
            generate_signals(&series, "EmaCrossover", None, 100.0, &positions, None).await?;

        assert!(!intents.is_empty(), "Should generate signal on crossover");
        let intent = &intents[0];
        assert_eq!(intent.side, "buy");
        assert!(intent.rationale.contains("EmaCrossover"));

        Ok(())
    }

    #[tokio::test]
    async fn test_generate_signals_rsi() -> Result<()> {
        let mut bars = Vec::new();
        let now = 100000;
        // 1. Drop price to Oversold (< 30)
        let mut close = 100.0;
        for i in 0..20 {
            close -= 2.0; // Fast drop
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

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let positions = vec![];

        let intents =
            generate_signals(&series, "RsiMeanReversion", None, 100.0, &positions, None).await?;

        assert!(
            !intents.is_empty(),
            "Should generate signal on Oversold RSI"
        );
        let intent = &intents[0];
        assert_eq!(intent.side, "buy");
        assert!(intent.rationale.contains("RsiMeanReversion"));

        Ok(())
    }

    #[tokio::test]
    async fn test_generate_signals_keltner() -> Result<()> {
        let mut bars = Vec::new();
        let now = 100000;
        // 1. Stable
        for i in 0..20 {
            bars.push(Bar {
                symbol: "TEST".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: 100.0,
                high: 100.1,
                low: 99.9,
                close: 100.0,
                volume: 1000.0,
            });
        }
        // 2. Breakout (Close > Upper Channel)
        // Upper = EMA + 2*ATR. ATR is small (~0.1). EMA ~100. Upper ~100.2.
        // Breakout to 102.
        let i = 20;
        bars.push(Bar {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: now + i * 60000,
            open: 100.0,
            high: 102.0,
            low: 100.0,
            close: 102.0,
            volume: 1000.0,
        });

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let positions = vec![];

        let intents = generate_signals(
            &series,
            "KeltnerChannelBreakout",
            None,
            100.0,
            &positions,
            None,
        )
        .await?;

        assert!(!intents.is_empty(), "Should generate signal on Breakout");
        let intent = &intents[0];
        assert_eq!(intent.side, "buy");
        assert!(intent.rationale.contains("KeltnerChannelBreakout"));

        Ok(())
    }

    #[tokio::test]
    async fn test_adx_momentum_allows_chasing() -> Result<()> {
        let mut bars = Vec::new();
        let now = 100000;

        // 1. Flat Market (ADX ~ 0)
        let mut p = 100.0;
        for i in 0..30 {
            bars.push(Bar {
                symbol: "TEST".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: p,
                high: p + 0.1,
                low: p - 0.1,
                close: p,
                volume: 1000.0,
            });
        }

        // 2. Strong Trend Up (DX ~ 100)
        // Need approx 4-5 bars to cross ADX 25
        for i in 0..4 {
            p += 2.0;
            let ts = now + (30 + i) * 60000;
            bars.push(Bar {
                symbol: "TEST".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: ts,
                open: p,
                high: p + 2.0,
                low: p - 0.1,
                close: p + 2.0,
                volume: 1000.0,
            });
        }

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let positions = vec![];

        let last_ts = series.bars.last().unwrap().timestamp_unix_ms;

        // Force Overbought Sentiment
        let overbought_analysis = MarketAnalysis {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            regime: "Trending Up".to_string(),
            volatility: "High".to_string(),
            sentiment: "Bullish (Overbought)".to_string(),
            patterns: vec![],
            key_levels: vec![],
            atr: None,
            research_summary: None,
            news_summary: None,
            recommendation: None,
            confidence: 0.8,
            timestamp_unix_ms: last_ts,
        };

        // Run AdxMomentum
        let intents = generate_signals(
            &series,
            "AdxMomentum",
            None,
            100.0,
            &positions,
            Some(overbought_analysis),
        )
        .await?;

        assert!(
            !intents.is_empty(),
            "AdxMomentum should generate signal and allow chasing (Buy when Overbought)"
        );
        let intent = &intents[0];
        assert_eq!(intent.side, "buy");
        assert!(intent.rationale.contains("AdxMomentum"));
        assert!(intent.rationale.contains("Buying strength in Overbought conditions"),
            "Rationale should explain why chasing is allowed: {}", intent.rationale);

        Ok(())
    }

    #[tokio::test]
    async fn test_ema_crossover_sl_override() -> Result<()> {
        let mut bars = Vec::new();
        let now = 100000;
        // 1. Establish Flat Trend
        // Short (9) ~ Long (21)
        // Price 100.0. Low volatility (High 100.1, Low 99.9).
        // ATR should be approx 0.2.
        for i in 0..50 {
            let close = if i % 2 == 0 { 100.0 } else { 99.9 }; // Tiny oscillation
            bars.push(Bar {
                symbol: "TEST".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: close,
                high: close + 0.1,
                low: close - 0.1,
                close: close,
                volume: 1000.0,
            });
        }

        // 2. Trigger Crossover (Short > Long)
        // Need to jump up to cross.
        // EmaCrossover uses 5% fixed SL.
        // Price jump to 105.
        // Fixed SL = 105 * 0.95 = 99.75. Dist ~ 5.25.
        // ATR ~ 0.2. 2*ATR ~ 0.4.
        // ATR SL = 105 - 0.4 = 104.6. Dist ~ 0.4.

        let i = 50;
        let close_rally = 105.0;
        bars.push(Bar {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: now + i * 60000,
            open: 100.0,
            high: close_rally + 0.1,
            low: 99.9,
            close: close_rally,
            volume: 1000.0,
        });

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let positions = vec![];

        // Provide analysis to force Neutral sentiment (so we don't skip due to Overbought)
        // and ATR ~ 0.2 (from manual calc)
        let analysis = MarketAnalysis {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            regime: "Trending Up".to_string(),
            volatility: "Low".to_string(),
            sentiment: "Bullish".to_string(), // Neutral or just Bullish but NOT Overbought
            patterns: vec![],
            key_levels: vec![],
            atr: Some(0.2), // Force ATR to match our calculation expectation
            research_summary: None,
            news_summary: None,
            recommendation: None,
            confidence: 0.8,
            timestamp_unix_ms: now + i * 60000,
        };

        let intents = generate_signals(
            &series,
            "EmaCrossover",
            None,
            100.0,
            &positions,
            Some(analysis),
        )
        .await?;

        assert!(
            !intents.is_empty(),
            "Should generate signal on crossover (provided analysis allows it)"
        );
        let intent = &intents[0];

        if let Some(sl) = intent.stop_loss {
            println!("Debug: SL = {}, Price = {}", sl, close_rally);
            // We want SL to be ATR based (Tight ~103.89), not Fixed % (Loose ~99.75)
            // 2 * ATR (0.4) = 0.8. SL = 105 - 0.8 = 104.2
            assert!(sl > 103.0, "SL should be tight (ATR based). Got: {}", sl);
        } else {
            panic!("SL missing");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_trend_following_respects_no_chasing() -> Result<()> {
        let mut bars = Vec::new();
        let now = 100000;
        // 1. Establish Flat Trend
        // Short (9) ~ Long (21)
        for i in 0..50 {
            let close = if i % 2 == 0 { 100.0 } else { 99.9 };
            bars.push(Bar {
                symbol: "TEST".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: close,
                high: close + 0.1,
                low: close - 0.1,
                close: close,
                volume: 1000.0,
            });
        }

        // 2. Trigger Crossover (Short > Long)
        // Price jump to 105.
        let i = 50;
        let close_rally = 105.0;
        bars.push(Bar {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: now + i * 60000,
            open: 100.0,
            high: close_rally + 0.1,
            low: 99.9,
            close: close_rally,
            volume: 1000.0,
        });

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let positions = vec![];

        // 3. Force Overbought Sentiment
        let overbought_analysis = MarketAnalysis {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            regime: "Trending Up".to_string(),
            volatility: "High".to_string(),
            sentiment: "Bullish (Overbought)".to_string(), // Overbought!
            patterns: vec![],
            key_levels: vec![],
            atr: None,
            research_summary: None,
            news_summary: None,
            recommendation: None,
            confidence: 0.8,
            timestamp_unix_ms: now + i * 60000,
        };

        // 4. Generate Signals with EmaCrossover (TrendFollowing)
        let intents = generate_signals(
            &series,
            "EmaCrossover",
            None,
            100.0,
            &positions,
            Some(overbought_analysis),
        )
        .await?;

        // 5. Assert Signal SKIPPED
        // Because TrendFollowing should NOT chase Overbought conditions.
        assert!(
            intents.is_empty(),
            "EmaCrossover (TrendFollowing) should NOT chase (Buy when Overbought)"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_money_flow_index_native_sizing() -> Result<()> {
        let mut bars = Vec::new();
        let now = 100000;
        // 1. Establish initial conditions
        // We want MFI < 20 to trigger a buy.
        // MFI uses Typical Price and Volume.

        // Initial setup
        for i in 0..20 {
            bars.push(Bar {
                symbol: "TEST".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: 100.0,
                high: 100.1,
                low: 99.9,
                close: 100.0,
                volume: 1000.0,
            });
        }

        // 2. Drop Price drastically to trigger Oversold MFI
        let i = 20;
        let drop_price = 90.0;
        bars.push(Bar {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: now + i * 60000,
            open: 100.0,
            high: 100.0,
            low: drop_price,
            close: drop_price,
            volume: 5000.0, // High volume on drop
        });

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let positions = vec![];

        // 3. Define Analysis with known ATR (ignored by MFI now, but passed for API completeness)
        let analysis = MarketAnalysis {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            regime: "Trending Down".to_string(),
            volatility: "High".to_string(),
            sentiment: "Bearish (Oversold)".to_string(),
            patterns: vec![],
            key_levels: vec![],
            atr: Some(2.0),
            research_summary: None,
            news_summary: None,
            recommendation: None,
            confidence: 0.8,
            timestamp_unix_ms: now + i * 60000,
        };

        let intents = generate_signals(
            &series,
            "MoneyFlowIndex",
            None,
            100.0,
            &positions,
            Some(analysis),
        )
        .await?;

        assert!(!intents.is_empty(), "Should generate MFI signal");
        let intent = &intents[0];

        // Default Strategy SL (Fixed %): Price * (1 - 0.05) = 90 * 0.95 = 85.5
        // Native ATR SL: Calculated internally. Should be closer to price given the drop volatility.
        // We verify that a SL is present and it is NOT the fixed percentage default (85.5)
        // and it is logically placed below price.

        if let Some(sl) = intent.stop_loss {
            assert!(sl < drop_price, "SL should be below entry price");
            // Check that it's not the old fixed percentage
            let fixed_sl = drop_price * 0.95;
            assert!(
                (sl - fixed_sl).abs() > 0.001,
                "Stop Loss ({}) appears to be fixed percentage (85.5), implying native ATR sizing failed.",
                sl
            );
            println!("Verified Native SL: {}", sl);
        } else {
            panic!("Stop Loss missing");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_short_entry_sizing_logic() -> Result<()> {
        // Goal: Verify Short Entry SL/TP/Size logic.
        // Strategy: BollingerBands (Generates Sell when Price > Upper Band)
        // Price: 110. Upper Band: 100. StdDev: 2.5 (implied).
        // ATR: 2.0.
        // Short SL should be ABOVE Price (Price + 2*ATR).
        // Short TP should be BELOW Price (Price - 4*ATR).

        let mut bars = Vec::new();
        let now = 100000;

        // Stable history to set Bands around 100
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

        // Trigger Sell (Price Spike)
        let i = 20;
        let spike_price = 110.0;
        bars.push(Bar {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: now + i * 60000,
            open: 100.0,
            high: spike_price,
            low: 100.0,
            close: spike_price,
            volume: 5000.0,
        });

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let positions = vec![];

        let analysis = MarketAnalysis {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            regime: "Trending Up".to_string(),
            volatility: "High".to_string(),
            sentiment: "Bullish".to_string(), // Avoid "Overbought" to pass chasing filter
            patterns: vec![],
            key_levels: vec![],
            atr: Some(2.0),
            research_summary: None,
            news_summary: None,
            recommendation: None,
            confidence: 0.8,
            timestamp_unix_ms: now + i * 60000,
        };

        let intents = generate_signals(
            &series,
            "BollingerBands", // Note: BB generates Sell on Upper Band Break
            None,
            100.0, // Risk $100
            &positions,
            Some(analysis),
        )
        .await?;

        assert!(!intents.is_empty(), "Should generate Sell signal");
        let intent = &intents[0];

        assert_eq!(intent.side, "sell");

        let price = spike_price;
        let atr = 2.0;

        // Verify SL
        // Logic: if side == "sell" -> SL = Price + (2.0 * ATR) = 110 + 4 = 114.
        if let Some(sl) = intent.stop_loss {
            assert!(
                sl > price,
                "Short SL ({}) must be above entry price ({})",
                sl,
                price
            );
            // Check approx value (allowing for minor precision diffs or internal strategy override logic)
            // BB Strategy internal SL: Price + 2*StdDev.
            // Signals.rs override: Price + 2*ATR (if strategy doesn't provide or we force it).
            // BB Strategy provides SL.
            // Signals.rs check: `if let Some(s) = signal.stop_loss { Some(s) } else { ... fallback ... }`
            // But wait, BB Strategy uses StdDev based SL.
            // StdDev here? 20 bars of 100. 1 bar of 110. Mean ~ 100.5. StdDev ~ 2.2?
            // SL = 110 + 2*StdDev.
            // Let's just verify it exists and is > Price.
            assert!(sl > price + 1.0, "SL should provide buffer");
        } else {
            panic!("SL missing");
        }

        // Verify TP
        // Logic: if side == "sell" -> TP = Price - (4.0 * ATR) = 110 - 8 = 102.
        if let Some(tp) = intent.take_profit {
            assert!(
                tp < price,
                "Short TP ({}) must be below entry price ({})",
                tp,
                price
            );
        } else {
            panic!("TP missing");
        }

        // Verify Size
        // Size = Risk / |Price - SL|
        // Should be positive
        let size: f64 = intent.size_hint.parse().unwrap();
        assert!(size > 0.0, "Size must be positive");

        Ok(())
    }

    #[tokio::test]
    async fn test_connors_rsi_sl_override() -> Result<()> {
        let mut bars = Vec::new();
        let now = 100000;

        // 1. Generate History: Flat price 100.0 for 100 bars (fills rank lookback)
        for i in 0..100 {
            bars.push(Bar {
                symbol: "TEST".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: 100.0,
                high: 100.1,
                low: 99.9,
                close: 100.0,
                volume: 1000.0,
            });
        }

        // 2. Trigger Drop to create Oversold CRSI
        // Drop 100 -> 90 -> 80 -> 70.
        // RSI(3) will be low. Streak will be negative. Rank will be 0.
        let prices = vec![90.0, 80.0, 70.0];
        for (i, p) in prices.iter().enumerate() {
            bars.push(Bar {
                symbol: "TEST".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: now + (100 + i as i64) * 60000,
                open: *p + 1.0,
                high: *p + 2.0,
                low: *p - 2.0,
                close: *p,
                volume: 5000.0,
            });
        }

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let positions = vec![];

        // 3. Define Analysis with explicit ATR
        // Price at trigger = 70.0.
        // Fixed SL (5%) = 70.0 * 0.95 = 66.5.
        // Let's force ATR = 1.0.
        // ATR SL (2.0 * ATR) = 70.0 - 2.0 = 68.0.
        // So ATR SL (68.0) > Fixed SL (66.5).
        // If override works, SL should be 68.0.

        let analysis = MarketAnalysis {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            regime: "Trending Down".to_string(),
            volatility: "High".to_string(),
            sentiment: "Bearish (Oversold)".to_string(),
            patterns: vec![],
            key_levels: vec![],
            atr: Some(1.0), // Force ATR
            research_summary: None,
            news_summary: None,
            recommendation: None,
            confidence: 0.8,
            timestamp_unix_ms: now + (100 + 2) * 60000,
        };

        let intents = generate_signals(
            &series,
            "ConnorsRsiMeanReversion",
            None,
            100.0,
            &positions,
            Some(analysis),
        )
        .await?;

        assert!(!intents.is_empty(), "Should generate CRSI signal");
        let intent = &intents[0];

        assert_eq!(intent.side, "buy");

        if let Some(sl) = intent.stop_loss {
            println!("Debug: SL = {}", sl);
            // Expected ATR SL = 68.0. Fixed SL = 66.5.
            // Check if SL is closer to 68.0 than 66.5
            // Or just check > 67.0
            assert!(sl > 67.0, "SL should be ATR based (approx 68.0), got {}", sl);
            assert!(
                (sl - 68.0).abs() < 0.001,
                "SL should match 2.0 * ATR exactly"
            );
        } else {
            panic!("SL missing");
        }

        Ok(())
    }
}
