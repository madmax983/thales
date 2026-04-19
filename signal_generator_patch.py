import re

with open('signal_generator.py', 'r') as f:
    code = f.read()

patch = """
            existing_rationales = set()
            try:
                for fname in ["Signals.md", "Signals_Archive.md"]:
                    if os.path.exists(fname):
                        with open(fname, "r") as f:
                            content = f.read()
                            # Extract existing rationales
                            for line in content.split('\\n'):
                                if line.startswith("- Clear reasoning (including historical context): "):
                                    # Normalize float representation for robust duplicate matching
                                    extracted_rationale = line.replace("- Clear reasoning (including historical context): ", "").strip()
                                    extracted_rationale = re.sub(r'\\d+\\.\\d{5,}', truncate_float, extracted_rationale)
                                    existing_rationales.add(extracted_rationale)
            except Exception:
                pass

            unique_intents = []
            for intent in symbol_intents:
                if intent.get('side', 'long') == primary_direction:
                    # Apply float truncation to intent rationale before comparison
                    intent_rationale = intent.get("rationale", "")
                    intent_rationale = re.sub(r'\\d+\\.\\d{5,}', truncate_float, intent_rationale).strip()

                    # Create a signature of the intent based on critical fields to filter duplicates
                    sig = (
                        intent.get("symbol"),
                        intent.get("side"),
                        intent.get("signal_type"),
                        intent_rationale
                    )
                    # Deduplicate using rationale against existing logs to avoid word-for-word duplicates
                    # as required by the "Avoid redundant or conflicting signals" persona rule.
                    if sig not in seen_params and intent_rationale not in existing_rationales:
                        seen_params.add(sig)
                        unique_intents.append(intent)
                    elif intent_rationale in existing_rationales:
                        print(f"Skipping redundant signal for {intent.get('symbol')}: Signal already exists in logs.", file=sys.stderr)
"""

old_code = """
            existing_rationales = set()
            try:
                for fname in ["Signals.md", "Signals_Archive.md"]:
                    if os.path.exists(fname):
                        with open(fname, "r") as f:
                            content = f.read()
                            # Extract existing rationales
                            for line in content.split('\\n'):
                                if line.startswith("- Clear reasoning (including historical context): "):
                                    existing_rationales.add(line.replace("- Clear reasoning (including historical context): ", "").strip())
            except Exception:
                pass

            unique_intents = []
            for intent in symbol_intents:
                if intent.get('side', 'long') == primary_direction:
                    # Create a signature of the intent based on critical fields to filter duplicates
                    sig = (
                        intent.get("symbol"),
                        intent.get("side"),
                        intent.get("signal_type"),
                        intent.get("rationale")
                    )
                    # Deduplicate using rationale against existing logs to avoid word-for-word duplicates
                    # as required by the "Avoid redundant or conflicting signals" persona rule.
                    if sig not in seen_params and intent.get("rationale", "").strip() not in existing_rationales:
                        seen_params.add(sig)
                        unique_intents.append(intent)
                    elif intent.get("rationale", "").strip() in existing_rationales:
                        print(f"Skipping redundant signal for {intent.get('symbol')}: Signal already exists in logs.", file=sys.stderr)
"""

code = code.replace(old_code.strip(), patch.strip())
with open('signal_generator.py', 'w') as f:
    f.write(code)
