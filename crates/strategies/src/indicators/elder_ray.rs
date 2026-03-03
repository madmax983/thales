use anyhow::{Context, Result};
use polars::prelude::*;

use super::ema;

/// Elder Ray Indicator Components
#[derive(Debug, Clone)]
pub struct ElderRayOutput {
    pub ema: Series,
    pub bull_power: Series,
    pub bear_power: Series,
}

/// Calculate the Elder Ray Index
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", and "close" columns
/// * `period` - Lookback period for the EMA (typically 13)
///
/// # Returns
/// `ElderRayOutput` containing the EMA, Bull Power, and Bear Power Series.
pub fn calculate(data: &DataFrame, period: usize) -> Result<ElderRayOutput> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    // Get "high" and "low" columns
    let high = data
        .column("high")
        .context("DataFrame must contain 'high' column")?;
    let low = data
        .column("low")
        .context("DataFrame must contain 'low' column")?;

    // Calculate EMA on "close"
    let ema_series = ema::calculate(data, period).context("Failed to calculate EMA")?;

    // Calculate Bull Power = High - EMA
    let mut bull_power_series = (high - &ema_series)?;
    bull_power_series.rename("bull_power");

    // Calculate Bear Power = Low - EMA
    let mut bear_power_series = (low - &ema_series)?;
    bear_power_series.rename("bear_power");

    Ok(ElderRayOutput {
        ema: ema_series,
        bull_power: bull_power_series,
        bear_power: bear_power_series,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_elder_ray_calculation() -> Result<()> {
        let df = df!(
            "high" => &[10.5, 11.5, 12.5, 13.5, 14.5],
            "low" => &[9.5, 10.5, 11.5, 12.5, 13.5],
            "close" => &[10.0, 11.0, 12.0, 13.0, 14.0]
        )?;

        // Period 3. EMA Calculation (from ema.rs tests)
        // 0: 10.0 -> None (sum 10)
        // 1: 11.0 -> None (sum 21)
        // 2: 12.0 -> SMA(10,11,12) = 11.0. Seed.
        // 3: 13.0 -> EMA = 12.0
        // 4: 14.0 -> EMA = 13.0

        // Bull Power = High - EMA
        // 2: 12.5 - 11.0 = 1.5
        // 3: 13.5 - 12.0 = 1.5
        // 4: 14.5 - 13.0 = 1.5

        // Bear Power = Low - EMA
        // 2: 11.5 - 11.0 = 0.5
        // 3: 12.5 - 12.0 = 0.5
        // 4: 13.5 - 13.0 = 0.5

        let result = calculate(&df, 3)?;

        let ema_out = result.ema.f64()?;
        assert!(ema_out.get(0).is_none());
        assert!(ema_out.get(1).is_none());
        assert_eq!(ema_out.get(2), Some(11.0));
        assert_eq!(ema_out.get(3), Some(12.0));
        assert_eq!(ema_out.get(4), Some(13.0));

        let bull_out = result.bull_power.f64()?;
        assert!(bull_out.get(0).is_none());
        assert!(bull_out.get(1).is_none());
        assert_eq!(bull_out.get(2), Some(1.5));
        assert_eq!(bull_out.get(3), Some(1.5));
        assert_eq!(bull_out.get(4), Some(1.5));

        let bear_out = result.bear_power.f64()?;
        assert!(bear_out.get(0).is_none());
        assert!(bear_out.get(1).is_none());
        assert_eq!(bear_out.get(2), Some(0.5));
        assert_eq!(bear_out.get(3), Some(0.5));
        assert_eq!(bear_out.get(4), Some(0.5));

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        Ok(())
    }
}
