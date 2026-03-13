import subprocess
import json
import os
import sys
import re
import shutil
import math
import time
from datetime import datetime

# Paths
CLI_PATH = "./target/release/thales-cli"
PORTFOLIO_PATH = "portfolio.md"
STRATEGIES_PATH = "strategies.md"
HISTORY_PATH = "history.json"
SIGNALS_PATH = "Signals.md"
ARCHIVE_PATH = "Signals_Archive.md"

MEAN_REVERSION_STRATEGIES = {
    "CmoMeanReversion",
    "WilliamsR",
    "RsiMeanReversion",
    "StochasticOscillator",
    "MoneyFlowIndex",
    "ZScoreMeanReversion",
    "ConnorsRsiMeanReversion",
    "VwapReversion",
    "BollingerBands",
    "StochRsiMeanReversion",
}
TREND_FOLLOWING_STRATEGIES = {
    "AlmaCrossover",
    "LinearRegressionTrend",
    "ParabolicSar",
    "EmaCrossover",
    "AdxMacdTrend",
    "Supertrend",
    "SupertrendEmaCrossover",
    "KeltnerChannelBreakout",
    "DonchianBreakout",
    "ChaikinMoneyFlow",
    "ElderRay",
    "VortexBreakout",
    "CciMomentum",
    "Macd",
    "MacdRsiTrend",
    "IchimokuCloud",
    "AroonOscillator",
    "TrixMomentum",
    "RocMomentum",
    "AwesomeOscillator",
    "AdxMomentum",
    "VwmaCrossover",
    "ChandelierExit",
    "ObvTrendFollowing",
    "TsiTrend",
    "WmaCrossover",
    "HmaCrossover",
    "KamaTrendFollowing",
}
BREAKOUT_STRATEGIES = {
    "Supertrend",
    "KeltnerChannelBreakout",
    "CciMomentum",
    "ParabolicSar",
    "DonchianBreakout",
}
EXECUTED_STATUSES = {"filled", "executed", "closed"}
SUBMITTED_STATUSES = {
    "submitted",
    "accepted",
    "new",
    "open",
    "pending",
    "partially_filled",
    "partially-filled",
}
REJECTED_STATUSES = {"rejected", "failed", "error", "canceled", "cancelled", "expired", "invalid"}

def run_command(args):
    """Runs a thales-cli command and returns the parsed JSON data."""
    cmd = [CLI_PATH] + args
    run_command.last_error = None
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, check=False)
        stdout = (result.stdout or "").strip()
        stderr = (result.stderr or "").strip()

        envelope = None
        if stdout:
            json_start = stdout.find("{")
            if json_start != -1:
                try:
                    envelope = json.loads(stdout[json_start:])
                except json.JSONDecodeError:
                    envelope = None

        if envelope and envelope.get("status") == "ok":
            return envelope.get("data")

        if envelope and envelope.get("status") == "error":
            errors = envelope.get("errors") or []
            reason = "; ".join(str(err) for err in errors) if errors else "Execution failed"
            run_command.last_error = reason
            print(f"Error executing {args}: {reason}")
            return None

        if result.returncode != 0:
            run_command.last_error = stderr or stdout or f"Command failed with exit code {result.returncode}"
            print(f"Command failed: {cmd}\nReason: {run_command.last_error}")
            return None

        run_command.last_error = "Failed to parse JSON output."
        print(f"Failed to parse JSON output from {args}")
        if stdout:
            print(stdout)
        return None
    except Exception as e:
        run_command.last_error = str(e)
        print(f"Exception running command {cmd}: {e}")
        return None

run_command.last_error = None

def get_active_strategies():
    """Parses strategies.md to find all active strategy names."""
    if not os.path.exists(STRATEGIES_PATH):
        print(f"Warning: {STRATEGIES_PATH} not found.")
        return []

    with open(STRATEGIES_PATH, "r") as f:
        content = f.read()

    strategies = []
    if "BollingerBandsMeanReversion" in content or "BollingerBands" in content:
        strategies.append("BollingerBands")
    if "EmaCrossover" in content:
        strategies.append("EmaCrossover")
    if "RsiMeanReversion" in content:
        strategies.append("RsiMeanReversion")
    if "Macd" in content:
        strategies.append("Macd")
    if "Supertrend" in content:
        strategies.append("Supertrend")
    if "DonchianBreakout" in content:
        strategies.append("DonchianBreakout")
    if "ParabolicSar" in content:
        strategies.append("ParabolicSar")
    if "KeltnerChannelBreakout" in content:
        strategies.append("KeltnerChannelBreakout")
    if "StochasticOscillator" in content:
        strategies.append("StochasticOscillator")
    if "AdxMomentum" in content:
        strategies.append("AdxMomentum")
    if "IchimokuCloud" in content:
        strategies.append("IchimokuCloud")
    if "CciMomentum" in content:
        strategies.append("CciMomentum")
    if "ChaikinMoneyFlow" in content:
        strategies.append("ChaikinMoneyFlow")
    if "ChandelierExit" in content:
        strategies.append("ChandelierExit")
    if "LinearRegressionTrend" in content:
        strategies.append("LinearRegressionTrend")
    if "ObvTrendFollowing" in content:
        strategies.append("ObvTrendFollowing")
    if "MoneyFlowIndex" in content:
        strategies.append("MoneyFlowIndex")
    if "ConnorsRsiMeanReversion" in content:
        strategies.append("ConnorsRsiMeanReversion")
    if "AlmaCrossover" in content:
        strategies.append("AlmaCrossover")
    if "AwesomeOscillator" in content:
        strategies.append("AwesomeOscillator")
    if "WilliamsR" in content:
        strategies.append("WilliamsR")
    if "VwmaCrossover" in content:
        strategies.append("VwmaCrossover")
    if "VwapReversion" in content:
        strategies.append("VwapReversion")
    if "VortexBreakout" in content:
        strategies.append("VortexBreakout")
    if "RocMomentum" in content:
        strategies.append("RocMomentum")
    if "TrixMomentum" in content:
        strategies.append("TrixMomentum")
    if "AdxMacdTrend" in content:
        strategies.append("AdxMacdTrend")
    if "ZScoreMeanReversion" in content:
        strategies.append("ZScoreMeanReversion")
    if "ElderRay" in content:
        strategies.append("ElderRay")
    if "AroonOscillator" in content:
        strategies.append("AroonOscillator")
    if "StochRsiMeanReversion" in content:
        strategies.append("StochRsiMeanReversion")
    if "MacdRsiTrend" in content:
        strategies.append("MacdRsiTrend")
    if "TsiTrend" in content:
        strategies.append("TsiTrend")
    if "HmaCrossover" in content:
        strategies.append("HmaCrossover")

    return strategies

def _normalize_text(value):
    return (value or "").strip().lower()

def classify_market_regime(analysis):
    """Classifies market regime into a small set used for strategy routing."""
    if not analysis:
        return "unknown"

    regime = _normalize_text(analysis.get("regime"))
    volatility = _normalize_text(analysis.get("volatility"))

    if "rang" in regime or "sideway" in regime:
        return "ranging"
    if "trend" in regime:
        if "up" in regime or "bull" in regime:
            return "trending_up"
        if "down" in regime or "bear" in regime:
            return "trending_down"
        return "trending"
    if "high" in volatility or "extreme" in volatility:
        return "volatile"
    return "unknown"

def select_strategies_for_analysis(active_strategies, analysis):
    """Selects an active strategy subset that matches the current regime."""
    if not active_strategies:
        return []

    # Per user request: Evaluate candidate against EVERY active strategy.
    # Regime filtering is disabled to allow all strategies to run.
    return list(active_strategies)

def strategy_regime_weight(strategy_name, analysis):
    """Returns a scoring weight for conflict resolution based on regime fit."""
    regime_class = classify_market_regime(analysis)

    if regime_class == "ranging":
        if strategy_name in MEAN_REVERSION_STRATEGIES:
            return 1.2
        if strategy_name in TREND_FOLLOWING_STRATEGIES:
            return 0.9
    elif regime_class in {"trending_up", "trending_down", "trending"}:
        if strategy_name in TREND_FOLLOWING_STRATEGIES:
            return 1.2
        if strategy_name in MEAN_REVERSION_STRATEGIES:
            return 0.9
    elif regime_class == "volatile":
        if strategy_name in BREAKOUT_STRATEGIES:
            return 1.2
        if strategy_name in MEAN_REVERSION_STRATEGIES:
            return 0.85

    return 1.0

def archive_signals(days=2):
    """Moves signals older than `days` from Signals.md to Signals_Archive.md."""
    if not os.path.exists(SIGNALS_PATH):
        return

    with open(SIGNALS_PATH, "r") as f:
        content = f.read()

    # Split using same logic as parsing
    # First chunk is usually header/preamble
    chunks = re.split(r"\n## ", content)

    keep_chunks = []
    archive_chunks = []

    now = int(datetime.now().timestamp() * 1000)
    cutoff_ms = days * 24 * 60 * 60 * 1000

    # Process chunks
    for i, chunk in enumerate(chunks):
        # Reconstruct full chunk text for writing back
        # If i > 0, it was split by "\n## ", so we need to add "## " back
        full_chunk_text = f"\n## {chunk}" if i > 0 else chunk

        # Check timestamp in JSON
        json_match = re.search(r"```json\s*(\{.*?\})\s*```", chunk, re.DOTALL)
        is_stale = False

        if json_match:
            try:
                data = json.loads(json_match.group(1))
                ts = data.get("timestamp_unix_ms", 0)
                if (now - ts) > cutoff_ms:
                    is_stale = True
            except:
                pass

        # Also check header timestamp if JSON missing/parse error?
        # Header format: **Timestamp (ms)**: 1771870472944
        if not is_stale and not json_match:
             ts_match = re.search(r"\*\*Timestamp \(ms\)\*\*:\s*(\d+)", chunk)
             if ts_match:
                 ts = int(ts_match.group(1))
                 if (now - ts) > cutoff_ms:
                     is_stale = True

        if is_stale:
            archive_chunks.append(full_chunk_text)
        else:
            keep_chunks.append(full_chunk_text)

    if archive_chunks:
        print(f"Archiving {len(archive_chunks)} stale signals to {ARCHIVE_PATH}...")

        # Append to archive
        with open(ARCHIVE_PATH, "a") as f:
            for chunk in archive_chunks:
                f.write(chunk)

        # Rewrite active signals
        with open(SIGNALS_PATH, "w") as f:
            # Join chunks. First chunk doesn't have newline prefix if it was original start
            # But keep_chunks[0] might be the original start OR a later chunk.
            # If keep_chunks[0] starts with "\n## ", and we write it, it might add extra newline at start of file?
            # Actually full_chunk_text includes "\n## " for i > 0.
            # If i=0 was kept, it has no prefix.
            # If i=0 was archived, keep_chunks[0] will have prefix "\n## ".
            # We should probably trim the first one if it starts with newline?

            output = "".join(keep_chunks)
            if output.startswith("\n"):
                output = output.lstrip("\n")
            f.write(output)

def get_executed_signal_refs():
    """Reads portfolio.md to get a set of already executed/submitted signal IDs."""
    if not os.path.exists(PORTFOLIO_PATH):
        return set()

    with open(PORTFOLIO_PATH, 'r') as f:
        content = f.read()

    executed_refs = set()
    match = re.search(r"## Executed Trades\n(.*?)(?:\n## |$)", content, re.DOTALL)
    if match:
        table_lines = match.group(1).strip().split('\n')
        for line in table_lines:
            if line.startswith('|') and not line.startswith('| Date/Time') and not line.startswith('|---'):
                parts = [p.strip() for p in line.split('|')]
                if len(parts) > 10:
                    signal_ref = parts[10]
                    if signal_ref and signal_ref != '-' and signal_ref != 'None':
                        executed_refs.add(signal_ref.replace("\\|", "|"))
    return executed_refs

def get_candidates_from_signals():
    """Parses Signals.md for potential candidates."""
    if not os.path.exists(SIGNALS_PATH):
        return []

    with open(SIGNALS_PATH, "r") as f:
        content = f.read()

    candidates = {}
    executed_refs = get_executed_signal_refs()

    # New parsing logic to handle sections and extract full context
    chunks = re.split(r"\n## ", content)
    for chunk in chunks:
        # Strip leading # and whitespace. This handles the very first chunk which might start with ##
        chunk = chunk.lstrip('#').strip()
        if not chunk: continue

        market = None
        symbol = None

        # Format 1: Market Analysis Report - <market> - <symbol>
        match1 = re.match(r"Market Analysis Report - (\w+) - ([\w/]+)", chunk)
        if match1:
            market = match1.group(1)
            symbol = match1.group(2)

        # Format 2: Symbol: <symbol> (<market>)
        if not symbol:
            match2 = re.match(r"Symbol: ([\w/]+) \((\w+)\)", chunk)
            if match2:
                symbol = match2.group(1)
                market = match2.group(2)

        if not symbol:
            continue

        if os.environ.get("SIMULATION") == "true":
            provider = "paper"
        else:
            provider = "kraken"

        # Extract JSON
        json_match = re.search(r"```json\s*(\{.*?\})\s*```", chunk, re.DOTALL)
        raw_json = None
        if json_match:
            try:
                raw_json = json.loads(json_match.group(1))
            except:
                pass

# Check for Staleness (24 hours = 86400000 ms)
        signal_ref = "NO_REF"
        expected_side = None
        if raw_json and raw_json.get("timestamp_unix_ms"):
            ts = raw_json["timestamp_unix_ms"]
            rec = raw_json.get("recommendation", "").lower()
            if "short" in rec or "sell" in rec: expected_side = "sell"
            elif "long" in rec or "buy" in rec: expected_side = "buy"

            side_str = expected_side if expected_side else "unknown"
            signal_ref = f"{market}:{symbol}:{side_str}:{ts}"

            if signal_ref in executed_refs:
                reason = f"Signal already executed/submitted (Ref: {signal_ref})"
                print(f"Skipping already processed signal for {symbol}: {reason}")

                dummy_intent = {
                     "symbol": symbol,
                     "intent_id": signal_ref,
                     "rationale": "Signal already processed"
                }
                log_skipped(dummy_intent, reason)
                continue

            now = int(datetime.now().timestamp() * 1000)
            if (now - ts) > 86400000:
                 age_hours = (now - ts) / 3600000
                 reason = f"Signal too old ({age_hours:.1f} hours > 24 hours)"
                 print(f"Skipping stale signal for {symbol}: {reason}")

                 # Log to portfolio.md
                 dummy_intent = {
                     "symbol": symbol,
                     "intent_id": signal_ref,
                     "rationale": "Stale signal from Signals.md"
                 }
                 log_skipped(dummy_intent, reason)
                 continue

        # Extract Research
        research_text = None
        res_match = re.search(r"\*\*Research\*\*:\s*(.*?)(?=\n\n|\n\*\*|\n###|\n\*Historical Context\*|$)", chunk, re.DOTALL)
        if res_match:
            research_text = res_match.group(1).strip().replace("\n", " ").replace("\r", " ")
        else:
             res_match_alt = re.search(r"\*External Research\*:\s*(.*?)(?=\n\n|\n\*\*|\n###|\n\*Historical Context\*|$)", chunk, re.DOTALL)
             if res_match_alt:
                 research_text = res_match_alt.group(1).strip().replace("\n", " ").replace("\r", " ")

        # Extract News
        news_text = None
        news_match = re.search(r"\*\*News\*\*:\s*(.*?)(?=\n\n|\n\*\*|\n###|\n\*Historical Context\*|$)", chunk, re.DOTALL)
        if news_match:
            news_text = news_match.group(1).strip().replace("\n", " ").replace("\r", " ")

        if raw_json:
            if research_text:
                raw_json["research_summary"] = research_text
            if news_text:
                raw_json["news_summary"] = news_text

        candidates[symbol] = {
            "provider": provider,
            "symbol": symbol,
            "market": market,
            "raw_analysis_json": raw_json,
            "expected_side": expected_side,
            "signal_ref": signal_ref
        }

    return list(candidates.values())

def fetch_positions(provider):
    """Fetches open positions for a provider."""
    # print(f"Fetching positions from {provider}...")
    positions = run_command(["get-positions", "--provider", provider])
    return positions if positions else []

def get_all_positions():
    """Fetches all open positions across providers and saves to temp file."""
    all_positions = []

    # Try fetching from both providers
    # If one fails (e.g. no creds), it returns empty list or None handled by fetch_positions
    if os.environ.get("SIMULATION") == "true":
        paper_pos = fetch_positions("paper")
        if paper_pos:
            all_positions.extend(paper_pos)
    else:
        kraken_pos = fetch_positions("kraken")
        if kraken_pos:
            all_positions.extend(kraken_pos)

        alpaca_pos = fetch_positions("alpaca")
        if alpaca_pos:
            all_positions.extend(alpaca_pos)

    temp_file = "temp_portfolio.json"
    with open(temp_file, "w") as f:
        json.dump(all_positions, f)

    return temp_file

def scan_markets():
    """Scans markets for candidates."""
    candidates = []

    if os.environ.get("SIMULATION") == "true":
        print("Scanning Paper (Simulation)...")
        paper_scan = run_command(["scan-market", "--provider", "paper"])
        if paper_scan:
            for symbol in paper_scan:
                # Infer market type
                market = "crypto" if "USD" in symbol and "SPY" not in symbol else "equities"
                candidates.append({"provider": "paper", "symbol": symbol, "market": market})
    else:
        # Crypto (Kraken)
        print("Scanning Kraken (Crypto)...")
        crypto = run_command(["scan-market", "--provider", "kraken", "--top-n", "10", "--min-volatility", "0.01", "--min-momentum", "0.0"])
        if crypto:
            for symbol in crypto:
                candidates.append({"provider": "kraken", "symbol": symbol, "market": "crypto"})

        # Equities (Kraken)
        print("Scanning Kraken (Equities)...")
        # Currently the thales-cli natively handles scanning equities via Alpaca, but the user requested us to ensure we use Kraken for BOTH crypto and equities.
        # We scan using alpaca to get the equity symbols, but assign the provider as "kraken" to route trades and data via Kraken.
        equities = run_command(["scan-market", "--provider", "alpaca"])
        if equities:
            for symbol in equities:
                candidates.append({"provider": "kraken", "symbol": symbol, "market": "equities"})

    return candidates

def evaluate_candidate(candidate, strategies, portfolio_path=None):
    """Fetches data and generates signals for a candidate using all active strategies."""
    provider = candidate["provider"]
    symbol = candidate["symbol"]

    # Fetch Data
    bars = run_command(["fetch-market-data", "--provider", provider, "--symbol", symbol, "--timeframe", "1h"])
    if not bars or not isinstance(bars, dict) or not bars.get("bars"):
        return []

    # Save temp bars
    safe_symbol = symbol.replace("/", "_")
    temp_bars_file = f"temp_bars_{safe_symbol}.json"
    with open(temp_bars_file, "w") as f:
        json.dump(bars, f)

    # Generate Analysis (for history + regime mapping)
    analysis = run_command(["analyze-market", "--input", temp_bars_file, "--no-report"])
    effective_analysis = candidate.get("raw_analysis_json") or analysis

    selected_strategies = select_strategies_for_analysis(strategies, effective_analysis)
    regime_label = classify_market_regime(effective_analysis)
    print(
        f"  {symbol}: regime={regime_label}, using strategies={selected_strategies}"
    )

    all_generated_intents = []
    temp_analysis_file = None
    if effective_analysis:
        temp_analysis_file = f"temp_analysis_{safe_symbol}.json"
        with open(temp_analysis_file, "w") as f:
            json.dump(effective_analysis, f)

    for strategy_name in selected_strategies:
        # Generate Signals
        args = ["generate-signals", "--input", temp_bars_file, "--strategy", strategy_name]
        if os.path.exists(HISTORY_PATH):
            args.extend(["--history", HISTORY_PATH])

        if portfolio_path and os.path.exists(portfolio_path):
            args.extend(["--portfolio", portfolio_path])

        # Pass explicit analysis so each strategy evaluates the same context.
        if temp_analysis_file and os.path.exists(temp_analysis_file):
            args.extend(["--analysis", temp_analysis_file])

        # Dynamic Risk Config
        risk_per_trade = os.environ.get("RISK_PER_TRADE", "100.0")
        args.extend(["--risk", risk_per_trade])

        intents = run_command(args)

        if intents:
            # Enrich intent with provider and strategy info
            for intent in intents:
                # Use scan provider for intent
                intent["provider"] = provider

                if effective_analysis:
                    intent["_market_analysis"] = effective_analysis

            all_generated_intents.extend(intents)

    if temp_analysis_file and os.path.exists(temp_analysis_file):
        os.remove(temp_analysis_file)

    # Cleanup temp bars
    if os.path.exists(temp_bars_file):
        os.remove(temp_bars_file)

    return all_generated_intents

def resolve_conflicts(intents, candidate):
    """
    Resolves conflicts among signals for the same candidate.
    - Filters out intents that conflict with Signals.md expected side.
    - If strategies conflict (Buy vs Sell), strictly REJECTS execution for that asset.
    - Returns single best intent.
    """
    if not intents:
        return []

    symbol = candidate["symbol"]
    signal_ref = candidate.get("signal_ref", "NO_REF")
    expected_side = candidate.get("expected_side")

    if expected_side:
        valid_intents = [i for i in intents if i["side"] == expected_side]
        if not valid_intents:
            invalid_sides = set(i["side"] for i in intents)
            reason = f"Cross-validation failed: Strategies generated {invalid_sides} but signal recommended {expected_side}."
            print(f"  {symbol}: {reason}")
            dummy_intent = {"symbol": symbol, "intent_id": signal_ref}
            log_skipped(dummy_intent, reason)
            return []
        intents = valid_intents

    sides = set(intent["side"] for intent in intents)
    if len(sides) > 1:
        reason = f"Conflict: Active strategies generated conflicting signals ({sides}) for {symbol}."
        print(f"  {symbol}: {reason}")
        dummy_intent = {"symbol": symbol, "intent_id": signal_ref}
        log_skipped(dummy_intent, reason)
        return []

    analysis = intents[0].get("_market_analysis")
    def score_intent(intent):
        confidence = float(intent.get("confidence", 0.0) or 0.0)
        strategy_name = intent.get("strategy", "")
        return confidence * strategy_regime_weight(strategy_name, analysis)

    intents.sort(
        key=lambda x: (score_intent(x), float(x.get("confidence", 0.0) or 0.0)),
        reverse=True,
    )
    best_intent = intents[0]

    if signal_ref != "NO_REF":
        best_intent["intent_id"] = signal_ref

    return [best_intent]

def update_history(intent):
    """Appends executed trade to history.json."""
    if "_market_analysis" not in intent:
        return

    analysis = intent["_market_analysis"]
    # Clean up internal field before saving? Or keep it separate.
    # We need to construct HistoryEntry: { intent, market_analysis, outcome }

    # Create a clean intent copy without internal fields
    clean_intent = intent.copy()
    if "_market_analysis" in clean_intent:
        del clean_intent["_market_analysis"]
    if "provider" in clean_intent: # provider is also internal
        del clean_intent["provider"]

    entry = {
        "intent": clean_intent,
        "market_analysis": analysis,
        "outcome": None
    }

    history = []
    if os.path.exists(HISTORY_PATH):
        try:
            with open(HISTORY_PATH, "r") as f:
                history = json.load(f)
        except Exception as e:
            print(f"Warning: Failed to load history.json: {e}")
            # Backup corrupted file
            timestamp = datetime.now().strftime("%Y%m%d%H%M%S")
            backup_path = f"{HISTORY_PATH}.bak.{timestamp}"
            try:
                shutil.copy(HISTORY_PATH, backup_path)
                print(f"Backed up corrupted history to {backup_path}")
            except Exception as copy_err:
                print(f"Failed to backup corrupted history: {copy_err}")
            history = []

    history.append(entry)

    with open(HISTORY_PATH, "w") as f:
        json.dump(history, f, indent=2)

def append_to_section(filepath, section_header, table_header, row):
    """
    Appends a row to a specific section's table in a markdown file.
    Creates the file/section/table if they don't exist.
    """
    if not os.path.exists(filepath):
        col_count = table_header.count("|") - 1
        if col_count < 1: col_count = 1
        separator = "|" + "---|" * col_count
        with open(filepath, "w") as f:
            f.write(f"{section_header}\n\n{table_header}\n{separator}\n{row}\n")
        return

    with open(filepath, "r") as f:
        lines = f.readlines()

    # Find section
    section_idx = -1
    clean_header = section_header.strip()
    for i, line in enumerate(lines):
        if line.strip() == clean_header:
            section_idx = i
            break

    if section_idx != -1:
        # Section exists.
        # Look for table separator line |---| to identify existing table
        table_start_idx = -1
        for i in range(section_idx + 1, len(lines)):
            if lines[i].strip().startswith("#"): # Start of next section
                break
            # Check for separator line (must contain |, -, and optionally space/newlines)
            stripped = lines[i].strip()
            if "|" in stripped and set(stripped).issubset(set("|- \n")):
                 # The line BEFORE is likely the header
                 table_start_idx = i - 1
                 break

        if table_start_idx != -1:
            # Table found. Find end of table.
            table_end_idx = table_start_idx + 2 # Skip header and separator
            while table_end_idx < len(lines):
                line = lines[table_end_idx].strip()
                if not line.startswith("|"):
                    break
                table_end_idx += 1

            lines.insert(table_end_idx, row + "\n")
        else:
            # Table not found in section. Insert after section header.
            # Add separator line as well based on header columns
            col_count = table_header.count("|") - 1
            if col_count < 1: col_count = 1
            separator = "|" + "---|" * col_count
            lines.insert(section_idx + 1, f"\n{table_header}\n{separator}\n{row}\n")

    else:
        # Section doesn't exist. Append to end of file.
        # Ensure we have a newline before if file not empty
        prefix = "\n" if lines and lines[-1].strip() != "" else ""

        # Build separator
        col_count = table_header.count("|") - 1
        if col_count < 1: col_count = 1
        separator = "|" + "---|" * col_count

        lines.append(f"{prefix}{section_header}\n\n{table_header}\n{separator}\n{row}\n")

    with open(filepath, "w") as f:
        f.writelines(lines)

def normalize_execution_status(exec_result):
    return str(exec_result.get("status", "")).strip().lower()

def classify_execution_outcome(exec_result):
    status = normalize_execution_status(exec_result)
    if status in EXECUTED_STATUSES:
        return "executed"
    if status in SUBMITTED_STATUSES:
        return "submitted"
    if status in REJECTED_STATUSES:
        return "rejected"
    return "unknown"

def log_trade(intent, result, slippage=None):
    """Logs executed trade to portfolio.md"""
    date_str = datetime.fromtimestamp(result["submitted_at_unix_ms"] / 1000).strftime("%Y-%m-%d %H:%M:%S")
    asset_class = intent.get("market", "-")
    symbol = intent["symbol"]

    # Enrich Action with Signal Type if available
    action = intent["side"]
    signal_type_raw = intent.get("signal_type", "")
    # Clean up Rust enum string if present (e.g. SignalType::Entry -> Entry)
    if "SignalType::" in signal_type_raw:
        signal_type_clean = signal_type_raw.replace("SignalType::", "")
    else:
        signal_type_clean = signal_type_raw

    if signal_type_clean:
        action = f"{action} ({signal_type_clean})"

    # Escape pipes in string fields to prevent markdown table corruption
    asset_class = asset_class.replace("|", "\\|")
    symbol = symbol.replace("|", "\\|")
    action = action.replace("|", "\\|")

    size = intent["size_hint"]
    price = str(intent.get("limit_price", "Market"))
    if price == "None": price = "Market"

    sl = str(intent.get("stop_loss", "-"))
    if sl == "None": sl = "-"

    tp = str(intent.get("take_profit", "-"))
    if tp == "None": tp = "-"

    # Calc max risk if possible
    max_risk = "-"
    if sl != "-" and price != "Market":
        try:
             entry = float(price)
             stop = float(sl)
             qty = float(size)
             max_risk = f"{abs(entry - stop) * qty:.2f}"
        except:
             pass

    signal_ref = intent["intent_id"].replace("|", "\\|")
    rationale = intent["rationale"].replace("\n", " ").replace("\r", " ").replace("|", "\\|")

    if slippage is not None:
        rationale += f" [Slippage: {slippage:.4f}%]"

    header = "| Date/Time | Asset Class | Symbol/Contract | Action | Size/Qty | Entry Price | SL | TP | Max Risk | Signal Ref | Rationale |"
    row = f"| {date_str} | {asset_class} | {symbol} | {action} | {size} | {price} | {sl} | {tp} | {max_risk} | {signal_ref} | {rationale} |"

    append_to_section(PORTFOLIO_PATH, "## Executed Trades", header, row)

def log_submitted(intent, result):
    """Logs accepted/submitted orders that are not yet filled."""
    date_str = datetime.fromtimestamp(result.get("submitted_at_unix_ms", int(datetime.now().timestamp() * 1000)) / 1000).strftime("%Y-%m-%d %H:%M:%S")
    asset_class = intent.get("market", "-")
    symbol = intent.get("symbol", "-")

    # Enrich Action with Signal Type if available
    action = intent.get("side", "-")
    signal_type_raw = intent.get("signal_type", "")
    # Clean up Rust enum string if present (e.g. SignalType::Entry -> Entry)
    if "SignalType::" in signal_type_raw:
        signal_type_clean = signal_type_raw.replace("SignalType::", "")
    else:
        signal_type_clean = signal_type_raw

    if signal_type_clean:
        action = f"{action} ({signal_type_clean})"

    # Escape pipes
    asset_class = asset_class.replace("|", "\\|")
    symbol = symbol.replace("|", "\\|")
    action = action.replace("|", "\\|")

    size = intent.get("size_hint", "0")
    price = str(intent.get("limit_price", "Market"))
    if price == "None": price = "Market"

    sl = str(intent.get("stop_loss", "-"))
    if sl == "None": sl = "-"

    tp = str(intent.get("take_profit", "-"))
    if tp == "None": tp = "-"

    # Calc max risk if possible
    max_risk = "-"
    if sl != "-" and price != "Market":
        try:
             entry = float(price)
             stop = float(sl)
             qty = float(size)
             max_risk = f"{abs(entry - stop) * qty:.2f}"
        except:
             pass

    signal_ref = intent.get("intent_id", "-").replace("|", "\\|")
    rationale = intent.get("rationale", "-").replace("\n", " ").replace("\r", " ").replace("|", "\\|")

    # The prompt required submitted or executed trades to be appended to portfolio.md in this exact 11 columns format
    header = "| Date/Time | Asset Class | Symbol/Contract | Action | Size/Qty | Entry Price | SL | TP | Max Risk | Signal Ref | Rationale |"
    row = f"| {date_str} | {asset_class} | {symbol} | {action} | {size} | {price} | {sl} | {tp} | {max_risk} | {signal_ref} | {rationale} |"

    # Still appending to Executed Trades since "After every execution, append to portfolio.md..." applies to placed orders
    append_to_section(PORTFOLIO_PATH, "## Executed Trades", header, row)

def log_skipped(intent, reason):
    """Logs skipped trade."""
    date_str = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    symbol = intent["symbol"].replace("|", "\\|")
    signal_ref = intent["intent_id"].replace("|", "\\|")
    reason = str(reason).replace("|", "\\|")

    header = "| Date/Time | Symbol | Signal Ref | Rejection Reason |"
    row = f"| {date_str} | {symbol} | {signal_ref} | {reason} |"

    append_to_section(PORTFOLIO_PATH, "## Skipped Signals", header, row)

def fetch_buying_power(provider, symbol):
    """Fetches provider buying power for sizing buy orders."""
    args = ["get-buying-power", "--provider", provider]
    if symbol:
        args.extend(["--symbol", symbol])

    data = run_command(args)
    if not isinstance(data, dict):
        return None, None

    amount_raw = data.get("amount")
    currency = str(data.get("currency", "USD"))
    try:
        amount = float(amount_raw)
    except (TypeError, ValueError):
        return None, None

    if not math.isfinite(amount):
        return None, None

    return amount, currency

def fetch_sellable_balance(provider, symbol):
    """Fetches provider sellable base-asset balance for a symbol."""
    if not symbol:
        return None, None

    data = run_command(["get-selling-power", "--provider", provider, "--symbol", symbol])
    if not isinstance(data, dict):
        return None, None

    amount_raw = data.get("amount")
    asset = str(data.get("asset", symbol))
    try:
        amount = float(amount_raw)
    except (TypeError, ValueError):
        return None, None

    if not math.isfinite(amount):
        return None, None

    return amount, asset

def format_size_hint(size):
    """Formats a numeric size as a compact decimal string."""
    formatted = f"{size:.8f}".rstrip("0").rstrip(".")
    return formatted if formatted else "0"

def adjust_buy_size_to_buying_power(intent, current_price, safety_buffer=0.99):
    """
    Caps buy size by available buying power.
    Returns (bool, reason). False means skip execution.
    """
    if not isinstance(intent, dict):
        return False, "Invalid intent format"

    if intent.get("side") != "buy":
        return True, "Not a buy signal"

    size_hint = intent.get("size_hint", "0")
    if size_hint == "max":
        return True, "Max sizing handled by provider"

    try:
        requested_size = float(size_hint)
    except (TypeError, ValueError):
        return False, f"Invalid size format: {size_hint}"

    if requested_size <= 0 or not math.isfinite(requested_size):
        return False, f"Invalid size: {size_hint}"

    if current_price is None:
        return False, "Unable to fetch current price for size validation"
    try:
        price = float(current_price)
    except (TypeError, ValueError):
        return False, "Invalid current price for size validation"

    if price <= 0 or not math.isfinite(price):
        return False, "Invalid current price for size validation"

    provider = intent.get("provider", "")
    symbol = intent.get("symbol", "")
    buying_power, currency = fetch_buying_power(provider, symbol)

    if buying_power is None:
        return False, f"Unable to determine buying power for {provider}:{symbol}"
    if buying_power <= 0:
        return False, f"No buying power available ({currency} {buying_power:.2f})"

    max_affordable_size = (buying_power * safety_buffer) / price
    if max_affordable_size <= 0 or not math.isfinite(max_affordable_size):
        return False, "Computed affordable size is invalid"

    if requested_size <= max_affordable_size:
        return True, "Size within buying power"

    adjusted_size = format_size_hint(max_affordable_size)
    if adjusted_size == "0":
        return False, f"No buying power available after buffer ({currency} {buying_power:.2f})"

    intent["size_hint"] = adjusted_size
    old_rationale = intent.get("rationale", "").strip()
    sizing_note = (
        f"Size adjusted from {size_hint} to {adjusted_size} based on "
        f"{currency} buying power ({buying_power:.2f})"
    )
    if old_rationale:
        intent["rationale"] = f"{old_rationale} [{sizing_note}]"
    else:
        intent["rationale"] = f"[{sizing_note}]"

    print(
        f"Adjusted buy size for {symbol}: {size_hint} -> {adjusted_size} "
        f"(buying power: {currency} {buying_power:.2f})"
    )
    return True, "Size adjusted to available buying power"

def adjust_sell_size_to_sellable_balance(intent):
    """
    Ensures sell size does not exceed available holdings.
    If requested size is too large, converts to `max` (sell all).
    Returns (bool, reason). False means skip execution.
    """
    if not isinstance(intent, dict):
        return False, "Invalid intent format"

    if intent.get("side") != "sell":
        return True, "Not a sell signal"

    size_hint = intent.get("size_hint", "0")
    if size_hint == "max":
        return True, "Max sizing handled by provider"

    try:
        requested_size = float(size_hint)
    except (TypeError, ValueError):
        return False, f"Invalid size format: {size_hint}"

    if requested_size <= 0 or not math.isfinite(requested_size):
        return False, f"Invalid size: {size_hint}"

    provider = intent.get("provider", "")
    symbol = intent.get("symbol", "")
    sellable_balance, asset = fetch_sellable_balance(provider, symbol)

    if sellable_balance is None:
        return False, f"Unable to determine sellable balance for {provider}:{symbol}"
    if sellable_balance <= 0:
        return False, f"No sellable balance available ({asset} {sellable_balance:.8f})"

    if requested_size <= sellable_balance:
        return True, "Size within sellable balance"

    intent["size_hint"] = format_size_hint(sellable_balance)
    old_rationale = intent.get("rationale", "").strip()
    sizing_note = (
        f"Sell size adjusted from {size_hint} to max based on "
        f"available {asset} balance ({sellable_balance:.8f})"
    )
    if old_rationale:
        intent["rationale"] = f"{old_rationale} [{sizing_note}]"
    else:
        intent["rationale"] = f"[{sizing_note}]"

    print(
        f"Adjusted sell size for {symbol}: {size_hint} -> max "
        f"(sellable: {asset} {sellable_balance:.8f})"
    )
    return True, "Size adjusted to available sellable balance"

def verify_risk(intent):
    """
    Risk Agent logic to verify trade intent before execution.
    Returns (bool, reason).
    """
    # Defensive checks
    if not isinstance(intent, dict):
        return False, "Invalid intent format"

    symbol = intent.get("symbol", "Unknown")
    side = intent.get("side", "unknown")
    signal_type = intent.get("signal_type", "Unknown")
    size_hint = intent.get("size_hint", "0")
    stop_loss = intent.get("stop_loss")
    confidence = intent.get("confidence", 0.0)

    # 1. Check Size
    if size_hint == "max":
        # Valid for Exit/ScaleOut
        pass
    else:
        try:
            size = float(size_hint)
            if math.isnan(size):
                 return False, f"Invalid size: NaN"
            if math.isinf(size):
                 return False, f"Invalid size: Infinity"
            if size <= 0:
                return False, f"Invalid size: {size_hint} (must be > 0)"

            # Kraken minimum order sizes (approximate, hardcoded for safety)
            # https://support.kraken.com/hc/en-us/articles/205893708-Minimum-order-size-volume-for-trading
            min_sizes = {
                "BTCUSD": 0.0001,
                "ETHUSD": 0.001,
            }
            if intent.get("provider") == "kraken" and symbol in min_sizes:
                if size < min_sizes[symbol]:
                    return False, f"Position size below exchange minimum for {symbol} ({size} < {min_sizes[symbol]})"
        except ValueError:
            return False, f"Invalid size format: {size_hint}"

    # 2. Check Stop Loss and Take Profit for Entries
    # Signal type might be "SignalType::Entry" string from Rust debug format
    # Or just "Entry"
    is_entry = False
    if signal_type:
        if "Entry" in signal_type or "ScaleIn" in signal_type:
            is_entry = True

    if is_entry:
        if stop_loss is None:
             return False, "Missing Stop Loss for Entry"
        if intent.get("take_profit") is None:
             return False, "Missing Take Profit for Entry"

    # 3. Check Confidence
    if confidence < 0.5:
        return False, f"Low confidence: {confidence}"

    return True, "Approved"

def manage_orders():
    """Checks open orders for fills and stale orders (> 5 mins)."""
    providers = []
    if os.environ.get("SIMULATION") == "true":
        providers.append("paper")
    else:
        # User requested direct API access to Kraken for BOTH crypto and equities.
        # Check Alpaca just in case since they want us to check current portfolio on Kraken and Alpaca
        providers.append("alpaca")
        providers.append("kraken")

    for provider in providers:
        # print(f"Checking open orders on {provider}...")
        orders = run_command(["get-open-orders", "--provider", provider])
        if not orders:
            continue

        now = int(datetime.now().timestamp() * 1000)
        for order in orders:
             submitted_at = order.get("submitted_at_unix_ms", 0)
             age_ms = now - submitted_at
             age_s = age_ms / 1000.0

             # Check for partial fills
             filled_qty = float(order.get("filled_qty", 0.0))
             qty = float(order.get("qty", 0.0))
             if filled_qty > 0 and filled_qty < qty:
                 log_msg = f"Partial fill: {filled_qty}/{qty} for {order['symbol']} ({provider})"
                 print(log_msg)
                 # We could log this to portfolio as a note or partial trade, but sticking to existing logic for now.
                 # Maybe append to skipped section as info?
                 dummy_intent = {
                     "symbol": order['symbol'],
                     "intent_id": f"PARTIAL-{order['id']}",
                     "rationale": log_msg
                 }
                 log_skipped(dummy_intent, "Partial Fill Notification")

             # Cancel stale orders
             if age_ms > 300000: # 5 minutes
                 print(f"Cancelling stale order {order['id']} ({order['symbol']}) - Age: {age_s:.0f}s")
                 run_command(["cancel-order", "--provider", provider, "--id", order['id']])

                 # Log cancellation
                 dummy_intent = {
                     "symbol": order['symbol'],
                     "intent_id": f"CANCEL-{order['id']}",
                     "rationale": f"Stale order ({age_s:.0f}s > 300s) canceled"
                 }
                 if filled_qty > 0 and filled_qty < qty:
                     dummy_intent["rationale"] = f"Stale partial order ({age_s:.0f}s > 300s) canceled, unfilled: {qty - filled_qty}"
                     log_skipped(dummy_intent, "Stale Partial Order Cancellation")
                 else:
                     log_skipped(dummy_intent, "Stale Order Cancellation")
             else:
                 # Adjust partial fill bounds if not stale
                 if filled_qty > 0 and filled_qty < qty:
                     # e.g., We might want to adjust a limit order if it's partially filled to ensure the rest gets filled.
                     # We can replace the order or adjust price based on market.
                     # Let's cancel the current order and issue a market order for the remaining amount
                     print(f"Order {order['id']} is partially filled. Adjusting remaining quantity: {qty - filled_qty}.")
                     run_command(["cancel-order", "--provider", provider, "--id", order['id']])

                     # Re-execute as a Market order for remaining amount to guarantee fill
                     remaining = format_size_hint(qty - filled_qty)
                     side = order.get("side", "buy")
                     symbol = order.get("symbol")

                     # We do not have the full original intent context here, so we construct a minimal one
                     adjustment_intent = {
                         "provider": provider,
                         "market": "unknown", # Could try to infer or pass down
                         "symbol": symbol,
                         "side": side,
                         "size_hint": remaining,
                         "confidence": 1.0, # High confidence for adjustment
                         "rationale": f"Adjusting partial fill for order {order['id']}",
                         "intent_id": f"ADJUST-{order['id']}",
                         "order_type": "market",
                         "execution_algo": "Market"
                     }

                     safe_symbol = symbol.replace("/", "_") if symbol else "unknown"
                     temp_intent_file = f"temp_intent_adjust_{safe_symbol}.json"
                     with open(temp_intent_file, "w") as f:
                         json.dump(adjustment_intent, f)

                     result = run_command(["execute-intent", "--provider", provider, "--input", temp_intent_file])

                     if os.path.exists(temp_intent_file):
                         os.remove(temp_intent_file)

                     if result:
                         exec_res = result[0] if isinstance(result, list) else result
                         log_submitted(adjustment_intent, exec_res)
                     else:
                         print(f"Failed to submit adjustment order for {order['id']}")

def get_latest_price(provider, symbol):
    """Fetches the latest close price for a symbol."""
    print(f"Fetching latest price for {symbol} on {provider}...")
    data = run_command(["fetch-market-data", "--provider", provider, "--symbol", symbol, "--timeframe", "1m"])

    if isinstance(data, dict):
        bars = data.get("bars", [])
    else:
        bars = data

    if bars and isinstance(bars, list) and len(bars) > 0:
        # Sort by timestamp just in case, though usually sorted
        bars.sort(key=lambda x: x.get("timestamp_unix_ms", 0))
        last_bar = bars[-1]
        price = last_bar.get("close")
        print(f"Latest price for {symbol}: {price}")
        return price
    print(f"Failed to fetch latest price for {symbol}")
    return None

def refine_intent(intent, current_price=None):
    """
    Refines trade intent with Algo Selection and Order Type.
    Updates intent in-place.
    """
    confidence = intent.get("confidence", 0.0)
    size_hint = intent.get("size_hint", "0")

    # 1. Algo Selection & Order Type
    # Default to Limit
    algo = "Limit"
    order_type = "limit"

    # Check for Large Size -> TWAP/VWAP
    is_large = False
    is_very_large = False
    try:
        if size_hint != "max":
            size = float(size_hint)
            # Simple heuristic: > 1000 units is "large".
            # In production this would depend on asset price and volume.
            if size > 10000.0:
                is_very_large = True
            elif size > 1000.0:
                is_large = True
    except:
        pass

    if is_very_large:
        algo = "VWAP"
        order_type = "limit" # Simulate VWAP with Limit for now, provider logs Algo
        print("Selected VWAP algorithm to minimize market impact for very large order.")
    elif is_large:
        algo = "TWAP"
        order_type = "limit" # Simulate TWAP with Limit for now, provider logs Algo
        print("Selected TWAP algorithm for large order.")
    elif confidence >= 0.8 or size_hint == "max":
        # High confidence or Exits -> Market (Urgent)
        algo = "Market"
        order_type = "market"
    else:
        # Standard Entry -> Limit
        algo = "Limit"
        order_type = "limit"

    # Handle Stop and Stop-Limit orders
    if intent.get("stop_price"):
        if intent.get("limit_price"):
            order_type = "stop-limit"
            algo = "Limit" # It's a stop-limit
            print("Selected Stop-Limit order type.")
        else:
            order_type = "stop"
            algo = "Market" # Trigger market order when stop hit
            print("Selected Stop order type.")

    intent["execution_algo"] = algo
    intent["order_type"] = order_type

    # 2. Slippage Control (Set Limit Price)
    if order_type == "limit" or order_type == "stop-limit":
        # Prefer strategy price if valid, else use current price
        if not intent.get("limit_price") and current_price:
             intent["limit_price"] = current_price

        if not intent.get("limit_price"):
            # Fallback if no current price and no strategy price
            print("Warning: No limit price available for Limit order. Falling back to Market.")
            intent["order_type"] = "market" if order_type == "limit" else "stop"
            intent["execution_algo"] = "Market"

    return intent

def monitor_execution(provider, order_id, expected_price):
    """
    Polls order status and calculates slippage.
    Returns executed price and slippage %.
    """
    print(f"Monitoring execution for order {order_id}...")

    # Poll for up to 30 seconds (15 * 2s) to allow for fills
    for _ in range(15):
        time.sleep(2)
        order = run_command(["get-order", "--provider", provider, "--id", order_id])
        if not order:
            continue

        status = order.get("status")
        if status == "filled":
            avg_price = order.get("average_fill_price")
            if avg_price:
                slippage = 0.0
                if expected_price and expected_price > 0:
                    # Slippage % = (Fill - Expected) / Expected
                    # For Buy: Positive is bad (paid more)
                    # For Sell: Negative is bad (sold less)
                    # Let's just log raw % diff
                    side = order.get("side", "")
                    if side:
                        side = side.lower()
                    if side == "buy":
                        slippage = (avg_price - expected_price) / expected_price * 100.0
                    elif side == "sell":
                        slippage = (expected_price - avg_price) / expected_price * 100.0
                    else:
                        slippage = (avg_price - expected_price) / expected_price * 100.0

                print(f"Order filled at {avg_price} (Expected: {expected_price}). Slippage: {slippage:.4f}%")
                return avg_price, slippage
            else:
                print("Order filled but no average price returned.")
                return None, None
        elif status == "canceled":
            print("Order canceled.")
            return None, None

    print("Monitoring timed out (Order likely still open).")
    return None, None

def main():
    if not os.path.exists(CLI_PATH):
        print("Error: thales-cli not found. Run cargo build.")
        return

    # 0. Manage Active Orders (Cancel Stale)
    manage_orders()

    # 0c. Update Signal History
    if os.environ.get("SIMULATION") == "true":
        run_command(["update-signal-history", "--input", "history.json", "--provider", "paper"])
    else:
        # Run for kraken provider to cover all assets
        run_command(["update-signal-history", "--input", "history.json", "--provider", "kraken"])
        run_command(["update-signal-history", "--input", "history.json", "--provider", "alpaca"])

    # 1. Identify Strategies
    strategies = get_active_strategies()
    if not strategies:
        print("No active strategies found in strategies.md. Doing nothing.")
        # Log why?
        with open(PORTFOLIO_PATH, "a") as f:
             f.write(f"\n# Execution Attempt {datetime.now()}\nNo active strategies found. Aborting.\n")
        return

    print(f"Active Strategies: {strategies}")

    # 2. Scan Markets + Get from Signals.md
    scanned_candidates = scan_markets()
    signal_candidates = get_candidates_from_signals()

    # 0b. Archive Stale Signals after they have been processed and logged as stale
    archive_signals(days=1)

    # Merge candidates (prefer signal candidates if duplicates?)
    # Priority: Signals.md candidates > Scanned candidates
    # We want to limit total analysis to top 3 candidates to follow "Pick the top 1–3 candidates" directive.

    # 1. Start with Signal candidates
    # Sort by confidence descending
    def get_confidence(c):
        raw = c.get("raw_analysis_json")
        if not raw:
            return 0.0
        return float(raw.get("confidence", 0.0) or 0.0)

    signal_candidates.sort(key=get_confidence, reverse=True)
    selected_candidates = signal_candidates[:]

    # 2. Fill remaining slots with Scanned candidates
    # Scanned candidates are already sorted (Kraken by volume) or static list (Alpaca)
    existing_symbols = set(c["symbol"] for c in selected_candidates)

    for cand in scanned_candidates:
        if cand["symbol"] not in existing_symbols:
            selected_candidates.append(cand)
            existing_symbols.add(cand["symbol"])

    # 3. Limit to top 3
    selected_candidates = selected_candidates[:3]

    print(f"Selected {len(selected_candidates)} candidates for deep analysis (from {len(scanned_candidates)} scanned + {len(signal_candidates)} signals).")
    for c in selected_candidates:
        print(f" - {c['symbol']} ({c.get('market', 'unknown')})")

    # 2b. Fetch Current Portfolio (Positions)
    print("Fetching open positions...")
    portfolio_path = get_all_positions()

    try:
        # 3. Generate Signals for selected candidates
        all_signals = []
        print("Evaluating candidates...")
        for cand in selected_candidates:
            # Generate signals from all strategies
            raw_signals = evaluate_candidate(cand, strategies, portfolio_path)

            # Resolve conflicts (per candidate)
            valid_signals = resolve_conflicts(raw_signals, cand)

            if valid_signals:
                print(f"  {cand['symbol']}: Selected {len(valid_signals)} valid signals.")
                all_signals.extend(valid_signals)
            elif raw_signals:
                print(f"  {cand['symbol']}: All {len(raw_signals)} signals rejected due to conflicts.")
            else:
                # No signals generated at all.
                # If this candidate came from Signals.md, we should log that we skipped it.
                if cand.get("raw_analysis_json"):
                     reason = f"No active strategy generated a signal (Strategies: {', '.join(strategies)})"
                     print(f"  {cand['symbol']}: {reason}")
                     # Create a dummy intent for logging
                     dummy_intent = {
                         "symbol": cand["symbol"],
                         "intent_id": cand.get("signal_ref", "NO_STRATEGY_SIGNAL"),
                         "rationale": "Signal from Signals.md not validated by any active strategy"
                     }
                     log_skipped(dummy_intent, reason)

    finally:
        # Cleanup portfolio file
        if os.path.exists(portfolio_path):
            os.remove(portfolio_path)

    # 4. Filter and Select Top 3
    if not all_signals:
        print("No signals generated. Doing nothing.")
        return

    # Sort by confidence descending
    all_signals.sort(key=lambda x: x.get("confidence", 0.0), reverse=True)

    # Take top 3
    top_signals = all_signals[:3]

    print(f"Selected top {len(top_signals)} signals for execution.")

    # 5. Execute
    for intent in top_signals:
        print("\n--- NEW SIGNAL ---")
        print(f"Symbol: {intent['symbol']} ({intent.get('market', 'unknown')})")
        print(f"Direction: {intent['side'].upper()}")
        print(f"Signal Type: {intent.get('signal_type', 'Unknown')}")
        print(f"Confidence: {intent.get('confidence', 0.0) * 100:.1f}%")
        print(f"Size Hint: {intent.get('size_hint', '0')}")
        print(f"Stop Loss: {intent.get('stop_loss', 'None')}")
        print(f"Take Profit: {intent.get('take_profit', 'None')}")
        print(f"Reasoning: {intent.get('rationale', 'None')}")
        print("------------------\n")

        # Risk Agent Check
        risk_ok, risk_reason = verify_risk(intent)
        if not risk_ok:
            print(f"Skipping {intent['symbol']}: {risk_reason}")
            log_skipped(intent, f"Rejected by Risk Agent: {risk_reason}")
            continue

        # Fetch latest price for execution logic
        current_price = get_latest_price(intent["provider"], intent["symbol"])

        # Cap buy size to available account funds before execution.
        size_ok, size_reason = adjust_buy_size_to_buying_power(intent, current_price)
        if not size_ok:
            print(f"Skipping {intent['symbol']}: {size_reason}")
            log_skipped(intent, size_reason)
            continue

        # Cap sell size to available holdings before execution.
        sell_ok, sell_reason = adjust_sell_size_to_sellable_balance(intent)
        if not sell_ok:
            print(f"Skipping {intent['symbol']}: {sell_reason}")
            log_skipped(intent, sell_reason)
            continue

        # Refine Intent (Algo Selection)
        intent = refine_intent(intent, current_price)
        print(f"Order Type: {intent['order_type'].upper()}")
        if intent.get("execution_algo"):
            print(f"Algorithm: {intent['execution_algo']}")

        # Ensure we always pass a stop loss, according to agent rules "Always set stop losses when available"
        if not intent.get("stop_loss"):
            print("Warning: Missing stop loss, adding safety default stop loss")
            # If no stop loss was provided by the strategy but we are going to trade,
            # risk management must apply. Let's apply a naive 5% safety buffer.
            # (Note: real stop loss override based on ATR is done in Rust, but we must enforce it here as well
            # if strategies bypassed it or generated direct signals)
            if current_price:
                if intent["side"] == "buy":
                    intent["stop_loss"] = current_price * 0.95
                elif intent["side"] == "sell":
                    intent["stop_loss"] = current_price * 1.05
            else:
                print("Skipping execution: Cannot determine safe stop loss without current price.")
                log_skipped(intent, "Missing stop loss and current price unavailable")
                continue

        provider = intent["provider"]
        print(f"Executing {intent['side']} {intent['symbol']} via {provider}...")

        # Write intent to file
        safe_symbol = intent['symbol'].replace("/", "_")
        temp_intent_file = f"temp_intent_{safe_symbol}.json"
        with open(temp_intent_file, "w") as f:
            json.dump(intent, f)

        # Execute
        result = run_command(["execute-intent", "--provider", provider, "--input", temp_intent_file])

        # Cleanup
        if os.path.exists(temp_intent_file):
            os.remove(temp_intent_file)

        if result:
            # Handle list response from execute-intent
            exec_res = result[0] if isinstance(result, list) else result
            outcome = classify_execution_outcome(exec_res)
            status = exec_res.get("status", "unknown")

            if outcome == "executed":
                print(f"Execution filled. Status: {status}")
                # Monitor Slippage when we have an expected limit price.
                expected_price = intent.get("limit_price")
                slippage = None
                if expected_price and exec_res.get("provider_order_id"):
                    _, slippage = monitor_execution(
                        provider, exec_res["provider_order_id"], expected_price
                    )

                log_trade(intent, exec_res, slippage)
                update_history(intent)
            elif outcome == "submitted":
                print(f"Order submitted (not filled yet). Status: {status}")
                log_submitted(intent, exec_res)
            elif outcome == "rejected":
                reason = f"Order rejected with status '{status}'"
                print(reason)
                log_skipped(intent, reason)
            else:
                reason = f"Unknown execution status '{status}'"
                print(reason)
                log_skipped(intent, reason)
        else:
            reason = run_command.last_error or "Execution failed."
            print(f"Execution failed: {reason}")
            log_skipped(intent, reason)

    # Log skipped signals (signals not selected in top 3)
    # Only if they were valid signals but we didn't select them.
    # We should log them as "Skipped" with reason "Lower priority/confidence".

    for intent in all_signals[3:]:
        log_skipped(intent, "Lower priority/confidence than top 3")

if __name__ == "__main__":
    main()
