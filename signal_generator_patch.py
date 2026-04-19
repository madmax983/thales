import re

with open('signal_generator.py', 'r') as f:
    content = f.read()

# Fix the duplicate logic problem directly in the script
# Currently it checks:
# if not any((ai.get("symbol") == sig[0] and ai.get("side") == sig[1] ... ) for ai in all_intents):
# but it uses rationale as part of the signature!
# The reviewer complained about the "exact, word-for-word duplicate of the signal immediately preceding the patch insertion point."
# Looking at the original output, it seems we generated a duplicate of the last signal BEFORE we appended.
# Why? Because we didn't check the EXISTING contents of Signals.md before generating!

# No wait, the reviewer said: "The newly appended SPY (short) signal is an exact, word-for-word duplicate of the signal immediately preceding the patch insertion point. This suggests the agent failed to check the existing history to filter out signals that were already active or previously generated for SPY."

# Let's fix signal_generator.py to check existing signals in Signals.md before generating!
