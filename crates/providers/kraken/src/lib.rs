//! Kraken API provider implementation.
//!
//! This crate provides an adapter for the Kraken cryptocurrency exchange.
//! It handles authentication, request signing, and data normalization.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::{Engine, engine::general_purpose::STANDARD};
use contracts::{Bar, ExecutionResult, TradeIntent};
use hmac::{Hmac, Mac};
use reqwest::blocking::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256, Sha512};
use thiserror::Error;

/// Configuration for the Kraken API client.
///
/// Use [`KrakenConfig::from_env`] to load from environment variables.
///
/// # Examples
///
/// ```rust
/// use kraken_provider::KrakenConfig;
///
/// unsafe {
///     std::env::set_var("KRAKEN_API_KEY", "key");
///     std::env::set_var("KRAKEN_API_SECRET", "secret");
/// }
///
/// let config = KrakenConfig::from_env().unwrap();
/// assert_eq!(config.api_key, "key");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KrakenConfig {
    /// The API Key provided by Kraken.
    pub api_key: String,
    /// The API Secret (Base64 encoded) provided by Kraken.
    pub api_secret: String,
    /// The base URL for the Kraken API (defaults to `<https://api.kraken.com>`).
    pub base_url: String,
}

impl KrakenConfig {
    /// Loads configuration from environment variables.
    ///
    /// # Required Variables
    ///
    /// - `KRAKEN_API_KEY`
    /// - `KRAKEN_API_SECRET`
    ///
    /// # Optional Variables
    ///
    /// - `KRAKEN_BASE_URL`
    pub fn from_env() -> Result<Self, KrakenProviderError> {
        Self::from_env_with(|key| std::env::var(key).ok())
    }

    /// Helper to load configuration from a custom source.
    pub fn from_env_with<F>(get: F) -> Result<Self, KrakenProviderError>
    where
        F: Fn(&str) -> Option<String>,
    {
        Ok(Self {
            api_key: required_var("KRAKEN_API_KEY", &get)?,
            api_secret: required_var("KRAKEN_API_SECRET", &get)?,
            base_url: get("KRAKEN_BASE_URL")
                .unwrap_or_else(|| "https://api.kraken.com".to_string()),
        })
    }
}

fn required_var<F>(name: &'static str, get: &F) -> Result<String, KrakenProviderError>
where
    F: Fn(&str) -> Option<String>,
{
    get(name).ok_or(KrakenProviderError::MissingEnvVar(name))
}

/// A synchronous client for the Kraken API.
///
/// This client handles the complexities of the Kraken API, including:
/// - Request signing (HMAC-SHA512).
/// - Nonce management.
/// - Data normalization to Thales contracts.
///
/// # Examples
///
/// ```rust
/// use kraken_provider::{KrakenClient, KrakenConfig};
///
/// let config = KrakenConfig {
///     api_key: "key".to_string(),
///     api_secret: "secret".to_string(),
///     base_url: "https://api.kraken.com".to_string(),
/// };
///
/// let client = KrakenClient::new(config);
/// ```
#[derive(Debug, Clone)]
pub struct KrakenClient {
    pub config: KrakenConfig,
    http: Client,
}

impl KrakenClient {
    /// Creates a new Kraken client.
    pub fn new(config: KrakenConfig) -> Self {
        Self {
            config,
            http: Client::new(),
        }
    }

    /// Executes a trade intent on Kraken.
    ///
    /// Translates the generic `TradeIntent` into a Kraken-specific order.
    /// Supports:
    /// - Market, Limit, Stop-Loss, Stop-Loss-Limit orders.
    /// - Conditional Close orders (Stop Loss / Take Profit).
    /// - "max" size hint (closes full position).
    pub fn execute_intent(
        &self,
        intent: &TradeIntent,
    ) -> Result<ExecutionResult, KrakenProviderError> {
        validate_side(&intent.side)?;
        if intent.size_hint != "max" {
            validate_size_hint(&intent.size_hint)?;
        }

        let nonce = now_unix_ms()?.to_string();
        let pair = normalize_pair(&intent.symbol);

        // Fetch pair info for precision
        let pair_info = self.get_pair_info(&pair)?;
        let price_decimals = pair_info.pair_decimals as usize;
        let volume_decimals = pair_info.lot_decimals as usize;

        let volume = if intent.size_hint == "max" {
            // Fetch open positions to find size
            let positions = self.fetch_open_positions()?;
            // Normalize pair for comparison (Kraken pairs can be weird)
            // But open positions return specific pair keys.
            // We sum up all positions for this pair.
            let mut total = 0.0;
            for (_key, pos) in positions {
                // Key might be like "XBTUSD" or "XXBTZUSD"
                // Pair we have is normalized.
                // Best effort matching.
                let pos_pair = normalize_pair(&pos.pair);
                if pos_pair == pair
                    && let Ok(v) = pos.vol.parse::<f64>()
                {
                    if let Ok(vc) = pos.vol_closed.parse::<f64>() {
                        total += v - vc;
                    } else {
                        total += v;
                    }
                }
            }
            if total <= 0.0 {
                return Err(KrakenProviderError::InvalidVolume(format!(
                    "No open position found for max exit for {}",
                    pair
                )));
            }
            format!("{:.1$}", total, volume_decimals)
        } else {
            // Reformat size hint to respect lot decimals
            let v = intent
                .size_hint
                .parse::<f64>()
                .map_err(|_| KrakenProviderError::InvalidVolume(intent.size_hint.clone()))?;
            format!("{:.1$}", v, volume_decimals)
        };

        let mut body = format!(
            "nonce={}&type={}&pair={}&volume={}",
            nonce, intent.side, pair, volume
        );

        let ordertype = match intent.order_type.as_str() {
            "market" => "market",
            "limit" => "limit",
            "stop" => "stop-loss",
            "stop_limit" => "stop-loss-limit",
            _ => "market",
        };
        body.push_str(&format!("&ordertype={}", ordertype));

        if ordertype == "limit" {
            if let Some(p) = intent.limit_price {
                body.push_str(&format!("&price={:.1$}", p, price_decimals));
            }
        } else if ordertype == "stop-loss" {
            if let Some(p) = intent.stop_price {
                body.push_str(&format!("&price={:.1$}", p, price_decimals));
            }
        } else if ordertype == "stop-loss-limit" {
            if let Some(p) = intent.stop_price {
                body.push_str(&format!("&price={:.1$}", p, price_decimals));
            }
            if let Some(p) = intent.limit_price {
                body.push_str(&format!("&price2={:.1$}", p, price_decimals));
            }
        }

        if let Some(sl) = intent.stop_loss {
            body.push_str("&close[ordertype]=stop-loss");
            body.push_str(&format!("&close[price]={:.1$}", sl, price_decimals));
        }

        let tif = intent.time_in_force.to_uppercase();
        match tif.as_str() {
            "GTC" | "IOC" => {
                body.push_str(&format!("&timeinforce={}", tif));
            }
            "DAY" => {
                return Err(KrakenProviderError::InvalidTimeInForce(
                    "DAY time-in-force not supported for Kraken. Use GTC or IOC.".to_string(),
                ));
            }
            _ => {
                // For other values, we can either error or pass through if we support more in future.
                // For safety, error on unknown.
                return Err(KrakenProviderError::InvalidTimeInForce(format!(
                    "Unsupported time-in-force: {}",
                    intent.time_in_force
                )));
            }
        }

        let path = "/0/private/AddOrder";
        let signature = sign_request(&self.config.api_secret, path, &nonce, &body)?;
        let url = format!("{}{}", self.config.base_url.trim_end_matches('/'), path);

        let response = self
            .http
            .post(url)
            .header("API-Key", &self.config.api_key)
            .header("API-Sign", signature)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .unwrap_or_else(|_| "unable to decode error body".to_string());
            return Err(KrakenProviderError::UnexpectedHttpStatus(status, body));
        }

        let api_response: KrakenApiResponse = response.json()?;
        if !api_response.error.is_empty() {
            return Err(KrakenProviderError::Api(api_response.error.join(", ")));
        }

        let provider_order_id = api_response
            .result
            .and_then(|result| result.txid.into_iter().next())
            .ok_or(KrakenProviderError::MissingTxid)?;

        let submitted_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| KrakenProviderError::Clock(err.to_string()))?
            .as_millis() as i64;

        if let Some(algo) = &intent.execution_algo {
            eprintln!("Executing with Algo: {}", algo);
        }

        Ok(ExecutionResult {
            schema_version: "v0".to_string(),
            intent_id: intent.intent_id.clone(),
            provider: "kraken".to_string(),
            provider_order_id,
            status: "submitted".to_string(),
            submitted_at_unix_ms,
        })
    }

    /// Fetches open orders.
    pub fn fetch_open_orders(&self) -> Result<Vec<contracts::Order>, KrakenProviderError> {
        let nonce = now_unix_ms()?.to_string();
        let body = format!("nonce={}", nonce);
        let path = "/0/private/OpenOrders";
        let signature = sign_request(&self.config.api_secret, path, &nonce, &body)?;
        let url = format!("{}{}", self.config.base_url.trim_end_matches('/'), path);

        let response = self
            .http
            .post(url)
            .header("API-Key", &self.config.api_key)
            .header("API-Sign", signature)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .unwrap_or_else(|_| "unable to decode error body".to_string());
            return Err(KrakenProviderError::UnexpectedHttpStatus(status, body));
        }

        let api_response: KrakenOpenOrdersResponse = response.json()?;
        if !api_response.error.is_empty() {
            return Err(KrakenProviderError::Api(api_response.error.join(", ")));
        }

        let mut orders = Vec::new();
        if let Some(open) = api_response.result.and_then(|r| r.open) {
            for (txid, info) in open {
                let qty = info.vol.parse::<f64>().unwrap_or(0.0);
                let filled_qty = info.vol_exec.parse::<f64>().unwrap_or(0.0);
                let cost = info.cost.parse::<f64>().unwrap_or(0.0);
                let avg_fill_price = if filled_qty > 0.0 {
                    Some(cost / filled_qty)
                } else {
                    None
                };

                let submitted_at = (info.opentm * 1000.0) as i64;

                orders.push(contracts::Order {
                    id: txid,
                    symbol: info.descr.pair,
                    qty,
                    filled_qty,
                    side: info.descr.type_,
                    order_type: info.descr.ordertype,
                    status: info.status,
                    submitted_at_unix_ms: submitted_at,
                    average_fill_price: avg_fill_price,
                });
            }
        }

        Ok(orders)
    }

    pub fn fetch_order(&self, order_id: &str) -> Result<contracts::Order, KrakenProviderError> {
        let nonce = now_unix_ms()?.to_string();
        let body = format!("nonce={}&txid={}", nonce, order_id);
        let path = "/0/private/QueryOrders";
        let signature = sign_request(&self.config.api_secret, path, &nonce, &body)?;
        let url = format!("{}{}", self.config.base_url.trim_end_matches('/'), path);

        let response = self
            .http
            .post(url)
            .header("API-Key", &self.config.api_key)
            .header("API-Sign", signature)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .unwrap_or_else(|_| "unable to decode error body".to_string());
            return Err(KrakenProviderError::UnexpectedHttpStatus(status, body));
        }

        let api_response: KrakenQueryOrdersResponse = response.json()?;
        if !api_response.error.is_empty() {
            return Err(KrakenProviderError::Api(api_response.error.join(", ")));
        }

        let info = api_response
            .result
            .and_then(|res| res.get(order_id).cloned())
            .ok_or_else(|| KrakenProviderError::Api(format!("Order {} not found", order_id)))?;

        let qty = info.vol.parse::<f64>().unwrap_or(0.0);
        let filled_qty = info.vol_exec.parse::<f64>().unwrap_or(0.0);
        let cost = info.cost.parse::<f64>().unwrap_or(0.0);
        let avg_fill_price = if filled_qty > 0.0 {
            Some(cost / filled_qty)
        } else {
            None
        };

        let submitted_at = (info.opentm * 1000.0) as i64;

        Ok(contracts::Order {
            id: order_id.to_string(),
            symbol: info.descr.pair,
            qty,
            filled_qty,
            side: info.descr.type_,
            order_type: info.descr.ordertype,
            status: info.status,
            submitted_at_unix_ms: submitted_at,
            average_fill_price: avg_fill_price,
        })
    }

    /// Cancels an order.
    pub fn cancel_order(&self, order_id: &str) -> Result<(), KrakenProviderError> {
        let nonce = now_unix_ms()?.to_string();
        let body = format!("nonce={}&txid={}", nonce, order_id);
        let path = "/0/private/CancelOrder";
        let signature = sign_request(&self.config.api_secret, path, &nonce, &body)?;
        let url = format!("{}{}", self.config.base_url.trim_end_matches('/'), path);

        let response = self
            .http
            .post(url)
            .header("API-Key", &self.config.api_key)
            .header("API-Sign", signature)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .unwrap_or_else(|_| "unable to decode error body".to_string());
            return Err(KrakenProviderError::UnexpectedHttpStatus(status, body));
        }

        let api_response: KrakenApiResponse = response.json()?;
        if !api_response.error.is_empty() {
            // "EOrder:Unknown order" is common if already filled/canceled.
            // We might treat it as Ok or Err.
            return Err(KrakenProviderError::Api(api_response.error.join(", ")));
        }

        Ok(())
    }

    /// Fetches asset pair information (decimals, etc).
    pub fn get_pair_info(&self, pair: &str) -> Result<KrakenAssetPairInfo, KrakenProviderError> {
        let url = format!(
            "{}/0/public/AssetPairs?pair={}",
            self.config.base_url.trim_end_matches('/'),
            pair
        );
        let response = self.http.get(&url).send()?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .unwrap_or_else(|_| "unable to decode error body".to_string());
            return Err(KrakenProviderError::UnexpectedHttpStatus(status, body));
        }

        let api_response: KrakenAssetPairsResponse = response.json()?;
        if !api_response.error.is_empty() {
            return Err(KrakenProviderError::Api(api_response.error.join(", ")));
        }

        api_response
            .result
            .and_then(|map| map.values().next().cloned())
            .ok_or_else(|| {
                KrakenProviderError::Api(format!("Asset pair info not found for {}", pair))
            })
    }

    /// Fetches historical price data (OHLCV).
    pub fn fetch_bars(
        &self,
        symbol: &str,
        timeframe: &str,
    ) -> Result<Vec<Bar>, KrakenProviderError> {
        let pair = normalize_pair(symbol);
        let interval = match timeframe {
            "1m" => 1,
            "5m" => 5,
            "15m" => 15,
            "30m" => 30,
            "1h" => 60,
            "4h" => 240,
            "1d" => 1440,
            "1w" => 10080,
            "15d" => 21600,
            _ => return Err(KrakenProviderError::InvalidTimeframe(timeframe.to_string())),
        };

        let url = format!(
            "{}/0/public/OHLC?pair={}&interval={}",
            self.config.base_url.trim_end_matches('/'),
            pair,
            interval
        );
        let response = self.http.get(&url).send()?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .unwrap_or_else(|_| "unable to decode error body".to_string());
            return Err(KrakenProviderError::UnexpectedHttpStatus(status, body));
        }

        let api_response: KrakenOhlcResponse = response.json()?;
        if !api_response.error.is_empty() {
            return Err(KrakenProviderError::Api(api_response.error.join(", ")));
        }

        let mut bars = Vec::new();
        if let Some(result) = api_response.result {
            for (key, val) in result {
                if key == "last" {
                    continue;
                }
                if let serde_json::Value::Array(arr) = val {
                    for item in arr {
                        if let serde_json::Value::Array(ohlc) = item {
                            // [time, open, high, low, close, vwap, volume, count]
                            let time = ohlc[0].as_i64().unwrap_or(0) * 1000;
                            let open = ohlc[1].as_str().unwrap_or("0").parse().unwrap_or(0.0);
                            let high = ohlc[2].as_str().unwrap_or("0").parse().unwrap_or(0.0);
                            let low = ohlc[3].as_str().unwrap_or("0").parse().unwrap_or(0.0);
                            let close = ohlc[4].as_str().unwrap_or("0").parse().unwrap_or(0.0);
                            let volume = ohlc[6].as_str().unwrap_or("0").parse().unwrap_or(0.0);

                            bars.push(Bar {
                                symbol: symbol.to_string(),
                                market: "crypto".to_string(),
                                timeframe: timeframe.to_string(),
                                timestamp_unix_ms: time,
                                open,
                                high,
                                low,
                                close,
                                volume,
                            });
                        }
                    }
                }
            }
        }
        // Sort by time
        bars.sort_by_key(|b| b.timestamp_unix_ms);
        Ok(bars)
    }

    /// Fetches ticker information for all pairs.
    pub fn fetch_tickers(&self) -> Result<HashMap<String, KrakenTickerInfo>, KrakenProviderError> {
        let url = format!(
            "{}/0/public/Ticker",
            self.config.base_url.trim_end_matches('/')
        );
        let response = self.http.get(&url).send()?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .unwrap_or_else(|_| "unable to decode error body".to_string());
            return Err(KrakenProviderError::UnexpectedHttpStatus(status, body));
        }

        let api_response: KrakenTickerResponse = response.json()?;
        if !api_response.error.is_empty() {
            return Err(KrakenProviderError::Api(api_response.error.join(", ")));
        }

        Ok(api_response.result.unwrap_or_default())
    }

    /// Fetches all open positions for the account.
    ///
    /// This requires the `OpenPositions` permission on the API key.
    pub fn fetch_open_positions(
        &self,
    ) -> Result<HashMap<String, KrakenOpenPosition>, KrakenProviderError> {
        let nonce = now_unix_ms()?.to_string();
        let body = format!("nonce={}", nonce);
        let path = "/0/private/OpenPositions";
        let signature = sign_request(&self.config.api_secret, path, &nonce, &body)?;
        let url = format!("{}{}", self.config.base_url.trim_end_matches('/'), path);

        let response = self
            .http
            .post(url)
            .header("API-Key", &self.config.api_key)
            .header("API-Sign", signature)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .unwrap_or_else(|_| "unable to decode error body".to_string());
            return Err(KrakenProviderError::UnexpectedHttpStatus(status, body));
        }

        let api_response: KrakenOpenPositionsResponse = response.json()?;
        if !api_response.error.is_empty() {
            return Err(KrakenProviderError::Api(api_response.error.join(", ")));
        }

        Ok(api_response.result.unwrap_or_default())
    }

    /// Fetches account balances.
    pub fn fetch_balance(&self) -> Result<HashMap<String, String>, KrakenProviderError> {
        let nonce = now_unix_ms()?.to_string();
        let body = format!("nonce={}", nonce);
        let path = "/0/private/Balance";
        let signature = sign_request(&self.config.api_secret, path, &nonce, &body)?;
        let url = format!("{}{}", self.config.base_url.trim_end_matches('/'), path);

        let response = self
            .http
            .post(url)
            .header("API-Key", &self.config.api_key)
            .header("API-Sign", signature)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .unwrap_or_else(|_| "unable to decode error body".to_string());
            return Err(KrakenProviderError::UnexpectedHttpStatus(status, body));
        }

        let api_response: KrakenBalanceResponse = response.json()?;
        if !api_response.error.is_empty() {
            return Err(KrakenProviderError::Api(api_response.error.join(", ")));
        }

        Ok(api_response.result.unwrap_or_default())
    }

    /// Returns available quote-currency balance for a trading symbol.
    pub fn get_buying_power_for_symbol(
        &self,
        symbol: &str,
    ) -> Result<(String, f64), KrakenProviderError> {
        let pair = normalize_pair(symbol);
        let quote = quote_currency_from_pair(&pair)
            .ok_or_else(|| KrakenProviderError::UnsupportedQuoteCurrency(pair.clone()))?;
        let balances = self.fetch_balance()?;
        let amount = parse_balance_for_currency(&balances, quote)
            .ok_or_else(|| KrakenProviderError::MissingBalanceForCurrency(quote.to_string()))?;
        Ok((quote.to_string(), amount))
    }

    /// Returns available base-asset balance that can be sold for a trading symbol.
    pub fn get_sellable_balance_for_symbol(
        &self,
        symbol: &str,
    ) -> Result<(String, f64), KrakenProviderError> {
        let pair = normalize_pair(symbol);
        let base = base_currency_from_pair(&pair)
            .ok_or_else(|| KrakenProviderError::UnsupportedBaseCurrency(pair.clone()))?;
        let balances = self.fetch_balance()?;
        let amount = parse_balance_for_currency(&balances, base)
            .ok_or_else(|| KrakenProviderError::MissingBalanceForCurrency(base.to_string()))?;
        Ok((base.to_string(), amount))
    }

    pub fn get_open_positions(&self) -> Result<Vec<contracts::Position>, KrakenProviderError> {
        let open_positions = self.fetch_open_positions()?;

        // Aggregate
        // Key: (Pair, Type)
        // Value: (Total Qty, Total Cost)
        let mut agg: HashMap<(String, String), (f64, f64)> = HashMap::new();

        for pos in open_positions.values() {
            let vol: f64 = pos.vol.parse().unwrap_or(0.0);
            let vol_closed: f64 = pos.vol_closed.parse().unwrap_or(0.0);
            let cost: f64 = pos.cost.parse().unwrap_or(0.0);

            let current_qty = vol - vol_closed;
            if current_qty <= 0.00000001 {
                continue;
            } // effectively closed

            // Proportional cost for remaining qty
            // Assuming cost is for initial 'vol'
            let current_cost = if vol > 0.0 {
                cost * (current_qty / vol)
            } else {
                0.0
            };

            let key = (normalize_pair(&pos.pair), pos.type_.clone());
            let entry = agg.entry(key).or_insert((0.0, 0.0));
            entry.0 += current_qty;
            entry.1 += current_cost;
        }

        let mut positions = Vec::new();
        for ((pair, side), (qty, total_cost)) in agg {
            let entry_price = if qty > 0.0 {
                Some(total_cost / qty)
            } else {
                None
            };
            let side = if side == "buy" { "long" } else { "short" }; // Map Kraken "buy"/"sell" to "long"/"short"

            positions.push(contracts::Position {
                symbol: pair,
                side: side.to_string(),
                qty,
                entry_price,
            });
        }

        Ok(positions)
    }
}

fn now_unix_ms() -> Result<i64, KrakenProviderError> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| KrakenProviderError::Clock(err.to_string()))?
        .as_millis() as i64)
}

fn validate_side(side: &str) -> Result<(), KrakenProviderError> {
    if side == "buy" || side == "sell" {
        Ok(())
    } else {
        Err(KrakenProviderError::InvalidSide(side.to_string()))
    }
}

fn validate_size_hint(size_hint: &str) -> Result<(), KrakenProviderError> {
    let volume = size_hint
        .parse::<f64>()
        .map_err(|_| KrakenProviderError::InvalidVolume(size_hint.to_string()))?;
    if volume <= 0.0 {
        return Err(KrakenProviderError::InvalidVolume(size_hint.to_string()));
    }
    Ok(())
}

fn normalize_pair(symbol: &str) -> String {
    symbol.replace('/', "").to_uppercase()
}

fn quote_currency_from_pair(pair: &str) -> Option<&'static str> {
    // Longest suffixes first.
    const KNOWN_QUOTES: [&str; 11] = [
        "USDT", "USDC", "USD", "EUR", "GBP", "JPY", "AUD", "CAD", "CHF", "BTC", "ETH",
    ];
    KNOWN_QUOTES.into_iter().find(|quote| pair.ends_with(quote))
}

fn base_currency_from_pair(pair: &str) -> Option<&str> {
    let quote = quote_currency_from_pair(pair)?;
    pair.strip_suffix(quote)
}

fn parse_balance_for_currency(balances: &HashMap<String, String>, currency: &str) -> Option<f64> {
    let upper = currency.to_uppercase();
    let mut keys = vec![upper.clone(), format!("Z{}", upper), format!("X{}", upper)];
    if upper == "BTC" {
        keys.push("XBT".to_string());
        keys.push("XXBT".to_string());
    }

    for key in keys {
        if let Some(raw) = balances.get(&key).and_then(|r| r.parse::<f64>().ok()) {
            return Some(raw);
        }
    }

    None
}

fn sign_request(
    api_secret: &str,
    path: &str,
    nonce: &str,
    body: &str,
) -> Result<String, KrakenProviderError> {
    let secret = STANDARD.decode(api_secret)?;

    let mut hasher = Sha256::new();
    hasher.update(nonce.as_bytes());
    hasher.update(body.as_bytes());
    let body_hash = hasher.finalize();

    let mut message = Vec::with_capacity(path.len() + body_hash.len());
    message.extend_from_slice(path.as_bytes());
    message.extend_from_slice(&body_hash);

    let mut mac = Hmac::<Sha512>::new_from_slice(&secret)
        .map_err(|err| KrakenProviderError::Signing(err.to_string()))?;
    mac.update(&message);
    let signature = mac.finalize().into_bytes();

    Ok(STANDARD.encode(signature))
}

#[derive(Debug, Clone, Deserialize)]
struct KrakenApiResponse {
    error: Vec<String>,
    result: Option<KrakenAddOrderResult>,
}

#[derive(Debug, Clone, Deserialize)]
struct KrakenAddOrderResult {
    txid: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct KrakenOhlcResponse {
    error: Vec<String>,
    result: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Deserialize)]
struct KrakenAssetPairsResponse {
    error: Vec<String>,
    result: Option<HashMap<String, KrakenAssetPairInfo>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KrakenAssetPairInfo {
    #[serde(default)]
    pub pair_decimals: u32,
    #[serde(default)]
    pub lot_decimals: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct KrakenTickerResponse {
    error: Vec<String>,
    result: Option<HashMap<String, KrakenTickerInfo>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KrakenTickerInfo {
    pub a: Vec<String>,
    pub b: Vec<String>,
    pub c: Vec<String>,
    pub v: Vec<String>,
    pub p: Vec<String>,
    pub t: Vec<i64>,
    pub l: Vec<String>,
    pub h: Vec<String>,
    pub o: String,
}

#[derive(Debug, Clone, Deserialize)]
struct KrakenOpenPositionsResponse {
    error: Vec<String>,
    result: Option<HashMap<String, KrakenOpenPosition>>,
}

#[derive(Debug, Clone, Deserialize)]
struct KrakenBalanceResponse {
    error: Vec<String>,
    result: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KrakenOpenPosition {
    pub ordertxid: String,
    pub pair: String,
    pub time: f64,
    #[serde(rename = "type")]
    pub type_: String,
    pub ordertype: String,
    pub cost: String,
    pub fee: String,
    pub vol: String,
    pub vol_closed: String,
    pub margin: String,
}

#[derive(Debug, Clone, Deserialize)]
struct KrakenOpenOrdersResponse {
    error: Vec<String>,
    result: Option<KrakenOpenOrdersResult>,
}

#[derive(Debug, Clone, Deserialize)]
struct KrakenOpenOrdersResult {
    open: Option<HashMap<String, KrakenOrderInfo>>,
}

#[derive(Debug, Clone, Deserialize)]
struct KrakenQueryOrdersResponse {
    error: Vec<String>,
    result: Option<HashMap<String, KrakenOrderInfo>>,
}

#[derive(Debug, Clone, Deserialize)]
struct KrakenOrderInfo {
    status: String,
    opentm: f64,
    vol: String,
    vol_exec: String,
    cost: String,
    descr: KrakenOrderDescription,
}

#[derive(Debug, Clone, Deserialize)]
struct KrakenOrderDescription {
    pair: String,
    #[serde(rename = "type")]
    type_: String,
    ordertype: String,
}

#[derive(Debug, Error)]
pub enum KrakenProviderError {
    #[error("missing required environment variable: {0}")]
    MissingEnvVar(&'static str),
    #[error("system clock error: {0}")]
    Clock(String),
    #[error("invalid side: {0}")]
    InvalidSide(String),
    #[error("invalid volume: {0}")]
    InvalidVolume(String),
    #[error("unsupported quote currency for pair: {0}")]
    UnsupportedQuoteCurrency(String),
    #[error("unsupported base currency for pair: {0}")]
    UnsupportedBaseCurrency(String),
    #[error("missing balance for currency: {0}")]
    MissingBalanceForCurrency(String),
    #[error("invalid kraken api secret encoding: {0}")]
    SecretDecode(#[from] base64::DecodeError),
    #[error("kraken request signing error: {0}")]
    Signing(String),
    #[error("http transport error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("unexpected kraken response status {0}: {1}")]
    UnexpectedHttpStatus(u16, String),
    #[error("kraken api error: {0}")]
    Api(String),
    #[error("kraken response missing txid")]
    MissingTxid,
    #[error("invalid time in force: {0}")]
    InvalidTimeInForce(String),
    #[error("invalid timeframe: {0}")]
    InvalidTimeframe(String),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}
