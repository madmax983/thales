use anyhow::Result;
use contracts::BarSeries;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkovConfig {
    pub state_threshold_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkovChainReport {
    pub symbol: String,
    pub transitions: HashMap<String, HashMap<String, f64>>,
    pub state_probabilities: HashMap<String, f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MarketState {
    Bull, // Return > threshold
    Bear, // Return < -threshold
    Flat, // |Return| <= threshold
}

impl MarketState {
    fn from_return(ret: f64, threshold: f64) -> Self {
        if ret > threshold {
            MarketState::Bull
        } else if ret < -threshold {
            MarketState::Bear
        } else {
            MarketState::Flat
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            MarketState::Bull => "Bull",
            MarketState::Bear => "Bear",
            MarketState::Flat => "Flat",
        }
    }
}

pub fn analyze_markov_chain(series: &BarSeries, config: MarkovConfig) -> Result<MarkovChainReport> {
    if series.bars.len() < 2 {
        return Err(anyhow::anyhow!(
            "At least two bars are required for Markov analysis"
        ));
    }

    let symbol = series.bars[0].symbol.clone();
    let threshold = config.state_threshold_pct;

    // Calculate states
    let mut states = Vec::with_capacity(series.bars.len() - 1);
    for i in 1..series.bars.len() {
        let prev = &series.bars[i - 1];
        let curr = &series.bars[i];
        let ret = (curr.close - prev.close) / prev.close;
        states.push(MarketState::from_return(ret, threshold));
    }

    // Count transitions
    let mut transition_counts: HashMap<String, HashMap<String, usize>> = HashMap::new();
    let mut state_counts: HashMap<String, usize> = HashMap::new();

    for i in 0..states.len() - 1 {
        let current_state = states[i].as_str().to_string();
        let next_state = states[i + 1].as_str().to_string();

        *state_counts.entry(current_state.clone()).or_insert(0) += 1;

        let next_counts = transition_counts.entry(current_state).or_default();
        *next_counts.entry(next_state).or_insert(0) += 1;
    }

    // Add the last state to state_counts
    if let Some(last_state) = states.last() {
        *state_counts
            .entry(last_state.as_str().to_string())
            .or_insert(0) += 1;
    }

    // Calculate probabilities
    let mut transitions: HashMap<String, HashMap<String, f64>> = HashMap::new();
    for (current_state, next_counts) in transition_counts {
        // We only divide by the count of current_state transitions we observed.
        // The total number of times current_state was a "from" state is the sum of its next_counts.
        let total_transitions_from_state: usize = next_counts.values().sum();
        let mut probs = HashMap::new();
        for (next_state, count) in next_counts {
            probs.insert(
                next_state,
                count as f64 / total_transitions_from_state as f64,
            );
        }
        transitions.insert(current_state, probs);
    }

    let total_states = states.len();
    let mut state_probabilities = HashMap::new();
    for (state, count) in state_counts {
        state_probabilities.insert(state, count as f64 / total_states as f64);
    }

    Ok(MarkovChainReport {
        symbol,
        transitions,
        state_probabilities,
    })
}

#[cfg(feature = "nova")]
pub fn print_ascii_markov_chain(report: &MarkovChainReport) {
    println!("\nMarkov Chain Analysis for {}", report.symbol);
    println!("--------------------------------------------------");

    println!("State Probabilities (Stationary Distribution Approx):");
    for state in ["Bull", "Bear", "Flat"] {
        let prob = report.state_probabilities.get(state).unwrap_or(&0.0);
        println!("{:>6}: {:.2}%", state, prob * 100.0);
    }

    println!("\nTransition Matrix:");
    println!(
        "{:>10} | {:>10} | {:>10} | {:>10}",
        "From \\ To", "Bull", "Bear", "Flat"
    );
    println!("--------------------------------------------------");

    for from_state in ["Bull", "Bear", "Flat"] {
        print!("{:>10} |", from_state);
        let to_probs = report.transitions.get(from_state);
        for to_state in ["Bull", "Bear", "Flat"] {
            let prob = to_probs.and_then(|p| p.get(to_state)).unwrap_or(&0.0);
            print!(" {:>9.2}% |", prob * 100.0);
        }
        println!();
    }
    println!("--------------------------------------------------\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    fn create_bar(close: f64) -> Bar {
        Bar {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            timeframe: "1d".to_string(),
            timestamp_unix_ms: 100000,
            open: 100.0,
            high: 100.0,
            low: 100.0,
            close,
            volume: 100.0,
        }
    }

    #[test]
    fn test_markov_chain() {
        let bars = vec![
            create_bar(100.0), // Base
            create_bar(105.0), // +5% (Bull)
            create_bar(110.0), // +4.7% (Bull)
            create_bar(110.0), // 0% (Flat)
            create_bar(100.0), // -9% (Bear)
            create_bar(90.0),  // -10% (Bear)
            create_bar(95.0),  // +5.5% (Bull)
        ];

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let config = MarkovConfig {
            state_threshold_pct: 0.02, // 2% threshold
        };

        // States: Bull, Bull, Flat, Bear, Bear, Bull
        // Transitions:
        // Bull -> Bull (1)
        // Bull -> Flat (1)
        // Flat -> Bear (1)
        // Bear -> Bear (1)
        // Bear -> Bull (1)

        let report = analyze_markov_chain(&series, config).unwrap();
        assert_eq!(report.symbol, "TEST");

        let bull_transitions = report.transitions.get("Bull").unwrap();
        assert_eq!(*bull_transitions.get("Bull").unwrap(), 0.5);
        assert_eq!(*bull_transitions.get("Flat").unwrap(), 0.5);

        let flat_transitions = report.transitions.get("Flat").unwrap();
        assert_eq!(*flat_transitions.get("Bear").unwrap(), 1.0);

        let bear_transitions = report.transitions.get("Bear").unwrap();
        assert_eq!(*bear_transitions.get("Bear").unwrap(), 0.5);
        assert_eq!(*bear_transitions.get("Bull").unwrap(), 0.5);

        let bull_prob = *report.state_probabilities.get("Bull").unwrap();
        assert_eq!(bull_prob, 3.0 / 6.0); // 3 Bulls out of 6 states
    }
}
