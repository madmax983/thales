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
//! - [`cmo_mean_reversion::CmoMeanReversion`] - Mean reversion strategy using Chande Momentum Oscillator.
//! - [`sma_crossover::SmaCrossover`] - Trend following using SMA crossovers.
//! - [`coppock_curve::CoppockCurve`] - Trend following using the Coppock Curve.

pub mod adl_momentum;
pub mod adx_macd_trend;
pub mod adx_momentum;
pub mod alma_crossover;
pub mod aroon_oscillator;
pub mod atr_breakout;
pub mod awesome_oscillator;
pub mod bollinger_bands;
pub mod bollinger_rsi;
pub mod bop_momentum;
pub mod cci_momentum;
pub mod chaikin_money_flow;
pub mod chandelier_exit;
pub mod choppiness_index_trend;
pub mod cmo_mean_reversion;
pub mod connors_rsi_mean_reversion;
pub mod coppock_curve;
pub mod dema_crossover;
pub mod disparity_index_reversion;
pub mod donchian_breakout;
pub mod double_ema_crossover;
pub mod dpo_breakout;
pub mod ease_of_movement;
pub mod elder_ray;
pub mod ema_crossover;
pub mod ema_rsi_trend;
pub mod fisher_transform_reversal;
pub mod force_index_trend;
pub mod hma_crossover;
pub mod hma_macd_trend;
pub mod ichimoku_cloud;
pub mod indicators;
pub mod kama_crossover;
pub mod kama_rsi_trend;
pub mod kdj_indicator;
pub mod kdj_strategy;
pub mod keltner_channel_breakout;
pub mod linear_regression_trend;
pub mod macd;
pub mod macd_rsi;
pub mod macd_trend_follower;
pub mod money_flow_index;
pub mod nvi_trend;
pub mod obv_trend;
pub mod parabolic_sar;
pub mod ppo_rsi_trend;
pub mod pvi_trend;
pub mod roc_momentum;
pub mod rsi_mean_reversion;
pub mod schaff_trend_cycle;
pub mod sma_crossover;
pub mod sma_rsi_trend;
pub mod stoch_rsi_mean_reversion;
pub mod stochastic_oscillator;
pub mod strategy;
pub mod supertrend;
pub mod supertrend_ema_crossover;
pub mod supertrend_rsi;
pub mod tema_crossover;
pub mod triple_ema_crossover;
pub mod triple_sma_crossover;
pub mod trix_crossover;
pub mod trix_momentum;
pub mod tsi_trend;
pub mod ttm_squeeze;
pub mod ulcer_index_mean_reversion;
pub mod vhf_trend;
pub mod volume_oscillator_trend;
pub mod volume_surge_reversal;
pub mod vortex_breakout;
pub mod vpt_trend;
pub mod vw_macd;
pub mod vwap_reversion;
pub mod vwap_rsi_trend;
pub mod vwma_crossover;
pub mod williams_r;
pub mod wma_crossover;
pub mod zlema_crossover;
pub mod zscore_mean_reversion;

pub mod chaikin_oscillator_momentum;
pub mod gator_oscillator;
pub mod kst_trend;
pub mod relative_vigor_index_trend;
pub mod ultimate_oscillator;
pub mod vwap_cci_trend;
pub mod typical_price_trend;
