# 🔭 Vantage: Spec for KDJ Indicator Trading Strategy

## 👤 User Story
As a Quantitative Trader, I want to deploy a mean-reversion and momentum strategy based on the KDJ indicator, so that I can automatically identify overbought/oversold conditions and capture trend reversals before they fully materialize.

## 💡 So What?
Traditional momentum oscillators like the RSI or Stochastic Oscillator can lag significantly, leaving traders to enter positions too late. By implementing the KDJ indicator—which introduces a highly sensitive divergence %J line to the Stochastic %K and %D lines—we provide traders with a powerful leading indicator. This enables earlier entries on reversals, maximizing potential profit margins and giving our users a competitive edge in choppy or turning markets.

## 📈 Metric Definition
Success = The KDJ strategy processes standard historical data and generates Buy/Sell signals based on the defined %K, %D, and %J crossover logic in under 100ms per typical dataset, while maintaining strict adherence to the `max_position_size` risk parameter. Backtest coverage must show positive expected win rates and explicitly defined Sharpe ratios/max drawdowns.

## 🔍 Gap Analysis
Currently, our strategy suite includes several momentum oscillators (like Awesome Oscillator, MACD, and Stochastic) but lacks the KDJ indicator. Traders seeking high-sensitivity, leading indicators for mean-reversion must either build external pipelines or use slower, lagging indicators within our CLI. By integrating the KDJ Strategy natively using Polars for fast vector operations, we close this gap and offer a highly sought-after tool for identifying immediate market turning points.

## ✅ Acceptance Criteria
- Must generate Long Entry signals when the %J line crosses above 0 OR %K crosses above %D while both are below 20.
- Must generate Short Entry signals when the %J line crosses below 100 OR %K crosses below %D while both are above 80.
- Must generate Long Exit signals when the %J line crosses above 100 OR %K crosses below %D.
- Must generate Short Exit signals when the %J line crosses below 0 OR %K crosses above %D.
- Must strictly enforce a fixed allocation of `max_position_size` per trade for risk management.
- Must include stop-loss logic integration.

## 🚫 Out of Scope
- Multi-timeframe KDJ correlation (Phase 2).
- Dynamic position sizing based on volatility (Position sizing is strictly fixed per specification).
