1. **Create the `price_dna` module**
   - Write the `crates/cli/src/experimental/price_dna.rs` file containing the `sequence_dna` function. This function maps price action to a biological sequence (A, C, T, G) based on direction and volume thresholds. Include a unit test validating this behavior.
2. **Register the module**
   - Add `pub mod price_dna;` to `crates/cli/src/experimental/mod.rs`.
3. **Integrate into CLI**
   - Update `crates/cli/src/main.rs` to add an `AnalyzePriceDna` variant to the `Commands` enum, protected by `#[cfg(feature = "nova")]`.
   - Add the corresponding match arm in the `run` function to execute `thales_cli::experimental::price_dna::sequence_dna` and print/return the output.
4. **Compile and test**
   - Run `cargo check --all-features`, `cargo test --all-features`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo fmt --all` to ensure code quality.
5. **Create PR description file**
   - Create `NOVA_PR_PRICE_DNA.md` with the required Nova feature pitch structure (Spark, Feature, Potential, Risk).
6. **Complete pre-commit steps**
   - Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.
7. **Submit the changes**
   - Call the `submit` tool to finalize the code implementation.
