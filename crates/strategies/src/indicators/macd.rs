use super::ema;
use anyhow::{Context, Result};
use polars::prelude::*;

/// Calculate Moving Average Convergence Divergence (MACD)
///
/// MACD is a trend-following momentum indicator that shows the relationship between two moving averages of a security’s price.
///
/// # Arguments
/// * `data` - DataFrame with a `close` column.
/// * `fast_period` - Fast EMA period (commonly 12).
/// * `slow_period` - Slow EMA period (commonly 26).
/// * `signal_period` - Signal Line EMA period (commonly 9).
///
/// # Returns
/// A tuple containing `(MACD Line, Signal Line, Histogram)` as Polars `Series`.
///
/// # Panics
/// This function does not panic. It will return an error if the `close` column is missing, or if any of the underlying EMA calculations fail.
///
/// # Edge Cases
/// - **Insufficient Data:** If the input DataFrame has fewer rows than the slowest period, the early values will be `None`.
/// - **Zero Periods:** If any period is set to 0, the underlying EMA calculation will return an error.
///
/// # Examples
/// ```rust
/// use polars::prelude::*;
/// use strategies::indicators::macd;
///
/// // Create sample price data
/// let df = df!(
///     "close" => &[10.0, 11.0, 12.0, 13.0, 14.0, 15.0]
/// ).unwrap();
///
/// // Calculate MACD with short periods for testing
/// let (macd, signal, hist) = macd::calculate(&df, 2, 4, 3).unwrap();
///
/// assert_eq!(macd.len(), 6);
/// assert_eq!(signal.len(), 6);
/// assert_eq!(hist.len(), 6);
/// ```
pub fn calculate(
    data: &DataFrame,
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
) -> Result<(Series, Series, Series)> {
    // 1. Calculate Fast EMA
    let fast_ema = ema::calculate(data, fast_period).context("Failed to calculate Fast EMA")?;

    // 2. Calculate Slow EMA
    let slow_ema = ema::calculate(data, slow_period).context("Failed to calculate Slow EMA")?;

    // 3. Calculate MACD Line = Fast EMA - Slow EMA
    let macd_line = fast_ema - slow_ema;
    let macd_line = macd_line.context("Failed to compute MACD Line")?;

    // 4. Calculate Signal Line = EMA(MACD Line, signal_period)
    // We need to create a temporary DataFrame with "close" column from MACD Line
    // to reuse ema::calculate.
    let temp_df = DataFrame::new(vec![Series::new("close", &macd_line)])?;
    let signal_line =
        ema::calculate(&temp_df, signal_period).context("Failed to calculate Signal Line")?;

    // 5. Calculate Histogram = MACD Line - Signal Line
    let histogram = &macd_line - &signal_line;
    let histogram = histogram.context("Failed to compute Histogram")?;

    Ok((macd_line, signal_line, histogram))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_macd_calculation() -> Result<()> {
        // Create a dummy DataFrame with predictable values
        // We'll use a simple linear trend for easier verification
        // Fast=2, Slow=4, Signal=2
        // Close: 10, 12, 14, 16, 18, 20

        let df = df!(
            "close" => &[10.0, 12.0, 14.0, 16.0, 18.0, 20.0]
        )?;

        // Fast EMA (2):
        // 0: None
        // 1: (10+12)/2 = 11.0 (seed)
        // 2: (14*2/3) + (11*1/3) = 9.33 + 3.66 = 13.0
        // 3: (16*2/3) + (13*1/3) = 10.66 + 4.33 = 15.0
        // 4: (18*2/3) + (15*1/3) = 12.0 + 5.0 = 17.0
        // 5: (20*2/3) + (17*1/3) = 13.33 + 5.66 = 19.0

        // Slow EMA (4):
        // 0: None
        // 1: None
        // 2: None
        // 3: (10+12+14+16)/4 = 13.0 (seed)
        // 4: (18*2/5) + (13*3/5) = 7.2 + 7.8 = 15.0
        // 5: (20*2/5) + (15*3/5) = 8.0 + 9.0 = 17.0

        // MACD Line = Fast - Slow
        // 0, 1, 2: None (due to Slow EMA)
        // 3: 15.0 - 13.0 = 2.0
        // 4: 17.0 - 15.0 = 2.0
        // 5: 19.0 - 17.0 = 2.0

        // Signal Line (2) of MACD Line
        // MACD: [None, None, None, 2.0, 2.0, 2.0]
        // 0..2: None
        // 3: None (need 2 valid points for SMA seed if period=2? No, period is count of valid points)
        // Wait, ema::calculate skips Nones.
        // It will see 2.0 at index 3. Count=1.
        // It will see 2.0 at index 4. Count=2. SMA(2.0, 2.0) = 2.0. Seed.
        // It will see 2.0 at index 5. EMA(2.0) = (2.0 * 2/3) + (2.0 * 1/3) = 2.0.

        // Histogram = MACD - Signal
        // 4: 2.0 - 2.0 = 0.0
        // 5: 2.0 - 2.0 = 0.0

        let (macd, signal, hist) = calculate(&df, 2, 4, 2)?;

        let macd_vals = macd.f64()?;
        let signal_vals = signal.f64()?;
        let hist_vals = hist.f64()?;

        // Helper for float comparison
        let assert_approx = |a: Option<f64>, b: Option<f64>| match (a, b) {
            (Some(v1), Some(v2)) => assert!((v1 - v2).abs() < 1e-10, "{} != {}", v1, v2),
            (None, None) => {}
            _ => panic!("Mismatch: {:?} != {:?}", a, b),
        };

        // Verify MACD Line
        assert!(macd_vals.get(0).is_none());
        assert!(macd_vals.get(1).is_none());
        assert!(macd_vals.get(2).is_none());
        assert_approx(macd_vals.get(3), Some(2.0));
        assert_approx(macd_vals.get(4), Some(2.0));
        assert_approx(macd_vals.get(5), Some(2.0));

        // Verify Signal Line
        assert!(signal_vals.get(3).is_none());
        assert_approx(signal_vals.get(4), Some(2.0));
        assert_approx(signal_vals.get(5), Some(2.0));

        // Verify Histogram
        assert!(hist_vals.get(3).is_none());
        assert_approx(hist_vals.get(4), Some(0.0));
        assert_approx(hist_vals.get(5), Some(0.0));

        Ok(())
    }
}
