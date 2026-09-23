## Run 2026-09-23-intraday — 12:12 CDT scheduled scan (paper)
- Worker interrupted before the gate; coordinator completed the run on the same schedule identity.
- 12 symbols scanned, real Yahoo bars (1h + 2y deep fetch): DDJI DIA DNDX EEM ESXF IJR IWM JETS MDY NQXF QQQ RSP.
- Vol forecasts (EWMA, adjusted_close) and WSB sentiment logged per symbol; Tradytics GEX pulled for DIA/IWM/QQQ.
- 11/12 symbols: no signals at the latest timestamp. 1 signal (ES=F Bollinger buy, conf 0.56) rejected by the Jev gate.
- Result: NO QUALIFYING SETUPS. Nothing executed (paper mode).


| Date/Time | Symbol | Signal Ref | Rejection Reason |
| --- | --- | --- | --- |
| 2026-09-23T17:24:30.609407234+00:00 | ES=F | futures:ES=F:buy:1790183127000 | instrument quality 0.96; verdict confidence 0.39 is below the 0.60 floor; model chose to skip (p=0.59); regime fit 0.32; conviction 1 (Weak. Marginally better than random.) |
