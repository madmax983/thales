//! Coverage for the Thales custom trading universe (`scan-market --provider paper`).
//!
//! The paper provider used to return a hardcoded three-symbol list
//! (BTCUSD/ETHUSD/SPY) that could masquerade as a complete scan. It now
//! serves the audited manifest in `crates/cli/universe/universe.json`
//! (embedded at compile time), and must fail closed if the manifest is
//! unreadable or invalid.

use std::io::Write;

use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;
use tempfile::NamedTempFile;

fn scan_market(args: &[&str], universe_path: Option<&str>) -> assert_cmd::assert::Assert {
    let mut cmd = cargo_bin_cmd!("thales-cli");
    // Make sure a developer-machine override cannot leak into the test.
    cmd.env_remove("THALES_UNIVERSE_PATH");
    if let Some(path) = universe_path {
        cmd.env("THALES_UNIVERSE_PATH", path);
    }
    cmd.args(args).assert()
}

fn envelope_data(output: &[u8]) -> Value {
    let stdout = String::from_utf8_lossy(output);
    serde_json::from_str::<Value>(&stdout).expect("stdout is JSON")
}

fn manifest_symbols() -> Vec<String> {
    let data = envelope_data(
        &scan_market(
            &["scan-market", "--provider", "paper", "--top-n", "500"],
            None,
        )
        .success()
        .get_output()
        .stdout
        .clone(),
    );
    assert_eq!(data["status"], "ok");
    data["data"]
        .as_array()
        .expect("data is an array")
        .iter()
        .map(|v| v.as_str().expect("symbol is a string").to_string())
        .collect()
}

#[test]
fn paper_scan_serves_the_full_manifest_universe() {
    let symbols = manifest_symbols();

    // The hardcoded trio can no longer masquerade as a complete universe.
    assert!(
        symbols.len() > 100,
        "universe should be broad, got {} symbols",
        symbols.len()
    );

    for required in [
        "META", "ES", "NQ", "BTCUSD", "ETHUSD", "RUT", "VIX", "AAPL", "NVDA", "SPY",
    ] {
        assert!(
            symbols.contains(&required.to_string()),
            "universe is missing {required}"
        );
    }

    // FB was renamed to META in 2022; it must not appear as a canonical symbol.
    assert!(
        !symbols.iter().any(|s| s == "FB"),
        "stale FB ticker must not be a canonical symbol"
    );
}

#[test]
fn top_n_truncates_in_manifest_priority_order() {
    let data = envelope_data(
        &scan_market(
            &["scan-market", "--provider", "paper", "--top-n", "5"],
            None,
        )
        .success()
        .get_output()
        .stdout
        .clone(),
    );
    let symbols: Vec<&str> = data["data"]
        .as_array()
        .expect("data is an array")
        .iter()
        .map(|v| v.as_str().expect("symbol is a string"))
        .collect();
    assert_eq!(symbols.len(), 5);
    // Manifest order is scan priority: Moontower coverage first.
    assert_eq!(symbols[0], "JETS");
}

#[test]
fn corrupt_manifest_fails_closed() {
    let mut file = NamedTempFile::new().expect("temp file");
    write!(file, "this is not json{{{{").expect("write temp file");
    file.flush().expect("flush");

    let assert = scan_market(
        &["scan-market", "--provider", "paper", "--top-n", "10"],
        Some(file.path().to_str().expect("path")),
    );
    let output = assert.failure().get_output().clone();
    let data = envelope_data(&output.stdout);
    assert_eq!(data["status"], "error");
    assert!(
        !data["errors"].as_array().expect("errors array").is_empty(),
        "failure must explain itself"
    );
}

#[test]
fn missing_manifest_path_fails_closed() {
    let assert = scan_market(
        &["scan-market", "--provider", "paper", "--top-n", "10"],
        Some("/nonexistent/thales-universe.json"),
    );
    let output = assert.failure().get_output().clone();
    let data = envelope_data(&output.stdout);
    assert_eq!(data["status"], "error");
}

#[test]
fn manifest_schema_invariants_hold() {
    // The embedded manifest source of truth, read from the crate tree.
    let raw = std::fs::read_to_string("universe/universe.json")
        .expect("universe.json is readable from the crate root");
    let manifest: Value = serde_json::from_str(&raw).expect("universe.json is valid JSON");
    let entries = manifest["entries"].as_array().expect("entries is an array");
    assert!(!entries.is_empty(), "manifest must not be empty");

    let mut canonicals = std::collections::HashSet::new();
    for entry in entries {
        let canonical = entry["canonical"].as_str().expect("canonical is a string");
        assert!(!canonical.trim().is_empty(), "blank canonical symbol");
        assert!(
            canonicals.insert(canonical),
            "duplicate canonical symbol: {canonical}"
        );
        assert!(
            !entry["asset_class"]
                .as_str()
                .unwrap_or("")
                .trim()
                .is_empty(),
            "{canonical} is missing its asset class"
        );
        assert!(
            !entry["provenance"]
                .as_array()
                .expect("provenance is an array")
                .is_empty(),
            "{canonical} is missing provenance"
        );
        assert!(
            entry["aliases"].is_object(),
            "{canonical} is missing provider aliases"
        );
    }

    let by_symbol = |s: &str| entries.iter().find(|e| e["canonical"] == s).expect(s);
    // FB must never resurface as a canonical symbol.
    assert!(entries.iter().all(|e| e["canonical"] != "FB"));
    // META carries the FB history explicitly.
    let meta = by_symbol("META");
    assert!(
        meta["legacy_aliases"]
            .as_array()
            .expect("legacy_aliases")
            .iter()
            .any(|a| a == "FB"),
        "META must record FB as a legacy alias"
    );
    // Indices are indices, not equities.
    for index in ["RUT", "VIX", "SPX", "NDX", "DJX"] {
        assert_eq!(by_symbol(index)["asset_class"], "index", "{index}");
    }
    // Legacy crypto and futures are present with honest classes.
    for crypto in ["BTCUSD", "ETHUSD"] {
        assert_eq!(by_symbol(crypto)["asset_class"], "crypto", "{crypto}");
    }
    for future in ["ES", "NQ"] {
        assert_eq!(by_symbol(future)["asset_class"], "future", "{future}");
        assert_eq!(
            by_symbol(future)["aliases"]["tradingview"],
            format!("{future}1!"),
            "{future} TradingView alias"
        );
    }
}
