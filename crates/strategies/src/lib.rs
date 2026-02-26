//! Trading strategies and signal generation logic.
//!
//! This crate contains the core logic for analyzing market data and generating
//! trade signals. It defines the `Strategy` trait which all strategies must implement,
//! and provides several standard implementations.
//!
//! # Available Strategies
//!
//! - [`bollinger_bands::BollingerBandsMeanReversion`] - Mean reversion using Bollinger Bands.
//! - [`ema_crossover::EmaCrossover`] - Trend following using EMA crossovers.
//! - [`rsi_mean_reversion::RsiMeanReversion`] - Mean reversion using RSI.
//! - [`macd::Macd`] - Trend following using MACD.
//! - [`supertrend::Supertrend`] - Trend following using Supertrend.
//! - [`donchian_breakout::DonchianBreakout`] - Trend following using Donchian Channels.
//! - [`parabolic_sar::ParabolicSar`] - Trend following using Parabolic SAR.
//! - [`keltner_channel_breakout::KeltnerChannelBreakout`] - Trend following using Keltner Channels.
//! - [`adx_momentum::AdxMomentum`] - Trend following using ADX and DMI.

pub mod bollinger_bands;
pub mod ema_crossover;
pub mod indicators;
pub mod rsi_mean_reversion;
pub mod strategy;
pub mod macd;
pub mod supertrend;
pub mod donchian_breakout;
pub mod parabolic_sar;
pub mod keltner_channel_breakout;
pub mod stochastic_oscillator;
pub mod adx_momentum;
pub mod ichimoku_cloud;
