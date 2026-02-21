# Scheduled Task Environment Contract

This runbook defines required environment variables for running `thales-cli` commands in scheduled cloud tasks.

## Alpaca

Required:
- `ALPACA_API_KEY`
- `ALPACA_API_SECRET`
- `ALPACA_BASE_URL`

Example:

```powershell
$env:ALPACA_API_KEY = "..."
$env:ALPACA_API_SECRET = "..."
$env:ALPACA_BASE_URL = "https://paper-api.alpaca.markets"
```

## Kraken

Required:
- `KRAKEN_API_KEY`
- `KRAKEN_API_SECRET` (base64-encoded secret from Kraken API key settings)

Optional:
- `KRAKEN_BASE_URL` (defaults to `https://api.kraken.com`)

Example:

```powershell
$env:KRAKEN_API_KEY = "..."
$env:KRAKEN_API_SECRET = "..."
$env:KRAKEN_BASE_URL = "https://api.kraken.com"
```

## Security Notes

- Do not commit secrets.
- Rotate API keys regularly.
- Use paper/sandbox environments for dry runs where available.
