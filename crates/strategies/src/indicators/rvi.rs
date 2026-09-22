use anyhow::Result;
use polars::prelude::*;
use std::collections::VecDeque;

/// Calculate Relative Vigor Index (RVI)
///
/// # Arguments
/// * `data` - DataFrame with "open", "high", "low", "close" columns
/// * `period` - Lookback period for SMA smoothing
///
/// # Returns
/// DataFrame containing "rvi" and "rvi_signal" columns.
pub fn calculate(data: &DataFrame, period: usize) -> Result<DataFrame> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    let open = data.column("open")?.f64()?;
    let high = data.column("high")?.f64()?;
    let low = data.column("low")?.f64()?;
    let close = data.column("close")?.f64()?;

    let len = close.len();

    let mut rvi_values = vec![None; len];
    let mut signal_values = vec![None; len];

    let mut numerator_vals = vec![0.0f64; len];
    let mut denominator_vals = vec![0.0f64; len];

    // Compute Num & Den for each period i >= 3
    for i in 3..len {
        let (o0, h0, l0, c0) = (open.get(i), high.get(i), low.get(i), close.get(i));
        let (o1, h1, l1, c1) = (
            open.get(i - 1),
            high.get(i - 1),
            low.get(i - 1),
            close.get(i - 1),
        );
        let (o2, h2, l2, c2) = (
            open.get(i - 2),
            high.get(i - 2),
            low.get(i - 2),
            close.get(i - 2),
        );
        let (o3, h3, l3, c3) = (
            open.get(i - 3),
            high.get(i - 3),
            low.get(i - 3),
            close.get(i - 3),
        );

        if let (
            Some(o0),
            Some(h0),
            Some(l0),
            Some(c0),
            Some(o1),
            Some(h1),
            Some(l1),
            Some(c1),
            Some(o2),
            Some(h2),
            Some(l2),
            Some(c2),
            Some(o3),
            Some(h3),
            Some(l3),
            Some(c3),
        ) = (
            o0, h0, l0, c0, o1, h1, l1, c1, o2, h2, l2, c2, o3, h3, l3, c3,
        ) {
            let o0d = if o0.is_finite() { o0 } else { 0.0 };
            let h0d = if h0.is_finite() { h0 } else { 0.0 };
            let l0d = if l0.is_finite() { l0 } else { 0.0 };
            let c0d = if c0.is_finite() { c0 } else { 0.0 };

            let o1d = if o1.is_finite() { o1 } else { 0.0 };
            let h1d = if h1.is_finite() { h1 } else { 0.0 };
            let l1d = if l1.is_finite() { l1 } else { 0.0 };
            let c1d = if c1.is_finite() { c1 } else { 0.0 };

            let o2d = if o2.is_finite() { o2 } else { 0.0 };
            let h2d = if h2.is_finite() { h2 } else { 0.0 };
            let l2d = if l2.is_finite() { l2 } else { 0.0 };
            let c2d = if c2.is_finite() { c2 } else { 0.0 };

            let o3d = if o3.is_finite() { o3 } else { 0.0 };
            let h3d = if h3.is_finite() { h3 } else { 0.0 };
            let l3d = if l3.is_finite() { l3 } else { 0.0 };
            let c3d = if c3.is_finite() { c3 } else { 0.0 };

            let two = 2.0f64;
            let six = 6.0f64;

            let a = c0d - o0d;
            let b = c1d - o1d;
            let c = c2d - o2d;
            let d = c3d - o3d;
            let num = (a + (two * b) + (two * c) + d) / six;
            numerator_vals[i] = num;

            let e = h0d - l0d;
            let f = h1d - l1d;
            let g = h2d - l2d;
            let h = h3d - l3d;
            let den = (e + (two * f) + (two * g) + h) / six;
            denominator_vals[i] = den;
        }
    }

    let period_f = period as f64;

    // SMA of Numerator and SMA of Denominator
    let mut num_window: VecDeque<f64> = VecDeque::with_capacity(period);
    let mut den_window: VecDeque<f64> = VecDeque::with_capacity(period);
    let mut num_sum = 0.0f64;
    let mut den_sum = 0.0f64;

    // To compute RVI at index i, we need SMA of num up to i, SMA of den up to i.
    // They are valid only for i >= 3 + period - 1
    for i in 3..len {
        let n = numerator_vals[i];
        let d = denominator_vals[i];

        num_window.push_back(n);
        num_sum += n;

        den_window.push_back(d);
        den_sum += d;

        if num_window.len() > period {
            if let Some(old_n) = num_window.pop_front() {
                num_sum -= old_n;
            }
            if let Some(old_d) = den_window.pop_front() {
                den_sum -= old_d;
            }
        }

        if num_window.len() == period {
            let num_sma = num_sum / period_f;
            let den_sma = den_sum / period_f;

            // Avoid division by zero
            if den_sma != 0.0 {
                let rvi = num_sma / den_sma;
                rvi_values[i] = Some(rvi);
            } else {
                rvi_values[i] = Some(0.0);
            }
        }
    }

    // Signal Line Calculation
    // Signal Line = (RVI + 2*RVI_1 + 2*RVI_2 + RVI_3) / 6
    for i in 3..len {
        if let (Some(r0), Some(r1), Some(r2), Some(r3)) = (
            rvi_values[i],
            rvi_values[i - 1],
            rvi_values[i - 2],
            rvi_values[i - 3],
        ) {
            let sig = (r0 + 2.0 * r1 + 2.0 * r2 + r3) / 6.0;
            signal_values[i] = Some(sig);
        }
    }

    let rvi_series = Series::new("rvi", rvi_values);
    let signal_series = Series::new("rvi_signal", signal_values);
    let df = DataFrame::new(vec![rvi_series, signal_series])?;

    Ok(df)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_rvi_calculation() -> Result<()> {
        let df = df!(
            "open"  => &[10.0, 10.5, 11.0, 11.5, 12.0, 12.5, 13.0, 13.5, 14.0],
            "high"  => &[11.0, 11.5, 12.0, 12.5, 13.0, 13.5, 14.0, 14.5, 15.0],
            "low"   => &[9.0,   9.5, 10.0, 10.5, 11.0, 11.5, 12.0, 12.5, 13.0],
            "close" => &[10.8, 11.2, 11.8, 12.2, 12.8, 13.2, 13.8, 14.2, 14.8]
        )?;

        // Period 2
        let result = calculate(&df, 2)?;
        assert_eq!(result.height(), 9);

        let rvi = result.column("rvi")?.f64()?;
        let sig = result.column("rvi_signal")?.f64()?;

        // Indices 0, 1, 2, 3 have numerator/denominator
        // index 3: num, den computed. num_window length 1.
        // index 4: num, den computed. num_window length 2. rvi[4] exists.
        assert!(rvi.get(0).is_none());
        assert!(rvi.get(3).is_none());
        assert!(rvi.get(4).is_some());

        // Signal requires 4 RVI values (i, i-1, i-2, i-3).
        // RVI starts at 4.
        // RVI[4], RVI[5], RVI[6], RVI[7] exist.
        // Signal[7] can be calculated.
        assert!(sig.get(6).is_none());
        assert!(sig.get(7).is_some());

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        Ok(())
    }
}
