//! Core logic for the Thales CLI.
//!
//! This crate implements the business logic required to run the Thales trading agent.
//! It orchestrates market analysis, signal generation, historical data retrieval (search history),
//! and reporting.
//!
//! # Modules
//!
//! - [`analysis`]: Market analysis using technical indicators and patterns.
//! - [`search_history`]: search history for historical trade context.
//! - [`reporting`]: Generation of markdown reports for the user.
//! - [`signals`]: Signal generation pipeline, connecting strategies to market data.

pub mod analysis;
pub mod backtest;
pub mod benchmark;
#[cfg(feature = "nova")]
pub mod black_swan;
#[cfg(feature = "nova")]
pub mod entropy;
#[cfg(feature = "nova")]
pub mod fear_and_greed;
pub mod history;
#[cfg(feature = "nova")]
pub mod markov_chain;
#[cfg(feature = "nova")]
pub mod monte_carlo;
pub mod optimizer;
#[cfg(feature = "nova")]
pub mod pairs_trading;
#[cfg(feature = "nova")]
pub mod pattern_match;
pub mod reporting;
pub mod search_history;
#[cfg(feature = "nova")]
pub mod seasonality;
pub mod signals;
pub mod strategy_factory;
#[cfg(feature = "nova")]
pub mod synthetic_data;
#[cfg(feature = "nova")]
pub mod volume_profile;
