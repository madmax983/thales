use anyhow::Result;
use strategies::adx_momentum::{AdxMomentum, AdxMomentumConfig};
use strategies::bollinger_bands::{BollingerBandsConfig, BollingerBandsMeanReversion};
use strategies::cci_momentum::{CciMomentum, CciMomentumConfig};
use strategies::donchian_breakout::{DonchianBreakout, DonchianBreakoutConfig};
use strategies::ema_crossover::{EmaCrossover, EmaCrossoverConfig};
use strategies::ichimoku_cloud::{IchimokuCloud, IchimokuCloudConfig};
use strategies::keltner_channel_breakout::{KeltnerChannelBreakout, KeltnerChannelBreakoutConfig};
use strategies::linear_regression_trend::{LinearRegressionTrend, LinearRegressionTrendConfig};
use strategies::macd::{Macd, MacdConfig};
use strategies::money_flow_index::{MoneyFlowIndex, MoneyFlowIndexConfig};
use strategies::obv_trend::{ObvTrendFollowing, ObvTrendFollowingConfig};
use strategies::parabolic_sar::{ParabolicSar, ParabolicSarConfig};
use strategies::rsi_mean_reversion::{RsiMeanReversion, RsiMeanReversionConfig};
use strategies::stochastic_oscillator::{StochasticOscillator, StochasticOscillatorConfig};
use strategies::strategy::Strategy;
use strategies::supertrend::{Supertrend, SupertrendConfig};

pub fn create_strategy(name: &str, symbol: &str) -> Result<Box<dyn Strategy>> {
    match name {
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
        _ => Err(anyhow::anyhow!("Unknown strategy: {}", name)),
    }
}

pub fn list_strategies() -> Vec<&'static str> {
    vec![
        "BollingerBands",
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
        "LinearRegressionTrend",
        "ObvTrendFollowing",
        "MoneyFlowIndex",
    ]
}
