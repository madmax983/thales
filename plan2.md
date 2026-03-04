Since `ema::calculate` takes a `&DataFrame` and expects a "close" column, I can't just pass the EMA Series back into it.
I could create a generic EMA function, or I can just choose a simpler strategy that only uses existing indicators or builds its own straightforwardly.

Let's do the **Rate of Change (ROC)** strategy... oh wait, ROC is already implemented.
What about **Commodity Channel Index (CCI)**? Already implemented (`cci_momentum.rs`).
**Awesome Oscillator (AO)**? Already implemented (`awesome_oscillator.rs`).
**Williams %R**? Already implemented (`williams_r.rs`).
**Vortex Indicator**? Already implemented (`vortex_breakout.rs`).
**Z-Score**? Already implemented (`zscore_mean_reversion.rs`).
**Connors RSI**? Already implemented (`connors_rsi_mean_reversion.rs`).
**Chaikin Money Flow**? Already implemented (`chaikin_money_flow.rs`).

Let's implement **Stochastic RSI (StochRSI)**.
StochRSI applies the Stochastic Oscillator formula to RSI values rather than price values.
Math:
StochRSI = (RSI - Lowest Low RSI) / (Highest High RSI - Lowest Low RSI)
Then typically smoothed with an SMA for %K and another SMA for %D.
Strategy: **StochRsiMeanReversion**
- Entry: StochRSI %K crosses above StochRSI %D in oversold region (e.g. < 20).
- Exit: StochRSI %K crosses below StochRSI %D in overbought region (e.g. > 80).

Let's check if StochRSI exists.
`crates/strategies/src/indicators/stoch_rsi.rs` does not exist in the list from `crates/strategies/src/indicators/mod.rs`.
Wait, another option is **MACD Histogram Reversal**?
Or **Gator Oscillator** (Bill Williams)?
Or **TSI (True Strength Index)**? I could build the True Strength Index indicator from scratch since it uses EMAs, I can just write the EMA logic inside the TSI indicator.
Or I can just stick to **StochRSI** which is very popular and effective.
Let's build **Stoch RSI**.
Wait, the user prompt is a generic template:
`# Trading Strategy Implementation Request`
`## Strategy Specification`
`**Name:** {{name}}`

Let's create **Stoch RSI Mean Reversion**.
Name: StochRsiMeanReversion
Description: Applies the Stochastic oscillator formula to the Relative Strength Index (RSI) to identify overbought and oversold conditions with greater sensitivity.
Rationale: Regular RSI can languish between 30 and 70 for extended periods. StochRSI is more sensitive and quickly identifies extremes in RSI itself.
