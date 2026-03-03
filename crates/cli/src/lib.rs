//! Core logic for the Thales CLI.
//!
//! This crate implements the business logic required to run the Thales trading agent.
//! It orchestrates market analysis, signal generation, historical data retrieval (RAG),
//! and reporting.
//!
//! # Modules
//!
//! - [`analysis`]: Market analysis using technical indicators and patterns.
//! - [`rag`]: Retrieval-Augmented Generation for historical trade context.
//! - [`reporting`]: Generation of markdown reports for the user.
//! - [`signals`]: Signal generation pipeline, connecting strategies to market data.

pub mod analysis;
pub mod backtest;
pub mod benchmark;
pub mod history;
pub mod rag;
pub mod reporting;
pub mod signals;
pub mod strategy_factory;
