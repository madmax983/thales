use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate On-Balance Volume (OBV)
///
/// # Arguments
/// * `data` - DataFrame with "close" and "volume" columns
///
/// # Returns
/// Series with OBV values.
pub fn calculate(data: &DataFrame) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }

    let close = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .f64()
        .context("Close column must be numeric (f64)")?;

    let volume = data
        .column("volume")
        .context("DataFrame must contain 'volume' column")?
        .f64()
        .context("Volume column must be numeric (f64)")?;

    let mut obv_values: Vec<Option<f64>> = Vec::with_capacity(close.len());
    let mut current_obv = Decimal::ZERO;
    let mut initialized = false;

    for i in 0..close.len() {
        let close_val = close.get(i);
        let vol_val = volume.get(i);

        if let (Some(c), Some(v)) = (close_val, vol_val) {
            let c_dec = Decimal::from_f64_retain(c).unwrap_or(Decimal::ZERO);
            let v_dec = Decimal::from_f64_retain(v).unwrap_or(Decimal::ZERO);

            if !initialized {
                // First valid point. OBV starts at 0.
                initialized = true;
                obv_values.push(Some(current_obv.to_f64().unwrap_or(0.0)));
            } else {
                // We need previous VALID close.
                // But simplified: check i-1. If i-1 was None, we can't determine direction.
                // However, standard Polars series usually don't have gaps in Close in this context.
                // If previous was None, we treat as no change or skip?
                // Let's check strict i-1.

                let prev_close_opt = close.get(i - 1);
                if let Some(prev_c) = prev_close_opt {
                    let prev_c_dec = Decimal::from_f64_retain(prev_c).unwrap_or(Decimal::ZERO);

                    if c_dec > prev_c_dec {
                        current_obv += v_dec;
                    } else if c_dec < prev_c_dec {
                        current_obv -= v_dec;
                    }
                    // If equal, no change

                    obv_values.push(Some(current_obv.to_f64().unwrap_or(0.0)));
                } else {
                    // Previous close missing, cannot determine direction.
                    // Push current OBV (flat) or None?
                    // Let's push None to indicate uncertainty, or just hold previous?
                    // If we hold previous, we assume no change.
                    // Let's hold previous.
                    obv_values.push(Some(current_obv.to_f64().unwrap_or(0.0)));
                }
            }
        } else {
            obv_values.push(None);
        }
    }

    let s = Series::new("obv", obv_values);
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_obv_calculation() -> Result<()> {
        // Price: 10, 11 (Up), 10.5 (Down), 10.5 (Equal), 12 (Up)
        // Vol:   100, 50,     20,         10,          100
        // OBV:   0
        //        0 + 50 = 50
        //        50 - 20 = 30
        //        30 (no change) = 30
        //        30 + 100 = 130

        let df = df!(
            "close" => &[10.0, 11.0, 10.5, 10.5, 12.0],
            "volume" => &[100.0, 50.0, 20.0, 10.0, 100.0]
        )?;

        let result = calculate(&df)?;
        let out = result.f64()?;

        assert_eq!(out.get(0), Some(0.0));
        assert_eq!(out.get(1), Some(50.0));
        assert_eq!(out.get(2), Some(30.0));
        assert_eq!(out.get(3), Some(30.0));
        assert_eq!(out.get(4), Some(130.0));

        Ok(())
    }

    #[test]
    fn test_missing_data() -> Result<()> {
        // Close: 10, null, 12
        // Vol:   100, 100, 100
        // OBV:   0,   null, 0 (since prev 10 vs 12 -> Up -> 0+100=100? No, prev was null, so we held?)

        // Let's trace my logic:
        // i=0: 10, 100 -> Init 0.
        // i=1: null -> Push None.
        // i=2: 12, 100 -> Prev (i=1) is null. Code says: if prev_close_opt is Some...
        // Wait, close.get(i-1) returns None if i-1 is null in Polars?
        // Yes, get(i) returns Option<f64>.

        // So at i=2, prev_close_opt is None.
        // My logic: else { obv_values.push(current) }
        // So OBV remains 0.

        // Need to ensure it's null in Polars Series.
        // Helper to make series with nulls
        let s_close = Series::new("close", &[Some(10.0), None, Some(12.0)]);
        let s_vol = Series::new("volume", &[100.0, 100.0, 100.0]);
        let df = DataFrame::new(vec![s_close, s_vol])?;

        let result = calculate(&df)?;
        let out = result.f64()?;

        assert_eq!(out.get(0), Some(0.0));
        assert!(out.get(1).is_none());
        assert_eq!(out.get(2), Some(0.0)); // Held previous value because comparison failed

        Ok(())
    }
}
