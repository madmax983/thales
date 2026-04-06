# 🗣️ Echo: Getting Started example is broken

**🤦 The Confusion:**
Tried to run the `story_demo` example mentioned in the README with `cargo run --features nova --example story_demo` but it failed with `error: no example target named 'story_demo' in default-run packages`. I thought I did something wrong or needed to install something else!

**🕵️ The Reality:**
Turns out the `story_demo` example doesn't actually exist in the `examples` folder of the repository. The README mentions it, but the code isn't there.

**💡 The Fix:**
Either add the `story_demo` example to the `examples/` directory so the command works, or remove the reference to it from the README to prevent confusing new users.
