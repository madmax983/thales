import re

with open("signal_generator.py", "r") as f:
    content = f.read()

def repl(match):
    return """def calculate_fallback_sl_tp(last_close, side, volatility_label, missing_sl, missing_tp, intent_sl):
    sl_pct = 0.05
    if "high" in volatility_label or "extreme" in volatility_label:
        sl_pct = 0.10
    elif "low" in volatility_label:
        sl_pct = 0.02

    tp_pct = sl_pct * 2.0

    new_sl = None
    new_tp = None

    if side in ["buy", "long"]:
        if missing_sl:
            new_sl_val = last_close * (1.0 - sl_pct)
            new_sl = format_price(new_sl_val)
        else:
            try:
                new_sl_val = float(intent_sl)
            except (ValueError, TypeError):
                new_sl_val = last_close * (1.0 - sl_pct)
                new_sl = format_price(new_sl_val)

        if missing_tp:
            sl_dist = last_close - new_sl_val
            if sl_dist > 0:
                new_tp = format_price(last_close + (sl_dist * 2.0))
            else:
                new_tp = format_price(last_close * (1.0 + tp_pct))
    elif side in ["sell", "short"]:
        if missing_sl:
            new_sl_val = last_close * (1.0 + sl_pct)
            new_sl = format_price(new_sl_val)
        else:
            try:
                new_sl_val = float(intent_sl)
            except (ValueError, TypeError):
                new_sl_val = last_close * (1.0 + sl_pct)
                new_sl = format_price(new_sl_val)

        if missing_tp:
            sl_dist = new_sl_val - last_close
            if sl_dist > 0:
                new_tp = format_price(last_close - (sl_dist * 2.0))
            else:
                new_tp = format_price(last_close * (1.0 - tp_pct))

    return new_sl, new_tp"""

content = re.sub(
    r"def calculate_fallback_sl_tp\(.*?\).*?return new_sl, new_tp",
    repl,
    content,
    flags=re.DOTALL
)

with open("signal_generator.py", "w") as f:
    f.write(content)
