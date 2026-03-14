# 🗣️ Echo: Getting Started examples have rough edges

🤦 **The Confusion:** Tried to run the `generate-signals` and `execute-intent` examples. The generated signals file had random `Risk-based Sizing` and `DEBUG:` logs mixed in with the JSON. Then, `execute-intent` with `kraken` failed with `EOrder:Insufficient funds` even though I have $35.39 available!

🕵️ **The Reality:** Turns out the CLI prints `stdout` and `stderr` to the console which breaks the JSON output if piped to a file without `2>/dev/null`. Also, the `kraken` provider has minimum order sizes that my $35.39 doesn't meet, but the error message doesn't explain that.

💡 **The Fix:** Add a huge banner in README saying 'PIPE STDERR TO /dev/null' or fix the CLI to not log to stdout. Also, improve the `kraken` insufficient funds error to show the minimum required amount.
