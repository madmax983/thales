//! Shared data contracts for the Thales ecosystem.
//!
//! This crate defines the core data structures used to communicate between
//! the CLI, providers, strategies, and other components. It acts as the
//! "vocabulary" of the system.
//!
//! All structs are `Serialize` and `Deserialize` to facilitate JSON-based communication.

use serde::{Deserialize, Serialize};

/// Represents a desire to make a trade.
///
/// This struct is the output of the signal generation phase and the input to the
/// execution phase. It captures *what* to trade, *how* to trade it, and *why*.
///
/// # Examples
///
/// ```rust
/// use contracts::TradeIntent;
///
/// let intent = TradeIntent {
///     intent_id: "crypto:BTCUSD:buy:v1".to_string(),
///     market: "crypto".to_string(),
///     symbol: "BTCUSD".to_string(),
///     side: "buy".to_string(),
///     size_hint: "0.5".to_string(),
///     confidence: 0.95,
///     rationale: "RSI Oversold".to_string(),
///     ..Default::default()
/// };
///
/// assert_eq!(intent.symbol, "BTCUSD");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradeIntent {
    /// A unique identifier for this specific intent (e.g., "crypto:BTCUSD:buy:v0").
    pub intent_id: String,
    /// The market type (e.g., "crypto", "equities").
    pub market: String,
    /// The symbol to trade (e.g., "BTCUSD", "AAPL").
    pub symbol: String,
    /// The side of the trade ("buy" or "sell").
    pub side: String,
    /// The size of the trade. Can be a specific amount (e.g., "1.5") or a hint like "max".
    pub size_hint: String,
    /// A score from 0.0 to 1.0 indicating the confidence in this trade.
    pub confidence: f64,
    /// The expected time horizon for the trade (e.g., "1h", "1d").
    pub horizon: String,
    /// A human-readable explanation of why this trade was generated.
    pub rationale: String,
    /// Conditions that would invalidate this trade idea.
    pub invalidation: String,
    /// The schema version for this data structure (e.g., "v0").
    pub schema_version: String,
    /// The specific type of signal (e.g., "Entry", "Exit", "ScaleIn").
    #[serde(default)]
    pub signal_type: Option<String>,
    /// The stop loss price level, if any.
    #[serde(default)]
    pub stop_loss: Option<f64>,
    /// The take profit price level, if any.
    #[serde(default)]
    pub take_profit: Option<f64>,
    /// The type of order to place ("market", "limit", "stop", "stop_limit").
    pub order_type: String,
    /// The limit price for Limit and Stop-Limit orders.
    #[serde(default)]
    pub limit_price: Option<f64>,
    /// The stop price for Stop and Stop-Limit orders.
    #[serde(default)]
    pub stop_price: Option<f64>,
    /// The time in force for the order (e.g., "GTC", "IOC", "day").
    pub time_in_force: String,
    /// The execution algorithm to use (e.g., "Market", "Limit", "TWAP", "VWAP").
    #[serde(default)]
    pub execution_algo: Option<String>,
    /// The strategy that generated this intent (e.g., "BollingerBands").
    #[serde(default)]
    pub strategy: String,
}

impl Default for TradeIntent {
    fn default() -> Self {
        Self {
            intent_id: String::new(),
            market: String::new(),
            symbol: String::new(),
            side: String::new(),
            size_hint: String::new(),
            confidence: 0.0,
            horizon: String::new(),
            rationale: String::new(),
            invalidation: String::new(),
            schema_version: "v0".to_string(),
            signal_type: None,
            stop_loss: None,
            take_profit: None,
            order_type: "market".to_string(),
            limit_price: None,
            stop_price: None,
            time_in_force: "day".to_string(),
            execution_algo: None,
            strategy: String::new(),
        }
    }
}

/// A single candlestick bar representing price data.
///
/// # Examples
///
/// ```rust
/// use contracts::Bar;
///
/// let bar = Bar {
///     symbol: "AAPL".to_string(),
///     market: "equities".to_string(),
///     timeframe: "1d".to_string(),
///     timestamp_unix_ms: 1622505600000,
///     open: 125.0,
///     high: 126.5,
///     low: 124.8,
///     close: 126.0,
///     volume: 1_000_000.0,
/// };
///
/// assert_eq!(bar.close, 126.0);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bar {
    /// The symbol this bar belongs to.
    pub symbol: String,
    /// The market type.
    pub market: String,
    /// The timeframe of the bar (e.g., "1m", "1h").
    pub timeframe: String,
    /// The start time of the bar in milliseconds since Unix epoch.
    pub timestamp_unix_ms: i64,
    /// The opening price.
    pub open: f64,
    /// The highest price during the interval.
    pub high: f64,
    /// The lowest price during the interval.
    pub low: f64,
    /// The closing price.
    pub close: f64,
    /// The volume traded during the interval.
    pub volume: f64,
}

/// A collection of bars for a specific symbol.
///
/// # Examples
///
/// ```rust
/// use contracts::{Bar, BarSeries};
///
/// let series = BarSeries {
///     schema_version: "v0".to_string(),
///     bars: vec![
///         Bar {
///             symbol: "AAPL".to_string(),
///             market: "equities".to_string(),
///             timeframe: "1d".to_string(),
///             timestamp_unix_ms: 1622505600000,
///             open: 125.0, high: 126.0, low: 124.0, close: 125.5, volume: 1000.0
///         }
///     ],
/// };
///
/// assert_eq!(series.bars.len(), 1);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BarSeries {
    /// The schema version.
    pub schema_version: String,
    /// The list of bars, typically sorted by timestamp.
    pub bars: Vec<Bar>,
}

/// A request to execute a specific trade intent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionRequest {
    /// The schema version.
    pub schema_version: String,
    /// The provider to use for execution (e.g., "kraken", "alpaca").
    pub provider: String,
    /// The trade intent to execute.
    pub intent: TradeIntent,
}

/// The result of an execution attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// The schema version.
    pub schema_version: String,
    /// The ID of the intent that was executed.
    pub intent_id: String,
    /// The provider used.
    pub provider: String,
    /// The order ID returned by the provider.
    pub provider_order_id: String,
    /// The status of the execution (e.g., "submitted").
    pub status: String,
    /// The timestamp when the order was submitted.
    pub submitted_at_unix_ms: i64,
}

/// The status of a response envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvelopeStatus {
    /// The operation was successful.
    Ok,
    /// The operation failed.
    Error,
}

/// A standard wrapper for all CLI outputs.
///
/// This envelope ensures consistent parsing of results, errors, and warnings
/// across all commands.
///
/// # Examples
///
/// ```rust
/// use contracts::{ResponseEnvelope, EnvelopeStatus};
///
/// let envelope = ResponseEnvelope {
///     status: EnvelopeStatus::Ok,
///     errors: vec![],
///     warnings: vec!["Low confidence".to_string()],
///     data: Some(42),
/// };
///
/// assert_eq!(envelope.data, Some(42));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResponseEnvelope<T> {
    /// The overall status of the operation.
    pub status: EnvelopeStatus,
    /// A list of error messages, if any.
    pub errors: Vec<String>,
    /// A list of warning messages, if any.
    pub warnings: Vec<String>,
    /// The payload data, if successful.
    pub data: Option<T>,
}

/// Detailed market analysis data.
///
/// This struct holds the result of technical and sentiment analysis on market data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketAnalysis {
    /// The symbol analyzed.
    pub symbol: String,
    /// The market type.
    pub market: String,
    /// The detected market regime (e.g., "Trending", "Range").
    pub regime: String,
    /// The market sentiment (e.g., "Bullish", "Bearish").
    pub sentiment: String,
    /// Detected chart patterns (e.g., "Double Bottom").
    pub patterns: Vec<String>,
    /// Key support and resistance levels.
    pub key_levels: Vec<f64>,
    /// Volatility description (e.g., "High", "Low").
    pub volatility: String,
    /// The Average True Range (ATR) value.
    #[serde(default)]
    pub atr: Option<f64>,
    /// Summary of external research (if injected).
    #[serde(default)]
    pub research_summary: Option<String>,
    /// Summary of relevant news (if injected).
    #[serde(default)]
    pub news_summary: Option<String>,
    /// Strategic recommendation based on the analysis.
    #[serde(default)]
    pub recommendation: Option<String>,
    /// Overall confidence score for the analysis.
    pub confidence: f64,
    /// Timestamp of the analysis.
    pub timestamp_unix_ms: i64,
}

/// Represents an open position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Position {
    /// The symbol of the position (e.g., "BTCUSD", "AAPL").
    pub symbol: String,
    /// The side of the position ("long" or "short").
    pub side: String,
    /// The quantity held.
    pub qty: f64,
    /// The average entry price.
    pub entry_price: Option<f64>,
}

/// Represents an active order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Order {
    /// The unique order ID.
    pub id: String,
    /// The symbol of the order (e.g., "BTCUSD", "AAPL").
    pub symbol: String,
    /// The quantity ordered.
    pub qty: f64,
    /// The quantity filled.
    pub filled_qty: f64,
    /// The side of the order ("buy" or "sell").
    pub side: String,
    /// The type of order ("market", "limit", etc.).
    pub order_type: String,
    /// The current status of the order (e.g., "new", "filled", "canceled").
    pub status: String,
    /// The timestamp when the order was submitted (Unix ms).
    pub submitted_at_unix_ms: i64,
    /// The average price at which the order was filled, if any.
    pub average_fill_price: Option<f64>,
}
