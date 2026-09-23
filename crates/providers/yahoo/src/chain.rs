//! Yahoo Finance options-chain provider (SPEC-006 v0).
//!
//! Wraps `GET /v7/finance/options/{ticker}` (`?date={unix}` for one
//! expiration). Unlike the chart API, the options endpoint requires Yahoo's
//! cookie+crumb flow: seed cookies from `https://fc.yahoo.com/`, fetch a crumb
//! from `/v1/test/getcrumb`, then request the chain with `?crumb=`. The seed
//! URL is overridable via `YAHOO_FC_SEED_URL` for tests; the API base via
//! `YAHOO_BASE_URL` as usual.
//!
//! Verified live 2026-09-23. Same posture as the chart provider: unofficial
//! endpoint, no SLA, fail closed on auth or schema drift, one pass per scan.

use super::{USER_AGENT, YahooClient, YahooProviderError};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

/// Cookie-seeding URL for the crumb flow (overridable for tests).
const FC_SEED_URL: &str = "https://fc.yahoo.com/";

/// IV below this (0.01%) is Yahoo's "no IV published" placeholder zone, not a
/// real volatility read. Anything under it fails the ATM-IV check closed.
const MIN_SANE_IV: f64 = 1e-4;

/// A single option quote from the chain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptionQuote {
    pub contract_symbol: String,
    pub strike: f64,
    pub bid: f64,
    pub ask: f64,
    pub iv: f64,
    pub open_interest: u64,
    pub volume: u64,
    pub in_the_money: bool,
    pub expiry_unix: i64,
    pub last_trade_unix: Option<i64>,
}

impl OptionQuote {
    /// Mid price; `(bid + ask) / 2`. May be 0 on illiquid contracts.
    pub fn mid(&self) -> f64 {
        (self.bid + self.ask) / 2.0
    }
}

/// All quotes for one expiration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExpirySlice {
    pub expiry_unix: i64,
    pub calls: Vec<OptionQuote>,
    pub puts: Vec<OptionQuote>,
}

/// A full chain snapshot for one underlying.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChainSnapshot {
    pub underlying: String,
    pub spot: f64,
    pub quoted_at_unix_ms: i64,
    pub expirations: Vec<ExpirySlice>,
}

impl ChainSnapshot {
    /// Days to expiry for a slice, from `now_unix`.
    pub fn dte(expiry_unix: i64, now_unix: i64) -> i64 {
        (expiry_unix - now_unix) / 86_400
    }

    /// Nearest expiry with at least `min_dte_days` to expiry.
    ///
    /// Skips 0DTE noise by default (`min_dte_days = 7` is the v0 convention).
    pub fn front_expiry(&self, min_dte_days: i64, now_unix: i64) -> Option<&ExpirySlice> {
        self.expirations
            .iter()
            .filter(|e| Self::dte(e.expiry_unix, now_unix) >= min_dte_days)
            .min_by_key(|e| e.expiry_unix)
    }

    /// Quote with the strike nearest spot.
    pub fn atm_quote<'a>(&self, quotes: &'a [OptionQuote]) -> Option<&'a OptionQuote> {
        quotes.iter().min_by(|a, b| {
            (a.strike - self.spot)
                .abs()
                .partial_cmp(&(b.strike - self.spot).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Mean of ATM call and ATM put IV on the front expiry — the v0 "IV read".
    ///
    /// Fails closed (`None`) when either leg is missing, non-positive,
    /// non-finite, or below [`MIN_SANE_IV`]: Yahoo emits `1e-5` as a
    /// "no IV published" placeholder, which must never become a fabricated
    /// near-zero volatility read.
    pub fn front_atm_iv(&self, min_dte_days: i64, now_unix: i64) -> Option<f64> {
        let front = self.front_expiry(min_dte_days, now_unix)?;
        let call_iv = self.atm_quote(&front.calls)?.iv;
        let put_iv = self.atm_quote(&front.puts)?.iv;
        if call_iv < MIN_SANE_IV || put_iv < MIN_SANE_IV {
            return None;
        }
        if !call_iv.is_finite() || !put_iv.is_finite() {
            return None;
        }
        Some((call_iv + put_iv) / 2.0)
    }
}

// --- raw Yahoo shapes -------------------------------------------------------

#[derive(Debug, Deserialize)]
struct RawChainResponse {
    #[serde(rename = "optionChain")]
    option_chain: RawOptionChain,
}

#[derive(Debug, Deserialize)]
struct RawOptionChain {
    result: Vec<RawChainResult>,
    error: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct RawChainResult {
    quote: RawQuote,
    #[serde(rename = "expirationDates", default)]
    _expiration_dates: Vec<i64>,
    #[serde(default)]
    _strikes: Vec<f64>,
    #[serde(default)]
    options: Vec<RawExpiry>,
}

#[derive(Debug, Deserialize)]
struct RawQuote {
    #[serde(rename = "regularMarketPrice", default)]
    regular_market_price: Option<f64>,
    #[serde(rename = "regularMarketTime", default)]
    regular_market_time: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RawExpiry {
    #[serde(rename = "expirationDate")]
    expiration_date: i64,
    #[serde(default)]
    calls: Vec<RawQuote2>,
    #[serde(default)]
    puts: Vec<RawQuote2>,
}

#[derive(Debug, Deserialize)]
struct RawQuote2 {
    #[serde(rename = "contractSymbol", default)]
    contract_symbol: String,
    #[serde(default)]
    strike: f64,
    #[serde(default)]
    bid: f64,
    #[serde(default)]
    ask: f64,
    #[serde(rename = "impliedVolatility", default)]
    implied_volatility: f64,
    #[serde(rename = "openInterest", default)]
    open_interest: Option<u64>,
    #[serde(default)]
    volume: Option<u64>,
    #[serde(rename = "inTheMoney", default)]
    in_the_money: bool,
    #[serde(rename = "lastTradeDate", default)]
    last_trade_date: Option<i64>,
}

/// Parses a Yahoo options-chain response body (pure; fixture-tested).
pub fn parse_chain_response(ticker: &str, body: &str) -> Result<ChainSnapshot, YahooProviderError> {
    let raw: RawChainResponse = serde_json::from_str(body)?;
    let result = raw
        .option_chain
        .result
        .into_iter()
        .next()
        .ok_or_else(|| YahooProviderError::EmptyResult(ticker.to_string()))?;
    if raw.option_chain.error.is_some() {
        return Err(YahooProviderError::Api(format!("chain error for {ticker}")));
    }
    let spot = result
        .quote
        .regular_market_price
        .filter(|s| *s > 0.0)
        .ok_or_else(|| YahooProviderError::EmptyResult(ticker.to_string()))?;
    let quoted_at_unix_ms = result.quote.regular_market_time.unwrap_or(0) * 1000;

    let convert = |expiry_unix: i64, qs: Vec<RawQuote2>| {
        qs.into_iter()
            .map(|q| OptionQuote {
                contract_symbol: q.contract_symbol,
                strike: q.strike,
                bid: q.bid,
                ask: q.ask,
                iv: q.implied_volatility,
                open_interest: q.open_interest.unwrap_or(0),
                volume: q.volume.unwrap_or(0),
                in_the_money: q.in_the_money,
                expiry_unix,
                last_trade_unix: q.last_trade_date,
            })
            .collect::<Vec<_>>()
    };

    let expirations = result
        .options
        .into_iter()
        .map(|e| ExpirySlice {
            expiry_unix: e.expiration_date,
            calls: convert(e.expiration_date, e.calls),
            puts: convert(e.expiration_date, e.puts),
        })
        .collect();

    Ok(ChainSnapshot {
        underlying: ticker.to_string(),
        spot,
        quoted_at_unix_ms,
        expirations,
    })
}

impl YahooClient {
    /// Fetches the default chain for `ticker` (front expiry quotes plus the
    /// full expiration list in the raw response).
    pub fn fetch_chain(&self, ticker: &str) -> Result<ChainSnapshot, YahooProviderError> {
        let body = self.chain_get(ticker, None)?;
        parse_chain_response(ticker, &body)
    }

    /// Fetches one expiration's quotes (`expiry_unix` as in `expirationDates`).
    pub fn fetch_chain_expiry(
        &self,
        ticker: &str,
        expiry_unix: i64,
    ) -> Result<ChainSnapshot, YahooProviderError> {
        let body = self.chain_get(ticker, Some(expiry_unix))?;
        parse_chain_response(ticker, &body)
    }

    /// Cookie+crumb authenticated GET against the options endpoint.
    ///
    /// Retries the whole flow once on crumb/cookie rejection, then fails
    /// closed — never returns a faked or partial chain.
    fn chain_get(&self, ticker: &str, date: Option<i64>) -> Result<String, YahooProviderError> {
        let seed_url =
            std::env::var("YAHOO_FC_SEED_URL").unwrap_or_else(|_| FC_SEED_URL.to_string());
        let base = self.config.base_url.trim_end_matches('/');

        for _ in 0..2 {
            let jar = Client::builder().cookie_store(true).build()?;
            // 1. Seed cookies. A failed seed means the crumb step will fail
            //    closed below; ignore the seed response itself.
            let _ = jar.get(&seed_url).header("User-Agent", USER_AGENT).send();
            // 2. Crumb.
            let crumb = jar
                .get(format!("{base}/v1/test/getcrumb"))
                .header("User-Agent", USER_AGENT)
                .send()?
                .text()?
                .trim()
                .to_string();
            if crumb.is_empty() || crumb.contains("error") || crumb.contains("Unauthorized") {
                continue;
            }
            // 3. Chain.
            let mut url = format!("{base}/v7/finance/options/{ticker}?crumb={crumb}");
            if let Some(d) = date {
                url = format!("{base}/v7/finance/options/{ticker}?date={d}&crumb={crumb}");
            }
            let body = jar
                .get(&url)
                .header("User-Agent", USER_AGENT)
                .send()?
                .text()?;
            if body.contains("Invalid Crumb") || body.contains("Invalid Cookie") {
                continue;
            }
            return Ok(body);
        }
        Err(YahooProviderError::ChainAuth(format!(
            "cookie/crumb flow rejected for {ticker}"
        )))
    }
}
