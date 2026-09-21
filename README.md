# Thales CLI 🏛️

**Thales CLI** is a JSON-first, modular trading toolkit written in Rust. It is designed to be the backbone of autonomous trading agents, providing a standardized interface for market data, signal generation, and trade execution.

> "Know thyself, and thou shalt know the universe and the gods." - Thales of Miletus

## 🚀 Quick Start

### Prerequisites
- **Rust**: Latest stable version (`rustup update stable`)
- **Environment Variables**: See [Configuration](#configuration)

> **Note on Experimental Features**: Some commands (e.g., `simulate-black-swan`, `analyze-optics`) are experimental and require the `--features nova` flag to compile and run (e.g., `cargo run --features nova -p thales-cli -- simulate-black-swan ...`). If you encounter an "unrecognized subcommand" error for an experimental feature, ensure you have enabled this feature flag.

> **REQUIRES FEATURE NOVA**: Running the `story_demo` example requires enabling the `nova` feature flag (e.g., `cargo run --features nova --example story_demo ...`).

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
- **`crates/providers/jev`**: Client for TypeSafe AI's System One API, used to judge signals before execution.

## 🛠️ Usage

Thales CLI follows a pipeline approach where commands output JSON envelopes that can be piped or saved to files.

### 1. Fetch Market Data
Fetch price data from a provider.

> **Note**: `kraken` and `alpaca` require API keys and will error without them. Use
> `--provider paper` to get synthetic data for testing with no keys configured.

```bash
cargo run -p thales-cli -- fetch-market-data \
  --provider kraken \
  --symbol XXBTZUSD \
  --timeframe 1h > market_data.json
```

### 2. Backtest Strategy
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
Run a strategy to generate trade intents for the **current** timestamp.

> **Note**: This command outputs signals **only if** the strategy triggers at the latest available data point (the last candle in your input file). If the output is empty (`[]`), it means no trading condition was met at that specific time — that is normal, not a failure. To see the output structure, generate synthetic bars with `--provider paper`, which reliably triggers `BollingerBands`.

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

### 5. Judge Signals (System One gate)

Strategies decide *whether they fired*. They cannot tell a major pair from a thin
novelty token, and their `confidence` is a mechanical byproduct rather than a
calibrated probability. `judge-signals` puts a judgement step in between, using
TypeSafe AI's [System One](https://docs.typesafe.ai) model **Jev**.

Jev is not autoregressive: it answers typed questions against a state and returns
each answer with a probability distribution and a confidence score. One round
trip answers all four questions the gate asks.

```bash
export TYPESAFE_API_KEY=sk-...

cargo run -p thales-cli -- judge-signals   --input signals.json   --analysis analysis.json   --bars market_data.json > judged.json
```

The output is the surviving `TradeIntent`s, so it pipes straight into
`execute-intent`:

```bash
cargo run -p thales-cli -- execute-intent --provider paper --input judged.json
```

**What the gate asks.** Four questions in one request: a three-way verdict
(`execute` / `reduce_size` / `skip`), an instrument-quality veto, a regime-fit
check, and a conviction rating.

**How it decides.** All judgement lives in the model's probabilities; the code
only thresholds them, so every rejection points at one rule and one number:

| # | Rule | Flag | Default |
| - | ---- | ---- | ------- |
| 1 | Instrument is worth trading at all | `--min-instrument-quality` | `0.5` |
| 2 | Model is confident in its own verdict | `--min-confidence` | `0.60` |
| 3 | Verdict is not `skip` | — | — |
| 4 | Chosen action carries enough probability | `--min-probability` | `0.55` |

On approval, `confidence` is replaced by the calibrated probability of `execute`,
and a `reduce_size` verdict scales `size_hint` by `--reduce-factor` (default `0.5`).

**Auditing.** The reasoning is appended to each intent's `rationale`, so it
survives into the execution record. `--emit report` returns the full audit trail
(every verdict, distribution and token count) instead of just the intents, and
`--log portfolio.md` appends rejected signals in the columns `AGENTS.md` specifies.

> **Note**: A missing `TYPESAFE_API_KEY` makes this command fail rather than pass
> signals through ungated. The gate can only remove or shrink signals, never create them.

### 6. Execute Trades
Execute the generated trade intents.

> **WARNING**: Using `--provider kraken` will execute a **LIVE** trade if you have API keys configured. Use `--provider paper` to simulate execution safely!

```bash
cargo run -p thales-cli -- execute-intent \
  --provider paper \
  --input signals.json
```

### 7. Check Buying Power
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

### TypeSafe AI (optional, required by `judge-signals` and `analyze-market --jev`)
- `TYPESAFE_API_KEY`: Your API Key
- `TYPESAFE_BASE_URL`: (Optional) Defaults to `https://api.typesafe.ai`
- `TYPESAFE_MODEL`: (Optional) Defaults to `jev-latest`

### Alpaca
- `ALPACA_API_KEY`: Your API Key
- `ALPACA_API_SECRET`: Your API Secret
- `ALPACA_BASE_URL`: API Base URL

## 🤝 Contributing

CI runs on every pull request and **gates auto-merge** — nothing lands unless all
of these pass, so run them locally first:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check -p thales-cli --features nova --all-targets   # experimental commands
```

1. **Documentation**: If you change public APIs, update the docs. Run `cargo doc --open` to verify.
2. **Testing**: Run `cargo test` to ensure no regressions.
3. **Formatting**: Run `cargo fmt` before committing.

### Secrets

Copy `.env.example` to `.env` and fill it in. `.env` is gitignored; never commit real keys.

## 📄 License

MIT
