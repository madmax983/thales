1. Use `write_file` with `crates/strategies/src/indicators/zlema.rs` to implement the ZLEMA indicator using `rust_decimal::Decimal`, taking `&DataFrame` and `period: usize`. Include 3 TDD test cases (`test_known_values`, `test_edge_cases`, `test_realistic_data`).
2. Use `run_in_bash_session` to run `git status` to verify the creation of the file.
3. Use `replace_with_git_merge_diff` with `crates/strategies/src/indicators/mod.rs` to add `pub mod zlema;`.
4. Use `run_in_bash_session` to run `git diff crates/strategies/src/indicators/mod.rs` to verify the module addition.
5. Use `run_in_bash_session` to run `cargo test --package strategies` to run tests for the new indicator.
6. Use `replace_with_git_merge_diff` with `indicators.md` to append ZLEMA documentation to the end of the file.
7. Use `run_in_bash_session` to run `git diff indicators.md` to verify the documentation update.
8. Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.
9. Use `submit` to submit changes.
