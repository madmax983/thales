//! A centralized factory for instantiating trading strategies.
//!
//! This module acts as the bridge between the CLI engine and the core `strategies` crate.
//! It maps string identifiers to concrete strategy implementations, providing them with
//! default configuration parameters.
//!
//! # Registration
//!
//! Whenever a new strategy is added to the `strategies` crate, it must be registered here
//! in two places:
//! 1. The [`create_strategy`] match arm to handle instantiation.
//! 2. The [`list_strategies`] vector to expose it to the CLI, backtester, and benchmark suite.
//!
//! # Examples
//!
//! ```
//! use thales_cli::strategy_factory::{create_strategy, list_strategies};
//!
//! // Check available strategies
//! let strategies = list_strategies();
//! assert!(strategies.contains(&"EmaCrossover"));
//!
//! // Instantiate a specific strategy
//! let strategy = create_strategy("EmaCrossover", "AAPL").unwrap();
//! assert_eq!(strategy.name(), "EmaCrossover");
//! ```

use anyhow::Result;
use strategies::adl_momentum::{AdlMomentum, AdlMomentumConfig};
use strategies::adx_macd_trend::{AdxMacdTrend, AdxMacdTrendConfig};
use strategies::adx_momentum::{AdxMomentum, AdxMomentumConfig};
use strategies::alma_crossover::{AlmaCrossover, AlmaCrossoverConfig};
use strategies::aroon_oscillator::{AroonOscillator, AroonOscillatorConfig};
use strategies::awesome_oscillator::{AwesomeOscillator, AwesomeOscillatorConfig};
use strategies::bollinger_bands::{BollingerBandsConfig, BollingerBandsMeanReversion};
use strategies::bop_momentum::{BopMomentum, BopMomentumConfig};
use strategies::cci_momentum::{CciMomentum, CciMomentumConfig};
use strategies::chaikin_money_flow::{ChaikinMoneyFlow, ChaikinMoneyFlowConfig};
use strategies::chaikin_oscillator_momentum::{
    ChaikinOscillatorMomentum, ChaikinOscillatorMomentumConfig,
};
use strategies::chandelier_exit::{ChandelierExit, ChandelierExitConfig};
use strategies::choppiness_index_trend::{ChoppinessIndexTrend, ChoppinessIndexTrendConfig};
use strategies::cmo_mean_reversion::{CmoMeanReversion, CmoMeanReversionConfig};
use strategies::connors_rsi_mean_reversion::{
    ConnorsRsiMeanReversion, ConnorsRsiMeanReversionConfig,
};
use strategies::coppock_curve::{CoppockCurve, CoppockCurveConfig};
use strategies::dema_crossover::{DemaCrossover, DemaCrossoverConfig};
use strategies::disparity_index_reversion::{
    DisparityIndexReversion, DisparityIndexReversionConfig,
};
use strategies::donchian_breakout::{DonchianBreakout, DonchianBreakoutConfig};
use strategies::double_ema_crossover::{DoubleEmaCrossover, DoubleEmaCrossoverConfig};
use strategies::dpo_breakout::{DpoBreakout, DpoBreakoutConfig};
use strategies::ease_of_movement::{EaseOfMovement, EaseOfMovementConfig};
use strategies::elder_ray::{ElderRay, ElderRayConfig};
use strategies::ema_crossover::{EmaCrossover, EmaCrossoverConfig};
use strategies::ema_rsi_trend::{EmaRsiTrendFollowing, EmaRsiTrendFollowingConfig};
use strategies::fisher_transform_reversal::{
    FisherTransformReversal, FisherTransformReversalConfig,
};
use strategies::force_index_trend::{ForceIndexTrend, ForceIndexTrendConfig};
use strategies::hma_crossover::{HmaCrossover, HmaCrossoverConfig};
use strategies::hma_macd_trend::{HmaMacdTrend, HmaMacdTrendConfig};
use strategies::ichimoku_cloud::{IchimokuCloud, IchimokuCloudConfig};
use strategies::kama_crossover::{KamaCrossover, KamaCrossoverConfig};
use strategies::kdj_indicator::{KdjIndicatorStrategy, KdjIndicatorStrategyConfig};
use strategies::kdj_strategy::{KdjStrategy, KdjStrategyConfig};
use strategies::keltner_channel_breakout::{KeltnerChannelBreakout, KeltnerChannelBreakoutConfig};
use strategies::kst_trend::{KstTrend, KstTrendConfig};
use strategies::linear_regression_trend::{LinearRegressionTrend, LinearRegressionTrendConfig};
use strategies::macd::{Macd, MacdConfig};
use strategies::macd_rsi::{MacdRsiTrend, MacdRsiTrendConfig};
use strategies::money_flow_index::{MoneyFlowIndex, MoneyFlowIndexConfig};
use strategies::nvi_trend::{NviTrend, NviTrendConfig};
use strategies::obv_trend::{ObvTrendFollowing, ObvTrendFollowingConfig};
use strategies::parabolic_sar::{ParabolicSar, ParabolicSarConfig};
use strategies::ppo_rsi_trend::{PpoRsiTrend, PpoRsiTrendConfig};
use strategies::relative_vigor_index_trend::{
    RelativeVigorIndexTrend, RelativeVigorIndexTrendConfig,
};
use strategies::roc_momentum::{RocMomentum, RocMomentumConfig};
use strategies::rsi_mean_reversion::{RsiMeanReversion, RsiMeanReversionConfig};
use strategies::schaff_trend_cycle::{SchaffTrendCycle, SchaffTrendCycleConfig};
use strategies::sma_crossover::{SmaCrossover, SmaCrossoverConfig};
use strategies::sma_rsi_trend::{SmaRsiTrend, SmaRsiTrendConfig};
use strategies::stoch_rsi_mean_reversion::{StochRsiMeanReversion, StochRsiMeanReversionConfig};
use strategies::stochastic_oscillator::{StochasticOscillator, StochasticOscillatorConfig};
use strategies::strategy::Strategy;
use strategies::supertrend::{Supertrend, SupertrendConfig};
use strategies::supertrend_ema_crossover::{SupertrendEmaCrossover, SupertrendEmaCrossoverConfig};
use strategies::supertrend_rsi::{SupertrendRsi, SupertrendRsiConfig};
use strategies::tema_crossover::{TemaCrossover, TemaCrossoverConfig};
use strategies::triple_ema_crossover::{TripleEmaCrossover, TripleEmaCrossoverConfig};
use strategies::triple_sma_crossover::{TripleSmaCrossover, TripleSmaCrossoverConfig};
use strategies::trix_crossover::{TrixCrossover, TrixCrossoverConfig};
use strategies::trix_momentum::{TrixMomentum, TrixMomentumConfig};
use strategies::tsi_trend::{TsiTrend, TsiTrendConfig};
use strategies::ttm_squeeze::{TtmSqueeze, TtmSqueezeConfig};
use strategies::ultimate_oscillator::{UltimateOscillator, UltimateOscillatorConfig};
use strategies::vhf_trend::{VhfTrendFollowing, VhfTrendFollowingConfig};
use strategies::volume_oscillator_trend::{VolumeOscillatorTrend, VolumeOscillatorTrendConfig};
use strategies::volume_surge_reversal::{VolumeSurgeReversal, VolumeSurgeReversalConfig};
use strategies::vortex_breakout::{VortexBreakout, VortexBreakoutConfig};
use strategies::vpt_trend::{VptTrend, VptTrendConfig};
use strategies::vw_macd::{VwMacd, VwMacdConfig};
use strategies::vwap_reversion::{VwapReversion, VwapReversionConfig};
use strategies::vwap_rsi_trend::{VwapRsiTrend, VwapRsiTrendConfig};
use strategies::vwma_crossover::{VwmaCrossover, VwmaCrossoverConfig};
use strategies::williams_r::{WilliamsR, WilliamsRConfig};
use strategies::wma_crossover::{WmaCrossover, WmaCrossoverConfig};
use strategies::zlema_crossover::{ZlemaCrossover, ZlemaCrossoverConfig};
use strategies::zscore_mean_reversion::{ZScoreMeanReversion, ZScoreMeanReversionConfig};

/// Instantiates a trading strategy by name.
///
/// This function looks up the provided strategy name and returns an initialized,
/// heap-allocated [`Strategy`] trait object. Default parameters are used for all
/// configurations (e.g., standard lengths like 14 for RSI, 20 for Bollinger Bands).
///
/// # Arguments
///
/// * `name` - The exact PascalCase name of the strategy (e.g., `"Macd"`).
/// * `symbol` - The trading pair or asset symbol (e.g., `"BTCUSD"`). This is injected
///   into the strategy config to ensure generated signals correctly identify the asset.
///
/// # Errors
///
/// Returns an error if the specified `name` does not match any registered strategy.
pub fn create_strategy(name: &str, symbol: &str) -> Result<Box<dyn Strategy>> {
    match name {
        "KdjIndicatorStrategy" => {
            let config = KdjIndicatorStrategyConfig {
                k_period: 9,
                k_smoothing: 3,
                d_period: 3,
                oversold_threshold: 20.0,
                overbought_threshold: 80.0,
                max_position_size: 100.0,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(KdjIndicatorStrategy::new(config)))
        }
        "KamaCrossover" => {
            let config = KamaCrossoverConfig {
                short_period: 10,
                long_period: 30,
                short_fast_ema_period: 2,
                short_slow_ema_period: 30,
                long_fast_ema_period: 2,
                long_slow_ema_period: 30,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(KamaCrossover::new(config)))
        }
        "ChoppinessIndexTrend" => {
            let config = ChoppinessIndexTrendConfig {
                chop_period: 14,
                chop_threshold: 61.8,
                sma_period: 20,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(ChoppinessIndexTrend::new(config)))
        }
        "AroonOscillator" => {
            let config = AroonOscillatorConfig {
                symbol: symbol.to_string(),
                ..AroonOscillatorConfig::default()
            };
            Ok(Box::new(AroonOscillator::new(config)))
        }
        "TrixMomentum" => {
            let config = TrixMomentumConfig {
                trix_period: 15,
                signal_period: 9,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(TrixMomentum::new(config)))
        }
        "TrixCrossover" => {
            let config = TrixCrossoverConfig {
                trix_period: 15,
                trix_signal_period: 9,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(TrixCrossover::new(config)))
        }
        "AwesomeOscillator" => {
            let config = AwesomeOscillatorConfig {
                fast_period: 5,
                slow_period: 34,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(AwesomeOscillator::new(config)))
        }
        "BollingerBands" | "BollingerBandsMeanReversion" => {
            let config = BollingerBandsConfig {
                window_size: 20,
                num_std_dev: 2.0,
                stop_loss_pct: 0.05,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(BollingerBandsMeanReversion::new(config)))
        }
        "FisherTransformReversal" => {
            let config = FisherTransformReversalConfig {
                period: 9,
                overbought_threshold: 1.5,
                oversold_threshold: -1.5,
                atr_period: 14,
                atr_multiplier: 2.0,
                max_position_size: 1.0,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(FisherTransformReversal::new(config)?))
        }
        "EmaCrossover" => {
            let config = EmaCrossoverConfig {
                short_window: 9,
                long_window: 21,
                stop_loss_pct: 0.05,
                atr_period: 14,
                atr_mult: 2.0,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(EmaCrossover::new(config)))
        }
        "KamaRsiTrend" => {
            use strategies::kama_rsi_trend::{KamaRsiTrend, KamaRsiTrendConfig};
            let config = KamaRsiTrendConfig {
                symbol: symbol.to_string(),
                ..Default::default()
            };
            Ok(Box::new(KamaRsiTrend::new(config)))
        }
        "EmaRsiTrendFollowing" => {
            let config = EmaRsiTrendFollowingConfig {
                short_ema_period: 9,
                long_ema_period: 21,
                rsi_period: 14,
                rsi_buy_threshold: 50.0,
                rsi_sell_threshold: 50.0,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(EmaRsiTrendFollowing::new(config)))
        }
        "SmaRsiTrend" => {
            let config = SmaRsiTrendConfig {
                sma_period: 50,
                rsi_period: 14,
                rsi_oversold: 30.0,
                rsi_overbought: 70.0,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(SmaRsiTrend::new(config)))
        }
        "SmaCrossover" => {
            let config = SmaCrossoverConfig {
                short_period: 9,
                long_period: 21,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(SmaCrossover::new(config)))
        }
        "ElderRay" => {
            let config = ElderRayConfig {
                ema_period: 13,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(ElderRay::new(config)))
        }
        "BopMomentum" => {
            let config = BopMomentumConfig {
                bop_sma_period: 14,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(BopMomentum::new(config)))
        }
        "RsiMeanReversion" => {
            let config = RsiMeanReversionConfig {
                period: 14,
                oversold_threshold: 30.0,
                overbought_threshold: 70.0,
                stop_loss_pct: 0.05,
                atr_period: 14,
                atr_mult: 2.0,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(RsiMeanReversion::new(config)))
        }
        "Macd" => {
            let config = MacdConfig {
                fast_period: 12,
                slow_period: 26,
                signal_period: 9,
                stop_loss_pct: 0.05,
                atr_period: 14,
                atr_mult: 2.0,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(Macd::new(config)))
        }
        "Supertrend" => {
            let config = SupertrendConfig {
                period: 10,
                factor: 3.0,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(Supertrend::new(config)))
        }
        "SupertrendEmaCrossover" => {
            let config = SupertrendEmaCrossoverConfig {
                supertrend_period: 10,
                supertrend_multiplier: 3.0,
                ema_short_period: 10,
                ema_long_period: 20,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(SupertrendEmaCrossover::new(config)))
        }
        "SupertrendRsi" => {
            let config = SupertrendRsiConfig {
                supertrend_period: 10,
                supertrend_multiplier: 3.0,
                rsi_period: 14,
                rsi_oversold: 30.0,
                rsi_overbought: 70.0,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(SupertrendRsi::new(config)))
        }
        "DonchianBreakout" => {
            let config = DonchianBreakoutConfig {
                entry_period: 20,
                exit_period: 10,
                stop_loss_atr_mult: 2.0,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(DonchianBreakout::new(config)))
        }
        "EaseOfMovement" => {
            let config = EaseOfMovementConfig {
                eom_period: 14,
                sma_period: 9,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(EaseOfMovement::new(config)?))
        }
        "ParabolicSar" => {
            let config = ParabolicSarConfig {
                start: 0.02,
                increment: 0.02,
                max: 0.2,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(ParabolicSar::new(config)))
        }
        "KeltnerChannelBreakout" => {
            let config = KeltnerChannelBreakoutConfig {
                ema_period: 20,
                atr_period: 10,
                atr_multiplier: 2.0,
                stop_loss_atr_mult: 2.0,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(KeltnerChannelBreakout::new(config)))
        }
        "StochasticOscillator" => {
            let config = StochasticOscillatorConfig {
                k_period: 14,
                k_smoothing: 3,
                d_period: 3,
                oversold_threshold: 20.0,
                overbought_threshold: 80.0,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(StochasticOscillator::new(config)))
        }
        "AlmaCrossover" => {
            let config = AlmaCrossoverConfig {
                fast_period: 9,
                slow_period: 21,
                offset: 0.85,
                sigma: 6.0,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(AlmaCrossover::new(config)))
        }
        "AdxMacdTrend" => {
            let config = AdxMacdTrendConfig {
                adx_period: 14,
                adx_threshold: 25.0,
                macd_fast_period: 12,
                macd_slow_period: 26,
                macd_signal_period: 9,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(AdxMacdTrend::new(config)))
        }
        "AdxMomentum" => {
            let config = AdxMomentumConfig {
                adx_period: 14,
                adx_threshold: 25.0,
                di_period: 14,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(AdxMomentum::new(config)))
        }
        "IchimokuCloud" => {
            let config = IchimokuCloudConfig {
                tenkan_period: 9,
                kijun_period: 26,
                senkou_span_b_period: 52,
                senkou_span_offset: 26,
                chikou_span_offset: 26,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(IchimokuCloud::new(config)))
        }
        "CciMomentum" => {
            let config = CciMomentumConfig {
                period: 20,
                buy_threshold: 100.0,
                sell_threshold: 0.0,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(CciMomentum::new(config)))
        }
        "ChaikinMoneyFlow" => {
            let config = ChaikinMoneyFlowConfig {
                period: 21,
                buy_threshold: 0.0,
                sell_threshold: 0.0,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(ChaikinMoneyFlow::new(config)))
        }
        "ChaikinOscillatorMomentum" => {
            let config = ChaikinOscillatorMomentumConfig {
                fast_period: 3,
                slow_period: 10,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(ChaikinOscillatorMomentum::new(config)))
        }
        "ChandelierExit" => {
            let config = ChandelierExitConfig {
                period: 22,
                atr_period: 22,
                multiplier: 3.0,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(ChandelierExit::new(config)))
        }
        "CmoMeanReversion" => {
            let config = CmoMeanReversionConfig {
                cmo_period: 9,
                oversold_threshold: -50.0,
                overbought_threshold: 50.0,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(CmoMeanReversion::new(config)))
        }
        "LinearRegressionTrend" => {
            let config = LinearRegressionTrendConfig {
                period: 20,
                slope_threshold: 0.0005, // 0.05% per bar approx
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(LinearRegressionTrend::new(config)))
        }
        "ForceIndexTrend" => {
            let config = ForceIndexTrendConfig {
                fi_period: 13,
                fi_ema_period: 13,
                price_ema_period: 22,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(ForceIndexTrend::new(config)))
        }
        "NviTrend" => {
            let config = NviTrendConfig {
                nvi_sma_period: 255,
                atr_period: 14,
                stop_loss_atr_mult: 2.0,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(NviTrend::new(config)))
        }
        "ObvTrendFollowing" => {
            let config = ObvTrendFollowingConfig {
                obv_sma_period: 20,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(ObvTrendFollowing::new(config)))
        }
        "MoneyFlowIndex" => {
            let config = MoneyFlowIndexConfig {
                period: 14,
                oversold_threshold: 20.0,
                overbought_threshold: 80.0,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(MoneyFlowIndex::new(config)))
        }
        "ConnorsRsiMeanReversion" => {
            let config = ConnorsRsiMeanReversionConfig {
                rsi_period: 3,
                streak_rsi_period: 2,
                rank_lookback: 100,
                oversold_threshold: 10.0,
                overbought_threshold: 90.0,
                stop_loss_pct: 0.05,
                exit_sma_period: Some(5), // Exit when Price > SMA(5)
                symbol: symbol.to_string(),
            };
            Ok(Box::new(ConnorsRsiMeanReversion::new(config)))
        }
        "WilliamsR" => {
            let config = WilliamsRConfig {
                period: 14,
                oversold_threshold: -80.0,
                overbought_threshold: -20.0,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(WilliamsR::new(config)))
        }
        "VwmaCrossover" => {
            let config = VwmaCrossoverConfig {
                vwma_period: 20,
                sma_period: 20,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(VwmaCrossover::new(config)))
        }
        "WmaCrossover" => {
            let config = WmaCrossoverConfig {
                short_window: 9,
                long_window: 21,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(WmaCrossover::new(config)))
        }
        "HmaCrossover" => {
            let config = HmaCrossoverConfig {
                short_period: 9,
                long_period: 21,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(HmaCrossover::new(config)))
        }
        "HmaMacdTrend" => {
            let config = HmaMacdTrendConfig {
                symbol: symbol.to_string(),
                ..Default::default()
            };
            Ok(Box::new(HmaMacdTrend::new(config)))
        }
        "VwapReversion" => {
            let config = VwapReversionConfig {
                vwma_period: 20,
                oversold_threshold_pct: 0.05,
                overbought_threshold_pct: 0.05,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(VwapReversion::new(config)))
        }
        "VwapRsiTrend" => {
            let config = VwapRsiTrendConfig {
                symbol: symbol.to_string(),
                ..VwapRsiTrendConfig::default()
            };
            Ok(Box::new(VwapRsiTrend::new(config)))
        }
        "VortexBreakout" => {
            let config = VortexBreakoutConfig {
                period: 14,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(VortexBreakout::new(config)))
        }
        "TripleSmaCrossover" => {
            let config = TripleSmaCrossoverConfig {
                short_period: 9,
                medium_period: 21,
                long_period: 50,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(TripleSmaCrossover::new(config)))
        }
        "TripleEmaCrossover" => {
            let config = TripleEmaCrossoverConfig {
                short_period: 9,
                medium_period: 21,
                long_period: 50,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(TripleEmaCrossover::new(config)?))
        }
        "UltimateOscillator" => {
            let config = UltimateOscillatorConfig {
                period1: 7,
                period2: 14,
                period3: 28,
                oversold_threshold: 30.0,
                overbought_threshold: 70.0,
                stop_loss_atr_mult: 2.0,
                max_position_size: 100.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(UltimateOscillator::new(config)?))
        }
        "RelativeVigorIndexTrend" => {
            let config = RelativeVigorIndexTrendConfig {
                period: 10,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(RelativeVigorIndexTrend::new(config)))
        }
        "VolumeOscillatorTrend" => {
            let config = VolumeOscillatorTrendConfig {
                short_period: 14,
                long_period: 28,
                price_sma_period: 20,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(VolumeOscillatorTrend::new(config)))
        }
        "VolumeSurgeReversal" => {
            let config = VolumeSurgeReversalConfig {
                rsi_period: 14,
                vol_short_period: 14,
                vol_long_period: 28,
                rsi_oversold: 30.0,
                rsi_overbought: 70.0,
                vol_threshold: 20.0,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(VolumeSurgeReversal::new(config)))
        }
        "VptTrendFollowing" => {
            let config = VptTrendConfig {
                vpt_sma_period: 20,
                price_sma_period: 20,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(VptTrend::new(config)))
        }
        "ZlemaCrossover" => {
            let config = ZlemaCrossoverConfig {
                symbol: symbol.to_string(),
                ..ZlemaCrossoverConfig::default()
            };
            Ok(Box::new(ZlemaCrossover::new(config)))
        }
        "ZScoreMeanReversion" => {
            let config = ZScoreMeanReversionConfig {
                period: 20,
                entry_threshold: 2.0,
                exit_threshold: 0.0,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(ZScoreMeanReversion::new(config)))
        }
        "AdlMomentum" => {
            let config = AdlMomentumConfig {
                adl_sma_period: 14,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(AdlMomentum::new(config)))
        }
        "BollingerRsiMeanReversion" => {
            use strategies::bollinger_rsi::{BollingerRsiConfig, BollingerRsiMeanReversion};
            let config = BollingerRsiConfig {
                bb_period: 20,
                bb_std_dev: 2.0,
                rsi_period: 14,
                rsi_oversold: 30.0,
                rsi_overbought: 70.0,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(BollingerRsiMeanReversion::new(config)))
        }

        "DemaCrossover" => {
            let config = DemaCrossoverConfig {
                short_period: 9,
                long_period: 21,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(DemaCrossover::new(config)))
        }
        "TemaCrossover" => {
            let config = TemaCrossoverConfig {
                symbol: symbol.to_string(),
                ..Default::default()
            };
            Ok(Box::new(TemaCrossover::new(config)))
        }
        "StochRsiMeanReversion" => {
            let config = StochRsiMeanReversionConfig {
                symbol: symbol.to_string(),
                ..Default::default()
            };
            Ok(Box::new(StochRsiMeanReversion::new(config)))
        }
        "RocMomentum" => {
            let config = RocMomentumConfig {
                period: 14,
                buy_threshold: 0.0,
                sell_threshold: 0.0,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(RocMomentum::new(config)))
        }
        "MacdRsiTrend" => {
            let config = MacdRsiTrendConfig {
                macd_fast_period: 12,
                macd_slow_period: 26,
                macd_signal_period: 9,
                rsi_period: 14,
                rsi_buy_threshold: 50.0,
                rsi_sell_threshold: 70.0,
                atr_period: 14,
                stop_loss_atr_mult: 2.0,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(MacdRsiTrend::new(config)))
        }
        "PpoRsiTrend" => {
            let config = PpoRsiTrendConfig {
                ppo_fast_period: 12,
                ppo_slow_period: 26,
                ppo_signal_period: 9,
                rsi_period: 14,
                rsi_buy_threshold: 50.0,
                rsi_sell_threshold: 50.0,
                atr_period: 14,
                stop_loss_atr_mult: 2.0,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(PpoRsiTrend::new(config)))
        }
        "KdjStrategy" => {
            let config = KdjStrategyConfig {
                symbol: symbol.to_string(),
                ..Default::default()
            };
            Ok(Box::new(KdjStrategy::new(config)))
        }
        "KstTrend" => {
            let config = KstTrendConfig {
                symbol: symbol.to_string(),
                ..Default::default()
            };
            Ok(Box::new(KstTrend::new(config)))
        }
        "TsiTrend" => {
            let config = TsiTrendConfig {
                long_period: 25,
                short_period: 13,
                signal_period: 7,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(TsiTrend::new(config)))
        }
        "VhfTrendFollowing" => {
            let config = VhfTrendFollowingConfig {
                vhf_period: 28,
                trend_threshold: 0.4,
                sma_period: 20,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                max_position_size: 100.0,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(VhfTrendFollowing::new(config)))
        }
        "SchaffTrendCycle" => {
            let config = SchaffTrendCycleConfig {
                fast_period: 23,
                slow_period: 50,
                cycle_period: 10,
                d_period: 3,
                oversold_threshold: 25.0,
                overbought_threshold: 75.0,
                atr_period: 14,
                stop_loss_atr_mult: 2.0,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(SchaffTrendCycle::new(config)))
        }
        "DisparityIndexReversion" => {
            let config = DisparityIndexReversionConfig {
                symbol: symbol.to_string(),
                ..Default::default()
            };
            Ok(Box::new(DisparityIndexReversion::new(config)))
        }
        "DpoBreakout" => {
            let config = DpoBreakoutConfig {
                symbol: symbol.to_string(),
                ..Default::default()
            };
            Ok(Box::new(DpoBreakout::new(config)))
        }
        "DoubleEmaCrossover" => {
            let config = DoubleEmaCrossoverConfig {
                symbol: symbol.to_string(),
                ..Default::default()
            };
            Ok(Box::new(DoubleEmaCrossover::new(config)))
        }
        "CoppockCurve" => {
            let config = CoppockCurveConfig {
                symbol: symbol.to_string(),
                ..Default::default()
            };
            Ok(Box::new(CoppockCurve::new(config)?))
        }
        "TtmSqueeze" => {
            let config = TtmSqueezeConfig {
                symbol: symbol.to_string(),
                ..Default::default()
            };
            Ok(Box::new(TtmSqueeze::new(config)?))
        }
        "VwMacd" => {
            let config = VwMacdConfig {
                fast_period: 12,
                slow_period: 26,
                signal_period: 9,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                max_position_size: 100.0,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(VwMacd::new(config)?))
        }
        "UlcerIndexMeanReversion" => {
            use strategies::ulcer_index_mean_reversion::{
                UlcerIndexMeanReversion, UlcerIndexMeanReversionConfig,
            };
            let config = UlcerIndexMeanReversionConfig {
                period: 14,
                entry_threshold: 10.0,
                exit_threshold: 2.0,
                stop_loss_pct: 0.05,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(UlcerIndexMeanReversion::new(config)))
        }
                "VwapCciTrend" => {
            use strategies::vwap_cci_trend::{VwapCciTrend, VwapCciTrendConfig};
            let config = VwapCciTrendConfig {
                symbol: symbol.to_string(),
                ..VwapCciTrendConfig::default()
            };
            Ok(Box::new(VwapCciTrend::new(config)))
        }
        _ => Err(anyhow::anyhow!("Unknown strategy: {}", name)),
    }
}

/// Returns a list of all registered strategy names.
///
/// These names correspond exactly to the string literals expected by [`create_strategy`].
/// This list powers the CLI auto-completion, benchmark suites, and agent scanning.
pub fn list_strategies() -> Vec<&'static str> {
    vec![
        "AdlMomentum",
        "BopMomentum",
        "KdjIndicatorStrategy",
        "KamaCrossover",
        "KamaRsiTrend",
        "AlmaCrossover",
        "AdxMacdTrend",
        "AroonOscillator",
        "BollingerBands",
        "ElderRay",
        "EmaCrossover",
        "FisherTransformReversal",
        "RsiMeanReversion",
        "Macd",
        "Supertrend",
        "SupertrendEmaCrossover",
        "SupertrendRsi",
        "DonchianBreakout",
        "EaseOfMovement",
        "ParabolicSar",
        "KeltnerChannelBreakout",
        "StochasticOscillator",
        "AdxMomentum",
        "IchimokuCloud",
        "CciMomentum",
        "ChaikinMoneyFlow",
        "ChaikinOscillatorMomentum",
        "ChandelierExit",
        "ChoppinessIndexTrend",
        "CmoMeanReversion",
        "LinearRegressionTrend",
        "ForceIndexTrend",
        "NviTrend",
        "ObvTrendFollowing",
        "MoneyFlowIndex",
        "ConnorsRsiMeanReversion",
        "AwesomeOscillator",
        "WilliamsR",
        "VwmaCrossover",
        "VwapReversion",
        "VortexBreakout",
        "TripleSmaCrossover",
        "TripleEmaCrossover",
        "UltimateOscillator",
        "RelativeVigorIndexTrend",
        "VolumeOscillatorTrend",
        "VolumeSurgeReversal",
        "VptTrendFollowing",
        "ZlemaCrossover",
        "ZScoreMeanReversion",
        "BollingerRsiMeanReversion",
        "StochRsiMeanReversion",
        "RocMomentum",
        "MacdRsiTrend",
        "TrixCrossover",
        "TsiTrend",
        "DemaCrossover",
        "TemaCrossover",
        "WmaCrossover",
        "HmaCrossover",
        "HmaMacdTrend",
        "SmaCrossover",
        "SmaRsiTrend",
        "EmaRsiTrendFollowing",
        "PpoRsiTrend",
        "KdjStrategy",
        "KstTrend",
        "VhfTrendFollowing",
        "SchaffTrendCycle",
        "DisparityIndexReversion",
        "DpoBreakout",
        "DoubleEmaCrossover",
        "CoppockCurve",
        "TtmSqueeze",
        "VwMacd",
        "UlcerIndexMeanReversion",
    ]
}
