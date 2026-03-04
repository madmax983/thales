1. **Understand Requirements**:
    - Strategy Name: `WilliamsR` (Already partially exists or is asked to be created? Wait, the template has `{{name}}`, `{{description}}`, but the memory says "Williams R" is an existing strategy or maybe I need to implement what the user asks. Wait, the user's prompt contains placeholders: `{{name}}`, `{{description}}`, `{{rationale}}`, etc., which I am supposed to fill in with my own strategy because the prompt says: "- Do web research and create a new strategy.")
    - Ah, the prompt says:
      "Do web research and create a new strategy."
      "Use Polars for data analysis and signal generation"
      "Strategy Type: {{strategy_type}}"
    - Wait, the prompt has literal placeholders `{{name}}`, etc.
    - Let me check what the user actually said. The user prompt literally contains `{{name}}`, `{{description}}`, `{{rationale}}`, `{{strategy_type}}`, `{{entry_conditions}}`, `{{exit_conditions}}`, `{{position_sizing}}`, `{{strategy_name}}`, `{{context}}`. It asks me to do web research and create a new strategy.
    - Okay, I need to invent a new strategy, fill out these details mentally or in the code, create the Rust implementation in `crates/strategies/src/`, register it in `strategy_factory.rs` and `lib.rs`, add it to `execute_cycle.py`, and update `strategies.md`.

Let me choose a well-known strategy that isn't implemented yet.
Checking existing strategies:
`BollingerBands`, `EmaCrossover`, `RsiMeanReversion`, `Macd`, `Supertrend`, `DonchianBreakout`, `ParabolicSar`, `KeltnerChannelBreakout`, `StochasticOscillator`, `AdxMomentum`, `IchimokuCloud`, `CciMomentum`, `LinearRegressionTrend`, `ObvTrendFollowing`, `MoneyFlowIndex`, `ConnorsRsiMeanReversion`, `AwesomeOscillator`, `WilliamsR`, `VwmaCrossover`, `VwapReversion`, `VortexBreakout`, `ZScoreMeanReversion`, `ChaikinMoneyFlow`, `ElderRay`, `AroonOscillator`, `ChandelierExit`, `RocMomentum`.

What's a good one that is missing?
- **TRIX** (Triple Exponential Average)
- **Coppock Curve**
- **Schaff Trend Cycle (STC)**
- **True Strength Index (TSI)**
- **Ultimate Oscillator**
- **Fisher Transform**
- **KAMA (Kaufman's Adaptive Moving Average)**
- **Gator Oscillator**
- **Fractal Chaos Bands**

Let's implement **True Strength Index (TSI)**.
TSI = 100 * ( EMA( EMA( Price Change, Long Period ), Short Period ) / EMA( EMA( |Price Change|, Long Period ), Short Period ) )
Or let's implement **Ultimate Oscillator**.

Let's go with **TRIX (Triple Exponential Average)**.
TRIX is a momentum oscillator that displays the percent rate of change of a triple exponentially smoothed moving average.
- Strategy Name: `TrixMomentum`
- Description: A momentum strategy based on the TRIX indicator, which uses a triple smoothed EMA to filter out insignificant price movements.
- Rationale: TRIX crossing above zero indicates bullish momentum, while crossing below zero indicates bearish momentum. Alternatively, TRIX crossing its signal line (an EMA of TRIX) generates signals. We will use the signal line crossover.

Wait, `EMA` is already available in `crates/strategies/src/indicators/ema.rs`. We can build `trix.rs` in indicators, then `trix_momentum.rs` in strategies.

Let's do **TripleEMA (TEMA)** or **TRIX**.
To keep it simple and robust, let's implement **TRIX**.
TRIX requires:
1. EMA1 = EMA(Close, Period)
2. EMA2 = EMA(EMA1, Period)
3. EMA3 = EMA(EMA2, Period)
4. TRIX = (EMA3 - Previous EMA3) / Previous EMA3 * 100
5. Signal Line = EMA(TRIX, Signal_Period)

Let's do **Fisher Transform**.
Fisher Transform:
1. Math: 0.5 * ln((1 + x) / (1 - x))
2. Used to normalize prices.

What about **Ultimate Oscillator**?
Uses 3 timeframes (e.g., 7, 14, 28) of Buying Pressure / True Range.

What about **Awesome Oscillator**? Already exists.
What about **Rate of Change (ROC)**? Already exists.

Let's do **TRIX (Triple Exponential Average)**.
Wait, let me verify if `ema` indicator can take an existing `Series` or if it only takes a `DataFrame`.
Let's check `crates/strategies/src/indicators/ema.rs`.
