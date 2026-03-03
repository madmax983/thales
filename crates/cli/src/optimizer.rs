use crate::backtest::{self, BacktestConfig};
use crate::strategy_factory;
use anyhow::Result;
use contracts::BarSeries;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationConfig {
    pub population_size: usize,
    pub generations: usize,
    pub mutation_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ParamType {
    #[serde(rename = "int")]
    Int { min: i64, max: i64 },
    #[serde(rename = "float")]
    Float { min: f64, max: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationRequest {
    pub strategy: String,
    pub initial_capital: f64,
    pub risk_per_trade: f64,
    pub config: OptimizationConfig,
    pub params: HashMap<String, ParamType>,
}

#[derive(Debug, Clone)]
struct Genome {
    genes: HashMap<String, GeneValue>,
    fitness: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum GeneValue {
    Int(i64),
    Float(f64),
}

impl GeneValue {
    fn to_json(&self) -> Value {
        match self {
            GeneValue::Int(v) => serde_json::json!(v),
            GeneValue::Float(v) => serde_json::json!(v),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OptimizationResult {
    pub best_params: HashMap<String, Value>,
    pub best_fitness: f64,
    pub generations_completed: usize,
}

pub async fn optimize(
    request: &OptimizationRequest,
    bars: &BarSeries,
) -> Result<OptimizationResult> {
    if bars.bars.is_empty() {
        return Err(anyhow::anyhow!("No market data provided"));
    }

    let mut rng = rand::thread_rng();
    let mut population: Vec<Genome> = Vec::with_capacity(request.config.population_size);

    // 1. Initialize Population
    for _ in 0..request.config.population_size {
        let mut genes = HashMap::new();
        for (name, param_type) in &request.params {
            let value = match param_type {
                ParamType::Int { min, max } => GeneValue::Int(rng.gen_range(*min..=*max)),
                ParamType::Float { min, max } => GeneValue::Float(rng.gen_range(*min..=*max)),
            };
            genes.insert(name.clone(), value);
        }
        population.push(Genome {
            genes,
            fitness: -f64::INFINITY,
        });
    }

    // 2. Evolution Loop
    for generation in 0..request.config.generations {
        // Evaluate Fitness
        for genome in &mut population {
            let mut params_json = serde_json::Map::new();
            for (k, v) in &genome.genes {
                params_json.insert(k.clone(), v.to_json());
            }
            // Add other required fields that are not optimized (e.g. symbol)
            // But strategies might need them in the config.
            // For now, we assume the strategy factory sets defaults, and update_params overrides them.
            // However, `update_params` usually deserializes the *whole* config struct.
            // So we need to ensure we merge with existing config or provide all fields.
            //
            // Strategy implementations use `serde_json::from_value` to update config.
            // If the JSON is missing fields, it might fail if they are not Option.
            //
            // Hack: We should probably fetch the default config first, convert to Value, merge, then update.
            // But we can't easily get the default config from the boxed trait object without casting.
            //
            // Alternative: The `OptimizationRequest` params should cover all required fields, OR
            // we rely on the fact that `serde_json::from_value` will error if missing fields.
            //
            // Most strategy configs have `symbol` which is not in params.
            // We should inject `symbol` from the bars.
            let symbol = bars.bars[0].symbol.clone();
            params_json.insert("symbol".to_string(), serde_json::json!(symbol));

            // Also inject stop_loss_pct if not in params, as most strategies have it.
            // Or assume user provides it in params if they want to optimize it.
            // Let's rely on user provided params + symbol.
            // If a required field is missing, update_params will fail. We'll catch it and give poor fitness.

            let fitness = evaluate_genome(
                &request.strategy,
                request.initial_capital,
                request.risk_per_trade,
                Value::Object(params_json),
                bars,
            )
            .await
            .unwrap_or(-1000.0);
            genome.fitness = fitness;
        }

        // Sort by fitness (descending)
        population.sort_by(|a, b| {
            b.fitness
                .partial_cmp(&a.fitness)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        println!(
            "Generation {}: Best Fitness = {:.2}%",
            generation + 1,
            population[0].fitness
        );

        if generation == request.config.generations - 1 {
            break;
        }

        // Selection & Reproduction (Elitism: keep top 1)
        let mut new_population = Vec::with_capacity(request.config.population_size);
        new_population.push(population[0].clone()); // Elitism

        while new_population.len() < request.config.population_size {
            let parent1 = tournament_select(&population, &mut rng);
            let parent2 = tournament_select(&population, &mut rng);

            let mut child_genes = crossover(&parent1.genes, &parent2.genes, &mut rng);
            mutate(
                &mut child_genes,
                &request.params,
                request.config.mutation_rate,
                &mut rng,
            );

            new_population.push(Genome {
                genes: child_genes,
                fitness: -f64::INFINITY,
            });
        }

        population = new_population;
    }

    let best = &population[0];
    let mut best_params_json = HashMap::new();
    for (k, v) in &best.genes {
        best_params_json.insert(k.clone(), v.to_json());
    }

    Ok(OptimizationResult {
        best_params: best_params_json,
        best_fitness: best.fitness,
        generations_completed: request.config.generations,
    })
}

async fn evaluate_genome(
    strategy_name: &str,
    initial_capital: f64,
    risk: f64,
    params: Value,
    bars: &BarSeries,
) -> Result<f64> {
    let symbol = bars.bars[0].symbol.clone();

    // 1. Create Strategy
    let mut strategy = strategy_factory::create_strategy(strategy_name, &symbol)?;

    // 2. Update Params
    // We need to merge with default config if possible, but here we just try to update.
    // If it fails due to missing fields, we might need to handle that.
    // For now, assume params contains all necessary fields.
    // Actually, `update_params` in strategies usually does `self.config = serde_json::from_value(params)?`.
    // So if `params` is missing fields, it will error.
    // This implies `OptimizationRequest` must include ALL config fields OR we need a way to get defaults.
    //
    // Workaround: We can instantiate a default strategy, serialize its config, merge with our params, then update.
    // But `Strategy` trait doesn't expose config.
    //
    // Let's assume for this MVP that the user provides a complete config in `params`,
    // or we catch the error.

    match strategy.update_params(params).await {
        Ok(_) => {
            let config = BacktestConfig {
                initial_capital,
                risk_per_trade: risk,
            };

            match backtest::run_backtest_with_strategy(bars, strategy, config).await {
                Ok(result) => Ok(result.metrics.total_return_pct),
                Err(_) => Ok(-1000.0), // Backtest failed
            }
        }
        Err(_) => {
            // Parameter update failed (e.g. invalid config)
            Ok(-1000.0)
        }
    }
}

fn tournament_select<'a>(population: &'a [Genome], rng: &mut impl Rng) -> &'a Genome {
    let k = 3;
    let mut best = &population[rng.gen_range(0..population.len())];
    for _ in 0..k - 1 {
        let contender = &population[rng.gen_range(0..population.len())];
        if contender.fitness > best.fitness {
            best = contender;
        }
    }
    best
}

fn crossover(
    p1: &HashMap<String, GeneValue>,
    p2: &HashMap<String, GeneValue>,
    rng: &mut impl Rng,
) -> HashMap<String, GeneValue> {
    let mut child = HashMap::new();
    for (k, v1) in p1 {
        if let Some(v2) = p2.get(k) {
            if rng.gen_bool(0.5) {
                child.insert(k.clone(), *v1);
            } else {
                child.insert(k.clone(), *v2);
            }
        }
    }
    child
}

fn mutate(
    genes: &mut HashMap<String, GeneValue>,
    params: &HashMap<String, ParamType>,
    rate: f64,
    rng: &mut impl Rng,
) {
    for (k, v) in genes.iter_mut() {
        if rng.gen_bool(rate) {
            if let Some(param_type) = params.get(k) {
                *v = match param_type {
                    ParamType::Int { min, max } => GeneValue::Int(rng.gen_range(*min..=*max)),
                    ParamType::Float { min, max } => GeneValue::Float(rng.gen_range(*min..=*max)),
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    #[tokio::test]
    async fn test_optimization_basic() {
        let mut bars = Vec::new();
        let now = 100000;

        // Simple cyclical data
        for i in 0..100 {
            let t = i as f64 / 10.0;
            let close = 100.0 + t.sin() * 10.0;
            bars.push(Bar {
                symbol: "TEST".to_string(),
                market: "crypto".to_string(),
                timeframe: "1h".to_string(),
                timestamp_unix_ms: now + i * 3600000,
                open: close,
                high: close + 1.0,
                low: close - 1.0,
                close,
                volume: 100.0,
            });
        }

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let config = OptimizationConfig {
            population_size: 5,
            generations: 2,
            mutation_rate: 0.1,
        };

        let mut params = HashMap::new();
        params.insert("period".to_string(), ParamType::Int { min: 2, max: 20 });
        params.insert(
            "oversold_threshold".to_string(),
            ParamType::Float {
                min: 10.0,
                max: 40.0,
            },
        );
        params.insert(
            "overbought_threshold".to_string(),
            ParamType::Float {
                min: 60.0,
                max: 90.0,
            },
        );
        // RsiMeanReversion also needs stop_loss_pct, let's include it to avoid failure
        params.insert(
            "stop_loss_pct".to_string(),
            ParamType::Float {
                min: 0.01,
                max: 0.1,
            },
        );

        let request = OptimizationRequest {
            strategy: "RsiMeanReversion".to_string(),
            initial_capital: 10000.0,
            risk_per_trade: 100.0,
            config,
            params,
        };

        let result = optimize(&request, &series).await;

        assert!(result.is_ok(), "Optimization failed: {:?}", result.err());
        let res = result.unwrap();

        println!("Best Params: {:?}", res.best_params);
        println!("Best Fitness: {:.2}%", res.best_fitness);

        assert!(res.best_params.contains_key("period"));
    }
}
