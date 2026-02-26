//! Paper Trading Provider
//!
//! A local simulator for executing trades without real money.
//! It persists positions to a JSON file.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use contracts::{Bar, ExecutionResult, TradeIntent};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const DEFAULT_PORTFOLIO_FILE: &str = "paper_portfolio.json";

/// Configuration for the Paper provider.
#[derive(Debug, Clone)]
pub struct PaperConfig {
    pub portfolio_path: PathBuf,
}

impl PaperConfig {
    pub fn from_env() -> Self {
        let path = std::env::var("PAPER_PORTFOLIO_PATH")
            .unwrap_or_else(|_| DEFAULT_PORTFOLIO_FILE.to_string());
        Self {
            portfolio_path: PathBuf::from(path),
        }
    }
}

/// A local paper trading client.
#[derive(Debug, Clone)]
pub struct PaperClient {
    pub config: PaperConfig,
    http: Client,
}

impl PaperClient {
    pub fn new(config: PaperConfig) -> Self {
        Self {
            config,
            http: Client::new(),
        }
    }

    /// Fetches mock market data for testing strategies.
    pub fn fetch_bars(&self, symbol: &str, timeframe: &str) -> Result<Vec<Bar>, PaperProviderError> {
        let mut bars = Vec::new();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| PaperProviderError::Clock(e.to_string()))?
            .as_millis() as i64;

        let interval_ms = match timeframe {
            "1m" => 60000,
            "5m" => 300000,
            "15m" => 900000,
            "1h" => 3600000,
            "4h" => 14400000,
            "1d" => 86400000,
            _ => 3600000, // Default 1h
        };

        // Determine trend based on symbol (match Signals.md scenarios)
        let is_crypto = symbol.contains("BTC") || symbol.contains("ETH") || symbol.contains("XBT");
        // Downtrend for BTC, Uptrend for ETH/SPY
        let is_downtrend = symbol.contains("BTC") || symbol.contains("XBT");

        // Start price
        let mut price = if symbol.contains("BTC") || symbol.contains("XBT") {
            65000.0
        } else if symbol.contains("ETH") {
            3000.0
        } else if symbol.contains("SPY") {
            500.0
        } else if is_crypto {
            10.0
        } else {
            150.0
        };

        // Generate 100 bars ending at now
        for i in (0..100).rev() {
            let timestamp = now - (i * interval_ms);

            // Trend component: Flat for 95 bars, then strong trend for last 5 bars
            // to trigger breakdown/breakout without being oversold/overbought for too long.
            let trend = if i < 5 {
                if is_downtrend { -0.02 } else { 0.02 }
            } else {
                0.0
            };
            // Volatility component (sine wave)
            let vol = (i as f64 * 0.2).sin() * 0.005;

            let change_pct = trend + vol;
            let open = price;
            let close = price * (1.0 + change_pct);
            let high = open.max(close) * 1.002;
            let low = open.min(close) * 0.998;

            bars.push(Bar {
                symbol: symbol.to_string(),
                market: if is_crypto { "crypto".to_string() } else { "equities".to_string() },
                timeframe: timeframe.to_string(),
                timestamp_unix_ms: timestamp,
                open,
                high,
                low,
                close,
                volume: 1000.0 + (i as f64 * 10.0),
            });

            price = close;
        }

        Ok(bars)
    }

    /// Executes a trade intent against the local paper portfolio.
    pub fn execute_intent(
        &self,
        intent: &TradeIntent,
    ) -> Result<ExecutionResult, PaperProviderError> {
        validate_side(&intent.side)?;
        if intent.size_hint != "max" {
            validate_size_hint(&intent.size_hint)?;
        }

        let mut portfolio = self.load_portfolio()?;
        let symbol = intent.symbol.to_uppercase();

        // Determine execution price
        let price = self.determine_price(intent)?;

        // Determine Quantity to trade
        // Note: Intent quantity is always positive. Side determines direction.
        let intent_qty = if intent.size_hint == "max" {
             // If max, we want to close the entire position.
             // So qty matches the absolute value of current position.
             if let Some(pos) = portfolio.positions.get(&symbol) {
                 pos.qty.abs()
             } else {
                 return Err(PaperProviderError::InvalidSizeHint(format!("No open position found for max exit for {}", symbol)));
             }
        } else {
            intent.size_hint.parse::<f64>().unwrap_or(0.0)
        };

        if intent_qty <= 0.0 {
             return Err(PaperProviderError::InvalidSizeHint("Quantity must be positive".to_string()));
        }

        // Update Portfolio
        let position = portfolio.positions.entry(symbol.clone()).or_insert(PaperPosition {
            symbol: symbol.clone(),
            qty: 0.0,
            avg_price: 0.0,
        });

        // Calculate trade impact
        // Trade Amount (signed): Buy -> +qty, Sell -> -qty
        let trade_amount = if intent.side == "buy" { intent_qty } else { -intent_qty };

        // Update Average Price only if increasing position size (absolute)
        // or flipping side.
        // If reducing position, Avg Price stays same (FIFO/Weighted assumption for PnL).

        let old_qty = position.qty;
        let new_qty = old_qty + trade_amount;

        if (old_qty >= 0.0 && trade_amount > 0.0) || (old_qty <= 0.0 && trade_amount < 0.0) {
            // Increasing position (Longer or Shorter)
            // New Avg = (OldVal + NewVal) / NewQty
            // Val = Qty * Price
            let total_val = (old_qty.abs() * position.avg_price) + (intent_qty * price);
            position.avg_price = total_val / new_qty.abs();
        }
        // Else: Reducing position or flipping.
        // If flipping (e.g. Long 10, Sell 20 -> Short 10), the new avg price for the Short part
        // should be the execution price.
        else if old_qty.signum() != new_qty.signum() && new_qty != 0.0 {
            // Flipped
            // The part that closed the old position uses old avg price (for PnL).
            // The part that opened the new position uses new execution price.
            position.avg_price = price;
        }

        position.qty = new_qty;

        // Clean up empty positions
        if position.qty.abs() < 1e-8 {
            portfolio.positions.remove(&symbol);
        }

        self.save_portfolio(&portfolio)?;

        let submitted_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| PaperProviderError::Clock(err.to_string()))?
            .as_millis() as i64;

        if let Some(algo) = &intent.execution_algo {
            eprintln!("Paper Executing with Algo: {}", algo);
        }

        Ok(ExecutionResult {
            schema_version: "v0".to_string(),
            intent_id: intent.intent_id.clone(),
            provider: "paper".to_string(),
            provider_order_id: format!("paper-{}-{}", symbol, submitted_at_unix_ms),
            status: "filled".to_string(),
            submitted_at_unix_ms,
        })
    }

    pub fn fetch_open_orders(&self) -> Result<Vec<contracts::Order>, PaperProviderError> {
        // Paper trading executes immediately, so there are no open orders.
        Ok(Vec::new())
    }

    pub fn cancel_order(&self, _order_id: &str) -> Result<(), PaperProviderError> {
        // No open orders to cancel.
        Ok(())
    }

    pub fn get_open_positions(&self) -> Result<Vec<contracts::Position>, PaperProviderError> {
        let portfolio = self.load_portfolio()?;
        Ok(portfolio.positions.values().map(|p| {
            let side = if p.qty >= 0.0 { "long" } else { "short" };
            contracts::Position {
                symbol: p.symbol.clone(),
                side: side.to_string(),
                qty: p.qty.abs(),
                entry_price: Some(p.avg_price),
            }
        }).collect())
    }

    fn load_portfolio(&self) -> Result<PaperPortfolio, PaperProviderError> {
        if !self.config.portfolio_path.exists() {
            return Ok(PaperPortfolio { positions: HashMap::new() });
        }
        let content = fs::read_to_string(&self.config.portfolio_path)?;
        if content.trim().is_empty() {
            return Ok(PaperPortfolio { positions: HashMap::new() });
        }
        let portfolio: PaperPortfolio = serde_json::from_str(&content)?;
        Ok(portfolio)
    }

    fn save_portfolio(&self, portfolio: &PaperPortfolio) -> Result<(), PaperProviderError> {
        let content = serde_json::to_string_pretty(portfolio)?;
        fs::write(&self.config.portfolio_path, content)?;
        Ok(())
    }

    /// Determines the execution price.
    fn determine_price(&self, intent: &TradeIntent) -> Result<f64, PaperProviderError> {
        if let Some(limit) = intent.limit_price {
            return Ok(limit);
        }

        if let Ok(price) = self.fetch_kraken_price(&intent.symbol) {
            return Ok(price);
        }

        if let Some(stop) = intent.stop_price {
            return Ok(stop);
        }

        // Fallback
        eprintln!("Warning: Using dummy price 100.0 for paper trade {}", intent.symbol);
        Ok(100.0)
    }

    fn fetch_kraken_price(&self, symbol: &str) -> Result<f64, PaperProviderError> {
        let pair = symbol.replace('/', "").to_uppercase();
        let url = format!("https://api.kraken.com/0/public/Ticker?pair={}", pair);

        let resp = self.http.get(&url).send()?;
        if !resp.status().is_success() {
             return Err(PaperProviderError::Api("Kraken request failed".to_string()));
        }

        let json: serde_json::Value = resp.json()?;
        if let Some(result) = json.get("result") {
            if let Some(obj) = result.as_object() {
                if let Some(ticker) = obj.values().next() {
                     if let Some(c) = ticker.get("c") {
                         if let Some(price_str) = c.get(0).and_then(|v| v.as_str()) {
                             if let Ok(price) = price_str.parse::<f64>() {
                                 return Ok(price);
                             }
                         }
                     }
                }
            }
        }
        Err(PaperProviderError::Api("Price not found".to_string()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PaperPortfolio {
    positions: HashMap<String, PaperPosition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PaperPosition {
    symbol: String,
    // Store signed qty: >0 Long, <0 Short
    qty: f64,
    avg_price: f64,
}

fn validate_side(side: &str) -> Result<(), PaperProviderError> {
    if side == "buy" || side == "sell" {
        Ok(())
    } else {
        Err(PaperProviderError::InvalidSide(side.to_string()))
    }
}

fn validate_size_hint(size_hint: &str) -> Result<(), PaperProviderError> {
    let qty = size_hint
        .parse::<f64>()
        .map_err(|_| PaperProviderError::InvalidSizeHint(size_hint.to_string()))?;
    if qty < 0.0 {
        return Err(PaperProviderError::InvalidSizeHint(size_hint.to_string()));
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum PaperProviderError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("http transport error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("invalid side: {0}")]
    InvalidSide(String),
    #[error("invalid size hint: {0}")]
    InvalidSizeHint(String),
    #[error("system clock error: {0}")]
    Clock(String),
    #[error("api error: {0}")]
    Api(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_paper_execution_limit_order() {
        let temp_file = NamedTempFile::new().unwrap();
        let config = PaperConfig {
            portfolio_path: temp_file.path().to_path_buf(),
        };
        let client = PaperClient::new(config);

        let intent = TradeIntent {
            intent_id: "test:1".to_string(),
            symbol: "BTCUSD".to_string(),
            side: "buy".to_string(),
            size_hint: "1.0".to_string(),
            limit_price: Some(50000.0),
            ..Default::default()
        };

        let result = client.execute_intent(&intent).unwrap();
        assert_eq!(result.status, "filled");

        let positions = client.get_open_positions().unwrap();
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].symbol, "BTCUSD");
        assert_eq!(positions[0].qty, 1.0);
        assert_eq!(positions[0].entry_price, Some(50000.0));
    }

    #[test]
    fn test_paper_execution_sell_partial() {
        let temp_file = NamedTempFile::new().unwrap();
        let config = PaperConfig {
            portfolio_path: temp_file.path().to_path_buf(),
        };
        let client = PaperClient::new(config);

        // Buy 1.0
        let buy_intent = TradeIntent {
            intent_id: "test:buy".to_string(),
            symbol: "BTCUSD".to_string(),
            side: "buy".to_string(),
            size_hint: "1.0".to_string(),
            limit_price: Some(50000.0),
            ..Default::default()
        };
        client.execute_intent(&buy_intent).unwrap();

        // Sell 0.5
        let sell_intent = TradeIntent {
            intent_id: "test:sell".to_string(),
            symbol: "BTCUSD".to_string(),
            side: "sell".to_string(),
            size_hint: "0.5".to_string(),
            limit_price: Some(60000.0),
            ..Default::default()
        };
        client.execute_intent(&sell_intent).unwrap();

        let positions = client.get_open_positions().unwrap();
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].qty, 0.5);
        assert_eq!(positions[0].entry_price, Some(50000.0)); // FIFO: entry price unchanged
    }
}
