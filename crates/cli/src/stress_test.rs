use crate::backtest::{self, BacktestConfig};
use crate::synthetic_data::{self, SyntheticDataConfig};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressTestConfig {
    pub strategy: String,
    pub symbol: String,
    pub initial_price: f64,
    pub scenarios: Vec<StressScenario>,
    pub num_bars: usize,
    pub initial_capital: f64,
    pub risk_per_trade: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressScenario {
    pub name: String,
    pub drift: f64,
    pub volatility: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressTestReport {
    pub strategy: String,
    pub symbol: String,
    pub results: Vec<ScenarioResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioResult {
    pub scenario_name: String,
    pub final_equity: f64,
    pub total_return_pct: f64,
    pub max_drawdown_pct: f64,
}

pub async fn run_stress_test(config: StressTestConfig) -> Result<StressTestReport> {
    let mut results = Vec::new();
    let start_time_ms = chrono::Utc::now().timestamp_millis();

    for scenario in config.scenarios {
        let synth_config = SyntheticDataConfig {
            symbol: config.symbol.clone(),
            initial_price: config.initial_price,
            drift: scenario.drift,
            volatility: scenario.volatility,
            num_bars: config.num_bars,
            timeframe: "1d".to_string(), // we can assume 1d for simplicity
            start_time_ms,
        };

        // Mashup 1: Generate synthetic data for this scenario
        let series = synthetic_data::generate_synthetic_data(synth_config)?;

        let backtest_config = BacktestConfig {
            initial_capital: config.initial_capital,
            risk_per_trade: config.risk_per_trade,
        };

        // Mashup 2: Run the backtest on the synthetic data
        let backtest_result =
            backtest::run_backtest(&series, &config.strategy, backtest_config).await;

        match backtest_result {
            Ok(result) => {
                results.push(ScenarioResult {
                    scenario_name: scenario.name,
                    final_equity: result.final_equity,
                    total_return_pct: result.metrics.total_return_pct,
                    max_drawdown_pct: result.metrics.max_drawdown_pct,
                });
            }
            Err(e) => {
                // Return an error if any scenario fails
                return Err(anyhow::anyhow!("Scenario {} failed: {}", scenario.name, e));
            }
        }
    }

    Ok(StressTestReport {
        strategy: config.strategy,
        symbol: config.symbol,
        results,
    })
}

pub fn print_ascii_report(report: &StressTestReport) {
    println!(
        "\n📊 Stress Test Report: {} on {}\n",
        report.strategy, report.symbol
    );
    println!(
        "{:<15} | {:<12} | {:<12} | {:<12}",
        "Scenario", "Final Equity", "Return %", "Max DD %"
    );
    println!("{}", "-".repeat(60));
    for res in &report.results {
        println!(
            "{:<15} | ${:<11.2} | {:<11.2}% | {:<11.2}%",
            res.scenario_name,
            res.final_equity,
            res.total_return_pct * 100.0,
            res.max_drawdown_pct * 100.0
        );
    }
    println!("\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_stress_test() {
        let config = StressTestConfig {
            strategy: "BollingerBands".to_string(),
            symbol: "SYNTH".to_string(),
            initial_price: 100.0,
            scenarios: vec![
                StressScenario {
                    name: "Bull Market".to_string(),
                    drift: 0.001,
                    volatility: 0.01,
                },
                StressScenario {
                    name: "Bear Market".to_string(),
                    drift: -0.001,
                    volatility: 0.02,
                },
                StressScenario {
                    name: "High Volatility".to_string(),
                    drift: 0.0,
                    volatility: 0.05,
                },
            ],
            num_bars: 100,
            initial_capital: 10000.0,
            risk_per_trade: 100.0,
        };

        let report = run_stress_test(config).await;
        assert!(report.is_ok(), "run_stress_test must succeed");
        let report = report.unwrap();
        assert_eq!(report.results.len(), 3);
        assert_eq!(report.results[0].scenario_name, "Bull Market");
        assert_eq!(report.results[1].scenario_name, "Bear Market");
        assert_eq!(report.results[2].scenario_name, "High Volatility");
    }
}
