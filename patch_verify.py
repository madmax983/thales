with open("scripts/verify_signal_generator.py", "r") as f:
    content = f.read()

content = content.replace('"timestamp_unix_ms": now - 86400000 # Yesterday', '"timestamp_unix_ms": now - 86400000, # Yesterday')
content = content.replace('"strategy": ""', '"strategy": "BollingerBands"')

with open("scripts/verify_signal_generator.py", "w") as f:
    f.write(content)
