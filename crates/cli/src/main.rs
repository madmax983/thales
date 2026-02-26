//! Thales CLI Entry Point
//!
//! This module defines the command-line interface for the Thales trading system.
//! It uses `clap` for argument parsing and delegates execution to specialized modules.
//!
//! # Key Commands
//!
//! * `fetch-market-data` - Retrieve OHLCV data from providers.
//! * `generate-signals` - Run strategies to produce trade intents.
//! * `execute-intent` - Execute trade intents via providers.
//! * `analyze-market` - Generate market reports.
//!
//! # Examples
//!
//! ```bash
//! # Fetch data
//! thales-cli fetch-market-data --provider kraken --symbol BTCUSD --timeframe 1h
//! ```

use std::{fs, path::PathBuf, process};

use alpaca_provider::{AlpacaClient, AlpacaConfig};
use clap::{Parser, Subcommand};
use contracts::{BarSeries, EnvelopeStatus, ExecutionResult, ResponseEnvelope, TradeIntent};
use kraken_provider::{KrakenClient, KrakenConfig};
use paper_provider::{PaperClient, PaperConfig};
use serde::Serialize;
use serde_json::json;
use thiserror::Error;

use thales_cli::{analysis, backtest, history, rag, reporting, signals};

/// The main entry point for the Thales CLI.
#[derive(Debug, Parser)]
#[command(name = "thales-cli", version, about = "Agent trading toolkit CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Available subcommands for the Thales CLI.
#[derive(Debug, Subcommand)]
enum Commands {
    /// Runs a backtest for a specific strategy on historical data.
    Backtest {
        /// Path to the JSON file containing historical bar data.
        #[arg(long)]
        input: PathBuf,
        /// Name of the strategy to test (e.g., "BollingerBands").
        #[arg(long)]
        strategy: String,
        /// Initial capital for the simulation.
        #[arg(long, default_value = "10000.0")]
        initial_capital: f64,
        /// Risk per trade as a percentage of capital (0-100) or fixed amount.
        #[arg(long, default_value = "100.0")]
        risk: f64,
    },
    /// Fetches historical market data (OHLCV) from a provider.
    FetchMarketData {
        /// The data provider to use ("kraken", "alpaca", "paper").
        #[arg(long)]
        provider: String,
        /// The symbol to fetch (e.g., "BTCUSD", "AAPL").
        #[arg(long)]
        symbol: String,
        /// The timeframe for the bars (e.g., "1m", "1h", "1d").
        #[arg(long)]
        timeframe: String,
    },
    /// Normalizes a bar series JSON file by sorting bars by timestamp.
    NormalizeBars {
        /// Path to the input JSON file.
        #[arg(long)]
        input: PathBuf,
    },
    /// Manually generates a trade intent for testing or manual execution.
    GenerateTradeIntent {
        /// The market type ("crypto", "equities").
        #[arg(long)]
        market: String,
        /// The symbol to trade.
        #[arg(long)]
        symbol: String,
        /// The side of the trade ("buy", "sell").
        #[arg(long)]
        side: String,
        /// The size hint (e.g., "0.1", "max").
        #[arg(long)]
        size_hint: String,
        /// Confidence score [0.0, 1.0].
        #[arg(long)]
        confidence: f64,
        /// Order type ("market", "limit", "stop", "stop_limit").
        #[arg(long, default_value = "market")]
        order_type: String,
        /// Limit price (required for Limit/Stop-Limit orders).
        #[arg(long)]
        limit_price: Option<f64>,
        /// Stop price (required for Stop/Stop-Limit orders).
        #[arg(long)]
        stop_price: Option<f64>,
        /// Time in force ("day", "gtc", "ioc").
        #[arg(long, default_value = "day")]
        time_in_force: String,
        /// Optional execution algorithm name.
        #[arg(long)]
        execution_algo: Option<String>,
    },
    /// Validates a TradeIntent JSON file against the schema.
    ValidateIntent {
        /// Path to the TradeIntent JSON file.
        #[arg(long)]
        input: PathBuf,
    },
    /// Fetches open orders from a provider.
    GetOpenOrders {
        /// The provider to query ("kraken", "alpaca", "paper").
        #[arg(long)]
        provider: String,
    },
    /// Cancels a specific order.
    CancelOrder {
        /// The provider to use.
        #[arg(long)]
        provider: String,
        /// The order ID to cancel.
        #[arg(long)]
        id: String,
    },
    /// Executes a trade intent using a specific provider.
    ExecuteIntent {
        /// The provider to execute with.
        #[arg(long)]
        provider: String,
        /// Path to the TradeIntent JSON file.
        #[arg(long)]
        input: PathBuf,
    },
    /// Analyzes market data to generate a market report.
    AnalyzeMarket {
        /// Path to the market data JSON file.
        #[arg(long)]
        input: PathBuf,
        /// Optional research summary string to include in the report.
        #[arg(long)]
        research: Option<String>,
        /// Optional news summary string to include in the report.
        #[arg(long)]
        news: Option<String>,
        /// If set, skips writing reports to Markdown files.
        #[arg(long)]
        no_report: bool,
    },
    /// Generates trade signals based on a strategy and market data.
    GenerateSignals {
        /// Path to the market data JSON file.
        #[arg(long)]
        input: PathBuf,
        /// The strategy to run.
        #[arg(long, default_value = "BollingerBands")]
        strategy: String,
        /// Optional path to historical trade data (for RAG).
        #[arg(long)]
        history: Option<PathBuf>,
        /// Risk amount per trade.
        #[arg(long, default_value = "100.0")]
        risk: f64,
        /// Optional path to current portfolio (for position awareness).
        #[arg(long)]
        portfolio: Option<PathBuf>,
        /// Optional path to pre-computed market analysis.
        #[arg(long)]
        analysis: Option<PathBuf>,
    },
    /// Updates the local signal history database with fresh price data.
    UpdateSignalHistory {
        /// Path to the history JSON file.
        #[arg(long)]
        input: PathBuf,
        /// The provider to fetch fresh data from.
        #[arg(long)]
        provider: String,
    },
    /// Scans the market for potential trading candidates.
    ScanMarket {
        /// The provider to use for scanning.
        #[arg(long)]
        provider: String,
        /// Number of top candidates to return.
        #[arg(long, default_value = "10")]
        top_n: usize,
        /// Minimum volatility threshold.
        #[arg(long, default_value = "0.01")]
        min_volatility: f64,
        /// Minimum momentum threshold.
        #[arg(long, default_value = "0.0")]
        min_momentum: f64,
    },
    /// Fetches open positions from a provider.
    GetPositions {
        /// The provider to query.
        #[arg(long)]
        provider: String,
    },
}

fn main() {
    let cli = Cli::parse();
    match run(cli.command) {
        Ok(output) => {
            println!("{output}");
        }
        Err(err) => {
            println!("{}", error_envelope(vec![err.to_string()]));
            process::exit(1);
        }
    }
}

fn run(command: Commands) -> Result<String, CliError> {
    match command {
        Commands::Backtest {
            input,
            strategy,
            initial_capital,
            risk,
        } => {
            let raw = fs::read_to_string(&input)?;
            let series: BarSeries = match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&raw)
            {
                Ok(envelope) => envelope
                    .data
                    .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                Err(_) => serde_json::from_str::<BarSeries>(&raw)?,
            };

            let config = backtest::BacktestConfig {
                initial_capital,
                risk_per_trade: risk,
            };

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| CliError::Provider(format!("Failed to create runtime: {}", e)))?;

            let result = rt
                .block_on(async { backtest::run_backtest(&series, &strategy, config).await })
                .map_err(|e| CliError::Validation(e.to_string()))?;

            ok_envelope(result)
        }
        Commands::FetchMarketData {
            provider,
            symbol,
            timeframe,
        } => {
            let bars = match provider.as_str() {
                "kraken" => {
                    let cfg =
                        KrakenConfig::from_env().map_err(|e| CliError::Provider(e.to_string()))?;
                    let client = KrakenClient::new(cfg);
                    client
                        .fetch_bars(&symbol, &timeframe)
                        .map_err(|e| CliError::Provider(e.to_string()))?
                }
                "alpaca" => {
                    let cfg =
                        AlpacaConfig::from_env().map_err(|e| CliError::Provider(e.to_string()))?;
                    let client = AlpacaClient::new(cfg);
                    client
                        .fetch_bars(&symbol, &timeframe)
                        .map_err(|e| CliError::Provider(e.to_string()))?
                }
                "paper" => {
                    let cfg = PaperConfig::from_env();
                    let client = PaperClient::new(cfg);
                    client
                        .fetch_bars(&symbol, &timeframe)
                        .map_err(|e| CliError::Provider(e.to_string()))?
                }
                _ => {
                    return Err(CliError::Validation(format!(
                        "Unsupported provider: {}",
                        provider
                    )));
                }
            };

            let series = BarSeries {
                schema_version: "v0".to_string(),
                bars,
            };
            ok_envelope(series)
        }
        Commands::NormalizeBars { input } => {
            let raw = fs::read_to_string(&input)?;
            let mut series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&raw) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&raw)?,
                };
            series.bars.sort_by_key(|bar| bar.timestamp_unix_ms);
            ok_envelope(series)
        }
        Commands::GenerateTradeIntent {
            market,
            symbol,
            side,
            size_hint,
            confidence,
            order_type,
            limit_price,
            stop_price,
            time_in_force,
            execution_algo,
        } => {
            let intent = TradeIntent {
                intent_id: format!("{market}:{symbol}:{side}:v0"),
                market,
                symbol,
                side,
                size_hint,
                confidence,
                horizon: "1h".to_string(),
                rationale: "generated by thales-cli".to_string(),
                invalidation: "manual review".to_string(),
                schema_version: "v0".to_string(),
                signal_type: None,
                stop_loss: None,
                take_profit: None,
                order_type,
                limit_price,
                stop_price,
                time_in_force,
                execution_algo,
            };
            ok_envelope(intent)
        }
        Commands::ValidateIntent { input } => {
            let intent: TradeIntent = read_json_file(&input)?;
            let errors = validate_intent(&intent);
            if errors.is_empty() {
                ok_envelope(json!({ "valid": true }))
            } else {
                Err(CliError::Validation(errors.join("; ")))
            }
        }
        Commands::ExecuteIntent { provider, input } => {
            let intents: Vec<TradeIntent> = match read_json_file::<Vec<TradeIntent>>(&input) {
                Ok(list) => list,
                Err(_) => {
                    let single: TradeIntent = read_json_file(&input)?;
                    vec![single]
                }
            };

            let mut results = Vec::new();
            for intent in intents {
                let errors = validate_intent(&intent);
                if !errors.is_empty() {
                    return Err(CliError::Validation(format!(
                        "Validation error for intent {}: {}",
                        intent.intent_id,
                        errors.join("; ")
                    )));
                }

                let result = execute_by_provider(&provider, &intent)?;
                results.push(result);
            }

            ok_envelope(results)
        }
        Commands::AnalyzeMarket {
            input,
            research,
            news,
            no_report,
        } => {
            let raw = fs::read_to_string(&input)?;
            let series: BarSeries = match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&raw)
            {
                Ok(envelope) => envelope
                    .data
                    .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                Err(_) => serde_json::from_str::<BarSeries>(&raw)?,
            };

            let mut analysis = analysis::analyze(&series);
            analysis.research_summary = research;
            analysis.news_summary = news;

            if !no_report {
                // Reporting Step
                let signals_path = PathBuf::from("Signals.md");
                let regime_path = PathBuf::from("Market_Regime.md");
                let volatility_path = PathBuf::from("Volatility_Regime.md");
                let research_path = PathBuf::from("Market_Research.md");

                let similar_trades =
                    rag::find_similar_trades(&analysis, &PathBuf::from("history.json"))
                        .unwrap_or_default();
                let previous_regime = reporting::read_last_regime(&signals_path, &analysis.symbol);

                let report = reporting::generate_report(
                    &analysis,
                    &similar_trades,
                    previous_regime.as_deref(),
                );

                if let Err(e) = reporting::append_to_signals_md(&signals_path, &report) {
                    eprintln!("Warning: Failed to write to Signals.md: {}", e);
                }

                // New reports
                let regime_report = reporting::generate_regime_report(&analysis);
                if let Err(e) = reporting::append_to_file(&regime_path, &regime_report) {
                    eprintln!("Warning: Failed to write to Market_Regime.md: {}", e);
                }

                let volatility_report = reporting::generate_volatility_report(&analysis);
                if let Err(e) = reporting::append_to_file(&volatility_path, &volatility_report) {
                    eprintln!("Warning: Failed to write to Volatility_Regime.md: {}", e);
                }

                let research_report = reporting::generate_research_report(&analysis);
                if let Err(e) = reporting::append_to_file(&research_path, &research_report) {
                    eprintln!("Warning: Failed to write to Market_Research.md: {}", e);
                }
            }

            ok_envelope(analysis)
        }
        Commands::GenerateSignals {
            input,
            strategy,
            history,
            risk,
            portfolio,
            analysis,
        } => {
            let raw = fs::read_to_string(&input)?;
            let series: BarSeries = match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&raw)
            {
                Ok(envelope) => envelope
                    .data
                    .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                Err(_) => serde_json::from_str::<BarSeries>(&raw)?,
            };

            let positions: Vec<contracts::Position> = if let Some(path) = portfolio {
                let raw_pos = fs::read_to_string(&path)?;
                // Handle envelope or raw list
                match serde_json::from_str::<ResponseEnvelope<Vec<contracts::Position>>>(&raw_pos) {
                    Ok(env) => env.data.unwrap_or_default(),
                    Err(_) => serde_json::from_str(&raw_pos).unwrap_or_default(),
                }
            } else {
                Vec::new()
            };

            let market_analysis: Option<contracts::MarketAnalysis> = if let Some(path) = analysis {
                let raw_analysis = fs::read_to_string(&path)?;
                match serde_json::from_str::<ResponseEnvelope<contracts::MarketAnalysis>>(
                    &raw_analysis,
                ) {
                    Ok(env) => env.data,
                    Err(_) => Some(serde_json::from_str(&raw_analysis)?),
                }
            } else {
                None
            };

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| CliError::Provider(format!("Failed to create runtime: {}", e)))?;

            let intents = rt
                .block_on(async {
                    signals::generate_signals(
                        &series,
                        &strategy,
                        history.as_deref(),
                        risk,
                        &positions,
                        market_analysis,
                    )
                    .await
                })
                .map_err(|e| CliError::Validation(e.to_string()))?;

            ok_envelope(intents)
        }
        Commands::UpdateSignalHistory { input, provider } => {
            let count = match provider.as_str() {
                "kraken" => {
                    let cfg =
                        KrakenConfig::from_env().map_err(|e| CliError::Provider(e.to_string()))?;
                    let client = KrakenClient::new(cfg);
                    history::update_history(&input, |sym, tf| {
                        client.fetch_bars(sym, tf).map_err(|e| anyhow::anyhow!(e))
                    })
                }
                "alpaca" => {
                    let cfg =
                        AlpacaConfig::from_env().map_err(|e| CliError::Provider(e.to_string()))?;
                    let client = AlpacaClient::new(cfg);
                    history::update_history(&input, |sym, tf| {
                        client.fetch_bars(sym, tf).map_err(|e| anyhow::anyhow!(e))
                    })
                }
                "paper" => {
                    let cfg = PaperConfig::from_env();
                    let client = PaperClient::new(cfg);
                    history::update_history(&input, |sym, tf| {
                        client.fetch_bars(sym, tf).map_err(|e| anyhow::anyhow!(e))
                    })
                }
                _ => {
                    return Err(CliError::Validation(format!(
                        "Unsupported provider: {}",
                        provider
                    )));
                }
            }
            .map_err(|e| CliError::Validation(e.to_string()))?;

            ok_envelope(json!({ "updated_count": count }))
        }
        Commands::ScanMarket {
            provider,
            top_n,
            min_volatility,
            min_momentum,
        } => {
            let symbols = scan_market(&provider, top_n, min_volatility, min_momentum)?;
            ok_envelope(symbols)
        }
        Commands::GetPositions { provider } => match provider.as_str() {
            "kraken" => {
                let cfg =
                    KrakenConfig::from_env().map_err(|e| CliError::Provider(e.to_string()))?;
                let client = KrakenClient::new(cfg);
                let positions = client
                    .get_open_positions()
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(positions)
            }
            "alpaca" => {
                let cfg =
                    AlpacaConfig::from_env().map_err(|e| CliError::Provider(e.to_string()))?;
                let client = AlpacaClient::new(cfg);
                let positions = client
                    .get_open_positions()
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(positions)
            }
            "paper" => {
                let cfg = PaperConfig::from_env();
                let client = PaperClient::new(cfg);
                let positions = client
                    .get_open_positions()
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(positions)
            }
            _ => Err(CliError::Validation(format!(
                "Unsupported provider: {}",
                provider
            ))),
        },
        Commands::GetOpenOrders { provider } => match provider.as_str() {
            "kraken" => {
                let cfg =
                    KrakenConfig::from_env().map_err(|e| CliError::Provider(e.to_string()))?;
                let client = KrakenClient::new(cfg);
                let orders = client
                    .fetch_open_orders()
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(orders)
            }
            "alpaca" => {
                let cfg =
                    AlpacaConfig::from_env().map_err(|e| CliError::Provider(e.to_string()))?;
                let client = AlpacaClient::new(cfg);
                let orders = client
                    .fetch_open_orders()
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(orders)
            }
            "paper" => {
                let cfg = PaperConfig::from_env();
                let client = PaperClient::new(cfg);
                let orders = client
                    .fetch_open_orders()
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(orders)
            }
            _ => Err(CliError::Validation(format!(
                "Unsupported provider: {}",
                provider
            ))),
        },
        Commands::CancelOrder { provider, id } => match provider.as_str() {
            "kraken" => {
                let cfg =
                    KrakenConfig::from_env().map_err(|e| CliError::Provider(e.to_string()))?;
                let client = KrakenClient::new(cfg);
                client
                    .cancel_order(&id)
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(json!({ "status": "canceled", "id": id }))
            }
            "alpaca" => {
                let cfg =
                    AlpacaConfig::from_env().map_err(|e| CliError::Provider(e.to_string()))?;
                let client = AlpacaClient::new(cfg);
                client
                    .cancel_order(&id)
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(json!({ "status": "canceled", "id": id }))
            }
            "paper" => {
                let cfg = PaperConfig::from_env();
                let client = PaperClient::new(cfg);
                client
                    .cancel_order(&id)
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(json!({ "status": "canceled", "id": id }))
            }
            _ => Err(CliError::Validation(format!(
                "Unsupported provider: {}",
                provider
            ))),
        },
    }
}

fn scan_market(
    provider: &str,
    top_n: usize,
    min_volatility: f64,
    min_momentum: f64,
) -> Result<Vec<String>, CliError> {
    match provider {
        "kraken" => {
            let cfg = KrakenConfig::from_env().map_err(|e| CliError::Provider(e.to_string()))?;
            let client = KrakenClient::new(cfg);
            let tickers = client
                .fetch_tickers()
                .map_err(|e| CliError::Provider(e.to_string()))?;

            // Filter and Sort
            let mut candidates: Vec<(String, f64, f64)> = Vec::new(); // (Symbol, Volume, Volatility)

            for (pair, info) in tickers {
                // Filter for USD pairs (usually end in USD or ZUSD)
                // Kraken pairs are weird: XXBTZUSD, XETHZUSD, ADAUSD
                if !pair.ends_with("USD") {
                    continue;
                }

                // Parse volume (24h) - 'v' field [today, 24h]
                let vol_str = info.v.get(1).unwrap_or(&"0".to_string()).clone();
                let vol_24h = vol_str.parse::<f64>().unwrap_or(0.0);

                // Parse High/Low for Volatility
                let high_str = info.h.get(1).unwrap_or(&"0".to_string()).clone();
                let low_str = info.l.get(1).unwrap_or(&"0".to_string()).clone();

                let high_24h = high_str.parse::<f64>().unwrap_or(0.0);
                let low_24h = low_str.parse::<f64>().unwrap_or(0.0);

                // Parse Open/Close for Momentum (24h)
                let open_str = info.o.clone();
                let close_str = info.c.get(0).unwrap_or(&"0".to_string()).clone();
                let open = open_str.parse::<f64>().unwrap_or(0.0);
                let close = close_str.parse::<f64>().unwrap_or(0.0);

                let momentum = if open > 0.0 {
                    (close - open) / open
                } else {
                    0.0
                };

                if low_24h > 0.0 {
                    let volatility = (high_24h - low_24h) / low_24h;
                    if volatility >= min_volatility && momentum >= min_momentum {
                        candidates.push((pair, vol_24h, volatility));
                    }
                }
            }

            // Sort by volume descending
            candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            let result: Vec<String> = candidates
                .into_iter()
                .take(top_n)
                .map(|(s, _, _)| s)
                .collect();
            Ok(result)
        }
        "alpaca" => {
            // Static watchlist for equities
            let watchlist = vec![
                "SPY", "QQQ", "TQQQ", "AAPL", "NVDA", "TSLA", "AMZN", "META", "MSFT", "AMD",
                "GOOGL",
            ];
            Ok(watchlist.into_iter().map(String::from).collect())
        }
        "paper" => {
            // Return static list for simulation
            Ok(vec![
                "BTCUSD".to_string(),
                "ETHUSD".to_string(),
                "SPY".to_string(),
            ])
        }
        _ => Err(CliError::Validation(format!(
            "Unsupported provider for scanning: {}",
            provider
        ))),
    }
}

fn execute_by_provider(provider: &str, intent: &TradeIntent) -> Result<ExecutionResult, CliError> {
    match provider {
        "alpaca" => {
            let cfg =
                AlpacaConfig::from_env().map_err(|err| CliError::Provider(err.to_string()))?;
            let client = AlpacaClient::new(cfg);
            client
                .execute_intent(intent)
                .map_err(|err| CliError::Provider(err.to_string()))
        }
        "kraken" => {
            let cfg =
                KrakenConfig::from_env().map_err(|err| CliError::Provider(err.to_string()))?;
            let client = KrakenClient::new(cfg);
            client
                .execute_intent(intent)
                .map_err(|err| CliError::Provider(err.to_string()))
        }
        "paper" => {
            let cfg = PaperConfig::from_env();
            let client = PaperClient::new(cfg);
            client
                .execute_intent(intent)
                .map_err(|err| CliError::Provider(err.to_string()))
        }
        other => Err(CliError::Validation(format!(
            "unsupported provider: {other}"
        ))),
    }
}

fn validate_intent(intent: &TradeIntent) -> Vec<String> {
    let mut errors = Vec::new();
    if intent.schema_version != "v0" {
        errors.push("schema_version must be v0".to_string());
    }
    if intent.symbol.trim().is_empty() {
        errors.push("symbol must not be empty".to_string());
    }
    if intent.side != "buy" && intent.side != "sell" {
        errors.push("side must be buy or sell".to_string());
    }
    if !(0.0..=1.0).contains(&intent.confidence) {
        errors.push("confidence must be in [0, 1]".to_string());
    }
    let valid_types = ["market", "limit", "stop", "stop_limit"];
    if !valid_types.contains(&intent.order_type.as_str()) {
        errors.push(format!(
            "order_type must be one of: {}",
            valid_types.join(", ")
        ));
    }
    if intent.time_in_force.trim().is_empty() {
        errors.push("time_in_force must not be empty".to_string());
    }
    errors
}

fn read_json_file<T>(path: &PathBuf) -> Result<T, CliError>
where
    T: serde::de::DeserializeOwned,
{
    let raw = fs::read_to_string(path)?;
    if let Ok(envelope) = serde_json::from_str::<ResponseEnvelope<T>>(&raw) {
        if let Some(data) = envelope.data {
            return Ok(data);
        }
    }
    let parsed = serde_json::from_str::<T>(&raw)?;
    Ok(parsed)
}

fn ok_envelope<T>(data: T) -> Result<String, CliError>
where
    T: Serialize,
{
    let envelope = ResponseEnvelope {
        status: EnvelopeStatus::Ok,
        errors: Vec::<String>::new(),
        warnings: Vec::<String>::new(),
        data: Some(data),
    };
    Ok(serde_json::to_string(&envelope)?)
}

fn error_envelope(errors: Vec<String>) -> String {
    let envelope = ResponseEnvelope::<serde_json::Value> {
        status: EnvelopeStatus::Error,
        errors,
        warnings: Vec::<String>::new(),
        data: None,
    };
    serde_json::to_string(&envelope).unwrap_or_else(|_| {
        "{\"status\":\"error\",\"errors\":[\"failed to serialize error envelope\"],\"warnings\":[],\"data\":null}"
            .to_string()
    })
}

#[derive(Debug, Error)]
enum CliError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("provider error: {0}")]
    Provider(String),
}
