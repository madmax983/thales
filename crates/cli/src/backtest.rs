use anyhow::Result;
use contracts::{BarSeries};
use polars::prelude::*;
use serde::{Deserialize, Serialize};
use strategies::strategy::{Signal, SignalType};
use crate::strategy_factory;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestConfig {
    pub initial_capital: f64,
    pub risk_per_trade: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResult {
    pub strategy: String,
    pub symbol: String,
    pub timeframe: String,
    pub initial_capital: f64,
    pub final_equity: f64,
    pub metrics: BacktestMetrics,
    pub trades: Vec<BacktestTrade>,
    pub equity_curve: Vec<EquityPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestMetrics {
    pub total_return_pct: f64,
    pub cagr: f64,
    pub max_drawdown_pct: f64,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestTrade {
    pub id: String,
    pub entry_time: i64,
    pub exit_time: i64,
    pub side: String,
    pub qty: f64,
    pub entry_price: f64,
    pub exit_price: f64,
    pub pnl: f64,
    pub pnl_pct: f64,
    pub exit_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityPoint {
    pub timestamp: i64,
    pub equity: f64,
}

struct PendingOrder {
    signal: Signal,
    #[allow(dead_code)]
    timestamp: i64,
}

struct OpenPosition {
    #[allow(dead_code)]
    symbol: String,
    side: String,
    qty: f64,
    entry_price: f64,
    entry_time: i64,
    stop_loss: Option<f64>,
    take_profit: Option<f64>,
}

pub async fn run_backtest(
    bars: &BarSeries,
    strategy_name: &str,
    config: BacktestConfig,
) -> Result<BacktestResult> {
    if bars.bars.is_empty() {
        return Err(anyhow::anyhow!("No bars provided for backtest"));
    }

    let symbol = bars.bars[0].symbol.clone();
    let timeframe = bars.bars[0].timeframe.clone();

    // 1. Prepare Data
    let df = bars_to_dataframe(bars)?;

    // 2. Instantiate Strategy
    let strategy = strategy_factory::create_strategy(strategy_name, &symbol)?;

    // 3. Generate All Signals
    let raw_signals = strategy.generate_signals(&df).await?;

    // Group signals by timestamp for efficient lookup
    // Assuming signals are sorted by timestamp, but let's map them.
    // Use a HashMap<Timestamp, Vec<Signal>>
    let mut signals_map: std::collections::HashMap<i64, Vec<Signal>> = std::collections::HashMap::new();
    for signal in raw_signals {
        signals_map.entry(signal.timestamp_ms).or_default().push(signal);
    }

    // 4. Simulation Loop
    let mut position: Option<OpenPosition> = None;
    let mut trades: Vec<BacktestTrade> = Vec::new();
    let mut equity_curve: Vec<EquityPoint> = Vec::new();
    let mut pending_orders: Vec<PendingOrder> = Vec::new();

    let mut max_equity = config.initial_capital;
    let mut max_drawdown = 0.0;

    for i in 0..bars.bars.len() {
        let bar = &bars.bars[i];
        let current_time = bar.timestamp_unix_ms;

        // A. Execute Pending Orders (Market Orders from Previous Tick)
        // We execute at Open of current bar
        if let Some(order) = pending_orders.pop() {
            // Only execute if we don't have a position (or handle scaling later)
            // For MVP, simplistic: 1 position at a time per symbol
            if position.is_none() {
                // Calculate Size
                // If SL provided, use risk based sizing. Else use size_hint or fallback.
                let sl = order.signal.stop_loss;
                let tp = order.signal.take_profit;

                let entry_price = bar.open; // Fill at Open

                // Basic Sizing Logic
                let qty = if let Some(stop) = sl {
                     let risk_dist = (entry_price - stop).abs();
                     if risk_dist > 0.0 {
                         config.risk_per_trade / risk_dist
                     } else {
                         0.0
                     }
                } else {
                    // Fallback: Use fixed size hint if numeric, else small default
                    if let Ok(s) = order.signal.size_hint.parse::<f64>() {
                        if s > 0.0 { s } else { 0.0 }
                    } else {
                        // "max" or invalid -> risk 1% of capital?
                        // Let's just default to risk_per_trade / (price * 0.01) (assuming 1% risk)
                        // Or just buy 1 unit if logic fails
                        1.0
                    }
                };

                if qty > 0.0 {
                    let side = if order.signal.side == "buy" { "long".to_string() } else { "short".to_string() };
                    position = Some(OpenPosition {
                        symbol: order.signal.symbol.clone(),
                        side,
                        qty,
                        entry_price,
                        entry_time: current_time,
                        stop_loss: sl,
                        take_profit: tp,
                    });
                }
            } else {
                // Check if this is an Exit signal for the existing position
                 if let Some(pos) = &position {
                     // If Signal is Exit and matches direction (e.g. Long Pos + Sell Signal)
                     let is_exit = order.signal.signal_type == SignalType::Exit;
                     let correct_side = (pos.side == "long" && order.signal.side == "sell") ||
                                        (pos.side == "short" && order.signal.side == "buy");

                     if is_exit && correct_side {
                         // Close Position at Open
                         let exit_price = bar.open;
                         let pnl = if pos.side == "long" {
                             (exit_price - pos.entry_price) * pos.qty
                         } else {
                             (pos.entry_price - exit_price) * pos.qty
                         };

                         // Note: We don't subtract cost basis because we track Cash + Position Value = Equity
                         // Wait, simpler: Cash is "Available Cash".
                         // When buying, we deduce cost?
                         // No, let's track Equity.
                         // Equity = Cash + Unrealized PnL.
                         // Actually, simpler model:
                         // Start Cash = 10000.
                         // Buy 1 BTC @ 10000. Cash = 0. Position = 1 BTC.
                         // Sell 1 BTC @ 11000. Cash = 11000. Position = 0.
                         // PnL = 1000.

                         // Re-do accounting:
                         // 1. Buy: Cash -= Price * Qty.
                         // 2. Sell: Cash += Price * Qty.
                         // But for Shorting?
                         // Short 1 BTC @ 10000. Cash = 20000 (10k collateral + 10k proceeds). Liability = 1 BTC.
                         // Cover 1 BTC @ 9000. Cash -= 9000. Cash = 11000. PnL = 1000.

                         // Let's stick to PnL accumulation for simplicity.
                         // Equity = Initial + Sum(Realized PnL) + Unrealized PnL.

                         trades.push(BacktestTrade {
                             id: order.signal.timestamp_ms.to_string(),
                             entry_time: pos.entry_time,
                             exit_time: current_time,
                             side: pos.side.clone(),
                             qty: pos.qty,
                             entry_price: pos.entry_price,
                             exit_price,
                             pnl,
                             pnl_pct: pnl / (pos.entry_price * pos.qty), // ROI on trade not account
                             exit_reason: order.signal.reason.clone(),
                         });

                         position = None;
                     }
                 }
            }
        }

        // B. Check Exits (SL/TP) for Open Position
        // Check High/Low of current bar
        if let Some(pos) = &position {
            let mut exit_price: Option<f64> = None;
            let mut reason = String::new();

            if pos.side == "long" {
                // Check SL (Low <= SL)
                if let Some(sl) = pos.stop_loss {
                    if bar.low <= sl {
                        // Slippage: If Open < SL, we gap down, fill at Open. Else fill at SL.
                        exit_price = Some(if bar.open < sl { bar.open } else { sl });
                        reason = "Stop Loss".to_string();
                    }
                }
                // Check TP (High >= TP)
                if exit_price.is_none() { // SL takes precedence usually
                    if let Some(tp) = pos.take_profit {
                        if bar.high >= tp {
                            // Slippage: If Open > TP, we gap up, fill at Open. Else fill at TP.
                            exit_price = Some(if bar.open > tp { bar.open } else { tp });
                            reason = "Take Profit".to_string();
                        }
                    }
                }
            } else { // Short
                 // Check SL (High >= SL)
                if let Some(sl) = pos.stop_loss {
                    if bar.high >= sl {
                        exit_price = Some(if bar.open > sl { bar.open } else { sl });
                        reason = "Stop Loss".to_string();
                    }
                }
                 // Check TP (Low <= TP)
                if exit_price.is_none() {
                    if let Some(tp) = pos.take_profit {
                        if bar.low <= tp {
                            exit_price = Some(if bar.open < tp { bar.open } else { tp });
                            reason = "Take Profit".to_string();
                        }
                    }
                }
            }

            if let Some(price) = exit_price {
                let pnl = if pos.side == "long" {
                     (price - pos.entry_price) * pos.qty
                 } else {
                     (pos.entry_price - price) * pos.qty
                 };

                 trades.push(BacktestTrade {
                     id: format!("{}-auto", current_time),
                     entry_time: pos.entry_time,
                     exit_time: current_time,
                     side: pos.side.clone(),
                     qty: pos.qty,
                     entry_price: pos.entry_price,
                     exit_price: price,
                     pnl,
                     pnl_pct: pnl / (pos.entry_price * pos.qty),
                     exit_reason: reason,
                 });

                 position = None;
            }
        }

        // C. Process New Signals (for Next Tick)
        // We look for signals generated at THIS timestamp.
        if let Some(sigs) = signals_map.get(&current_time) {
            // Sort signals?
            // Prioritize Exits over Entries?
            // For now, just take the first valid one.
            for sig in sigs {
                // If we have a position, ignore Entry signals unless we support scaling (MVP: No)
                // If we don't have a position, ignore Exit signals.
                let is_entry = sig.signal_type == SignalType::Entry || sig.signal_type == SignalType::ScaleIn;
                let is_exit = sig.signal_type == SignalType::Exit || sig.signal_type == SignalType::ScaleOut;

                if position.is_none() && is_entry {
                    // Enrich Signal with SL/TP if missing?
                    // Strategy usually provides them. If not, maybe we should add default?
                    // For now, trust strategy.
                    pending_orders.push(PendingOrder {
                        signal: sig.clone(),
                        timestamp: current_time,
                    });
                    break; // Only take one action per bar
                } else if position.is_some() && is_exit {
                    pending_orders.push(PendingOrder {
                        signal: sig.clone(),
                        timestamp: current_time,
                    });
                    break;
                }
            }
        }

        // D. Update Equity Curve
        let unrealized_pnl = if let Some(pos) = &position {
            if pos.side == "long" {
                (bar.close - pos.entry_price) * pos.qty
            } else {
                (pos.entry_price - bar.close) * pos.qty
            }
        } else {
            0.0
        };

        // Equity = Initial + Realized + Unrealized
        let realized_pnl: f64 = trades.iter().map(|t| t.pnl).sum();
        let current_equity = config.initial_capital + realized_pnl + unrealized_pnl;

        equity_curve.push(EquityPoint {
            timestamp: current_time,
            equity: current_equity,
        });

        if current_equity > max_equity {
            max_equity = current_equity;
        }
        let drawdown = (max_equity - current_equity) / max_equity;
        if drawdown > max_drawdown {
            max_drawdown = drawdown;
        }
    }

    // 5. Finalize Metrics
    let realized_pnl: f64 = trades.iter().map(|t| t.pnl).sum();
    let final_equity = config.initial_capital + realized_pnl; // Close all at end?
    // Let's assume open positions are marked to market at final close
    let total_return_pct = (final_equity - config.initial_capital) / config.initial_capital * 100.0;

    let win_count = trades.iter().filter(|t| t.pnl > 0.0).count();
    let loss_count = trades.iter().filter(|t| t.pnl <= 0.0).count();
    let win_rate = if !trades.is_empty() { win_count as f64 / trades.len() as f64 } else { 0.0 };

    let gross_profit: f64 = trades.iter().filter(|t| t.pnl > 0.0).map(|t| t.pnl).sum();
    let gross_loss: f64 = trades.iter().filter(|t| t.pnl <= 0.0).map(|t| t.pnl.abs()).sum();
    let profit_factor = if gross_loss > 0.0 { gross_profit / gross_loss } else { gross_profit };

    Ok(BacktestResult {
        strategy: strategy_name.to_string(),
        symbol,
        timeframe,
        initial_capital: config.initial_capital,
        final_equity,
        metrics: BacktestMetrics {
            total_return_pct,
            cagr: 0.0, // Need annualized logic
            max_drawdown_pct: max_drawdown * 100.0,
            win_rate,
            profit_factor,
            total_trades: trades.len(),
            winning_trades: win_count,
            losing_trades: loss_count,
        },
        trades,
        equity_curve,
    })
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
    use contracts::Bar;

    #[tokio::test]
    async fn test_backtest_simple_profit() -> Result<()> {
        // Create a scenario where we buy at 100, sell at 110.
        // We need a strategy that triggers this.
        // RSI < 30 -> Buy. RSI > 70 -> Sell.

        let mut bars = Vec::new();
        let now = 100000;

        // 1. Drop price to trigger RSI Oversold (Buy)
        for i in 0..20 {
            let close = 100.0 - (i as f64); // Drop from 100 to 80
            bars.push(Bar {
                symbol: "TEST".to_string(), market: "equities".to_string(), timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: close, high: close + 1.0, low: close - 1.0, close: close, volume: 1000.0,
            });
        }

        // 2. Rally to trigger RSI Overbought (Sell)
        for i in 20..40 {
             let close = 80.0 + ((i - 20) as f64) * 2.0; // Rise from 80 to 120
             bars.push(Bar {
                symbol: "TEST".to_string(), market: "equities".to_string(), timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: close, high: close + 1.0, low: close - 1.0, close: close, volume: 1000.0,
            });
        }

        let series = BarSeries { schema_version: "v0".to_string(), bars };

        let config = BacktestConfig {
            initial_capital: 10000.0,
            risk_per_trade: 100.0,
        };

        let result = run_backtest(&series, "RsiMeanReversion", config).await?;

        // We expect at least one trade
        assert!(!result.trades.is_empty());

        // Verify Profit
        // We bought low (around 80) and sold high (around 120)
        // Note: First trade might be stopped out if entry is too early, so we check overall profitability
        assert!(result.trades.iter().any(|t| t.pnl > 0.0));
        assert!(result.metrics.total_return_pct > 0.0);

        Ok(())
    }

    #[tokio::test]
    async fn test_backtest_stop_loss() -> Result<()> {
        // Scenario: Buy, then crash.
        let mut bars = Vec::new();
        let now = 100000;

        // 1. Drop to trigger Buy
        for i in 0..20 {
            let close = 100.0 - (i as f64);
            bars.push(Bar {
                symbol: "TEST".to_string(), market: "equities".to_string(), timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: close, high: close, low: close, close: close, volume: 1000.0
            });
        }

        // 2. Crash further
        for i in 20..30 {
             let close = 80.0 - ((i - 20) as f64) * 5.0; // Crash fast
             bars.push(Bar {
                symbol: "TEST".to_string(), market: "equities".to_string(), timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: close, high: close, low: close, close: close, volume: 1000.0
            });
        }

        let series = BarSeries { schema_version: "v0".to_string(), bars };
        let config = BacktestConfig { initial_capital: 10000.0, risk_per_trade: 100.0 };

        let result = run_backtest(&series, "RsiMeanReversion", config).await?;

        assert!(!result.trades.is_empty());
        let trade = &result.trades[0];

        // Should have hit Stop Loss
        assert!(trade.pnl < 0.0);
        assert_eq!(trade.exit_reason, "Stop Loss");

        Ok(())
    }
}
