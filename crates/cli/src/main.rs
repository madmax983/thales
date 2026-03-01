use std::{fs, path::PathBuf, process};

use alpaca_provider::{AlpacaClient, AlpacaConfig};
use clap::{Parser, Subcommand};
use contracts::{BarSeries, EnvelopeStatus, ExecutionResult, ResponseEnvelope, TradeIntent};
use kraken_provider::{KrakenClient, KrakenConfig};
use paper_provider::{PaperClient, PaperConfig};
use serde::Serialize;
use serde_json::json;
use thiserror::Error;

use thales_cli::{analysis, backtest, benchmark, history, optimizer, rag, reporting, signals};

#[cfg(feature = "nova")]
use thales_cli::monte_carlo;
#[cfg(feature = "nova")]
use thales_cli::volume_profile;

#[derive(Debug, Parser)]
#[command(name = "thales-cli", version, about = "Agent trading toolkit CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Backtest {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        strategy: String,
        #[arg(long, default_value = "10000.0")]
        initial_capital: f64,
        #[arg(long, default_value = "100.0")]
        risk: f64,
    },
    Benchmark {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value = "10000.0")]
        initial_capital: f64,
        #[arg(long, default_value = "100.0")]
        risk: f64,
        #[arg(long, default_value = "total_return")]
        sort_by: String,
    },
    FetchMarketData {
        #[arg(long)]
        provider: String,
        #[arg(long)]
        symbol: String,
        #[arg(long)]
        timeframe: String,
    },
    NormalizeBars {
        #[arg(long)]
        input: PathBuf,
    },
    GenerateTradeIntent {
        #[arg(long)]
        market: String,
        #[arg(long)]
        symbol: String,
        #[arg(long)]
        side: String,
        #[arg(long)]
        size_hint: String,
        #[arg(long)]
        confidence: f64,
        #[arg(long, default_value = "market")]
        order_type: String,
        #[arg(long)]
        limit_price: Option<f64>,
        #[arg(long)]
        stop_price: Option<f64>,
        #[arg(long, default_value = "day")]
        time_in_force: String,
        #[arg(long)]
        execution_algo: Option<String>,
        #[arg(long, default_value = "manual")]
        strategy: String,
    },
    ValidateIntent {
        #[arg(long)]
        input: PathBuf,
    },
    GetOrder {
        #[arg(long)]
        provider: String,
        #[arg(long)]
        id: String,
    },
    GetOpenOrders {
        #[arg(long)]
        provider: String,
    },
    CancelOrder {
        #[arg(long)]
        provider: String,
        #[arg(long)]
        id: String,
    },
    ExecuteIntent {
        #[arg(long)]
        provider: String,
        #[arg(long)]
        input: PathBuf,
    },
    AnalyzeMarket {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        research: Option<String>,
        #[arg(long)]
        news: Option<String>,
        #[arg(long)]
        no_report: bool,
    },
    GenerateSignals {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value = "BollingerBands")]
        strategy: String,
        #[arg(long)]
        history: Option<PathBuf>,
        #[arg(long, default_value = "100.0")]
        risk: f64,
        #[arg(long)]
        portfolio: Option<PathBuf>,
        #[arg(long)]
        analysis: Option<PathBuf>,
    },
    UpdateSignalHistory {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        provider: String,
    },
    ScanMarket {
        #[arg(long)]
        provider: String,
        #[arg(long, default_value = "10")]
        top_n: usize,
        #[arg(long, default_value = "0.01")]
        min_volatility: f64,
        #[arg(long, default_value = "0.0")]
        min_momentum: f64,
    },
    GetPositions {
        #[arg(long)]
        provider: String,
    },
    GetBuyingPower {
        #[arg(long)]
        provider: String,
        #[arg(long)]
        symbol: Option<String>,
    },
    GetSellingPower {
        #[arg(long)]
        provider: String,
        #[arg(long)]
        symbol: String,
    },
    OptimizeStrategy {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        config: PathBuf,
    },
    #[cfg(feature = "nova")]
    MonteCarlo {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value = "1000")]
        iterations: usize,
        #[arg(long)]
        horizon: Option<usize>,
        #[arg(long)]
        initial_capital: Option<f64>,
    },
    #[cfg(feature = "nova")]
    AnalyzeVolumeProfile {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value = "50")]
        num_bins: usize,
        #[arg(long, default_value = "0.70")]
        value_area_pct: f64,
        #[arg(long)]
        visualize: bool,
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

            ok_envelope(result, vec![])
        }
        Commands::Benchmark {
            input,
            initial_capital,
            risk,
            sort_by,
        } => {
            let raw = fs::read_to_string(&input)?;
            let series: BarSeries = match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&raw)
            {
                Ok(envelope) => envelope
                    .data
                    .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                Err(_) => serde_json::from_str::<BarSeries>(&raw)?,
            };

            let config = benchmark::BenchmarkConfig {
                initial_capital,
                risk_per_trade: risk,
                sort_by,
            };

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| CliError::Provider(format!("Failed to create runtime: {}", e)))?;

            let result = rt
                .block_on(async { benchmark::run_benchmark(&series, config).await })
                .map_err(|e| CliError::Validation(e.to_string()))?;

            ok_envelope(result, vec![])
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
            ok_envelope(series, vec![])
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
            ok_envelope(series, vec![])
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
            strategy,
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
                strategy,
            };
            ok_envelope(intent, vec![])
        }
        Commands::ValidateIntent { input } => {
            let intent: TradeIntent = read_json_file(&input)?;
            let errors = validate_intent(&intent);
            if errors.is_empty() {
                ok_envelope(json!({ "valid": true }), vec![])
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

            ok_envelope(results, vec![])
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
                    rag::find_similar_trades(&analysis, &PathBuf::from("history.json"), None)
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

            ok_envelope(analysis, vec![])
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

            let mut warnings = Vec::new();
            if intents.is_empty() {
                warnings.push("No signals triggered. Strategies only generate signals if the condition is met on the latest available candle.".to_string());
            }

            ok_envelope(intents, warnings)
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

            ok_envelope(json!({ "updated_count": count }), vec![])
        }
        Commands::ScanMarket {
            provider,
            top_n,
            min_volatility,
            min_momentum,
        } => {
            let symbols = scan_market(&provider, top_n, min_volatility, min_momentum)?;
            ok_envelope(symbols, vec![])
        }
        Commands::GetPositions { provider } => match provider.as_str() {
            "kraken" => {
                let cfg =
                    KrakenConfig::from_env().map_err(|e| CliError::Provider(e.to_string()))?;
                let client = KrakenClient::new(cfg);
                let positions = client
                    .get_open_positions()
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(positions, vec![])
            }
            "alpaca" => {
                let cfg =
                    AlpacaConfig::from_env().map_err(|e| CliError::Provider(e.to_string()))?;
                let client = AlpacaClient::new(cfg);
                let positions = client
                    .get_open_positions()
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(positions, vec![])
            }
            "paper" => {
                let cfg = PaperConfig::from_env();
                let client = PaperClient::new(cfg);
                let positions = client
                    .get_open_positions()
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(positions, vec![])
            }
            _ => Err(CliError::Validation(format!(
                "Unsupported provider: {}",
                provider
            ))),
        },
        Commands::GetBuyingPower { provider, symbol } => match provider.as_str() {
            "kraken" => {
                let symbol = symbol.ok_or(CliError::Validation(
                    "symbol is required for kraken buying power lookup".to_string(),
                ))?;
                let cfg =
                    KrakenConfig::from_env().map_err(|e| CliError::Provider(e.to_string()))?;
                let client = KrakenClient::new(cfg);
                let (currency, amount) = client
                    .get_buying_power_for_symbol(&symbol)
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(json!({ "currency": currency, "amount": amount }), vec![])
            }
            "alpaca" => {
                let cfg =
                    AlpacaConfig::from_env().map_err(|e| CliError::Provider(e.to_string()))?;
                let client = AlpacaClient::new(cfg);
                let amount = client
                    .get_buying_power()
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(json!({ "currency": "USD", "amount": amount }), vec![])
            }
            "paper" => {
                let cfg = PaperConfig::from_env();
                let client = PaperClient::new(cfg);
                let amount = client
                    .get_buying_power()
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(json!({ "currency": "USD", "amount": amount }), vec![])
            }
            _ => Err(CliError::Validation(format!(
                "Unsupported provider: {}",
                provider
            ))),
        },
        Commands::GetSellingPower { provider, symbol } => match provider.as_str() {
            "kraken" => {
                let cfg =
                    KrakenConfig::from_env().map_err(|e| CliError::Provider(e.to_string()))?;
                let client = KrakenClient::new(cfg);
                let (asset, amount) = client
                    .get_sellable_balance_for_symbol(&symbol)
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(json!({ "asset": asset, "amount": amount }), vec![])
            }
            "alpaca" => {
                let cfg =
                    AlpacaConfig::from_env().map_err(|e| CliError::Provider(e.to_string()))?;
                let client = AlpacaClient::new(cfg);
                let positions = client
                    .get_open_positions()
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                let wanted = symbol.to_uppercase();
                let amount: f64 = positions
                    .into_iter()
                    .filter(|p| p.symbol.to_uppercase() == wanted && p.qty > 0.0)
                    .map(|p| p.qty)
                    .sum();
                ok_envelope(json!({ "asset": wanted, "amount": amount }), vec![])
            }
            "paper" => {
                let cfg = PaperConfig::from_env();
                let client = PaperClient::new(cfg);
                let positions = client
                    .get_open_positions()
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                let wanted = symbol.to_uppercase();
                let amount: f64 = positions
                    .into_iter()
                    .filter(|p| p.symbol.to_uppercase() == wanted && p.qty > 0.0)
                    .map(|p| p.qty)
                    .sum();
                ok_envelope(json!({ "asset": wanted, "amount": amount }), vec![])
            }
            _ => Err(CliError::Validation(format!(
                "Unsupported provider: {}",
                provider
            ))),
        },
        Commands::GetOrder { provider, id } => match provider.as_str() {
            "kraken" => {
                let cfg =
                    KrakenConfig::from_env().map_err(|e| CliError::Provider(e.to_string()))?;
                let client = KrakenClient::new(cfg);
                let order = client
                    .fetch_order(&id)
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(order, vec![])
            }
            "alpaca" => {
                let cfg =
                    AlpacaConfig::from_env().map_err(|e| CliError::Provider(e.to_string()))?;
                let client = AlpacaClient::new(cfg);
                let order = client
                    .fetch_order(&id)
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(order, vec![])
            }
            "paper" => {
                let cfg = PaperConfig::from_env();
                let client = PaperClient::new(cfg);
                let order = client
                    .fetch_order(&id)
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(order, vec![])
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
                ok_envelope(orders, vec![])
            }
            "alpaca" => {
                let cfg =
                    AlpacaConfig::from_env().map_err(|e| CliError::Provider(e.to_string()))?;
                let client = AlpacaClient::new(cfg);
                let orders = client
                    .fetch_open_orders()
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(orders, vec![])
            }
            "paper" => {
                let cfg = PaperConfig::from_env();
                let client = PaperClient::new(cfg);
                let orders = client
                    .fetch_open_orders()
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(orders, vec![])
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
                ok_envelope(json!({ "status": "canceled", "id": id }), vec![])
            }
            "alpaca" => {
                let cfg =
                    AlpacaConfig::from_env().map_err(|e| CliError::Provider(e.to_string()))?;
                let client = AlpacaClient::new(cfg);
                client
                    .cancel_order(&id)
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(json!({ "status": "canceled", "id": id }), vec![])
            }
            "paper" => {
                let cfg = PaperConfig::from_env();
                let client = PaperClient::new(cfg);
                client
                    .cancel_order(&id)
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(json!({ "status": "canceled", "id": id }), vec![])
            }
            _ => Err(CliError::Validation(format!(
                "Unsupported provider: {}",
                provider
            ))),
        },
        Commands::OptimizeStrategy { input, config } => {
            let raw_bars = fs::read_to_string(&input)?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&raw_bars) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&raw_bars)?,
                };

            let raw_config = fs::read_to_string(&config)?;
            let request: optimizer::OptimizationRequest = serde_json::from_str(&raw_config)?;

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| CliError::Provider(format!("Failed to create runtime: {}", e)))?;

            let result = rt
                .block_on(async { optimizer::optimize(&request, &series).await })
                .map_err(|e| CliError::Validation(e.to_string()))?;

            ok_envelope(result, vec![])
        }
        #[cfg(feature = "nova")]
        Commands::MonteCarlo {
            input,
            iterations,
            horizon,
            initial_capital,
        } => {
            let raw = fs::read_to_string(&input)?;
            let backtest: backtest::BacktestResult =
                match serde_json::from_str::<ResponseEnvelope<backtest::BacktestResult>>(&raw) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<backtest::BacktestResult>(&raw)?,
                };

            let config = monte_carlo::MonteCarloConfig {
                iterations,
                horizon,
                initial_capital,
            };

            let report = monte_carlo::run_simulation(&backtest, config)
                .map_err(|e| CliError::Validation(e.to_string()))?;

            ok_envelope(report, vec![])
        }
        #[cfg(feature = "nova")]
        Commands::AnalyzeVolumeProfile {
            input,
            num_bins,
            value_area_pct,
            visualize,
        } => {
            let raw = fs::read_to_string(&input)?;
            let series: BarSeries = match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&raw)
            {
                Ok(envelope) => envelope
                    .data
                    .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                Err(_) => serde_json::from_str::<BarSeries>(&raw)?,
            };

            let config = volume_profile::VolumeProfileConfig {
                num_bins,
                value_area_pct,
            };

            let report = volume_profile::analyze_volume_profile(&series, config)
                .map_err(|e| CliError::Validation(e.to_string()))?;

            if visualize {
                volume_profile::print_ascii_profile(&report);
            }

            ok_envelope(report, vec![])
        }
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
                let close_str = info.c.first().unwrap_or(&"0".to_string()).clone();
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
    if let Ok(envelope) = serde_json::from_str::<ResponseEnvelope<T>>(&raw)
        && let Some(data) = envelope.data
    {
        return Ok(data);
    }
    let parsed = serde_json::from_str::<T>(&raw)?;
    Ok(parsed)
}

fn ok_envelope<T>(data: T, warnings: Vec<String>) -> Result<String, CliError>
where
    T: Serialize,
{
    let envelope = ResponseEnvelope {
        status: EnvelopeStatus::Ok,
        errors: Vec::<String>::new(),
        warnings,
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
