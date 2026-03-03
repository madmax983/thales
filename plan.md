1. **Research and Specify Strategy:**
   - I need to pick a new trading strategy that isn't currently implemented. The existing ones are (among others): BollingerBands, ElderRay, EmaCrossover, RsiMeanReversion, Macd, Supertrend, DonchianBreakout, ParabolicSar, KeltnerChannelBreakout, StochasticOscillator, AdxMomentum, IchimokuCloud, CciMomentum, ChaikinMoneyFlow, LinearRegressionTrend, ObvTrendFollowing, MoneyFlowIndex, ConnorsRsiMeanReversion, AwesomeOscillator, WilliamsR, VwmaCrossover, VwapReversion, VortexBreakout, ZScoreMeanReversion.
   - Let's implement **Aroon Oscillator**.
   - **Name:** `AroonOscillator`
   - **Type:** TrendFollowing
   - **Description:** Aroon Oscillator uses Aroon Up and Aroon Down to measure the strength of a trend. Aroon Up measures the number of periods since the highest high within the period, and Aroon Down measures periods since the lowest low. The oscillator is the difference between Up and Down.
   - **Entry:** Aroon Oscillator crosses above 0 (or a specific threshold like +50).
   - **Exit:** Aroon Oscillator crosses below 0 (or a specific threshold like -50).
   - **Position Sizing:** max/100.
   - **Risk Management:** Implement ATR based Stop Loss.

2. **Implement Indicator (`crates/strategies/src/indicators/aroon.rs`):**
   - Calculate Aroon Up and Aroon Down.
   - Calculate Aroon Oscillator.
   - In Polars, `rolling_argmax` or `rolling_argmin` can be used to find the index of highest/lowest, but Polars expressions are powerful enough to do it. Oh wait, `rolling_max` and finding when it equals the current can work, but really we need `rolling_argmax` / `rolling_argmin` which tells us how many periods ago the extreme occurred. Or we can just calculate it in Rust using a rolling window natively if Polars is tricky. But let's check Polars documentation or use a `.apply` if needed, or see how Donchian does it. (Actually, Donchian uses rolling_max/min, not argmax). We can just use an iterator in Rust on the `f64` slice for maximum efficiency and exactness since Polars `rolling_argmax` might be tricky. Let's write `aroon` calculation function in `crates/strategies/src/indicators/aroon.rs`.

3. **Implement Strategy (`crates/strategies/src/aroon_oscillator.rs`):**
   - Define `AroonOscillatorConfig` (period, buy_threshold, sell_threshold, stop_loss_atr_mult, atr_period).
   - Implement `Strategy` trait for `AroonOscillator`.
   - Write unit tests for signal generation and parameter validation, including mock data.

4. **Integrate Strategy:**
   - Add `pub mod aroon_oscillator;` to `crates/strategies/src/lib.rs`.
   - Add indicator `pub mod aroon;` to `crates/strategies/src/indicators/mod.rs`.
   - Add to `crates/cli/src/strategy_factory.rs` under `create_strategy` and `list_strategies`.
   - Add `AroonOscillator` to `TREND_FOLLOWING_STRATEGIES` in `execute_cycle.py`.
   - Update `strategies.md` with the specification.

5. **Tests & Pre-commit Check:**
   - Run `cargo test -p strategies`
   - Run `cargo test -p thales-cli`
   - Run `python -m unittest test_execute_cycle.py`
   - Ensure pre-commit steps are done using the `pre_commit_instructions` tool to guarantee testing, verification, review, and reflection.

6. **Submit:**
   - Submit the PR with changes.
