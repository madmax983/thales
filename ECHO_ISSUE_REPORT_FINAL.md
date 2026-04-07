# 🗣️ Echo: Getting Started example is broken

**🤦 The Confusion:**
Tried to run the `story_demo` using the command in the README (`cargo run --features nova --example story_demo ...`). The compiler said `error: no example target named 'story_demo' in default-run packages`.

**🕵️ The Reality:**
The `story_demo` example does not exist anywhere in the codebase.

**💡 The Fix:**
Either create the missing `story_demo` example or remove the broken command from the README.md.
