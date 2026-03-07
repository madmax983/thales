use polars::prelude::*;

/// Calculates the Exponential Moving Average (EMA) directly from a Series.
/// Matches the logic in `ema::calculate` but operates on a Series for composability.
fn series_ema(series: &Series, period: usize) -> Result<Series, PolarsError> {
    if period == 0 || series.is_empty() {
        return Ok(Series::new_empty(series.name(), &DataType::Float64));
    }

    let chunked = series.f64()?;
    let len = chunked.len();
    let mut ema_values: Vec<Option<f64>> = Vec::with_capacity(len);

    let k = 2.0 / (period as f64 + 1.0);
    let mut count = 0;
    let mut window_sum = 0.0;
    let mut prev_ema: Option<f64> = None;

    for i in 0..len {
        if let Some(val) = chunked.get(i) {
            if count < period {
                window_sum += val;
                count += 1;

                if count == period {
                    let seed = window_sum / (period as f64);
                    ema_values.push(Some(seed));
                    prev_ema = Some(seed);
                } else {
                    ema_values.push(None);
                }
            } else if let Some(prev) = prev_ema {
                let ema = (val * k) + (prev * (1.0 - k));
                ema_values.push(Some(ema));
                prev_ema = Some(ema);
            } else {
                ema_values.push(None);
            }
        } else {
            ema_values.push(None);
            count = 0;
            window_sum = 0.0;
            prev_ema = None;
        }
    }

    Ok(Series::new(series.name(), ema_values))
}

/// Calculates the Triple Exponential Moving Average (TEMA).
///
/// TEMA = (3 * EMA1) - (3 * EMA2) + EMA3
/// where:
/// EMA1 = EMA(price)
/// EMA2 = EMA(EMA1)
/// EMA3 = EMA(EMA2)
///
/// Returns a Series of `f64`.
pub fn tema(series: &Series, period: usize) -> Result<Series, PolarsError> {
    if period == 0 || series.is_empty() {
        return Ok(Series::new_empty(series.name(), &DataType::Float64));
    }

    let ema1 = series_ema(series, period)?;
    let ema2 = series_ema(&ema1, period)?;
    let ema3 = series_ema(&ema2, period)?;

    let ema1_chunked = ema1.f64()?;
    let ema2_chunked = ema2.f64()?;
    let ema3_chunked = ema3.f64()?;

    let len = series.len();

    let values: Vec<Option<f64>> = (0..len)
        .map(|i| {
            let e1 = ema1_chunked.get(i);
            let e2 = ema2_chunked.get(i);
            let e3 = ema3_chunked.get(i);

            if let (Some(e1_val), Some(e2_val), Some(e3_val)) = (e1, e2, e3) {
                Some((3.0 * e1_val) - (3.0 * e2_val) + e3_val)
            } else {
                None
            }
        })
        .collect();

    Ok(Series::new(series.name(), values))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tema_basic() -> Result<(), PolarsError> {
        let s = Series::new(
            "close".into(),
            &[10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0],
        );
        let t = tema(&s, 2)?;
        assert_eq!(t.len(), 10);

        let values = t.f64()?;
        // EMA1 takes 2 elements to compute first value (index 1)
        // EMA2 takes 2 elements of EMA1 to compute first value (index 2)
        // EMA3 takes 2 elements of EMA2 to compute first value (index 3)
        // So first TEMA value is at index 3
        assert!(values.get(0).is_none());
        assert!(values.get(1).is_none());
        assert!(values.get(2).is_none());
        assert!(values.get(3).is_some());

        // Ensure values are populated correctly
        assert!(values.get(4).unwrap() > 0.0);

        Ok(())
    }

    #[test]
    fn test_tema_empty() -> Result<(), PolarsError> {
        let s = Series::new_empty("close".into(), &DataType::Float64);
        let t = tema(&s, 5)?;
        assert_eq!(t.len(), 0);
        Ok(())
    }
}
