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

```bash
cargo run -p thales-cli -- fetch-market-data \
  --provider kraken \
  --symbol XXBTZUSD \
  --timeframe 1h > market_data.json
```

### 2. Generate Signals
Run a strategy on the fetched data to generate trade intents.

```bash
cargo run -p thales-cli -- generate-signals \
  --input market_data.json \
  --strategy BollingerBandsMeanReversion > signals.json
```

### 3. Execute Trades
Execute the generated trade intents.

```bash
cargo run -p thales-cli -- execute-intent \
  --provider kraken \
  --input signals.json
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
