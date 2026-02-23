use crate::analysis;
use crate::rag;
use anyhow::Result;
use contracts::{BarSeries, TradeIntent};
use polars::prelude::*;
use std::path::Path;
use strategies::bollinger_bands::{BollingerBandsConfig, BollingerBandsMeanReversion};
use strategies::strategy::Strategy;

pub async fn generate_signals(
    bars: &BarSeries,
    strategy_name: &str,
    history_path: Option<&Path>,
) -> Result<Vec<TradeIntent>> {
    // 1. Analyze Market
    let market_analysis = analysis::analyze(bars);

    // 2. Prepare Data for Strategy
    let df = bars_to_dataframe(bars)?;

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
    let mut signals_today = 0; // Simple counter for daily limit

    // Filter for latest signals only
    let latest_timestamp = bars.bars.last().map(|b| b.timestamp_unix_ms).unwrap_or(0);

    for signal in raw_signals {
        // Filter: Only keep signals from the latest bar
        if signal.timestamp_ms != latest_timestamp {
            continue;
        }

        // Filter: Limit to 3 signals per day (naive check per batch)
        if signals_today >= 3 {
            break;
        }

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

        // Position Sizing (Volatility based)
        // Simple logic: Base size 100, adjusted by volatility.
        // Volatility "High" -> 0.5x, "Medium" -> 1.0x, "Low" -> 2.0x
        let size_multiplier = match market_analysis.volatility.as_str() {
            "High" => 0.5,
            "Low" => 2.0,
            _ => 1.0,
        };

        let base_size: f64 = 100.0; // Placeholder base unit
        let size = (base_size * size_multiplier).round();

        // SL / TP Calculation
        // Use volatility from analysis or assume ATR-like proxy
        // For simplicity, we use % based on volatility description
        let (sl_pct, tp_pct) = match market_analysis.volatility.as_str() {
            "High" => (0.05, 0.10), // Wider stops for high vol
            "Low" => (0.01, 0.02),  // Tighter stops for low vol
            _ => (0.02, 0.04),
        };

        let last_close = bars.bars.last().map(|b| b.close).unwrap_or(100.0);

        let (stop_loss, take_profit) = if signal.side == "buy" {
            (
                Some(last_close * (1.0 - sl_pct)),
                Some(last_close * (1.0 + tp_pct)),
            )
        } else {
            (
                Some(last_close * (1.0 + sl_pct)),
                Some(last_close * (1.0 - tp_pct)),
            )
        };

        let intent = TradeIntent {
            intent_id: format!("{}:{}:{}:{}", market_analysis.market, signal.symbol, signal.side, signal.timestamp_ms),
            market: market_analysis.market.clone(),
            symbol: signal.symbol.clone(),
            side: signal.side.clone(),
            size_hint: size.to_string(),
            confidence: signal.confidence,
            horizon: "1d".to_string(),
            rationale: format!("Strategy: {}. Reason: {}. Market: {}. {}", strategy.name(), signal.reason, market_analysis.regime, historical_context),
            invalidation: "Price hits Stop Loss".to_string(),
            schema_version: "v0".to_string(),
            stop_loss,
            take_profit,
        };

        intents.push(intent);
        signals_today += 1;
    }

    Ok(intents)
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
}
