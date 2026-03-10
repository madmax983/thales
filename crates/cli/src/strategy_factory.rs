use anyhow::Result;
use strategies::adx_macd_trend::{AdxMacdTrend, AdxMacdTrendConfig};
use strategies::adx_momentum::{AdxMomentum, AdxMomentumConfig};
use strategies::aroon_oscillator::{AroonOscillator, AroonOscillatorConfig};
use strategies::awesome_oscillator::{AwesomeOscillator, AwesomeOscillatorConfig};
use strategies::bollinger_bands::{BollingerBandsConfig, BollingerBandsMeanReversion};
use strategies::cci_momentum::{CciMomentum, CciMomentumConfig};
use strategies::chaikin_money_flow::{ChaikinMoneyFlow, ChaikinMoneyFlowConfig};
use strategies::chandelier_exit::{ChandelierExit, ChandelierExitConfig};
use strategies::connors_rsi_mean_reversion::{
    ConnorsRsiMeanReversion, ConnorsRsiMeanReversionConfig,
};
use strategies::donchian_breakout::{DonchianBreakout, DonchianBreakoutConfig};
use strategies::elder_ray::{ElderRay, ElderRayConfig};
use strategies::ema_crossover::{EmaCrossover, EmaCrossoverConfig};
use strategies::ichimoku_cloud::{IchimokuCloud, IchimokuCloudConfig};
use strategies::keltner_channel_breakout::{KeltnerChannelBreakout, KeltnerChannelBreakoutConfig};
use strategies::linear_regression_trend::{LinearRegressionTrend, LinearRegressionTrendConfig};
use strategies::macd::{Macd, MacdConfig};
use strategies::macd_rsi::{MacdRsiTrend, MacdRsiTrendConfig};
use strategies::money_flow_index::{MoneyFlowIndex, MoneyFlowIndexConfig};
use strategies::obv_trend::{ObvTrendFollowing, ObvTrendFollowingConfig};
use strategies::parabolic_sar::{ParabolicSar, ParabolicSarConfig};
use strategies::roc_momentum::{RocMomentum, RocMomentumConfig};
use strategies::rsi_mean_reversion::{RsiMeanReversion, RsiMeanReversionConfig};
use strategies::stoch_rsi_mean_reversion::{StochRsiMeanReversion, StochRsiMeanReversionConfig};
use strategies::stochastic_oscillator::{StochasticOscillator, StochasticOscillatorConfig};
use strategies::strategy::Strategy;
use strategies::supertrend::{Supertrend, SupertrendConfig};
use strategies::trix_momentum::{TrixMomentum, TrixMomentumConfig};
use strategies::tema_crossover::{TemaCrossover, TemaCrossoverConfig};
use strategies::tsi_trend::{TsiTrend, TsiTrendConfig};
use strategies::vortex_breakout::{VortexBreakout, VortexBreakoutConfig};
use strategies::vwap_reversion::{VwapReversion, VwapReversionConfig};
use strategies::vwma_crossover::{VwmaCrossover, VwmaCrossoverConfig};
use strategies::williams_r::{WilliamsR, WilliamsRConfig};
use strategies::zscore_mean_reversion::{ZScoreMeanReversion, ZScoreMeanReversionConfig};
use strategies::dema_crossover::{DemaCrossover, DemaCrossoverConfig};

pub fn create_strategy(name: &str, symbol: &str) -> Result<Box<dyn Strategy>> {
    match name {
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
        "ElderRay" => {
            let config = ElderRayConfig {
                ema_period: 13,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(ElderRay::new(config)))
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
        "DonchianBreakout" => {
            let config = DonchianBreakoutConfig {
                entry_period: 20,
                exit_period: 10,
                stop_loss_atr_mult: 2.0,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(DonchianBreakout::new(config)))
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
        "ChandelierExit" => {
            let config = ChandelierExitConfig {
                period: 22,
                atr_period: 22,
                multiplier: 3.0,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(ChandelierExit::new(config)))
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
        "VortexBreakout" => {
            let config = VortexBreakoutConfig {
                period: 14,
                stop_loss_atr_mult: 2.0,
                atr_period: 14,
                symbol: symbol.to_string(),
            };
            Ok(Box::new(VortexBreakout::new(config)))
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
        "DemaCrossover" => {
            let config = DemaCrossoverConfig {
                symbol: symbol.to_string(),
                ..Default::default()
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
        _ => Err(anyhow::anyhow!("Unknown strategy: {}", name)),
    }
}

pub fn list_strategies() -> Vec<&'static str> {
    vec![
        "AdxMacdTrend",
        "AroonOscillator",
        "BollingerBands",
        "ElderRay",
        "EmaCrossover",
        "RsiMeanReversion",
        "Macd",
        "Supertrend",
        "DonchianBreakout",
        "ParabolicSar",
        "KeltnerChannelBreakout",
        "StochasticOscillator",
        "AdxMomentum",
        "IchimokuCloud",
        "CciMomentum",
        "ChaikinMoneyFlow",
        "ChandelierExit",
        "LinearRegressionTrend",
        "ObvTrendFollowing",
        "MoneyFlowIndex",
        "ConnorsRsiMeanReversion",
        "AwesomeOscillator",
        "WilliamsR",
        "VwmaCrossover",
        "VwapReversion",
        "VortexBreakout",
        "ZScoreMeanReversion",
        "StochRsiMeanReversion",
        "RocMomentum",
        "MacdRsiTrend",
        "TsiTrend",
        "TemaCrossover",
        "DemaCrossover",
    ]
}
