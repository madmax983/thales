#!/usr/bin/env bash
# fetch-with-alpaca.sh — run `thales-cli` with Mark's saved Alpaca paper credentials.
#
# Alpaca needs TWO secrets per request (APCA-API-KEY-ID + APCA-API-SECRET-KEY),
# so they live in the Secure Vault as two connectors: `custom.alpaca-key` and
# `custom.alpaca-secret`. This wrapper fetches a short-lived surrogate for each
# from authd at scan time and exports them as ALPACA_API_KEY / ALPACA_API_SECRET
# for exactly one thales-cli invocation. Surrogates are held only in the child
# process's environment — never written to disk, never logged, never committed.
#
# Sentinel/authd replaces each surrogate with the real secret on the approved
# outbound request to data.alpaca.markets, which is why this works through the
# compiled Rust binary without any code change.
#
# EGRESS SCOPE: both connectors are scoped to data.alpaca.markets ONLY.
# ALPACA_BASE_URL is set to the paper trading host because AlpacaConfig
# requires the var, but no approved egress exists for it — the order/position
# endpoints are unreachable from scans. Paper-only, enforced by the plumbing.
#
# If either credential cannot be obtained (no authd socket, connector revoked),
# the wrapper exits non-zero and thales-cli never runs. Fetch fails closed.
#
# Usage (from ~/workspace/thales):
#   ./scripts/fetch-with-alpaca.sh fetch-market-data \
#       --provider alpaca --symbol SPY --timeframe 1d
set -u

fetch_surrogate() {
    local connector="$1"
    python3 - <<PYEOF
import sys
sys.path.insert(0, "/opt/hatch/skills/skill-creator/bin")
import dynamic_credentials as dc
try:
    entry = dc.dynamic_credential_entry("$connector", "access_token")
    surrogate = str(entry.get("surrogate", "")).strip()
    if not surrogate.startswith("hsurr:"):
        raise RuntimeError("authd did not return a surrogate value")
    sys.stdout.write(surrogate)
except Exception as e:  # noqa: BLE001 - report and fail closed
    print(f"fetch-with-alpaca: cannot obtain $connector credential: {e}", file=sys.stderr)
    sys.exit(1)
PYEOF
}

ALPACA_API_KEY="$(fetch_surrogate custom.alpaca-key)" \
    || { echo "fetch-with-alpaca: key id unavailable — aborting" >&2; exit 1; }
export ALPACA_API_KEY

# NOTE: the secret connector (custom.alpaca-secret) could never complete its
# hosted setup ("failed to connect" on the vault page, repeatedly). Mark then
# saved the secret as a website-login password (vault entry "alpaca.markets").
# That stored fine, but authd refuses surrogates for it: website-login
# credentials are browser-fill only ("not available to the sandbox"). So the
# secret is safely in the vault but unreachable from scans. If a working
# custom.* secret connector ever lands, point this back at it.
ALPACA_API_SECRET="$(fetch_surrogate custom.alpaca-secret)" \
    || { echo "fetch-with-alpaca: secret unavailable — aborting" >&2; exit 1; }
export ALPACA_API_SECRET

# Required by AlpacaConfig::from_env; inert — no approved egress to this host.
export ALPACA_BASE_URL="https://paper-api.alpaca.markets"

exec ./target/debug/thales-cli "$@"
