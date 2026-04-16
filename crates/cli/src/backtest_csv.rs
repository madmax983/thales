//! CSV Export for Backtest Results
//!
//! This module provides functionality to export the results of a backtest into a standard
//! Comma-Separated Values (CSV) format. This allows traders to load their strategy's
//! performance history into external tools like Excel, Python (Pandas), or R for further
//! quantitative analysis.
//!
//! # Core Concepts
//! - **Interoperability:** CSVs are universally accepted, bridging the gap between Rust's high-speed engine and data science tools.
//! - **Trade Log:** The exported file contains an atomic record of every completed trade, including entry/exit prices, PnL, and reasons for exit.
//!
//! # Examples
//!
//! ```no_run
//! use std::path::Path;
//! use thales_cli::backtest_csv::export_backtest_csv;
//! use thales_cli::backtest::BacktestResult;
//!
//! // Assume `result` is a populated BacktestResult
//! # let result = BacktestResult {
//! #     strategy: "Test".to_string(), symbol: "TEST".to_string(), timeframe: "1d".to_string(),
//! #     initial_capital: 10000.0, final_equity: 10000.0,
//! #     trades: vec![], metrics: thales_cli::backtest::BacktestMetrics {
//! #         total_return_pct: 0.0, cagr: 0.0, max_drawdown_pct: 0.0, win_rate: 0.0,
//! #         profit_factor: 0.0, total_trades: 0, winning_trades: 0, losing_trades: 0,
//! #     }, equity_curve: vec![]
//! # };
//! let output_path = Path::new("backtest_output.csv");
//! export_backtest_csv(&result, output_path).unwrap();
//! ```

use crate::backtest::BacktestResult;
use anyhow::Result;
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub fn export_backtest_csv(result: &BacktestResult, output_path: &Path) -> Result<()> {
    let mut file = File::create(output_path)?;
    writeln!(
        file,
        "trade_id,entry_time,exit_time,side,qty,entry_price,exit_price,pnl,pnl_pct,exit_reason"
    )?;

    for trade in &result.trades {
        writeln!(
            file,
            "{},{},{},{},{},{},{},{},{},{}",
            trade.id,
            trade.entry_time,
            trade.exit_time,
            trade.side,
            trade.qty,
            trade.entry_price,
            trade.exit_price,
            trade.pnl,
            trade.pnl_pct,
            trade.exit_reason
        )?;
    }
    Ok(())
}
