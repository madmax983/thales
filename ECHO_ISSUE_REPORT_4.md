# 🗣️ Echo: File Not Found Errors are confusing

## 🤦 **The Confusion:**
When I misspell a filename or try to run `generate-signals` on a file that isn't there (e.g. `invalid_file.json`), the tool just spits out: `{"status":"error","errors":["io error: No such file or directory (os error 2)"],"warnings":[],"data":null}`. As a user, "os error 2" is not very friendly. I have to guess which file it was complaining about.

## 🕵️ **The Reality:**
The CLI bubbles up a raw `std::io::Error` directly. It doesn't tell me *which* file it failed to open, it just gives me the OS error code.

## 💡 **The Fix:**
Wrap the IO error to include the filename that failed to open. Something like "Could not open input file 'invalid_file.json': No such file or directory".

---

# 🗣️ Echo: JSON Parse Errors leak implementation details

## 🤦 **The Confusion:**
I accidentally tried to run `execute-intent` on my market data file (`dummy_data.json`) instead of the signals file. I got: `{"status":"error","errors":["json error: missing field \`intent_id\` at line 236 column 1"],"warnings":[],"data":null}`. I don't know what an `intent_id` is, and line 236 doesn't help me understand I passed the completely wrong type of data.

## 🕵️ **The Reality:**
The `execute-intent` command blindly tries to parse whatever JSON file you give it into a `TradeIntent` struct. When it hits `dummy_data.json` (which contains `Bar` data), it fails deep in `serde_json` and prints that raw parsing error.

## 💡 **The Fix:**
Provide a higher-level error check or message. If the parser fails, maybe hint to the user: "Failed to parse TradeIntents. Did you pass market data instead of signals?". Or at least wrap the error so it's not just "missing field...".
