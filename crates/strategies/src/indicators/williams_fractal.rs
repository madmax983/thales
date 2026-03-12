use anyhow::Result;
use polars::prelude::*;

/// Calculates Williams Fractals.
///
/// A bullish (down) fractal is a low surrounded by `window` higher lows on both sides.
/// A bearish (up) fractal is a high surrounded by `window` lower highs on both sides.
///
/// # Arguments
///
/// * `data` - DataFrame containing "high" and "low" columns.
/// * `window` - Number of bars on each side to define a fractal (typically 2).
///
/// # Returns
///
/// A tuple of `(bullish_fractal_series, bearish_fractal_series)` where each is a boolean Series.
pub fn calculate(data: &DataFrame, window: usize) -> Result<(Series, Series)> {
    let high = data.column("high")?.f64()?;
    let low = data.column("low")?.f64()?;

    let len = data.height();
    let mut bullish_fractal = vec![Some(false); len];
    let mut bearish_fractal = vec![Some(false); len];

    if len <= window * 2 {
        return Ok((
            Series::new("bullish_fractal", bullish_fractal),
            Series::new("bearish_fractal", bearish_fractal),
        ));
    }

    for i in window..(len - window) {
        if let Some(current_high) = high.get(i) {
            let mut is_bearish = true;
            for j in 1..=window {
                if let (Some(prev_high), Some(next_high)) = (high.get(i - j), high.get(i + j)) {
                    // Bearish fractal: current high is strictly higher than neighbors
                    if current_high <= prev_high || current_high <= next_high {
                        is_bearish = false;
                        break;
                    }
                } else {
                    is_bearish = false;
                    break;
                }
            }
            if is_bearish {
                bearish_fractal[i] = Some(true);
            }
        }

        if let Some(current_low) = low.get(i) {
            let mut is_bullish = true;
            for j in 1..=window {
                if let (Some(prev_low), Some(next_low)) = (low.get(i - j), low.get(i + j)) {
                    // Bullish fractal: current low is strictly lower than neighbors
                    if current_low >= prev_low || current_low >= next_low {
                        is_bullish = false;
                        break;
                    }
                } else {
                    is_bullish = false;
                    break;
                }
            }
            if is_bullish {
                bullish_fractal[i] = Some(true);
            }
        }
    }

    Ok((
        Series::new("bullish_fractal", bullish_fractal),
        Series::new("bearish_fractal", bearish_fractal),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_williams_fractal() -> Result<()> {
        let df = df!(
            "high" => &[1.0, 2.0, 3.0, 2.0, 1.0],
            "low" => &[3.0, 2.0, 1.0, 2.0, 3.0],
            "close" => &[2.0, 2.0, 2.0, 2.0, 2.0]
        )?;

        let (bullish, bearish) = calculate(&df, 2)?;
        let bull_arr = bullish.bool()?;
        let bear_arr = bearish.bool()?;

        // Bearish fractal expects high to be higher than 2 neighbors on each side
        // At index 2: high is 3.0, neighbors are 1.0, 2.0 and 2.0, 1.0. 3.0 is highest!
        assert_eq!(bear_arr.get(2), Some(true));

        // Bullish fractal expects low to be lower than 2 neighbors on each side
        // At index 2: low is 1.0, neighbors are 3.0, 2.0 and 2.0, 3.0. 1.0 is lowest!
        assert_eq!(bull_arr.get(2), Some(true));

        // Other indices should be false
        assert_eq!(bear_arr.get(0), Some(false));
        assert_eq!(bull_arr.get(1), Some(false));

        Ok(())
    }

    #[test]
    fn test_williams_fractal_empty_or_small() -> Result<()> {
        let df = df!(
            "high" => &[1.0, 2.0],
            "low" => &[1.0, 2.0]
        )?;

        let (bullish, bearish) = calculate(&df, 2)?;
        assert_eq!(bullish.len(), 2);
        assert_eq!(bearish.len(), 2);

        Ok(())
    }
}
