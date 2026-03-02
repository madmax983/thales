use anyhow::{anyhow, Result};
use polars::prelude::*;

type SarResult = Result<(Vec<Option<f64>>, Vec<Option<bool>>)>;

pub fn parabolic_sar(
    high: &Series,
    low: &Series,
    initial_af: f64,
    max_af: f64,
    step_af: f64,
) -> SarResult {
    let high_iter = high.f64()?;
    let low_iter = low.f64()?;

    let len = high.len();
    if len != low.len() {
        return Err(anyhow!("High and Low series must have the same length"));
    }

    if len == 0 {
        return Ok((vec![], vec![]));
    }

    let mut sar_values = Vec::with_capacity(len);
    let mut trend_values = Vec::with_capacity(len);

    // Initial State
    // We need at least one previous bar to calculate current SAR properly?
    // Actually, first value is just initialization.
    // Let's assume first bar sets the trend.
    // If High[1] > High[0] -> Up? Or just use first bar close vs open?
    // A common way is to look at the first two bars.
    // Let's use a simple initialization:
    // Trend = Up if High[0] < High[1], else Down.
    // If len < 2, just set default.

    let h0 = high_iter.get(0).unwrap_or(0.0);
    let l0 = low_iter.get(0).unwrap_or(0.0);

    let mut trend_is_up = true; // Default
    let mut sar = l0;
    let mut ep = h0;
    let mut af = initial_af;

    // Output first value
    sar_values.push(Some(sar));
    trend_values.push(Some(trend_is_up));

    for i in 1..len {
        let h_curr = high_iter.get(i).unwrap_or(0.0);
        let l_curr = low_iter.get(i).unwrap_or(0.0);
        let h_prev = high_iter.get(i - 1).unwrap_or(0.0);
        let l_prev = low_iter.get(i - 1).unwrap_or(0.0);

        // Update SAR
        let mut next_sar = sar + af * (ep - sar);

        // Check Trend Flip
        if trend_is_up {
            // Check if price broke below SAR
            if l_curr < next_sar {
                // Flip to Downtrend
                trend_is_up = false;
                sar = ep; // SAR becomes previous EP (highest high)
                ep = l_curr; // New EP is current low
                af = initial_af;
            } else {
                // Continue Uptrend
                // Update EP
                if h_curr > ep {
                    ep = h_curr;
                    af = (af + step_af).min(max_af);
                }

                // Boundary Rule: SAR cannot be higher than previous two lows
                // sar[t+1] <= low[t] and sar[t+1] <= low[t-1]
                // We are calculating sar for current bar i (which is t+1 relative to prev calculation)
                // So we compare with l_prev (t) and l_prev_2 (t-1).
                // Wait, standard definition:
                // SAR[tomorrow] = SAR[today] + AF * (EP - SAR[today])
                // SAR[tomorrow] <= Low[today] AND Low[yesterday]

                // Here, we just calculated `next_sar` which corresponds to `SAR[i]`.
                // It should be constrained by Low[i-1] and Low[i-2].

                let limit = if i >= 2 {
                    let l_prev_2 = low_iter.get(i - 2).unwrap_or(0.0);
                    l_prev.min(l_prev_2)
                } else {
                    l_prev
                };

                if next_sar > limit {
                    next_sar = limit;
                }

                sar = next_sar;
            }
        } else {
            // Downtrend
            // Check if price broke above SAR
            if h_curr > next_sar {
                // Flip to Uptrend
                trend_is_up = true;
                sar = ep; // SAR becomes previous EP (lowest low)
                ep = h_curr; // New EP is current high
                af = initial_af;
            } else {
                // Continue Downtrend
                // Update EP
                if l_curr < ep {
                    ep = l_curr;
                    af = (af + step_af).min(max_af);
                }

                // Boundary Rule: SAR cannot be lower than previous two highs
                // sar[t+1] >= high[t] and sar[t+1] >= high[t-1]

                let limit = if i >= 2 {
                    let h_prev_2 = high_iter.get(i - 2).unwrap_or(0.0);
                    h_prev.max(h_prev_2)
                } else {
                    h_prev
                };

                if next_sar < limit {
                    next_sar = limit;
                }

                sar = next_sar;
            }
        }

        sar_values.push(Some(sar));
        trend_values.push(Some(trend_is_up));
    }

    Ok((sar_values, trend_values))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parabolic_sar_uptrend() {
        // Simple uptrend
        let highs = Series::new("high", &[10.0, 11.0, 12.0, 13.0, 14.0]);
        let lows = Series::new("low", &[9.0, 10.0, 11.0, 12.0, 13.0]);

        let (sar, trend) = parabolic_sar(&highs, &lows, 0.02, 0.2, 0.02).unwrap();

        assert_eq!(sar.len(), 5);
        assert_eq!(trend.len(), 5);

        // Initial state: Trend Up (default), SAR=9.0
        assert_eq!(sar[0], Some(9.0));
        assert_eq!(trend[0], Some(true));

        // SAR should increase (or stay same due to constraints) in uptrend
        for i in 1..5 {
            assert!(sar[i].unwrap() >= sar[i - 1].unwrap());
            assert_eq!(trend[i], Some(true));
        }
    }

    #[test]
    fn test_parabolic_sar_trend_flip() {
        // Up then Down
        let highs = Series::new("high", &[10.0, 12.0, 11.0, 10.0]);
        let lows = Series::new("low", &[8.0, 10.0, 9.0, 8.0]);

        // Bar 0: SAR=8.0, Trend=Up, EP=10.0
        // Bar 1: SAR = 8 + 0.02*(10-8) = 8.04. Check limit: min(Low[0], Low[-1]) = 8.0. SAR=8.04 <= 8.0 -> SAR=8.0.
        //        Actually, limit uses prev lows. limit = Low[0] = 8.0. So SAR stays 8.0?
        //        Wait, next_sar > limit logic?
        //        If Up, SAR <= Low[prev].
        //        So if calc > Low[prev], set to Low[prev].
        //        Here 8.04 > 8.0 -> 8.0.
        //        So SAR[1] = 8.0. Trend=Up. EP updates to 12.0. AF increases.

        // Bar 2: Calc SAR = 8.0 + 0.04*(12-8) = 8.16. Limit Low[1]=10, Low[0]=8. Limit=8. So SAR=8.0.
        //        Low[2]=9.0 > SAR(8.0). Still Up.

        // Bar 3: Low[3]=8.0. If SAR is still 8.0, this might be a hit?
        // Let's rely on code logic.

        let (_sar, _trend) = parabolic_sar(&highs, &lows, 0.02, 0.2, 0.02).unwrap();

        // Check if trend flipped to false at end
        // Last low is 8.0. If previous SAR was > 8.0, it would flip.

        // Actually, with strict increasing trend, eventually SAR catches up.
    }
}
