use anyhow::Result;
use polars::prelude::*;

use crate::indicators::{bollinger_bands, keltner_channels, sma};

/// Calculate TTM Squeeze
///
/// Returns a tuple of two Series:
/// 1. `squeeze`: A boolean Series where `true` means the market is in a squeeze (Bollinger Bands are inside Keltner Channels).
/// 2. `momentum`: A f64 Series representing the momentum (e.g., Close - SMA(Close)).
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", "close" columns
/// * `bb_period` - Lookback period for Bollinger Bands
/// * `bb_std_dev` - Standard deviation multiplier for Bollinger Bands
/// * `kc_period` - Lookback period for Keltner Channels
/// * `kc_mult` - ATR multiplier for Keltner Channels
/// * `mom_period` - Lookback period for Momentum (SMA of close)
pub fn calculate(
    data: &DataFrame,
    bb_period: usize,
    bb_std_dev: f64,
    kc_period: usize,
    kc_mult: f64,
    mom_period: usize,
) -> Result<(Series, Series)> {
    if data.height() == 0 {
        return Ok((
            Series::new("squeeze", Vec::<Option<bool>>::new()),
            Series::new("momentum", Vec::<Option<f64>>::new()),
        ));
    }

    // Calculate Bollinger Bands
    let (bb_lower, _, bb_upper) = bollinger_bands::calculate(data, bb_period, bb_std_dev)?;
    let bb_lower_arr = bb_lower.f64()?;
    let bb_upper_arr = bb_upper.f64()?;

    // Calculate Keltner Channels
    // The keltner_channels::calculate function signature is:
    // pub fn calculate(data: &DataFrame, ema_period: usize, atr_period: usize, atr_multiplier: f64)
    // We will use kc_period for both ema_period and atr_period for simplicity.
    let (kc_lower, _, kc_upper) = keltner_channels::calculate(data, kc_period, kc_period, kc_mult)?;
    let kc_lower_arr = kc_lower.f64()?;
    let kc_upper_arr = kc_upper.f64()?;

    // Calculate Momentum (Close - SMA(Close))
    let close = data.column("close")?.f64()?;
    let sma_close = sma::calculate(data, mom_period)?;
    let sma_close_arr = sma_close.f64()?;

    let mut squeeze_vals = Vec::with_capacity(data.height());
    let mut momentum_vals = Vec::with_capacity(data.height());

    for i in 0..data.height() {
        // Squeeze logic
        let sqz = match (
            bb_lower_arr.get(i),
            bb_upper_arr.get(i),
            kc_lower_arr.get(i),
            kc_upper_arr.get(i),
        ) {
            (Some(bb_l), Some(bb_u), Some(kc_l), Some(kc_u)) => {
                // Squeeze is ON if Bollinger Bands are strictly inside Keltner Channels
                Some(bb_l > kc_l && bb_u < kc_u)
            }
            _ => None,
        };
        squeeze_vals.push(sqz);

        // Momentum logic
        let mom = match (close.get(i), sma_close_arr.get(i)) {
            (Some(c), Some(s)) => Some(c - s),
            _ => None,
        };
        momentum_vals.push(mom);
    }

    let squeeze_series = Series::new("squeeze", squeeze_vals);
    let momentum_series = Series::new("momentum", momentum_vals);

    Ok((squeeze_series, momentum_series))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_ttm_squeeze_calculation() -> Result<()> {
        let df = df!(
            "open"  => &[10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0],
            "high"  => &[12.0, 12.0, 12.0, 12.0, 12.0, 12.0, 12.0],
            "low"   => &[8.0, 8.0, 8.0, 8.0, 8.0, 8.0, 8.0],
            "close" => &[10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0]
        )?;

        // Short periods for testing
        let (squeeze, momentum) = calculate(&df, 3, 2.0, 3, 1.5, 3)?;

        assert_eq!(squeeze.len(), 7);
        assert_eq!(momentum.len(), 7);

        // Check if the values are correct
        let sq_arr = squeeze.bool()?;
        let mom_arr = momentum.f64()?;

        // The first 2 elements should be None because of the lookback period
        assert_eq!(sq_arr.get(0), None);
        assert_eq!(sq_arr.get(1), None);
        assert_eq!(sq_arr.get(2), Some(true)); // Constant price -> 0 std dev -> BB width = 0 -> inside KC (width > 0)

        assert_eq!(mom_arr.get(0), None);
        assert_eq!(mom_arr.get(1), None);
        assert_eq!(mom_arr.get(2), Some(0.0)); // Close = SMA

        Ok(())
    }
}
