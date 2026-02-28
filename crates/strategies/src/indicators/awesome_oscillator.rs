use anyhow::{anyhow, Result};
use polars::prelude::*;

/// Calculates the Awesome Oscillator (AO)
///
/// AO = SMA((High + Low) / 2, fast_period) - SMA((High + Low) / 2, slow_period)
/// Default periods: 5 and 34.
pub fn calculate(data: &DataFrame, fast_period: usize, slow_period: usize) -> Result<Series> {
    if !data.get_column_names().contains(&"high") || !data.get_column_names().contains(&"low") {
        return Err(anyhow!("DataFrame must contain 'high' and 'low' columns"));
    }

    let high = data.column("high")?;
    let low = data.column("low")?;

    let high_f64 = high.f64()?;
    let low_f64 = low.f64()?;

    // Calculate median price
    let median_price: Series = high_f64
        .into_iter()
        .zip(low_f64.into_iter())
        .map(|(h, l)| {
            if let (Some(h_val), Some(l_val)) = (h, l) {
                Some((h_val + l_val) / 2.0)
            } else {
                None
            }
        })
        .collect();

    // Create a temporary DataFrame to calculate SMA
    let temp_df = DataFrame::new(vec![Series::new("close", median_price)])?;

    // Use existing SMA calculator, but it expects "close" column
    let fast_sma = crate::indicators::sma::calculate(&temp_df, fast_period)?;
    let slow_sma = crate::indicators::sma::calculate(&temp_df, slow_period)?;

    let fast_f64 = fast_sma.f64()?;
    let slow_f64 = slow_sma.f64()?;

    let ao_values: Series = fast_f64
        .into_iter()
        .zip(slow_f64.into_iter())
        .map(|(f, s)| {
            if let (Some(f_val), Some(s_val)) = (f, s) {
                Some(f_val - s_val)
            } else {
                None
            }
        })
        .collect();

    let ao_series = Series::new("awesome_oscillator", ao_values);
    Ok(ao_series)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_ao() -> Result<()> {
        let highs = vec![
            Some(10.0),
            Some(12.0),
            Some(14.0),
            Some(16.0),
            Some(18.0),
            Some(20.0),
        ];
        let lows = vec![
            Some(8.0),
            Some(10.0),
            Some(12.0),
            Some(14.0),
            Some(16.0),
            Some(18.0),
        ];

        let data = df!(
            "high" => highs,
            "low" => lows
        )?;

        // median: [9.0, 11.0, 13.0, 15.0, 17.0, 19.0]
        // SMA(2) fast:
        // i=0: None
        // i=1: (9+11)/2 = 10.0
        // i=2: (11+13)/2 = 12.0
        // i=3: (13+15)/2 = 14.0
        // i=4: (15+17)/2 = 16.0
        // i=5: (17+19)/2 = 18.0

        // SMA(4) slow:
        // i=0: None
        // i=1: None
        // i=2: None
        // i=3: (9+11+13+15)/4 = 12.0
        // i=4: (11+13+15+17)/4 = 14.0
        // i=5: (13+15+17+19)/4 = 16.0

        // AO = SMA(2) - SMA(4)
        // i=3: 14.0 - 12.0 = 2.0
        // i=4: 16.0 - 14.0 = 2.0
        // i=5: 18.0 - 16.0 = 2.0

        let ao = calculate(&data, 2, 4)?;
        let ao_f64 = ao.f64()?;

        assert_eq!(ao_f64.get(3), Some(2.0));
        assert_eq!(ao_f64.get(4), Some(2.0));
        assert_eq!(ao_f64.get(5), Some(2.0));

        Ok(())
    }

    #[test]
    fn test_missing_columns() {
        let data = df!("close" => &[1.0, 2.0]).unwrap();
        let result = calculate(&data, 5, 34);
        assert!(result.is_err());
    }
}
