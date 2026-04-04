# 🔭 Vantage: Spec for Market Scanner Command

## 👤 User Story
As a Quantitative Trader, I want to scan the market for assets that meet specific volatility and momentum thresholds, so that I can automatically discover promising trading candidates without manual analysis.

## 💡 So What?
Manual market scanning is time-consuming and prone to human error. By providing a dedicated CLI command to scan the market, we enable users to programmatically filter thousands of assets based on actionable metrics. This directly increases the efficiency of their trading pipelines and the likelihood of finding profitable setups, translating to higher user engagement and platform utility.

## 📈 Metric Definition
Success = The scanner command processes a provider's ticker list and returns a list of the top matching candidate symbols in under 2 seconds.

## 🔍 Gap Analysis
Currently, our backend contains logic for scanning markets based on volume, volatility, and momentum, but this capability is not exposed to the user through the command-line interface. Users cannot access this functionality from the terminal, forcing them to rely on external tools for asset discovery. We need to bridge this gap by wiring the existing backend logic to a public command.

## ✅ Acceptance Criteria
- Must expose a dedicated market scanning command in the CLI.
- Must accept arguments for the data provider, the maximum number of results, minimum volatility, and minimum momentum.
- Must output the list of matching symbols in our standard formatted output.

## 🚫 Out of Scope
- Real-time streaming scanners.
- Adding new scanning metrics beyond volatility, volume, and momentum.
