🌟 Nova: [Price Density]

💡 The Spark: "I noticed we have volume profile which is useful, but we lack a way to analyze purely time-based price density. We have all the price data, but we're not using it to see where price 'spends' the most time."

🚀 The Feature: "Implemented `calculate_price_density` in `src/experimental/price_density.rs`."

🔮 The Potential: "Could be used as a primitive alternative to volume profile, purely based on price action, which is particularly useful for markets where volume data is unreliable or unavailable. This can be further integrated into our market analysis pipelines."

⚠️ Risk: "Low. Isolated in `src/experimental/price_density.rs` and behind the `nova` feature flag."