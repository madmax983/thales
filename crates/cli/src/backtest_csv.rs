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
