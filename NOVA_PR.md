# 🌟 Nova: CLI Help Hints for Experimental Features

## 💡 The Spark
Users attempting to use commands documented in our help text receive "unrecognized subcommand" errors because they lack a feature flag, assuming the CLI is broken.

## 🚀 The Feature
Implemented an `after_help` attribute in the `Cli` struct using clap to provide a clear hint that "Nova" experimental features require the `--features nova` flag.

## 🔭 The Potential
Increases discoverability of our experimental features, correctly guiding users to enable the feature flag rather than throwing confusing "unrecognized subcommand" errors, thus increasing engagement.

## ⚠️ Risk
Low. Isolated to a simple clap attribute in the CLI entry point.
