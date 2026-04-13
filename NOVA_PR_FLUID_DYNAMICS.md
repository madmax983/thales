🌟 Nova: Market Fluid Dynamics

💡 **The Spark:** "I noticed we analyze price levels and ranges (Market Gravity, Kinematics), but we don't treat the market like a fluid. What if we measure the *pressure* of volume pushing price, and the *viscosity* resisting it?"

🚀 **The Feature:** "Implemented `FluidDynamicsReport` and `analyze_fluid_dynamics`. It calculates Flow Pressure (directional volume push), Viscosity (volume needed to move price by 1 unit), and Turbulence (fluctuation relative to net movement)."

🔮 **The Potential:** "Could be used to identify trend exhaustion (high pressure but rising viscosity) or predict breakouts (low turbulence transitioning to high pressure)."

⚠️ **Risk:** "Low. Isolated in `crates/cli/src/experimental/market_fluid_dynamics.rs` behind the `#[cfg(feature = "nova")]` flag."