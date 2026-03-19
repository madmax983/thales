1. **Explore codebase and requirements.**
   - I will check the `indicators.md` file and other files to understand the ZLEMA specification or any unrendered templates. I notice the prompt mentions `{{name}}`, `{{description}}`, `{{rationale}}`, `{{inputs}}`, `{{output_type}}`, `{{context}}`. Since the variables are unrendered, I will act autonomously and implement a standard missing technical indicator. I have selected **ZLEMA (Zero Lag Exponential Moving Average)**.
2. **Implement `zlema.rs`**.
   - Create a new file `crates/strategies/src/indicators/zlema.rs` containing the ZLEMA implementation using `rust_decimal::Decimal`, taking a `&DataFrame` and a `period: usize`.
   - The implementation will use `rust_decimal::Decimal` internally and handle errors correctly without `unwrap()`.
   - The lag is calculated as `(period - 1) / 2`. The adjusted data is `Close + (Close - Close[lag])`. Then an EMA is applied to this adjusted data.
   - Include 3 comprehensive test cases: `test_known_values`, `test_edge_cases`, `test_realistic_data`.
3. **Register module**.
   - Update `crates/strategies/src/indicators/mod.rs` to include `pub mod zlema;`.
4. **Run tests**.
   - Use `run_in_bash_session` to run `cargo test --package strategies`.
5. **Update documentation**.
   - Append ZLEMA documentation to `indicators.md`.
6. **Pre-commit step**.
   - Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.
7. **Submit**.
   - Use `submit` to submit the changes.
