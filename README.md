# Thales CLI 🏛️

**Thales CLI** is a JSON-first, modular trading toolkit written in Rust. It is designed to be the backbone of autonomous trading agents, providing a standardized interface for market data, signal generation, and trade execution.

> "Know thyself, and thou shalt know the universe and the gods." - Thales of Miletus

## 🚀 Quick Start

### Prerequisites
- **Rust**: Latest stable version (`rustup update stable`)
- **Environment Variables**: See [Configuration](#configuration)

### Build
```bash
cargo build --release -p thales-cli
```

### run
```bash
# Fetch market data (requires API keys)
cargo run -p thales-cli -- fetch-market-data --provider kraken --symbol XXBTZUSD --timeframe 1h
```

## 🏗️ Architecture

The workspace is organized into modular crates:

- **`crates/cli`**: The command-line interface and entry point. Orchestrates the other crates.
- **`crates/contracts`**: Pure data structures (structs/enums) defining the shared "language" (e.g., `TradeIntent`, `BarSeries`).
- **`crates/providers`**: Adapters for external APIs (e.g., `kraken`, `alpaca`).
- **`crates/strategies`**: Trading logic and signal generation (e.g., `BollingerBands`, `RsiMeanReversion`).

## 🛠️ Usage

Thales CLI follows a pipeline approach where commands output JSON envelopes that can be piped or saved to files.

### 1. Fetch Market Data
Fetch OHLCV data from a provider.

> **Note**: If no API keys are provided, this command will return synthetic scaffolding data for testing purposes.

```bash
cargo run -p thales-cli -- fetch-market-data \
  --provider kraken \
  --symbol XXBTZUSD \
  --timeframe 1h > market_data.json
```

### 2. Verify Strategy (Backtest)
Run a backtest on the fetched data to see how the strategy performs over time.

```bash
cargo run -p thales-cli -- backtest \
  --input market_data.json \
  --strategy BollingerBands \
  --initial-capital 10000 \
  --risk 0.01 > backtest_results.json
```

### 3. Benchmark Strategies
Compare all available strategies on a dataset to find the best performer.

```bash
cargo run -p thales-cli -- benchmark \
  --input market_data.json \
  --initial-capital 10000 \
  --risk 100 \
  --sort-by total_return > benchmark_results.json
```

### 4. Generate Signals
Run a strategy on the fetched data to generate trade intents for the **current** timestamp.

> **Note**: This command outputs signals **only if** the strategy triggers at the latest available data point (the last candle in your input file). If the output is empty (`[]`), it means no trading condition was met at that specific time. Use `backtest` to verify strategy logic on historical data.

```bash
cargo run -p thales-cli -- generate-signals \
  --input market_data.json \
  --strategy BollingerBands > signals.json
```

**Troubleshooting Empty Signals:**
If you get `{"status":"ok", "data":[], "warnings": ["No signals triggered..."]}`:
1.  **Check Backtest:** Run the backtest command (step 2) to ensure the strategy actually trades this asset on historical data.
2.  **Check Data Freshness:** Ensure your `market_data.json` includes the most recent candle.
3.  **Market Conditions:** The strategy simply might not have a setup right now. This is normal.

### 5. Execute Trades
Execute the generated trade intents.

```bash
cargo run -p thales-cli -- execute-intent \
  --provider kraken \
  --input signals.json
```

### 6. Check Buying Power
Fetch available buying power before sizing new buy orders.

```bash
cargo run -p thales-cli -- get-buying-power \
  --provider kraken \
  --symbol XBT/USD
```

## ⚙️ Configuration

Set the following environment variables based on your provider:

### Kraken
- `KRAKEN_API_KEY`: Your API Key
- `KRAKEN_API_SECRET`: Your API Secret (Base64 encoded)
- `KRAKEN_BASE_URL`: (Optional) Defaults to `https://api.kraken.com`

### Alpaca
- `ALPACA_API_KEY`: Your API Key
- `ALPACA_API_SECRET`: Your API Secret
- `ALPACA_BASE_URL`: API Base URL

## 🤝 Contributing

1. **Documentation**: If you change public APIs, update the docs. Run `cargo doc --open` to verify.
2. **Testing**: Run `cargo test` to ensure no regressions.
3. **Formatting**: Run `cargo fmt` before committing.

## 📄 License

MIT
