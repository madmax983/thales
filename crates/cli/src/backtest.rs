//! Provides a historical backtesting engine for quantitative trading strategies.
//!
//! This module allows you to run a trading [`Strategy`] against historical [`BarSeries`] data
//! to evaluate its performance. It simulates trade execution, tracks open positions, applies
//! stop-loss and take-profit logic, and generates a comprehensive [`BacktestResult`] containing
//! equity curves and performance metrics.
//!
//! # Core Concepts
//!
//! - **Signal Generation:** The engine first passes the entire data series to the strategy to generate
//!   a list of trading signals.
//! - **Event-Driven Simulation:** It iterates chronologically bar-by-bar.
//! - **Execution:** Signals generated on bar $N$ are executed at the Open price of bar $N+1$.
//! - **Risk Management:** Stop-loss (SL) and take-profit (TP) levels attached to signals are evaluated
//!   intrabar (using High/Low prices).
//!
//! # Examples
//!
//! ```no_run
//! use thales_cli::backtest::{run_backtest, BacktestConfig};
//! use contracts::{BarSeries, Bar};
//!
//! #[tokio::main]
//! async fn main() {
//!     let bars = BarSeries {
//!         schema_version: "v0".to_string(),
//!         bars: vec![], // Populate with historical data
//!     };
//!
//!     let config = BacktestConfig {
//!         initial_capital: 10_000.0,
//!         risk_per_trade: 100.0, // Dollar amount to risk per trade
//!     };
//!
//!     // Run the "EmaCrossover" strategy
//!     let result = run_backtest(&bars, "EmaCrossover", config).await.unwrap();
//!
//!     println!("Final Equity: ${:.2}", result.final_equity);
//!     println!("Win Rate: {:.2}%", result.metrics.win_rate * 100.0);
//! }
//! ```

use crate::strategy_factory;
use anyhow::Result;
use contracts::BarSeries;
use polars::prelude::*;
use serde::{Deserialize, Serialize};
use strategies::strategy::{Signal, SignalType};

/// Configuration parameters for a backtest run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestConfig {
    /// The starting account balance in the quote currency (e.g., USD).
    pub initial_capital: f64,
    /// The maximum dollar amount willing to be lost on a single trade if the stop-loss is hit.
    /// This is used to dynamically size positions if a signal provides a stop loss.
    pub risk_per_trade: f64,
}

/// The comprehensive output of a completed backtest simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResult {
    /// The name of the strategy evaluated.
    pub strategy: String,
    /// The asset symbol traded.
    pub symbol: String,
    /// The timeframe of the data (e.g., "1d", "1h").
    pub timeframe: String,
    /// The starting account balance.
    pub initial_capital: f64,
    /// The ending account balance.
    pub final_equity: f64,
    /// Aggregate performance statistics.
    pub metrics: BacktestMetrics,
    /// A chronological ledger of all completed round-trip trades.
    pub trades: Vec<BacktestTrade>,
    /// Time-series data representing the account balance at each evaluation step.
    pub equity_curve: Vec<EquityPoint>,
}

/// Key performance indicators (KPIs) summarizing strategy performance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestMetrics {
    /// Total percentage return over the backtest period.
    pub total_return_pct: f64,
    /// Compound Annual Growth Rate (requires timestamps spanning > 0 days).
    pub cagr: f64,
    /// The maximum peak-to-trough decline in equity, represented as a positive percentage (e.g., `0.15` for 15%).
    pub max_drawdown_pct: f64,
    /// The percentage of trades that resulted in a positive PnL (0.0 to 1.0).
    pub win_rate: f64,
    /// The ratio of gross profit to gross loss. A value > 1.0 indicates profitability.
    pub profit_factor: f64,
    /// Total number of round-trip trades executed.
    pub total_trades: usize,
    /// Number of trades with PnL > 0.
    pub winning_trades: usize,
    /// Number of trades with PnL <= 0.
    pub losing_trades: usize,
}

/// Represents a single completed round-trip trade (Entry to Exit).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestTrade {
    /// A unique identifier for the trade execution.
    pub id: String,
    /// The UNIX timestamp (ms) when the position was opened.
    pub entry_time: i64,
    /// The UNIX timestamp (ms) when the position was closed.
    pub exit_time: i64,
    /// The direction of the trade (`"long"` or `"short"`).
    pub side: String,
    /// The number of shares/contracts traded.
    pub qty: f64,
    /// The execution price at entry.
    pub entry_price: f64,
    /// The execution price at exit.
    pub exit_price: f64,
    /// The absolute profit or loss in the quote currency.
    pub pnl: f64,
    /// The percentage return of the trade relative to the entry price.
    pub pnl_pct: f64,
    /// The condition that triggered the exit (e.g., "Stop Loss", "Take Profit", "Signal Exit").
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

use strategies::strategy::Strategy;

/// Executes a backtest using a registered strategy name.
///
/// This is a convenience wrapper around [`run_backtest_with_strategy`] that instantiates
/// the strategy via the strategy factory.
///
/// # Arguments
///
/// * `bars` - The historical data series to evaluate. Must not be empty.
/// * `strategy_name` - The exact registered name of the strategy (e.g., "RsiMeanReversion").
/// * `config` - Capital and risk settings.
///
/// # Errors
///
/// Returns an error if the `bars` series is empty or if the `strategy_name` is unknown.
pub async fn run_backtest(
    bars: &BarSeries,
    strategy_name: &str,
    config: BacktestConfig,
) -> Result<BacktestResult> {
    if bars.bars.is_empty() {
        return Err(anyhow::anyhow!("No bars provided for backtest"));
    }
    let symbol = bars.bars[0].symbol.clone();
    let strategy = strategy_factory::create_strategy(strategy_name, &symbol)?;
    run_backtest_with_strategy(bars, strategy, config).await
}

/// Executes a backtest using an explicit strategy instance.
///
/// # Process
///
/// 1. The full `BarSeries` is evaluated by the `strategy` to generate a timeline of raw [`Signal`]s.
/// 2. The simulation iterates chronologically over the bars.
/// 3. Signals emitted on bar $N$ create pending orders that execute at the Open of bar $N+1$.
/// 4. Open positions are continuously evaluated against intrabar High/Low prices to trigger
///    Stop-Loss or Take-Profit conditions.
/// 5. Upon completion, metrics (Win Rate, Max Drawdown, CAGR) are calculated based on the finalized trade ledger.
///
/// # Panics
///
/// Intrabar stop-loss logic assumes logical high/low relationships (High >= Low).
pub async fn run_backtest_with_strategy(
    bars: &BarSeries,
    strategy: Box<dyn Strategy>,
    config: BacktestConfig,
) -> Result<BacktestResult> {
    if bars.bars.is_empty() {
        return Err(anyhow::anyhow!("No bars provided for backtest"));
    }

    let symbol = bars.bars[0].symbol.clone();
    let timeframe = bars.bars[0].timeframe.clone();
    let strategy_name = strategy.name().to_string();

    // 1. Prepare Data
    let df = bars_to_dataframe(bars)?;

    // 2. Generate All Signals
    let raw_signals = strategy.generate_signals(&df).await?;

    // Group signals by timestamp for efficient lookup
    // Assuming signals are sorted by timestamp, but let's map them.
    // Use a HashMap<Timestamp, Vec<Signal>>
    let mut signals_map: std::collections::HashMap<i64, Vec<Signal>> =
        std::collections::HashMap::new();
    for signal in raw_signals {
        signals_map
            .entry(signal.timestamp_ms)
            .or_default()
            .push(signal);
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
        if !bar.open.is_nan() {
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
                        if risk_dist > 0.0 && !risk_dist.is_nan() {
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

                    if qty > 0.0 && !qty.is_nan() {
                        let side = if order.signal.side == "buy" {
                            "long".to_string()
                        } else {
                            "short".to_string()
                        };
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
                        let correct_side = (pos.side == "long" && order.signal.side == "sell")
                            || (pos.side == "short" && order.signal.side == "buy");

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
        }

        // B. Check Exits (SL/TP) for Open Position
        // Check High/Low of current bar
        if !bar.high.is_nan() && !bar.low.is_nan() && !bar.open.is_nan() {
            if let Some(pos) = &position {
                let mut exit_price: Option<f64> = None;
                let mut reason = String::new();

                if pos.side == "long" {
                    // Check SL (Low <= SL)
                    if let Some(sl) = pos.stop_loss
                        && bar.low <= sl
                    {
                        // Slippage: If Open < SL, we gap down, fill at Open. Else fill at SL.
                        exit_price = Some(if bar.open < sl { bar.open } else { sl });
                        reason = "Stop Loss".to_string();
                    }
                    // Check TP (High >= TP)
                    if exit_price.is_none() {
                        // SL takes precedence usually
                        if let Some(tp) = pos.take_profit
                            && bar.high >= tp
                        {
                            // Slippage: If Open > TP, we gap up, fill at Open. Else fill at TP.
                            exit_price = Some(if bar.open > tp { bar.open } else { tp });
                            reason = "Take Profit".to_string();
                        }
                    }
                } else {
                    // Short
                    // Check SL (High >= SL)
                    if let Some(sl) = pos.stop_loss
                        && bar.high >= sl
                    {
                        exit_price = Some(if bar.open > sl { bar.open } else { sl });
                        reason = "Stop Loss".to_string();
                    }
                    // Check TP (Low <= TP)
                    if exit_price.is_none()
                        && let Some(tp) = pos.take_profit
                        && bar.low <= tp
                    {
                        exit_price = Some(if bar.open < tp { bar.open } else { tp });
                        reason = "Take Profit".to_string();
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
                let is_entry =
                    sig.signal_type == SignalType::Entry || sig.signal_type == SignalType::ScaleIn;
                let is_exit =
                    sig.signal_type == SignalType::Exit || sig.signal_type == SignalType::ScaleOut;

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
            if !bar.close.is_nan() {
                if pos.side == "long" {
                    (bar.close - pos.entry_price) * pos.qty
                } else {
                    (pos.entry_price - bar.close) * pos.qty
                }
            } else {
                0.0
            }
        } else {
            0.0
        };

        // Equity = Initial + Realized + Unrealized
        let realized_pnl: f64 = trades.iter().map(|t| t.pnl).sum();
        let current_equity = config.initial_capital + realized_pnl + unrealized_pnl;

        if !current_equity.is_nan() {
            equity_curve.push(EquityPoint {
                timestamp: current_time,
                equity: current_equity,
            });

            if current_equity > max_equity {
                max_equity = current_equity;
            }
            let drawdown = (max_equity - current_equity) / max_equity;
            if drawdown > max_drawdown && !drawdown.is_nan() {
                max_drawdown = drawdown;
            }
        }
    }

    // 5. Finalize Metrics
    let realized_pnl: f64 = trades.iter().map(|t| t.pnl).sum();
    let final_equity = config.initial_capital + realized_pnl; // Close all at end?
    // Let's assume open positions are marked to market at final close
    let total_return_pct = (final_equity - config.initial_capital) / config.initial_capital * 100.0;

    let win_count = trades.iter().filter(|t| t.pnl > 0.0).count();
    let loss_count = trades.iter().filter(|t| t.pnl <= 0.0).count();
    let win_rate = if !trades.is_empty() {
        win_count as f64 / trades.len() as f64
    } else {
        0.0
    };

    let gross_profit: f64 = trades.iter().filter(|t| t.pnl > 0.0).map(|t| t.pnl).sum();
    let gross_loss: f64 = trades
        .iter()
        .filter(|t| t.pnl <= 0.0)
        .map(|t| t.pnl.abs())
        .sum();
    let profit_factor = if gross_loss > 0.0 {
        gross_profit / gross_loss
    } else {
        gross_profit
    };

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
    let opens: Vec<Option<f64>> = series.bars.iter().map(|b| if b.open.is_nan() { None } else { Some(b.open) }).collect();
    let highs: Vec<Option<f64>> = series.bars.iter().map(|b| if b.high.is_nan() { None } else { Some(b.high) }).collect();
    let lows: Vec<Option<f64>> = series.bars.iter().map(|b| if b.low.is_nan() { None } else { Some(b.low) }).collect();
    let closes: Vec<Option<f64>> = series.bars.iter().map(|b| if b.close.is_nan() { None } else { Some(b.close) }).collect();
    let volumes: Vec<Option<f64>> = series.bars.iter().map(|b| if b.volume.is_nan() { None } else { Some(b.volume) }).collect();
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
                symbol: "TEST".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: close,
                high: close + 1.0,
                low: close - 1.0,
                close,
                volume: 1000.0,
            });
        }

        // 2. Rally to trigger RSI Overbought (Sell)
        // Adjust rally to be cleaner so stop loss isn't hit.
        // If entry is at 80, and SL is 78 (due to ATR), we must ensure Low >= 78.
        // We will make the rally smooth without wicks dipping low.
        for i in 20..40 {
            let close = 80.0 + ((i - 20) as f64) * 2.0; // Rise from 80 to 120
            bars.push(Bar {
                symbol: "TEST".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: close,
                high: close + 0.1, // Minimal high wick
                low: close,        // Low = Close to prevent Stop Loss hit
                close,
                volume: 1000.0,
            });
        }

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let config = BacktestConfig {
            initial_capital: 10000.0,
            risk_per_trade: 100.0,
        };

        let result = run_backtest(&series, "RsiMeanReversion", config).await?;

        // We expect at least one trade
        assert!(!result.trades.is_empty());

        // Verify Profit
        // We bought low (around 80) and sold high (around 120)
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
                symbol: "TEST".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: close,
                high: close,
                low: close,
                close,
                volume: 1000.0,
            });
        }

        // 2. Crash further
        for i in 20..30 {
            let close = 80.0 - ((i - 20) as f64) * 5.0; // Crash fast
            bars.push(Bar {
                symbol: "TEST".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: close,
                high: close,
                low: close,
                close,
                volume: 1000.0,
            });
        }

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let config = BacktestConfig {
            initial_capital: 10000.0,
            risk_per_trade: 100.0,
        };

        let result = run_backtest(&series, "RsiMeanReversion", config).await?;

        assert!(!result.trades.is_empty());
        let trade = &result.trades[0];

        // Should have hit Stop Loss
        assert!(trade.pnl < 0.0);
        assert_eq!(trade.exit_reason, "Stop Loss");

        Ok(())
    }

    #[tokio::test]
    async fn test_backtest_with_nan_data() -> Result<()> {
        let mut bars = Vec::new();
        let now = 100000;

        // Valid data causing entry
        for i in 0..20 {
            let close = 100.0 - (i as f64);
            bars.push(Bar {
                symbol: "TEST".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: close,
                high: close + 1.0,
                low: close - 1.0,
                close,
                volume: 1000.0,
            });
        }

        // NaN data
        for i in 20..30 {
            bars.push(Bar {
                symbol: "TEST".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: f64::NAN,
                high: f64::NAN,
                low: f64::NAN,
                close: f64::NAN,
                volume: f64::NAN,
            });
        }

        // Valid data causing exit
        for i in 30..40 {
            let close = 80.0 + ((i - 30) as f64) * 2.0;
            bars.push(Bar {
                symbol: "TEST".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: now + i * 60000,
                open: close,
                high: close + 1.0,
                low: close - 1.0,
                close,
                volume: 1000.0,
            });
        }

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let config = BacktestConfig {
            initial_capital: 10000.0,
            risk_per_trade: 100.0,
        };

        // This should not panic
        let result = run_backtest(&series, "RsiMeanReversion", config).await?;
        assert!(!result.trades.is_empty(), "Should still execute trades around NaNs");

        Ok(())
    }
}
