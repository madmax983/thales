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
//! - [`linear_regression_trend::LinearRegressionTrend`] - Trend following using Linear Regression Slope.
//! - [`obv_trend::ObvTrendFollowing`] - Trend following using On-Balance Volume (OBV).
//! - [`money_flow_index::MoneyFlowIndex`] - Mean reversion using Money Flow Index (MFI).
//! - [`connors_rsi_mean_reversion::ConnorsRsiMeanReversion`] - Mean reversion using Connors RSI (CRSI).
//! - [`awesome_oscillator::AwesomeOscillator`] - Momentum strategy using Awesome Oscillator (AO).

pub mod adx_macd_trend;
pub mod adx_momentum;
pub mod aroon_oscillator;
pub mod awesome_oscillator;
pub mod bollinger_bands;
pub mod cci_momentum;
pub mod chaikin_money_flow;
pub mod chandelier_exit;
pub mod connors_rsi_mean_reversion;
pub mod donchian_breakout;
pub mod elder_ray;
pub mod ema_crossover;
pub mod ichimoku_cloud;
pub mod indicators;
pub mod keltner_channel_breakout;
pub mod linear_regression_trend;
pub mod macd;
pub mod macd_rsi;
pub mod money_flow_index;
pub mod obv_trend;
pub mod parabolic_sar;
pub mod roc_momentum;
pub mod rsi_mean_reversion;
pub mod sma_crossover;
pub mod stoch_rsi_mean_reversion;
pub mod stochastic_oscillator;
pub mod strategy;
pub mod supertrend;
pub mod tema_crossover;
pub mod trix_momentum;
pub mod tsi_trend;
pub mod vortex_breakout;
pub mod vwap_reversion;
pub mod vwma_crossover;
pub mod williams_r;
pub mod zscore_mean_reversion;
