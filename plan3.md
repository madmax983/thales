Ah, the %K crosses above %D, BUT in the test condition:
`k_p <= d_p && k_c > d_c && k_c < self.config.oversold_threshold`

At `i=15`: `%K = 33.3`, `%D = 11.1`. Prior `%K = 0.0`, `%D = 0.0`.
So `%K > %D` and prior was `<=`. But `%K` is `33.3`, which is NOT `< 20` (oversold_threshold is 20).
So it crosses, but *after* it leaves the oversold region!

If I want it to trigger, I should set `oversold_threshold` to `40.0` or change the price array so it crosses earlier.
Or I can just change the config for `oversold_threshold` in the test to `40.0`.
Let's change `oversold_threshold` to `40.0` and `overbought_threshold` to `60.0` in `test_stoch_rsi_signals` so it definitely triggers.
