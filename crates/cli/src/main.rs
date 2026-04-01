use std::{path::PathBuf, process};

use alpaca_provider::{AlpacaClient, AlpacaConfig};
use clap::{Parser, Subcommand};
use contracts::{BarSeries, EnvelopeStatus, ExecutionResult, ResponseEnvelope, TradeIntent};
use kraken_provider::{KrakenClient, KrakenConfig};
use paper_provider::{PaperClient, PaperConfig};
use serde::Serialize;
use serde_json::json;
use thiserror::Error;

use thales_cli::{
    analysis, backtest, benchmark, history, optimizer, reporting, search_history, signals,
};

#[cfg(feature = "nova")]
use thales_cli::black_swan;
#[cfg(feature = "nova")]
use thales_cli::entropy;
#[cfg(feature = "nova")]
use thales_cli::experimental::candlestick_patterns;
#[cfg(feature = "nova")]
use thales_cli::experimental::cycle_analysis;
#[cfg(feature = "nova")]
use thales_cli::experimental::market_energy;
#[cfg(feature = "nova")]
use thales_cli::experimental::market_seismology;
#[cfg(feature = "nova")]
use thales_cli::experimental::renko_entropy;
#[cfg(feature = "nova")]
use thales_cli::experimental::resonance;
#[cfg(feature = "nova")]
use thales_cli::experimental::similarity_search;
#[cfg(feature = "nova")]
use thales_cli::experimental::strategy_correlation;
#[cfg(feature = "nova")]
use thales_cli::fear_and_greed;
#[cfg(feature = "nova")]
use thales_cli::fractal_dimension;
#[cfg(feature = "nova")]
use thales_cli::market_phases;
#[cfg(feature = "nova")]
use thales_cli::markov_chain;
#[cfg(feature = "nova")]
use thales_cli::monte_carlo;
#[cfg(feature = "nova")]
use thales_cli::order_blocks;
#[cfg(feature = "nova")]
use thales_cli::pattern_match;
#[cfg(feature = "nova")]
use thales_cli::renko;
#[cfg(feature = "nova")]
use thales_cli::seasonality;
#[cfg(feature = "nova")]
use thales_cli::synthetic_data;
#[cfg(feature = "nova")]
use thales_cli::volume_profile;

#[derive(Debug, Parser)]
#[command(name = "thales-cli", version, about = "Agent trading toolkit CLI")]
struct Cli {
    #[arg(long, global = true)]
    raw: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[cfg(feature = "nova")]
    ExportCsv {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    #[cfg(feature = "nova")]
    ExportCard {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value = "Market Entity")]
        title: String,
    },
    #[cfg(feature = "nova")]
    ExportObj {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    #[cfg(feature = "nova")]
    MarketWeather {
        #[arg(long)]
        input: PathBuf,
    },
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
    #[cfg(feature = "nova")]
    AnalyzeRenkoEntropy {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value = "10.0")]
        brick_size: f64,
        #[arg(long)]
        visualize: bool,
    },
    #[cfg(feature = "nova")]
    AnalyzeOrderBlocks {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value = "3")]
        atr_period: usize,
        #[arg(long, default_value = "2.0")]
        expansion_multiplier: f64,
        #[arg(long)]
        visualize: bool,
    },
    #[cfg(feature = "nova")]
    AnalyzeMarketPhases {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value = "50")]
        short_window: usize,
        #[arg(long, default_value = "200")]
        long_window: usize,
        #[arg(long)]
        visualize: bool,
    },
    #[cfg(feature = "nova")]
    AnalyzeFractalDimension {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value = "10")]
        k_max: usize,
        #[arg(long)]
        visualize: bool,
    },
    #[cfg(feature = "nova")]
    AnalyzeMarkovChain {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value = "0.01")]
        state_threshold_pct: f64,
        #[arg(long)]
        visualize: bool,
    },
    #[cfg(feature = "nova")]
    AnalyzeEntropy {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value = "20")]
        num_bins: usize,
        #[arg(long)]
        visualize: bool,
    },
    #[cfg(feature = "nova")]
    AnalyzeFearAndGreed {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value = "20")]
        period: usize,
        #[arg(long)]
        visualize: bool,
    },
    #[cfg(feature = "nova")]
    PatternMatch {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value = "10")]
        window_size: usize,
        #[arg(long, default_value = "5")]
        top_k: usize,
        #[arg(long, default_value = "5")]
        forward_horizon: usize,
        #[arg(long)]
        visualize: bool,
    },
    #[cfg(feature = "nova")]
    AnalyzeSeasonality {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value = "DayOfWeek")]
        period: String,
        #[arg(long)]
        visualize: bool,
    },
    #[cfg(feature = "nova")]
    GenerateSyntheticData {
        #[arg(long, default_value = "SYNTH")]
        symbol: String,
        #[arg(long, default_value = "100.0")]
        initial_price: f64,
        #[arg(long, default_value = "0.0001")]
        drift: f64,
        #[arg(long, default_value = "0.01")]
        volatility: f64,
        #[arg(long, default_value = "100")]
        num_bars: usize,
        #[arg(long, default_value = "1d")]
        timeframe: String,
    },
    #[cfg(feature = "nova")]
    SimulateBlackSwan {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        event_type: String,
        #[arg(long)]
        start_index: usize,
        #[arg(long)]
        duration: usize,
        #[arg(long)]
        multiplier: Option<f64>,
        #[arg(long)]
        seed: Option<u64>,
    },
    #[cfg(feature = "nova")]
    AnalyzeRenko {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value = "10.0")]
        brick_size: f64,
        #[arg(long)]
        visualize: bool,
    },
    #[cfg(feature = "nova")]
    AnalyzeSimilarity {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value = "10")]
        window_size: usize,
        #[arg(long, default_value = "5")]
        top_k: usize,
        #[arg(long)]
        visualize: bool,
    },
    #[cfg(feature = "nova")]
    AnalyzeCycles {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value = "5")]
        max_cycles: usize,
        #[arg(long)]
        visualize: bool,
    },
    #[cfg(feature = "nova")]
    AnalyzeStrategyCorrelation {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        strategies: Option<Vec<String>>,
        #[arg(long, default_value = "10000.0")]
        initial_capital: f64,
        #[arg(long, default_value = "100.0")]
        risk: f64,
        #[arg(long)]
        visualize: bool,
    },
    #[cfg(feature = "nova")]
    AnalyzeCandlestickPatterns {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value = "10")]
        window_size: usize,
        #[arg(long, default_value = "0.1")]
        doji_threshold_pct: f64,
        #[arg(long)]
        visualize: bool,
    },
    #[cfg(feature = "nova")]
    #[cfg(feature = "nova")]
    AnalyzeEnergy {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value = "14")]
        window: usize,
        #[arg(long)]
        visualize: bool,
    },
    #[cfg(feature = "nova")]
    AnalyzeResonance {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value = "14")]
        window_size: usize,
        #[arg(long, default_value = "100.0")]
        amplitude_threshold: f64,
        #[arg(long)]
        visualize: bool,
    },
    #[cfg(feature = "nova")]
    AnalyzeSonification {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value = "48")]
        min_pitch: u8,
        #[arg(long, default_value = "84")]
        max_pitch: u8,
        #[arg(long)]
        visualize: bool,
    },
    #[cfg(feature = "nova")]
    AnalyzeSeismology {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value = "14")]
        window_size: usize,
        #[arg(long, default_value = "5.0")]
        tremor_threshold: f64,
        #[arg(long)]
        visualize: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    match run(cli.command, cli.raw) {
        Ok(output) => {
            println!("{output}");
        }
        Err(err) => {
            println!("{}", error_envelope(vec![err.to_string()]));
            process::exit(1);
        }
    }
}

fn run(command: Commands, raw: bool) -> Result<String, CliError> {
    match command {
        #[cfg(feature = "nova")]
        Commands::ExportCsv { input, output } => {
            let file_content = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&file_content) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&file_content)?,
                };
            thales_cli::experimental::export::export_bar_series_to_csv(&series, &output)
                .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;
            ok_envelope(json!({"status": "success", "file": output}), vec![], raw)
        }
        #[cfg(feature = "nova")]
        Commands::ExportCard {
            input,
            output,
            title,
        } => {
            let series: BarSeries = read_json_file(&input)?;
            thales_cli::experimental::trading_card::export_trading_card_svg(
                &series, &title, output,
            )
            .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;
            ok_envelope("Exported Trading Card successfully", vec![], raw)
        }
        #[cfg(feature = "nova")]
        Commands::ExportObj { input, output } => {
            let file_content = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&file_content) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&file_content)?,
                };
            thales_cli::experimental::export::export_bar_series_to_obj(&series, &output)
                .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;
            ok_envelope(json!({"status": "success", "file": output}), vec![], raw)
        }
        #[cfg(feature = "nova")]
        Commands::MarketWeather { input } => {
            let file_content = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&file_content) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&file_content)?,
                };
            let weather = thales_cli::experimental::market_weather::calculate_weather(&series)
                .ok_or_else(|| {
                    CliError::Validation("Not enough data to calculate weather".to_string())
                })?;
            ok_envelope(json!(weather), vec![], raw)
        }
        Commands::Backtest {
            input,
            strategy,
            initial_capital,
            risk,
        } => {
            let raw_str = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&raw_str) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&raw_str)?,
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
                .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;

            ok_envelope(result, vec![], raw)
        }
        Commands::Benchmark {
            input,
            initial_capital,
            risk,
            sort_by,
        } => {
            let raw_str = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&raw_str) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&raw_str)?,
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
                .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;

            ok_envelope(result, vec![], raw)
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
            ok_envelope(series, vec![], raw)
        }
        Commands::NormalizeBars { input } => {
            let raw_str = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let mut series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&raw_str) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&raw_str)?,
                };
            series.bars.sort_by_key(|bar| bar.timestamp_unix_ms);
            ok_envelope(series, vec![], raw)
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
            ok_envelope(intent, vec![], raw)
        }
        Commands::ValidateIntent { input } => {
            let intent: TradeIntent = read_json_file(&input)?;
            let errors = validate_intent(&intent);
            if errors.is_empty() {
                ok_envelope(json!({ "valid": true }), vec![], raw)
            } else {
                Err(CliError::Validation(errors.join("; ")))
            }
        }
        Commands::ExecuteIntent { provider, input } => {
            let intents: Vec<TradeIntent> = match read_json_file::<Vec<TradeIntent>>(&input) {
                Ok(list) => list,
                Err(_) => {
                    let single: TradeIntent = read_json_file(&input).map_err(|e| CliError::Validation(format!("Failed to parse TradeIntents from '{}'. Did you pass market data instead of signals? (Original error: {})", input.display(), e)))?;
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

            ok_envelope(results, vec![], raw)
        }
        Commands::AnalyzeMarket {
            input,
            research,
            news,
            no_report,
        } => {
            let raw_str = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&raw_str) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&raw_str)?,
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

                let similar_trades = search_history::find_similar_trades(
                    &analysis,
                    &PathBuf::from("history.json"),
                    None,
                )
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

            ok_envelope(analysis, vec![], raw)
        }
        Commands::GenerateSignals {
            input,
            strategy,
            history,
            risk,
            portfolio,
            analysis,
        } => {
            let raw_str = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&raw_str) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&raw_str)?,
                };

            let positions: Vec<contracts::Position> = if let Some(path) = portfolio {
                let raw_pos = std::fs::read_to_string(&path).map_err(|e| {
                    std::io::Error::new(
                        e.kind(),
                        format!("Could not open input file '{}': {}", path.display(), e),
                    )
                })?;
                // Handle envelope or raw list
                match serde_json::from_str::<ResponseEnvelope<Vec<contracts::Position>>>(&raw_pos) {
                    Ok(env) => env.data.unwrap_or_default(),
                    Err(_) => serde_json::from_str(&raw_pos).unwrap_or_default(),
                }
            } else {
                Vec::new()
            };

            let market_analysis: Option<contracts::MarketAnalysis> = if let Some(path) = analysis {
                let raw_analysis = std::fs::read_to_string(&path).map_err(|e| {
                    std::io::Error::new(
                        e.kind(),
                        format!("Could not open input file '{}': {}", path.display(), e),
                    )
                })?;
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
                .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;

            let mut warnings = Vec::new();
            if intents.is_empty() {
                let latest_time = series
                    .bars
                    .last()
                    .map(|b| {
                        chrono::DateTime::from_timestamp_millis(b.timestamp_unix_ms)
                            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();

                let warning_msg = if !latest_time.is_empty() {
                    format!(
                        "No signals triggered for latest candle ({}). Strategies only generate signals if the condition is met on the latest available candle.",
                        latest_time
                    )
                } else {
                    "No signals triggered. Strategies only generate signals if the condition is met on the latest available candle.".to_string()
                };

                warnings.push(warning_msg.clone());

                // If running interactively, alert the user directly
                use std::io::IsTerminal;
                if std::io::stdout().is_terminal() {
                    eprintln!("⚠️ {}", warning_msg);
                }
            }

            ok_envelope(intents, warnings, raw)
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
            .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;

            ok_envelope(json!({ "updated_count": count }), vec![], raw)
        }
        Commands::ScanMarket {
            provider,
            top_n,
            min_volatility,
            min_momentum,
        } => {
            let symbols = scan_market(&provider, top_n, min_volatility, min_momentum)?;
            ok_envelope(symbols, vec![], raw)
        }
        Commands::GetPositions { provider } => match provider.as_str() {
            "kraken" => {
                let cfg =
                    KrakenConfig::from_env().map_err(|e| CliError::Provider(e.to_string()))?;
                let client = KrakenClient::new(cfg);
                let positions = client
                    .get_open_positions()
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(positions, vec![], raw)
            }
            "alpaca" => {
                let cfg =
                    AlpacaConfig::from_env().map_err(|e| CliError::Provider(e.to_string()))?;
                let client = AlpacaClient::new(cfg);
                let positions = client
                    .get_open_positions()
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(positions, vec![], raw)
            }
            "paper" => {
                let cfg = PaperConfig::from_env();
                let client = PaperClient::new(cfg);
                let positions = client
                    .get_open_positions()
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(positions, vec![], raw)
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
                ok_envelope(
                    json!({ "currency": currency, "amount": amount }),
                    vec![],
                    raw,
                )
            }
            "alpaca" => {
                let cfg =
                    AlpacaConfig::from_env().map_err(|e| CliError::Provider(e.to_string()))?;
                let client = AlpacaClient::new(cfg);
                let amount = client
                    .get_buying_power()
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(json!({ "currency": "USD", "amount": amount }), vec![], raw)
            }
            "paper" => {
                let cfg = PaperConfig::from_env();
                let client = PaperClient::new(cfg);
                let amount = client
                    .get_buying_power()
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(json!({ "currency": "USD", "amount": amount }), vec![], raw)
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
                ok_envelope(json!({ "asset": asset, "amount": amount }), vec![], raw)
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
                ok_envelope(json!({ "asset": wanted, "amount": amount }), vec![], raw)
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
                ok_envelope(json!({ "asset": wanted, "amount": amount }), vec![], raw)
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
                ok_envelope(order, vec![], raw)
            }
            "alpaca" => {
                let cfg =
                    AlpacaConfig::from_env().map_err(|e| CliError::Provider(e.to_string()))?;
                let client = AlpacaClient::new(cfg);
                let order = client
                    .fetch_order(&id)
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(order, vec![], raw)
            }
            "paper" => {
                let cfg = PaperConfig::from_env();
                let client = PaperClient::new(cfg);
                let order = client
                    .fetch_order(&id)
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(order, vec![], raw)
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
                ok_envelope(orders, vec![], raw)
            }
            "alpaca" => {
                let cfg =
                    AlpacaConfig::from_env().map_err(|e| CliError::Provider(e.to_string()))?;
                let client = AlpacaClient::new(cfg);
                let orders = client
                    .fetch_open_orders()
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(orders, vec![], raw)
            }
            "paper" => {
                let cfg = PaperConfig::from_env();
                let client = PaperClient::new(cfg);
                let orders = client
                    .fetch_open_orders()
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(orders, vec![], raw)
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
                ok_envelope(json!({ "status": "canceled", "id": id }), vec![], raw)
            }
            "alpaca" => {
                let cfg =
                    AlpacaConfig::from_env().map_err(|e| CliError::Provider(e.to_string()))?;
                let client = AlpacaClient::new(cfg);
                client
                    .cancel_order(&id)
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(json!({ "status": "canceled", "id": id }), vec![], raw)
            }
            "paper" => {
                let cfg = PaperConfig::from_env();
                let client = PaperClient::new(cfg);
                client
                    .cancel_order(&id)
                    .map_err(|e| CliError::Provider(e.to_string()))?;
                ok_envelope(json!({ "status": "canceled", "id": id }), vec![], raw)
            }
            _ => Err(CliError::Validation(format!(
                "Unsupported provider: {}",
                provider
            ))),
        },
        Commands::OptimizeStrategy { input, config } => {
            let raw_bars = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&raw_bars) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&raw_bars)?,
                };

            let raw_config = std::fs::read_to_string(&config).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", config.display(), e),
                )
            })?;
            let request: optimizer::OptimizationRequest = serde_json::from_str(&raw_config)?;

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| CliError::Provider(format!("Failed to create runtime: {}", e)))?;

            let result = rt
                .block_on(async { optimizer::optimize(&request, &series).await })
                .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;

            ok_envelope(result, vec![], raw)
        }
        #[cfg(feature = "nova")]
        Commands::MonteCarlo {
            input,
            iterations,
            horizon,
            initial_capital,
        } => {
            let raw_str = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let backtest: backtest::BacktestResult = match serde_json::from_str::<
                ResponseEnvelope<backtest::BacktestResult>,
            >(&raw_str)
            {
                Ok(envelope) => envelope
                    .data
                    .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                Err(_) => serde_json::from_str::<backtest::BacktestResult>(&raw_str)?,
            };

            let config = monte_carlo::MonteCarloConfig {
                iterations,
                horizon,
                initial_capital,
            };

            let report = monte_carlo::run_simulation(&backtest, config)
                .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;

            ok_envelope(report, vec![], raw)
        }
        #[cfg(feature = "nova")]
        Commands::AnalyzeRenkoEntropy {
            input,
            brick_size,
            visualize,
        } => {
            let file_content = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&file_content) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&file_content)?,
                };

            let config = renko_entropy::RenkoEntropyConfig { brick_size };

            let report = renko_entropy::analyze_renko_entropy(&series, config)
                .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;

            if visualize {
                renko_entropy::print_ascii_renko_entropy(&report);
            }

            ok_envelope(report, vec![], raw)
        }
        #[cfg(feature = "nova")]
        Commands::AnalyzeVolumeProfile {
            input,
            num_bins,
            value_area_pct,
            visualize,
        } => {
            let raw_str = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&raw_str) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&raw_str)?,
                };

            let config = volume_profile::VolumeProfileConfig {
                num_bins,
                value_area_pct,
            };

            let report = volume_profile::analyze_volume_profile(&series, config)
                .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;

            if visualize {
                volume_profile::print_ascii_profile(&report);
            }

            ok_envelope(report, vec![], raw)
        }
        #[cfg(feature = "nova")]
        Commands::AnalyzeMarketPhases {
            input,
            short_window,
            long_window,
            visualize,
        } => {
            let file_content = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&file_content) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&file_content)?,
                };

            let config = market_phases::MarketPhasesConfig {
                short_window,
                long_window,
            };
            let report = market_phases::analyze_market_phases(&series, config)
                .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;

            if visualize {
                market_phases::print_ascii_market_phases(&report);
            }

            ok_envelope(report, vec![], raw)
        }
        #[cfg(feature = "nova")]
        Commands::AnalyzeFractalDimension {
            input,
            k_max,
            visualize,
        } => {
            let file_content = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&file_content) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&file_content)?,
                };

            let config = fractal_dimension::FractalConfig { k_max };

            let report = fractal_dimension::analyze_fractal_dimension(&series, config)
                .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;

            if visualize {
                fractal_dimension::print_ascii_fractal_dimension(&report);
            }

            ok_envelope(report, vec![], raw)
        }
        #[cfg(feature = "nova")]
        Commands::AnalyzeMarkovChain {
            input,
            state_threshold_pct,
            visualize,
        } => {
            let file_content = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&file_content) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&file_content)?,
                };

            let config = markov_chain::MarkovConfig {
                state_threshold_pct,
            };

            let report = markov_chain::analyze_markov_chain(&series, config)
                .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;

            if visualize {
                markov_chain::print_ascii_markov_chain(&report);
            }

            ok_envelope(report, vec![], raw)
        }
        #[cfg(feature = "nova")]
        Commands::AnalyzeEntropy {
            input,
            num_bins,
            visualize,
        } => {
            let file_content = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&file_content) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&file_content)?,
                };

            let config = entropy::EntropyConfig { num_bins };

            let report = entropy::analyze_entropy(&series, config)
                .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;

            if visualize {
                entropy::print_ascii_entropy(&report);
            }

            ok_envelope(report, vec![], raw)
        }
        #[cfg(feature = "nova")]
        Commands::AnalyzeFearAndGreed {
            input,
            period,
            visualize,
        } => {
            let file_content = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&file_content) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&file_content)?,
                };

            let config = fear_and_greed::FearAndGreedConfig { period };

            let report = fear_and_greed::analyze_fear_and_greed(&series, config)
                .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;

            if visualize {
                fear_and_greed::print_ascii_fear_and_greed(&report);
            }

            ok_envelope(report, vec![], raw)
        }
        #[cfg(feature = "nova")]
        Commands::PatternMatch {
            input,
            window_size,
            top_k,
            forward_horizon,
            visualize,
        } => {
            let file_content = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&file_content) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&file_content)?,
                };

            let config = pattern_match::PatternMatchConfig {
                window_size,
                top_k,
                forward_horizon,
            };

            let report = pattern_match::analyze_patterns(&series, config)
                .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;

            if visualize {
                pattern_match::print_ascii_patterns(&report);
            }

            ok_envelope(report, vec![], raw)
        }
        #[cfg(feature = "nova")]
        Commands::AnalyzeSeasonality {
            input,
            period,
            visualize,
        } => {
            let file_content = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&file_content) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&file_content)?,
                };

            let p = match period.as_str() {
                "DayOfWeek" => seasonality::SeasonalityPeriod::DayOfWeek,
                "MonthOfYear" => seasonality::SeasonalityPeriod::MonthOfYear,
                "HourOfDay" => seasonality::SeasonalityPeriod::HourOfDay,
                _ => {
                    return Err(CliError::Validation(format!(
                        "Invalid period type: {}",
                        period
                    )));
                }
            };

            let config = seasonality::SeasonalityConfig { period: p };

            let report = seasonality::analyze_seasonality(&series, config)
                .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;

            if visualize {
                seasonality::print_ascii_seasonality(&report);
            }

            ok_envelope(report, vec![], raw)
        }
        #[cfg(feature = "nova")]
        Commands::GenerateSyntheticData {
            symbol,
            initial_price,
            drift,
            volatility,
            num_bars,
            timeframe,
        } => {
            let config = synthetic_data::SyntheticDataConfig {
                symbol,
                initial_price,
                drift,
                volatility,
                num_bars,
                timeframe,
                start_time_ms: chrono::Utc::now().timestamp_millis(),
            };

            let series = synthetic_data::generate_synthetic_data(config)
                .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;

            ok_envelope(series, vec![], raw)
        }
        #[cfg(feature = "nova")]
        Commands::SimulateBlackSwan {
            input,
            event_type,
            start_index,
            duration,
            multiplier,
            seed,
        } => {
            let file_content = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&file_content) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&file_content)?,
                };

            let mult = multiplier.unwrap_or(match event_type.as_str() {
                "FlashCrash" => 0.30,      // 30% drop
                "VolatilitySpike" => 5.0,  // 5x volatility
                "LiquidityFreeze" => 0.05, // 95% volume drop
                _ => {
                    return Err(CliError::Validation(format!(
                        "Unknown event_type: {}",
                        event_type
                    )));
                }
            });

            let event = match event_type.as_str() {
                "FlashCrash" => black_swan::BlackSwanEvent::FlashCrash {
                    drop_pct: mult,
                    duration_bars: duration,
                },
                "VolatilitySpike" => black_swan::BlackSwanEvent::VolatilitySpike {
                    multiplier: mult,
                    duration_bars: duration,
                },
                "LiquidityFreeze" => black_swan::BlackSwanEvent::LiquidityFreeze {
                    volume_multiplier: mult,
                    duration_bars: duration,
                },
                _ => {
                    return Err(CliError::Validation(format!(
                        "Unknown event_type: {}",
                        event_type
                    )));
                }
            };

            let config = black_swan::BlackSwanConfig {
                event,
                start_index,
                seed,
            };

            let modified_series = black_swan::inject_black_swan(&series, config)
                .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;

            ok_envelope(modified_series, vec![], raw)
        }
        #[cfg(feature = "nova")]
        Commands::AnalyzeRenko {
            input,
            brick_size,
            visualize,
        } => {
            let file_content = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&file_content) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&file_content)?,
                };

            let config = renko::RenkoConfig { brick_size };

            let report = renko::analyze_renko(&series, config)
                .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;

            if visualize {
                renko::print_ascii_renko(&report);
            }

            if visualize {
                renko::print_ascii_renko(&report);
            }

            ok_envelope(report, vec![], raw)
        }
        #[cfg(feature = "nova")]
        Commands::AnalyzeOrderBlocks {
            input,
            atr_period,
            expansion_multiplier,
            visualize,
        } => {
            let file_content = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&file_content) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&file_content)?,
                };

            let config = order_blocks::OrderBlocksConfig {
                atr_period,
                expansion_multiplier,
            };

            let report = order_blocks::analyze_order_blocks(&series, config)
                .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;

            if visualize {
                order_blocks::print_ascii_order_blocks(&report);
            }

            ok_envelope(report, vec![], raw)
        }
        #[cfg(feature = "nova")]
        Commands::AnalyzeSimilarity {
            input,
            window_size,
            top_k,
            visualize,
        } => {
            let file_content = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&file_content) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&file_content)?,
                };

            let config = similarity_search::SimilarityConfig { window_size, top_k };

            let report = similarity_search::find_similar_patterns(&series, config)
                .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;

            if visualize {
                similarity_search::print_ascii_similarities(&report);
            }

            ok_envelope(report, vec![], raw)
        }
        #[cfg(feature = "nova")]
        Commands::AnalyzeCycles {
            input,
            max_cycles,
            visualize,
        } => {
            let file_content = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&file_content) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&file_content)?,
                };

            let config = cycle_analysis::CycleConfig { max_cycles };

            let report = cycle_analysis::analyze_cycles(&series, config)
                .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;

            if visualize {
                cycle_analysis::print_ascii_cycles(&report);
            }

            ok_envelope(report, vec![], raw)
        }
        #[cfg(feature = "nova")]
        Commands::AnalyzeStrategyCorrelation {
            input,
            strategies,
            initial_capital,
            risk,
            visualize,
        } => {
            let file_content = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&file_content) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&file_content)?,
                };

            let config = strategy_correlation::CorrelationConfig {
                initial_capital,
                risk_per_trade: risk,
                strategies: strategies.unwrap_or_default(),
            };

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| CliError::Provider(format!("Failed to create runtime: {}", e)))?;

            let report = rt
                .block_on(async {
                    strategy_correlation::analyze_correlations(&series, config).await
                })
                .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;

            if visualize {
                strategy_correlation::print_ascii_correlations(&report);
            }

            ok_envelope(report, vec![], raw)
        }
        #[cfg(feature = "nova")]
        Commands::AnalyzeCandlestickPatterns {
            input,
            window_size,
            doji_threshold_pct,
            visualize,
        } => {
            let file_content = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&file_content) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&file_content)?,
                };

            let config = candlestick_patterns::CandlestickPatternsConfig {
                window_size,
                doji_threshold_pct,
            };

            let report = candlestick_patterns::analyze_candlestick_patterns(&series, config)
                .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;

            if visualize {
                candlestick_patterns::print_ascii_patterns(&report);
            }

            ok_envelope(report, vec![], raw)
        }
        #[cfg(feature = "nova")]
        #[cfg(feature = "nova")]
        Commands::AnalyzeEnergy {
            input,
            window,
            visualize,
        } => {
            let file_content = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&file_content) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&file_content)?,
                };

            let config = market_energy::EnergyConfig { window };

            let report = market_energy::analyze_energy(&series, config)
                .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;

            if visualize {
                market_energy::print_ascii_energy(&report);
            }

            ok_envelope(report, vec![], raw)
        }
        #[cfg(feature = "nova")]
        Commands::AnalyzeResonance {
            input,
            window_size,
            amplitude_threshold,
            visualize,
        } => {
            let file_content = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&file_content) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&file_content)?,
                };

            let config = resonance::ResonanceConfig {
                window_size,
                amplitude_threshold,
            };

            let report = resonance::analyze_resonance(&series, config)
                .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;

            if visualize {
                resonance::print_ascii_resonance(&report);
            }

            ok_envelope(report, vec![], raw)
        }
        #[cfg(feature = "nova")]
        Commands::AnalyzeSonification {
            input,
            min_pitch,
            max_pitch,
            visualize,
        } => {
            let file_content = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&file_content) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&file_content)?,
                };

            let config = thales_cli::experimental::sonification::SonificationConfig {
                min_pitch,
                max_pitch,
            };

            let report =
                thales_cli::experimental::sonification::analyze_sonification(&series, config)
                    .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;

            if visualize {
                thales_cli::experimental::sonification::print_ascii_sonification(&report);
            }

            ok_envelope(report, vec![], raw)
        }
        #[cfg(feature = "nova")]
        Commands::AnalyzeSeismology {
            input,
            window_size,
            tremor_threshold,
            visualize,
        } => {
            let file_content = std::fs::read_to_string(&input).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not open input file '{}': {}", input.display(), e),
                )
            })?;
            let series: BarSeries =
                match serde_json::from_str::<ResponseEnvelope<BarSeries>>(&file_content) {
                    Ok(envelope) => envelope
                        .data
                        .ok_or(CliError::Validation("Envelope has no data".to_string()))?,
                    Err(_) => serde_json::from_str::<BarSeries>(&file_content)?,
                };

            let config = market_seismology::SeismologyConfig {
                window_size,
                tremor_threshold,
            };

            let report = market_seismology::analyze_seismology(&series, config)
                .map_err(|e: anyhow::Error| CliError::Validation(e.to_string()))?;

            if visualize {
                market_seismology::print_ascii_seismology(&report);
            }

            ok_envelope(report, vec![], raw)
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
    let raw_str = std::fs::read_to_string(path).map_err(|e| {
        std::io::Error::new(
            e.kind(),
            format!("Could not open file '{}': {}", path.display(), e),
        )
    })?;
    if let Ok(envelope) = serde_json::from_str::<ResponseEnvelope<T>>(&raw_str)
        && let Some(data) = envelope.data
    {
        return Ok(data);
    }
    let parsed = serde_json::from_str::<T>(&raw_str)?;
    Ok(parsed)
}

fn ok_envelope<T>(data: T, warnings: Vec<String>, raw: bool) -> Result<String, CliError>
where
    T: Serialize,
{
    if raw {
        return Ok(serde_json::to_string(&data)?);
    }
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
