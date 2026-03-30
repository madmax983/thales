import json
with open('mock_btc_signal.json', 'r') as f:
    data = json.load(f)

print(data['data'][0]['stop_loss'])
