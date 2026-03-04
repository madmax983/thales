1. *Scan Universe and Check Portfolio*
   - Execute `cargo run -p thales-cli -- scan-market --provider kraken --top-n 3` to get top crypto candidates.
   - Execute `cargo run -p thales-cli -- scan-market --provider alpaca --top-n 3` to get top equity candidates.
   - Execute `cargo run -p thales-cli -- get-positions --provider kraken` and `cargo run -p thales-cli -- get-positions --provider alpaca` to review the current portfolio.

2. *Select Top 1-3 Candidates*
   - Based on the scans, select the top candidates (max 3 total) to analyze deeply. We will select one from Kraken and one from Alpaca to maintain diversification.
   - Write a python script to iterate over these candidates and generate signals using all active strategies from `strategies.md`. The script will use the `thales-cli fetch-market-data`, `thales-cli analyze-market`, and `thales-cli generate-signals` tools.

3. *Evaluate Candidates and Handle Conflicts*
   - For each candidate, analyze `Signals.md` to see if there are pending signals.
   - For each candidate, evaluate against all active strategies.
   - If a candidate has conflicting signals (buy/sell), log the conflict to `portfolio.md` and do not trade that asset.
   - If no valid signal is generated for an asset, log it to `portfolio.md` as skipped.

4. *Execute Valid Intents*
   - For candidates with a clear, non-conflicting signal, execute the trade using `cargo run -p thales-cli -- execute-intent ...`
   - Log the execution details to `portfolio.md` using the exact required 11-column format.

5. *Complete Pre-Commit Steps*
   - Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.
