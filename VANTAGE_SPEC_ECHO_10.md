# 🔭 Vantage: Spec for Echo: Getting Started example is broken

## 👤 User Story
As a New User exploring the `story_demo`, I want the README to clearly indicate that the `nova` feature is required, so that I can successfully run the example without encountering `NarrativeGenerator not found` compiler errors.

## 💡 So What?
If new users attempt to run the `story_demo` without the required feature flag, they will face compilation errors and assume the codebase is broken or incomplete. This creates immediate friction and ruins the onboarding experience. Adding a clear requirement banner ensures users can successfully run the demo on their first attempt.

## 📈 Metric Definition
Success = The README contains a prominent, unmissable banner or warning explicitly stating that running the `story_demo` requires enabling the `nova` feature flag, completely eliminating compilation errors related to missing `nova` components for users following the guide.

## 🔍 Gap Analysis
Currently, the README's getting started section for `story_demo` lacks the necessary instructions about the `nova` feature. The actual codebase requires this feature to resolve the `NarrativeGenerator`, creating a gap between the documentation and the technical requirements.

## ✅ Acceptance Criteria
- Must add a prominent warning or banner in the README near the `story_demo` example.
- The banner must explicitly state 'REQUIRES FEATURE NOVA' (or similar clear wording).
- The provided example command for `story_demo` must include `--features nova`.

## 🚫 Out of Scope
- Modifying the underlying `story_demo` code to run without the `nova` feature.
- Changing the feature-gating architecture of the CLI.
