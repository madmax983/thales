//! Integration tests for the Yahoo options-chain parser, using a saved fixture.
//! No network access in tests.
//!
//! Fixture: SPY chain quoted 2026-09-23, single expiry 2026-09-30
//! (7 DTE at quote time), near-money strikes 755–762.

use yahoo_provider::{ChainSnapshot, parse_chain_response};

const FIXTURE: &str = include_str!("fixtures/spy_chain.json");
/// 2026-09-23T00:00:00Z — expiry 2026-09-30 is exactly 7 DTE.
const NOW: i64 = 1_790_121_600;
const EXPIRY: i64 = 1_790_726_400; // 2026-09-30T00:00:00Z

fn snapshot() -> ChainSnapshot {
    parse_chain_response("SPY", FIXTURE).unwrap()
}

#[test]
fn fixture_parses_spot_and_single_expiry() {
    let snap = snapshot();
    assert_eq!(snap.underlying, "SPY");
    assert!((snap.spot - 768.82).abs() < 1e-9);
    assert_eq!(snap.expirations.len(), 1);

    let slice = &snap.expirations[0];
    assert_eq!(slice.expiry_unix, EXPIRY);
    assert_eq!(slice.calls.len(), 8);
    assert_eq!(slice.puts.len(), 8);

    let call = &slice.calls[7];
    assert_eq!(call.contract_symbol, "SPY260930C00762000");
    assert!((call.strike - 762.0).abs() < 1e-9);
    assert!((call.mid() - (9.32 + 9.59) / 2.0).abs() < 1e-9);
    assert_eq!(call.open_interest, 2291);
    assert!(call.in_the_money); // 762 call under 768.82 spot is ITM
}

#[test]
fn front_expiry_skips_short_dated_by_default() {
    let snap = snapshot();
    // 7 DTE passes the v0 default bar.
    assert!(snap.front_expiry(7, NOW).is_some());
    // A stricter bar finds nothing — never fabricate a nearer expiry.
    assert!(snap.front_expiry(8, NOW).is_none());
}

#[test]
fn atm_quote_picks_strike_nearest_spot() {
    let snap = snapshot();
    let slice = &snap.expirations[0];
    let atm_call = snap.atm_quote(&slice.calls).unwrap();
    let atm_put = snap.atm_quote(&slice.puts).unwrap();
    // Fixture strikes top out at 762; nearest to 768.82 spot.
    assert!((atm_call.strike - 762.0).abs() < 1e-9);
    assert!((atm_put.strike - 762.0).abs() < 1e-9);
}

#[test]
fn front_atm_iv_averages_call_and_put() {
    let snap = snapshot();
    let iv = snap.front_atm_iv(7, NOW).unwrap();
    let expected = (0.12214012634277345 + 0.10950597534179687) / 2.0;
    assert!((iv - expected).abs() < 1e-9);
}

#[test]
fn front_atm_iv_fails_closed_on_bad_iv() {
    let mut snap = snapshot();
    snap.expirations[0].calls[7].iv = 0.0; // Yahoo reports 0.0 on some contracts
    assert!(snap.front_atm_iv(7, NOW).is_none());
}

#[test]
fn front_atm_iv_rejects_yahoo_missing_iv_placeholder() {
    let mut snap = snapshot();
    // Yahoo emits 1e-5 where it publishes no IV (seen on far strikes live).
    // That must fail closed, never become a fabricated ~0% volatility read.
    snap.expirations[0].calls[7].iv = 0.00001;
    snap.expirations[0].puts[7].iv = 0.00001;
    assert!(snap.front_atm_iv(7, NOW).is_none());
}

#[test]
fn empty_result_is_an_error_not_an_empty_snapshot() {
    let body = r#"{"optionChain":{"result":[],"error":null}}"#;
    assert!(parse_chain_response("SPY", body).is_err());
}
