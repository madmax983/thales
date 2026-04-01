import re

with open("signal_generator.py", "r") as f:
    content = f.read()

# Replace format_signal size formatting from format_price to format_size
def replace_format(match):
    return """    size = format_size(intent.get('size_hint', '0'))
    sl = format_price(intent.get('stop_loss', 'None'))
    tp = format_price(intent.get('take_profit', 'None'))"""

content = re.sub(
    r"    size = format_price\(intent\.get\('size_hint', '0'\)\)\n    sl = format_price\(intent\.get\('stop_loss', 'None'\)\)\n    tp = format_price\(intent\.get\('take_profit', 'None'\)\)",
    replace_format,
    content,
    flags=re.DOTALL
)

with open("signal_generator.py", "w") as f:
    f.write(content)
