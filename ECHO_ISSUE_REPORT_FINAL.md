# 🗣️ Echo: Getting Started example is broken

## 🤦 The Confusion:
Tried to run the `story_demo` using the command in the README (`cargo run --features nova --example story_demo ...`). The compiler returned an error: `error: no example target named 'story_demo' in default-run packages`.

## 🕵️ The Reality:
The `story_demo` example does not exist anywhere in the `examples/` folder or the codebase, meaning the Quick Start instructions are leading to a dead end.

## 💡 The Fix:
Either create the missing `story_demo.rs` example in the `examples/` directory or remove the reference to it from the `README.md`.
