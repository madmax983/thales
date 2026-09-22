#!/usr/bin/env bash
# judge-with-jev.sh — run `thales-cli judge-signals` with Mark's saved Jev credential.
#
# The Jev API key lives in the Secure Vault (connector `custom.typesafe`) and is
# never visible to agents. This wrapper fetches a short-lived surrogate token
# from authd at scan time and exports it as TYPESAFE_API_KEY for exactly one
# judge-signals invocation. The surrogate is held only in the child process's
# environment — it is never written to disk, never logged, never committed.
#
# Sentinel/authd replaces the surrogate with the real key on the approved
# outbound request to api.typesafe.ai, which is why this works through the
# compiled Rust binary without any code change.
#
# If the credential cannot be obtained (no authd socket, connector revoked),
# judge-signals runs WITHOUT the key and fails closed: no signal is approved.
# That is the safe outcome — an ungated execution is never acceptable.
#
# Usage (from ~/workspace/thales):
#   ./scripts/judge-with-jev.sh --input runs/<run-id>/signals-BTCUSD.json \
#       --analysis runs/<run-id>/analysis-BTCUSD.json \
#       --bars runs/<run-id>/bars-BTCUSD.json \
#       --portfolio runs/<run-id>/positions.json \
#       --emit report --log runs/<run-id>/audit.md
set -u

SURROGATE="$(python3 - <<'PYEOF'
import sys
sys.path.insert(0, "/opt/hatch/skills/skill-creator/bin")
import dynamic_credentials as dc
try:
    entry = dc.dynamic_credential_entry("custom.typesafe", "access_token")
    surrogate = str(entry.get("surrogate", "")).strip()
    if not surrogate.startswith("hsurr:"):
        raise RuntimeError("authd did not return a surrogate value")
    sys.stdout.write(surrogate)
except Exception as e:  # noqa: BLE001 - report and fail closed
    print(f"judge-with-jev: cannot obtain Jev credential: {e}", file=sys.stderr)
    sys.exit(1)
PYEOF
)" || { echo "judge-with-jev: credential unavailable — gate will fail closed" >&2; exit 1; }

export TYPESAFE_API_KEY="$SURROGATE"
unset SURROGATE
# TYPESAFE_BASE_URL intentionally left at the CLI default (https://api.typesafe.ai).
exec ./target/debug/thales-cli judge-signals "$@"
