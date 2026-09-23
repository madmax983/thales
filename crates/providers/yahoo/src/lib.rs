//! Yahoo Finance market-data provider.
//!
//! Wraps Yahoo Finance's public chart API
//! (`https://query1.finance.yahoo.com/v8/finance/chart/<SYMBOL>`) with a
//! browser User-Agent. No API key, no signup, no credentials of any kind.
//!
//! This is an **unofficial** endpoint with no SLA: Yahoo can throttle or
//! change it at any time. Use it gently (one pass per scan), never panic on
//! a surprising response, and treat every error as "no data".

use contracts::Bar;
use reqwest::blocking::Client;
use serde::Deserialize;
use thiserror::Error;

pub mod chain;
pub use chain::{ChainSnapshot, ExpirySlice, OptionQuote, parse_chain_response};

const DEFAULT_BASE_URL: &str = "https://query1.finance.yahoo.com";
const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0 Safari/537.36";

/// Configuration for the Yahoo Finance client.
///
/// There are no credentials. The only knob is the base URL, overridable via
/// `YAHOO_BASE_URL` for tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YahooConfig {
    pub base_url: String,
}

impl Default for YahooConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_string(),
        }
    }
}

impl YahooConfig {
    /// Loads configuration from the environment.
    ///
    /// No required variables. `YAHOO_BASE_URL` overrides the default.
    pub fn from_env() -> Self {
        Self {
            base_url: std::env::var("YAHOO_BASE_URL")
                .unwrap_or_else(|_| DEFAULT_BASE_URL.to_string()),
        }
    }
}

/// A synchronous client for Yahoo Finance's public chart API.
#[derive(Debug, Clone)]
pub struct YahooClient {
    pub config: YahooConfig,
    http: Client,
}

impl YahooClient {
    /// Creates a new Yahoo client.
    pub fn new(config: YahooConfig) -> Self {
        Self {
            config,
            http: Client::new(),
        }
    }

    /// Fetches historical bars for `ticker` (a Yahoo symbol such as `SPY`,
    /// `^VIX`, `ES=F`, or `BTC-USD`).
    ///
    /// Supported timeframes: `1d`, `1h`, `1wk`. Anything else is an
    /// [`YahooProviderError::InvalidTimeframe`]. Uses the default history
    /// depth for the timeframe; see [`fetch_bars_with_history`] to request
    /// deeper history (needed for volatility estimation).
    pub fn fetch_bars(
        &self,
        ticker: &str,
        timeframe: &str,
    ) -> Result<Vec<Bar>, YahooProviderError> {
        self.fetch_bars_with_history(ticker, timeframe, None)
    }

    /// Fetches historical bars with an explicit history depth.
    ///
    /// `history` overrides the default `range`: one of `1y`, `2y`, `5y`,
    /// `10y`, `max`. Intraday (`1h`) bars are capped at `2y` — Yahoo serves
    /// at most ~730 days of hourly data and returns an empty result beyond
    /// that, so requesting more fails closed here instead.
    pub fn fetch_bars_with_history(
        &self,
        ticker: &str,
        timeframe: &str,
        history: Option<&str>,
    ) -> Result<Vec<Bar>, YahooProviderError> {
        let (interval, range) = timeframe_params_with_history(timeframe, history)?;
        let url = format!(
            "{}/v8/finance/chart/{}?interval={}&range={}",
            self.config.base_url.trim_end_matches('/'),
            ticker,
            interval,
            range
        );
        let response = self
            .http
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .send()?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .unwrap_or_else(|_| "unable to decode error body".to_string());
            return Err(YahooProviderError::UnexpectedHttpStatus(status, body));
        }

        let body = response.text()?;
        parse_chart_response(ticker, timeframe, &body)
    }
}

/// Maps a Thales timeframe to Yahoo's `interval` and `range` parameters.
///
/// - `1d` → `interval=1d&range=6mo` (roughly 126 trading days)
/// - `1h` → `interval=1h&range=1mo`
/// - `1wk` → `interval=1wk&range=2y`
pub fn timeframe_params(timeframe: &str) -> Result<(String, String), YahooProviderError> {
    timeframe_params_with_history(timeframe, None)
}

/// Maps a Thales timeframe plus an optional history override to Yahoo's
/// `interval` and `range` parameters.
///
/// `history` is one of `1y`, `2y`, `5y`, `10y`, `max`. Hourly bars are
/// capped at `2y` (Yahoo's intraday limit); anything deeper is an
/// [`YahooProviderError::InvalidHistory`] rather than a silent empty result.
pub fn timeframe_params_with_history(
    timeframe: &str,
    history: Option<&str>,
) -> Result<(String, String), YahooProviderError> {
    let (interval, default_range) = match timeframe {
        "1d" => ("1d", "6mo"),
        "1h" => ("1h", "1mo"),
        "1wk" => ("1wk", "2y"),
        other => return Err(YahooProviderError::InvalidTimeframe(other.to_string())),
    };
    let Some(history) = history else {
        return Ok((interval.to_string(), default_range.to_string()));
    };
    match history {
        "1y" | "2y" | "5y" | "10y" | "max" => {}
        other => return Err(YahooProviderError::InvalidHistory(other.to_string())),
    }
    if timeframe == "1h" && !matches!(history, "1y" | "2y") {
        return Err(YahooProviderError::InvalidHistory(format!(
            "{history} exceeds Yahoo's ~730-day intraday limit (timeframe 1h)"
        )));
    }
    Ok((interval.to_string(), history.to_string()))
}

/// Infers the Thales market label from a Yahoo ticker:
/// `^`-prefixed → `index`, `=F`-suffixed → `futures`, `-USD`-suffixed →
/// `crypto`, everything else → `equities`.
pub fn infer_market(ticker: &str) -> &'static str {
    if ticker.starts_with('^') {
        "index"
    } else if ticker.ends_with("=F") {
        "futures"
    } else if ticker.ends_with("-USD") {
        "crypto"
    } else {
        "equities"
    }
}

/// Parses a Yahoo chart API response body into [`Bar`]s.
///
/// Bars with any null OHLC value are skipped (Yahoo returns null rows for
/// e.g. the not-yet-complete current session); null volume becomes `0.0`
/// (indices and some futures have no volume series). Timestamps are Unix
/// seconds converted to milliseconds.
pub fn parse_chart_response(
    ticker: &str,
    timeframe: &str,
    body: &str,
) -> Result<Vec<Bar>, YahooProviderError> {
    let response: YahooChartResponse = serde_json::from_str(body)?;
    let chart = response.chart;

    if let Some(err) = chart.error {
        return Err(YahooProviderError::Api(format!(
            "{}: {}",
            err.code, err.description
        )));
    }

    let result = chart
        .result
        .and_then(|mut results| results.drain(..).next())
        .ok_or_else(|| YahooProviderError::EmptyResult(ticker.to_string()))?;

    let indicators = result
        .indicators
        .ok_or_else(|| YahooProviderError::EmptyResult(ticker.to_string()))?;

    let quote = indicators
        .quote
        .and_then(|mut quotes| quotes.drain(..).next())
        .ok_or_else(|| YahooProviderError::EmptyResult(ticker.to_string()))?;

    // Yahoo nests adjusted closes one level deeper: indicators.adjclose[0].adjclose.
    let adjcloses: Vec<Option<f64>> = indicators
        .adjclose
        .and_then(|mut a| a.drain(..).next())
        .and_then(|a| a.adjclose)
        .unwrap_or_default();

    let timestamps = result.timestamp.unwrap_or_default();
    let opens = quote.open.unwrap_or_default();
    let highs = quote.high.unwrap_or_default();
    let lows = quote.low.unwrap_or_default();
    let closes = quote.close.unwrap_or_default();
    let volumes = quote.volume.unwrap_or_default();

    let market = infer_market(ticker);
    let mut bars = Vec::new();
    for (i, &ts_seconds) in timestamps.iter().enumerate() {
        let (Some(open), Some(high), Some(low), Some(close)) = (
            opens.get(i).copied().flatten(),
            highs.get(i).copied().flatten(),
            lows.get(i).copied().flatten(),
            closes.get(i).copied().flatten(),
        ) else {
            continue; // skip null bars rather than fabricating values
        };
        let volume = volumes.get(i).copied().flatten().unwrap_or(0.0);
        let adjusted_close = adjcloses.get(i).copied().flatten();
        bars.push(Bar {
            symbol: ticker.to_string(),
            market: market.to_string(),
            timeframe: timeframe.to_string(),
            timestamp_unix_ms: ts_seconds * 1000,
            open,
            high,
            low,
            close,
            volume,
            adjusted_close,
        });
    }

    Ok(bars)
}

#[derive(Debug, Deserialize)]
struct YahooChartResponse {
    chart: YahooChart,
}

#[derive(Debug, Deserialize)]
struct YahooChart {
    result: Option<Vec<YahooResult>>,
    error: Option<YahooError>,
}

#[derive(Debug, Deserialize)]
struct YahooError {
    code: String,
    description: String,
}

#[derive(Debug, Deserialize)]
struct YahooResult {
    timestamp: Option<Vec<i64>>,
    indicators: Option<YahooIndicators>,
}

#[derive(Debug, Deserialize)]
struct YahooIndicators {
    quote: Option<Vec<YahooQuote>>,
    adjclose: Option<Vec<YahooAdjClose>>,
}

#[derive(Debug, Deserialize)]
struct YahooAdjClose {
    adjclose: Option<Vec<Option<f64>>>,
}

#[derive(Debug, Deserialize)]
struct YahooQuote {
    open: Option<Vec<Option<f64>>>,
    high: Option<Vec<Option<f64>>>,
    low: Option<Vec<Option<f64>>>,
    close: Option<Vec<Option<f64>>>,
    volume: Option<Vec<Option<f64>>>,
}

#[derive(Debug, Error)]
pub enum YahooProviderError {
    #[error("invalid timeframe: {0} (supported: 1d, 1h, 1wk)")]
    InvalidTimeframe(String),
    #[error("invalid history: {0} (supported: 1y, 2y, 5y, 10y, max; 1h capped at 2y)")]
    InvalidHistory(String),
    #[error("http transport error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unexpected yahoo response status {0}: {1}")]
    UnexpectedHttpStatus(u16, String),
    #[error("yahoo api error: {0}")]
    Api(String),
    #[error("yahoo returned no usable result for {0}")]
    EmptyResult(String),
    #[error("options chain auth failed: {0}")]
    ChainAuth(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_to_public_endpoint() {
        let cfg = YahooConfig::default();
        assert_eq!(cfg.base_url, "https://query1.finance.yahoo.com");
    }

    #[test]
    fn config_from_env_reads_override() {
        unsafe {
            std::env::set_var("YAHOO_BASE_URL", "https://example.test");
        }
        let cfg = YahooConfig::from_env();
        assert_eq!(cfg.base_url, "https://example.test");
        unsafe {
            std::env::remove_var("YAHOO_BASE_URL");
        }
    }

    #[test]
    fn timeframe_mapping_covers_all_supported() {
        assert_eq!(
            timeframe_params("1d").unwrap(),
            ("1d".to_string(), "6mo".to_string())
        );
        assert_eq!(
            timeframe_params("1h").unwrap(),
            ("1h".to_string(), "1mo".to_string())
        );
        assert_eq!(
            timeframe_params("1wk").unwrap(),
            ("1wk".to_string(), "2y".to_string())
        );
        assert!(matches!(
            timeframe_params("1m"),
            Err(YahooProviderError::InvalidTimeframe(_))
        ));
    }

    #[test]
    fn history_override_replaces_default_range() {
        assert_eq!(
            timeframe_params_with_history("1d", Some("2y")).unwrap(),
            ("1d".to_string(), "2y".to_string())
        );
        assert_eq!(
            timeframe_params_with_history("1d", Some("max")).unwrap(),
            ("1d".to_string(), "max".to_string())
        );
        assert_eq!(
            timeframe_params_with_history("1wk", Some("10y")).unwrap(),
            ("1wk".to_string(), "10y".to_string())
        );
        assert_eq!(
            timeframe_params_with_history("1h", Some("2y")).unwrap(),
            ("1h".to_string(), "2y".to_string())
        );
        // No override -> defaults.
        assert_eq!(
            timeframe_params_with_history("1d", None).unwrap(),
            timeframe_params("1d").unwrap()
        );
    }

    #[test]
    fn history_validation_fails_closed() {
        assert!(matches!(
            timeframe_params_with_history("1d", Some("3mo")),
            Err(YahooProviderError::InvalidHistory(_))
        ));
        // 1h is capped at ~730d of intraday data.
        assert!(matches!(
            timeframe_params_with_history("1h", Some("5y")),
            Err(YahooProviderError::InvalidHistory(_))
        ));
        assert!(matches!(
            timeframe_params_with_history("1h", Some("max")),
            Err(YahooProviderError::InvalidHistory(_))
        ));
    }

    #[test]
    fn parse_chart_response_reads_adjclose() {
        let body = serde_json::json!({
            "chart": {
                "result": [{
                    "timestamp": [1700000000, 1700086400, 1700172800],
                    "indicators": {
                        "quote": [{
                            "open": [100.0, 101.0, null],
                            "high": [102.0, 103.0, 104.0],
                            "low": [99.0, 100.0, 101.0],
                            "close": [101.0, 102.0, 103.0],
                            "volume": [1000.0, null, 1200.0]
                        }],
                        "adjclose": [{
                            "adjclose": [99.5, 100.5, 101.5]
                        }]
                    }
                }],
                "error": null
            }
        })
        .to_string();
        let bars = parse_chart_response("SPY", "1d", &body).unwrap();
        // The null-open bar is skipped; adjclose follows the bar index.
        assert_eq!(bars.len(), 2);
        assert_eq!(bars[0].adjusted_close, Some(99.5));
        assert_eq!(bars[0].volume, 1000.0);
        assert_eq!(bars[1].adjusted_close, Some(100.5));
        assert_eq!(bars[1].volume, 0.0); // null volume -> 0.0
    }

    #[test]
    fn parse_chart_response_without_adjclose_yields_none() {
        let body = serde_json::json!({
            "chart": {
                "result": [{
                    "timestamp": [1700000000],
                    "indicators": {
                        "quote": [{
                            "open": [100.0],
                            "high": [102.0],
                            "low": [99.0],
                            "close": [101.0],
                            "volume": [1000.0]
                        }]
                    }
                }],
                "error": null
            }
        })
        .to_string();
        let bars = parse_chart_response("SPY", "1d", &body).unwrap();
        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].adjusted_close, None);
    }

    #[test]
    fn market_inference() {
        assert_eq!(infer_market("^VIX"), "index");
        assert_eq!(infer_market("^GSPC"), "index");
        assert_eq!(infer_market("ES=F"), "futures");
        assert_eq!(infer_market("BTC-USD"), "crypto");
        assert_eq!(infer_market("SPY"), "equities");
        assert_eq!(infer_market("UVXY"), "equities");
    }
}
