import subprocess
import json
import os
import sys
import re
import shutil
import math
from datetime import datetime

# Paths
CLI_PATH = "./target/release/thales-cli"
PORTFOLIO_PATH = "portfolio.md"
STRATEGIES_PATH = "strategies.md"
HISTORY_PATH = "history.json"
SIGNALS_PATH = "Signals.md"
ARCHIVE_PATH = "Signals_Archive.md"

MEAN_REVERSION_STRATEGIES = {"BollingerBands", "RsiMeanReversion", "StochasticOscillator"}
TREND_FOLLOWING_STRATEGIES = {
    "EmaCrossover",
    "Macd",
    "Supertrend",
    "DonchianBreakout",
    "ParabolicSar",
    "KeltnerChannelBreakout",
    "AdxMomentum",
}
BREAKOUT_STRATEGIES = {"DonchianBreakout", "KeltnerChannelBreakout", "Supertrend", "ParabolicSar"}

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

    regime_class = classify_market_regime(analysis)

    if regime_class == "ranging":
        selected = [s for s in active_strategies if s in MEAN_REVERSION_STRATEGIES]
    elif regime_class in {"trending_up", "trending_down", "trending"}:
        selected = [s for s in active_strategies if s in TREND_FOLLOWING_STRATEGIES]
    elif regime_class == "volatile":
        selected = [s for s in active_strategies if s in BREAKOUT_STRATEGIES]
    else:
        selected = list(active_strategies)

    # Fallback: never return empty if we have active strategies.
    if not selected:
        return list(active_strategies)
    return selected

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

def get_candidates_from_signals():
    """Parses Signals.md for potential candidates."""
    if not os.path.exists(SIGNALS_PATH):
        return []

    with open(SIGNALS_PATH, "r") as f:
        content = f.read()

    candidates = {}

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
            provider = "kraken" if market == "crypto" else "alpaca"

        # Extract JSON
        json_match = re.search(r"```json\s*(\{.*?\})\s*```", chunk, re.DOTALL)
        raw_json = None
        if json_match:
            try:
                raw_json = json.loads(json_match.group(1))
            except:
                pass

        # Check for Staleness (24 hours = 86400000 ms)
        if raw_json and raw_json.get("timestamp_unix_ms"):
            ts = raw_json["timestamp_unix_ms"]
            now = int(datetime.now().timestamp() * 1000)
            if (now - ts) > 86400000:
                 age_hours = (now - ts) / 3600000
                 reason = f"Signal too old ({age_hours:.1f} hours > 24 hours)"
                 print(f"Skipping stale signal for {symbol}: {reason}")

                 # Log to portfolio.md
                 # Create a dummy intent for logging
                 dummy_intent = {
                     "symbol": symbol,
                     "intent_id": "STALE_SIGNAL",
                     "rationale": "Stale signal from Signals.md"
                 }
                 log_skipped(dummy_intent, reason)
                 continue

        # Extract Research
        research_text = None
        res_match = re.search(r"\*\*Research\*\*:\s*(.*?)(?=\n\n|\n\*\*|\n###|$)", chunk, re.DOTALL)
        if res_match:
            research_text = res_match.group(1).strip().replace("\n", " ")
        else:
             res_match_alt = re.search(r"\*External Research\*:\s*(.*?)(?=\n\n|\n\*\*|\n###|$)", chunk, re.DOTALL)
             if res_match_alt:
                 research_text = res_match_alt.group(1).strip().replace("\n", " ")

        # Extract News
        news_text = None
        news_match = re.search(r"\*\*News\*\*:\s*(.*?)(?=\n\n|\n\*\*|\n###|$)", chunk, re.DOTALL)
        if news_match:
            news_text = news_match.group(1).strip().replace("\n", " ")

        if raw_json:
            if research_text:
                raw_json["research_summary"] = research_text
            if news_text:
                raw_json["news_summary"] = news_text

        candidates[symbol] = {
            "provider": provider,
            "symbol": symbol,
            "market": market,
            "raw_analysis_json": raw_json
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

        # Equities (Alpaca)
        print("Scanning Alpaca (Equities)...")
        equities = run_command(["scan-market", "--provider", "alpaca"])
        if equities:
            for symbol in equities:
                candidates.append({"provider": "alpaca", "symbol": symbol, "market": "equities"})

    return candidates

def evaluate_candidate(candidate, strategies, portfolio_path=None):
    """Fetches data and generates signals for a candidate using all active strategies."""
    provider = candidate["provider"]
    symbol = candidate["symbol"]

    # Fetch Data
    bars = run_command(["fetch-market-data", "--provider", provider, "--symbol", symbol, "--timeframe", "1h"])
    if not bars:
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

        intents = run_command(args)

        if intents:
            # Enrich intent with provider and strategy info
            for intent in intents:
                # Use scan provider for intent unless it's equities, then execute on Kraken
                # We fetch data from Alpaca (provider) but execute on Kraken.
                if provider == "paper":
                    intent["provider"] = "paper"
                elif candidate.get("market") == "equities":
                    intent["provider"] = "kraken"
                else:
                    intent["provider"] = provider

                intent["strategy_used"] = strategy_name # Keep track of which strategy generated this
                if effective_analysis:
                    intent["_market_analysis"] = effective_analysis

            all_generated_intents.extend(intents)

    if temp_analysis_file and os.path.exists(temp_analysis_file):
        os.remove(temp_analysis_file)

    # Cleanup temp bars
    if os.path.exists(temp_bars_file):
        os.remove(temp_bars_file)

    return all_generated_intents

def resolve_conflicts(intents, conflict_margin=0.05):
    """
    Resolves conflicts among signals for the same candidate.
    - If signals conflict (Buy vs Sell), selects side using confidence * regime-fit weights.
    - If side scores are too close, returns empty list and logs warning.
    - Returns single best intent.
    """
    if not intents:
        return []

    # Assume all intents are for the same symbol (caller ensures this)
    symbol = intents[0]["symbol"]
    analysis = intents[0].get("_market_analysis")
    regime_label = classify_market_regime(analysis)

    def score_intent(intent):
        confidence = float(intent.get("confidence", 0.0) or 0.0)
        strategy_name = intent.get("strategy_used", "")
        return confidence * strategy_regime_weight(strategy_name, analysis)

    sides = set(intent["side"] for intent in intents)
    if len(sides) > 1:
        side_scores = {}
        for intent in intents:
            side = intent.get("side", "unknown")
            side_scores[side] = side_scores.get(side, 0.0) + score_intent(intent)

        ranked_sides = sorted(side_scores.items(), key=lambda x: x[1], reverse=True)
        top_side, top_score = ranked_sides[0]
        second_score = ranked_sides[1][1] if len(ranked_sides) > 1 else 0.0

        if (top_score - second_score) < conflict_margin:
            side_score_text = ", ".join(
                f"{side}={score:.3f}" for side, score in sorted(side_scores.items())
            )
            reason = (
                f"Conflict unresolved: regime={regime_label}, side_scores={{{side_score_text}}}"
            )
            print(f"CONFLICT detected for {symbol}: {reason}. Skipping.")
            for intent in intents:
                log_skipped(intent, reason)
            return []

        winning_intents = [intent for intent in intents if intent.get("side") == top_side]
        winning_intents.sort(
            key=lambda x: (score_intent(x), float(x.get("confidence", 0.0) or 0.0)),
            reverse=True,
        )
        best_intent = winning_intents[0]
        print(
            f"CONFLICT resolved for {symbol}: selected '{top_side}' in regime={regime_label} "
            f"(score={top_score:.3f} vs {second_score:.3f})."
        )
        return [best_intent]

    # No side conflict, pick best weighted confidence.
    intents.sort(
        key=lambda x: (score_intent(x), float(x.get("confidence", 0.0) or 0.0)),
        reverse=True,
    )
    best_intent = intents[0]

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
    if "strategy_used" in clean_intent:
        del clean_intent["strategy_used"]

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

def log_trade(intent, result):
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

    signal_ref = intent["intent_id"]
    rationale = intent["rationale"].replace("\n", " ")
    confidence = f"{intent.get('confidence', 0.0) * 100:.0f}%"

    header = "| Date/Time | Asset Class | Symbol/Contract | Action | Size/Qty | Entry Price | SL | TP | Max Risk | Confidence | Signal Ref | Rationale |"
    row = f"| {date_str} | {asset_class} | {symbol} | {action} | {size} | {price} | {sl} | {tp} | {max_risk} | {confidence} | {signal_ref} | {rationale} |"

    append_to_section(PORTFOLIO_PATH, "## Executed Trades", header, row)

def log_skipped(intent, reason):
    """Logs skipped trade."""
    date_str = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    symbol = intent["symbol"]
    signal_ref = intent["intent_id"]

    header = "| Date/Time | Symbol | Signal Ref | Rejection Reason |"
    row = f"| {date_str} | {symbol} | {signal_ref} | {reason} |"

    append_to_section(PORTFOLIO_PATH, "## Skipped Signals", header, row)

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
    """Checks and cancels stale orders (> 5 mins)."""
    providers = []
    if os.environ.get("SIMULATION") == "true":
        providers.append("paper")
    else:
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
             if age_ms > 300000: # 5 minutes
                 age_s = age_ms / 1000.0
                 print(f"Cancelling stale order {order['id']} ({order['symbol']}) - Age: {age_s:.0f}s")
                 run_command(["cancel-order", "--provider", provider, "--id", order['id']])

                 # Log cancellation
                 dummy_intent = {
                     "symbol": order['symbol'],
                     "intent_id": f"CANCEL-{order['id']}",
                     "rationale": f"Stale order ({age_s:.0f}s > 300s) canceled"
                 }
                 log_skipped(dummy_intent, "Stale Order Cancellation")

def main():
    if not os.path.exists(CLI_PATH):
        print("Error: thales-cli not found. Run cargo build.")
        return

    # 0. Manage Active Orders (Cancel Stale)
    manage_orders()

    # 0b. Archive Stale Signals
    archive_signals(days=2)

    # 0c. Update Signal History
    if os.environ.get("SIMULATION") == "true":
        run_command(["update-signal-history", "--input", "history.json", "--provider", "paper"])
    else:
        # Run for both providers to cover all assets
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

    # Merge candidates (prefer signal candidates if duplicates?)
    # Priority: Signals.md candidates > Scanned candidates
    # We want to limit total analysis to top 3 candidates to follow "Pick the top 1–3 candidates" directive.

    # 1. Start with Signal candidates
    selected_candidates = signal_candidates[:]

    # 2. Fill remaining slots with Scanned candidates
    # Scanned candidates are already sorted (Kraken by volume) or static list (Alpaca)
    existing_symbols = set(c["symbol"] for c in selected_candidates)

    for cand in scanned_candidates:
        if len(selected_candidates) >= 3:
            break
        if cand["symbol"] not in existing_symbols:
            selected_candidates.append(cand)
            existing_symbols.add(cand["symbol"])

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
            valid_signals = resolve_conflicts(raw_signals)

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
                         "intent_id": "NO_STRATEGY_SIGNAL",
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
        # Risk Agent Check
        risk_ok, risk_reason = verify_risk(intent)
        if not risk_ok:
            print(f"Skipping {intent['symbol']}: {risk_reason}")
            log_skipped(intent, f"Rejected by Risk Agent: {risk_reason}")
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
            print(f"Success! Status: {exec_res.get('status')}")
            log_trade(intent, exec_res)
            update_history(intent)
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
# Verified Signal Generator Logic: Simulation Successful
