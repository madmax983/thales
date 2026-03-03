use anyhow::Result;
use polars::prelude::*;

/// Calculates the Volume Weighted Average Price (VWAP) over a rolling window.
///
/// Typical Price = (High + Low + Close) / 3
/// VWAP = Sum(Typical Price * Volume) / Sum(Volume)
///
/// If `period` is provided, calculates a rolling VWAP.
/// If `period` is not provided (or is 0), calculates a cumulative VWAP
/// (often anchored to the start of a day or trading session).
/// In this implementation, we will stick to a rolling window for consistency
/// with other indicators in continuous time series data.
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    if period == 0 {
        return Err(anyhow::anyhow!("VWAP period must be greater than 0"));
    }

    let high = data.column("high")?.f64()?;
    let low = data.column("low")?.f64()?;
    let close = data.column("close")?.f64()?;
    let volume = data.column("volume")?.f64()?;

    let mut vwap_values = Vec::with_capacity(close.len());
    let mut typical_price_vol_sum = 0.0;
    let mut vol_sum = 0.0;

    for i in 0..close.len() {
        let h = high.get(i).unwrap_or(0.0);
        let l = low.get(i).unwrap_or(0.0);
        let c = close.get(i).unwrap_or(0.0);
        let v = volume.get(i).unwrap_or(0.0);

        let typical_price = (h + l + c) / 3.0;
        let tp_vol = typical_price * v;

        typical_price_vol_sum += tp_vol;
        vol_sum += v;

        // If we exceed the period, subtract the value that falls out of the window
        if i >= period {
            let out_idx = i - period;
            let out_h = high.get(out_idx).unwrap_or(0.0);
            let out_l = low.get(out_idx).unwrap_or(0.0);
            let out_c = close.get(out_idx).unwrap_or(0.0);
            let out_v = volume.get(out_idx).unwrap_or(0.0);

            let out_tp = (out_h + out_l + out_c) / 3.0;
            typical_price_vol_sum -= out_tp * out_v;
            vol_sum -= out_v;
        }

        if i < period - 1 {
            // Not enough data for the full period, but we can emit the running VWAP or None
            // For VWAP, emitting the running value is often useful (like an anchored VWAP).
            // Let's emit the running value.
            if vol_sum > 0.0 {
                vwap_values.push(Some(typical_price_vol_sum / vol_sum));
            } else {
                vwap_values.push(None);
            }
        } else {
            if vol_sum > 0.0 {
                vwap_values.push(Some(typical_price_vol_sum / vol_sum));
            } else {
                // To avoid division by zero if volume sum is zero
                vwap_values.push(None);
            }
        }
    }

    Ok(Series::new("vwap".into(), vwap_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_vwap_calculation() -> Result<()> {
        let df = df!(
            "high"   => &[10.0, 12.0, 14.0, 12.0, 10.0],
            "low"    => &[8.0,  10.0, 12.0, 10.0, 8.0],
            "close"  => &[9.0,  11.0, 13.0, 11.0, 9.0],
            "volume" => &[100.0, 200.0, 300.0, 200.0, 100.0]
        )?;

        // Typical Prices (TP):
        // i=0: (10+8+9)/3 = 9
        // i=1: (12+10+11)/3 = 11
        // i=2: (14+12+13)/3 = 13
        // i=3: (12+10+11)/3 = 11
        // i=4: (10+8+9)/3 = 9

        // TP * Vol:
        // i=0: 9 * 100 = 900
        // i=1: 11 * 200 = 2200
        // i=2: 13 * 300 = 3900
        // i=3: 11 * 200 = 2200
        // i=4: 9 * 100 = 900

        // Rolling VWAP (period=3):
        // i=0: sum(TP*V)=900, sum(V)=100 -> 9.0
        // i=1: sum(TP*V)=3100, sum(V)=300 -> 10.333...
        // i=2: sum(TP*V)=7000, sum(V)=600 -> 11.666... (Full window)
        // i=3: sum(TP*V)=2200+3900+2200=8300, sum(V)=200+300+200=700 -> 11.857...
        // i=4: sum(TP*V)=3900+2200+900=7000, sum(V)=300+200+100=600 -> 11.666...

        let vwap = calculate(&df, 3)?;
        let vwap_f64 = vwap.f64()?;

        assert_eq!(vwap_f64.len(), 5);
        assert!((vwap_f64.get(0).unwrap() - 9.0).abs() < 1e-6);
        assert!((vwap_f64.get(1).unwrap() - 10.333333).abs() < 1e-6);
        assert!((vwap_f64.get(2).unwrap() - 11.666666).abs() < 1e-6);
        assert!((vwap_f64.get(3).unwrap() - 11.857142).abs() < 1e-6);
        assert!((vwap_f64.get(4).unwrap() - 11.666666).abs() < 1e-6);

        Ok(())
    }
}
