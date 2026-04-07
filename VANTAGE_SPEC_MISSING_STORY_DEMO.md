# 🔭 Vantage: Spec for Missing Story Demo Example

## 👤 User Story
As a New User, I want the `story_demo` example referenced in the README to actually exist in the codebase, so that I can successfully run it and learn about the Nova features without encountering compiler errors about missing targets.

## 💡 So What?
Referencing non-existent examples in the README severely damages user trust and creates a frustrating onboarding experience. If users cannot run the documented examples, they will abandon the tool. Fixing this ensures a smooth first-time user experience and reduces support overhead.

## 📈 Metric Definition
Success = Users can successfully execute `cargo run --features nova --example story_demo` without any "no example target named 'story_demo'" errors.

## 🔍 Gap Analysis
The README instructs users to run `cargo run --features nova --example story_demo ...`. However, there is no `story_demo.rs` file within an `examples/` directory in the repository, leading to immediate failure when the command is copied and pasted.

## ✅ Acceptance Criteria
- Create a functional `story_demo.rs` file in the appropriate `examples/` directory.
- The example must compile and run successfully when the `nova` feature is enabled.
- Alternatively, remove the reference to the `story_demo` example entirely from the README if the feature is deprecated or not ready.

## 🚫 Out of Scope
- Expanding the `story_demo` to include complex, untested trading logic.
- Refactoring the entire `nova` feature set.
