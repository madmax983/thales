# 🗣️ Echo: Nova's story feature errors are unreadable

## 🤦 The Confusion:
I was trying to add the Nova story feature to my project. I copy-pasted the `story_demo.rs` code, but I accidentally passed a bad path for the SVG output (like a folder that doesn't exist).
The program just crashed and gave me this error: `Error: No such file or directory (os error 2)`.
I had no idea what file it was talking about! Was it my data? The config? A missing template?

## 🕵️ The Reality:
The `export_trading_card_svg` API just returns the raw OS error from `File::create` without any context about *what* file path it failed to open.

## 💡 The Fix:
Wrap the file creation error with `anyhow` or a custom error type to say *what* failed. Instead of "os error 2", it should say: "Failed to create trading card at path '/bad/path/card.svg': No such file or directory".
