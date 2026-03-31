# 🔭 Vantage: Spec for Enforcing Signal Structure in Analysis Reports

## 👤 User Story
As a Trading Execution Agent, I want analysis reports to always include a standardized JSON trade signal structure, so that I can automatically parse and execute the recommended trades without manual intervention.

## 💡 So What?
If analysis reports lack an actionable trade signal structure, the execution pipeline breaks, resulting in rejected trades (e.g., "Analysis report found but no actionable trade signal structure" as seen in `trade_rejections.md`). This manual bottleneck completely defeats the purpose of an autonomous trading system. By programmatically enforcing a standardized JSON signal structure in all analysis outputs, we ensure seamless interoperability between the Market Analyst agent and the Execution Agent, maximizing trade capture and system reliability.

## 📈 Metric Definition
Success = 100% of generated market analysis reports that contain a trade recommendation also include a valid, programmatically parsable JSON `TradeIntent` structure, reducing execution rejections due to missing structures to 0.

## 🔍 Gap Analysis
Currently, the Market Analyst agent can generate textual analysis reports (often logged to `Signals.md`), but it does not consistently append the required machine-readable JSON structure when a trade is recommended. This discrepancy causes downstream agents to fail during parsing. Competing algorithmic trading frameworks enforce strict schema validation at the boundary between analysis and execution. We need to implement a verification step in the analyst's output pipeline to ensure the signal structure is always present when applicable.

## ✅ Acceptance Criteria
- Must validate that any analysis report recommending a trade appends a structured JSON payload.
- The appended JSON payload must strictly conform to the expected `TradeIntent` schema.
- If the JSON structure is missing, the system should either gracefully default to a "Hold" state or attempt to regenerate the payload, rather than silently passing un-actionable text to the execution agent.

## 🚫 Out of Scope
- Modifying the underlying logic or indicators used to generate the actual market analysis.
- Altering the execution logic of the downstream execution agent.
