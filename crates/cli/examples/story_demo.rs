//! Story Demo Example
//!
//! This example demonstrates how to use the experimental trading card feature
//! available in the `nova` module. It creates a dummy `BarSeries`, calculates
//! RPG-style stats (Power, Agility, Stamina) based on price action, and exports
//! a visual Trading Card in SVG format.
//!
//! Requires the `nova` feature flag to be enabled:
//! `cargo run --features nova --example story_demo`

use anyhow::Result;

#[cfg(feature = "nova")]
use contracts::{Bar, BarSeries};
#[cfg(feature = "nova")]
use thales_cli::experimental::trading_card::{calculate_stats, export_trading_card_svg};

fn main() -> Result<()> {
    println!("Welcome to the Thales CLI Story Demo!");

    #[cfg(feature = "nova")]
    {
        let bars = vec![
            Bar {
                symbol: "DRGN".to_string(),
                market: "equities".to_string(),
                timeframe: "1d".to_string(),
                timestamp_unix_ms: 1600000000000,
                open: 100.0,
                high: 150.0,
                low: 90.0,
                close: 140.0,
                volume: 50000.0,
            },
            Bar {
                symbol: "DRGN".to_string(),
                market: "equities".to_string(),
                timeframe: "1d".to_string(),
                timestamp_unix_ms: 1600086400000,
                open: 140.0,
                high: 180.0,
                low: 130.0,
                close: 175.0,
                volume: 75000.0,
            },
        ];
        let series = BarSeries {
            schema_version: "v1".to_string(),
            bars,
        };

        let stats = calculate_stats(&series);
        println!("Calculated Stats for {}:", series.bars[0].symbol);
        println!("  Power (Volatility): {}", stats.power);
        println!("  Agility (Momentum): {}", stats.agility);
        println!("  Stamina (Volume):   {}", stats.stamina);

        let path = "dragon_card.svg";
        export_trading_card_svg(&series, "The Dragon", path)?;
        println!("Exported trading card to {}", path);
    }

    #[cfg(not(feature = "nova"))]
    {
        println!("The 'nova' feature is not enabled. Please run with '--features nova'.");
    }

    Ok(())
}
