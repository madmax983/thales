//! Technical Indicators
//!
//! This module provides a collection of technical indicators used for market analysis and signal generation.
//! Each indicator is implemented as a standalone function that takes a Polars `DataFrame` and optional parameters, returning a `Series`.
//!
//! # Available Indicators
//!
//! - [`adx`] - Average Directional Index
//! - [`atr`] - Average True Range
//! - [`awesome_oscillator`] - Awesome Oscillator
//! - [`bollinger_bands`] - Bollinger Bands
//! - [`cci`] - Commodity Channel Index
//! - [`connors_rsi`] - Connors Relative Strength Index
//! - [`donchian_channels`] - Donchian Channels
//! - [`ema`] - Exponential Moving Average
//! - [`ichimoku`] - Ichimoku Cloud
//! - [`keltner_channels`] - Keltner Channels
//! - [`linear_regression`] - Linear Regression
//! - [`macd`] - Moving Average Convergence Divergence
//! - [`mfi`] - Money Flow Index
//! - [`obv`] - On-Balance Volume
//! - [`parabolic_sar`] - Parabolic Stop and Reverse
//! - [`rsi`] - Relative Strength Index
//! - [`sma`] - Simple Moving Average
//! - [`stochastic`] - Stochastic Oscillator
//! - [`supertrend`] - Supertrend
//! - [`williams_r`] - Williams %R

pub mod adx;
pub mod atr;
pub mod awesome_oscillator;
pub mod bollinger_bands;
pub mod cci;
pub mod connors_rsi;
pub mod donchian_channels;
pub mod ema;
pub mod ichimoku;
pub mod keltner_channels;
pub mod linear_regression;
pub mod macd;
pub mod mfi;
pub mod obv;
pub mod parabolic_sar;
pub mod rsi;
pub mod sma;
pub mod stochastic;
pub mod supertrend;
pub mod williams_r;
