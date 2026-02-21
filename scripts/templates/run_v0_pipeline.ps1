param(
    [string]$Provider = "alpaca",
    [string]$Market = "equities",
    [string]$Symbol = "AAPL",
    [string]$Timeframe = "1m",
    [string]$Side = "buy",
    [string]$SizeHint = "1",
    [double]$Confidence = 0.7
)

$ErrorActionPreference = "Stop"

$runId = Get-Date -Format "yyyyMMdd-HHmmss"
$outDir = Join-Path "artifacts" $runId
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

$fetchOut = Join-Path $outDir "fetch.json"
$barsOut = Join-Path $outDir "bars.json"
$intentOut = Join-Path $outDir "intent.json"
$execOut = Join-Path $outDir "execution.json"

cargo run -p thales-cli -- fetch-market-data --provider $Provider --symbol $Symbol --timeframe $Timeframe > $fetchOut
cargo run -p thales-cli -- normalize-bars --input $fetchOut > $barsOut
cargo run -p thales-cli -- generate-trade-intent --market $Market --symbol $Symbol --side $Side --size-hint $SizeHint --confidence $Confidence > $intentOut
cargo run -p thales-cli -- validate-intent --input $intentOut
cargo run -p thales-cli -- execute-intent --provider $Provider --input $intentOut > $execOut

Write-Output "Pipeline complete. Artifacts written to $outDir"
