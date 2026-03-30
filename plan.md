Wait, the code review states that my change to `crates/providers/kraken/src/lib.rs` is redundant because `XXBTZUSD` already ends with `"USD"`, and the existing logic uses `.ends_with()`.
Let's check the memory.
The memory explicitly says:
"When updating the Kraken API provider (`crates/providers/kraken/src/lib.rs`) to correctly parse quote currencies for legacy pairs with `Z` and `X` prefixes (e.g., `XXBTZUSD`, `XETHZUSD`), do not add `Z`-prefixed quotes to generic suffix matchers like `KNOWN_QUOTES`. Doing so breaks modern assets ending in 'Z' (e.g., Tezos `XTZUSD`). Instead, use exact explicit string matching for these legacy pairs before falling back to suffix checks."

Ah! The reviewer says it's redundant because `XXBTZUSD` ends with `USD`, so `.ends_with("USD")` will match it, returning `"USD"`, which means the base currency will be `XXBTZ` (since it strips `"USD"`).
Wait, if it strips `"USD"`, the base currency is `XXBTZ`. But the actual base currency is `XXBT` (or `BTC`).
Let's look at `base_currency_from_pair`:
```rust
fn base_currency_from_pair(pair: &str) -> Option<&str> {
    let quote = quote_currency_from_pair(pair)?;
    pair.strip_suffix(quote)
}
```
If `pair` is `XXBTZUSD`, and `quote` is `"USD"`, then `base_currency_from_pair` returns `"XXBTZ"`. But `XXBTZ` is not a valid balance key in `parse_balance_for_currency`!
Wait, in `parse_balance_for_currency`:
```rust
fn parse_balance_for_currency(balances: &HashMap<String, String>, currency: &str) -> Option<f64> {
    let upper = currency.to_uppercase();
    let mut keys = vec![upper.clone(), format!("Z{}", upper), format!("X{}", upper)];
    if upper == "BTC" {
        keys.push("XBT".to_string());
        keys.push("XXBT".to_string());
    }
```
If `base_currency_from_pair` returns `"XXBTZ"`, `currency` is `"XXBTZ"`. The keys checked will be `"XXBTZ"`, `"ZXXBTZ"`, `"XXXBTZ"`. None of these is `"XXBT"`. So it will fail to find the balance!
Therefore, the quote currency for `XXBTZUSD` should NOT be `"USD"`. It must be `"ZUSD"` so that `strip_suffix` returns `"XXBT"`.
But the memory says: "do not add `Z`-prefixed quotes to generic suffix matchers like `KNOWN_QUOTES`. Doing so breaks modern assets ending in 'Z' (e.g., Tezos `XTZUSD`). Instead, use exact explicit string matching for these legacy pairs before falling back to suffix checks."
Wait, if `quote_currency_from_pair("XXBTZUSD")` returns `"ZUSD"`, then `base` is `"XXBT"`.
My implementation:
```rust
    if pair == "XXBTZUSD" || pair == "XETHZUSD" || pair == "XXRPZUSD" || pair == "XLTCZUSD" || pair == "XZECZUSD" || pair == "XXMRZUSD" || pair == "XREPZUSD" || pair == "XETCZUSD" {
        return Some("USD");
    }
```
If I return `"USD"`, the base is `"XXBTZ"`, which is wrong!
I need to return `"ZUSD"`!
Let me re-read the memory: "When updating the Kraken API provider... to correctly parse quote currencies for legacy pairs... Instead, use exact explicit string matching for these legacy pairs before falling back to suffix checks."
So I should return `"ZUSD"`, `"ZEUR"`, `"ZCAD"`, `"ZGBP"`, `"ZJPY"` for these pairs!
Let's fix this!
