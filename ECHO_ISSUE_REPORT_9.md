# 🗣️ Echo: Developer Experience Audit

## 🤦 The Confusion:
I wanted to test out the `Nova` experimental features because they sound super cool. I saw commands in the CLI help for things like `simulate-black-swan`, so I tried running:
`cargo run -p thales-cli -- simulate-black-swan --input dummy_data.json --event-type FlashCrash --start-index 10 --duration 5`
The CLI just yelled at me: `error: unrecognized subcommand 'simulate-black-swan'`. But it's literally in the source code as `SimulateBlackSwan`! I was so confused why a command that clearly exists is telling me it doesn't exist.

## 🕵️ The Reality:
It turns out these commands are hidden behind a feature flag! I had to run `cargo run --features nova -p thales-cli -- simulate-black-swan ...` to actually use them.

## 💡 The Fix:
Please add a clear note in the `README.md` or the CLI help text saying that experimental commands require the `--features nova` flag to be enabled. I shouldn't have to guess how to unlock these features.
