import re

with open('crates/strategies/src/gator_oscillator.rs', 'r') as f:
    content = f.read()

# Update the loop to track state to only emit on transition
orig = """        for i in 1..upper_f64.len() {
            let curr_upper = upper_f64.get(i);
            let prev_upper = upper_f64.get(i - 1);

            let curr_lower = lower_f64.get(i);
            let prev_lower = lower_f64.get(i - 1);

            let curr_close = close_series.get(i);
            let curr_ts = timestamp_series.get(i);
            let curr_atr = atr_f64.get(i);"""

new = """        let mut was_expanding = false;
        let mut was_contracting = false;

        for i in 1..upper_f64.len() {
            let curr_upper = upper_f64.get(i);
            let prev_upper = upper_f64.get(i - 1);

            let curr_lower = lower_f64.get(i);
            let prev_lower = lower_f64.get(i - 1);

            let curr_close = close_series.get(i);
            let curr_ts = timestamp_series.get(i);
            let curr_atr = atr_f64.get(i);"""
content = content.replace(orig, new)

orig2 = """                // Upper is positive, so expanding means c_up > p_up.
                // Lower is negative, so expanding (abs increasing) means c_low < p_low.
                let is_expanding = c_up > p_up && c_low < p_low;

                // Contracting means abs decreasing.
                // c_up < p_up and c_low > p_low
                let is_contracting = c_up < p_up && c_low > p_low;

                let sl_dist = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO)
                    * Decimal::from_f64_retain(self.config.stop_loss_atr_mult)
                        .unwrap_or(Decimal::ZERO);

                if is_expanding {
                    // Long Entry
                    let sl_price = (Decimal::from_f64_retain(close_price).unwrap_or(Decimal::ZERO)
                        - sl_dist)
                        .to_f64()
                        .unwrap_or(0.0);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: self.config.max_position_size.to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl_price),
                        take_profit: None,
                        reason: "Gator histograms expanding".to_string(),
                        timestamp_ms: ts,
                    });

                    // Short Exit
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // exit short implies buying
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "Gator histograms expanding (Short Exit)".to_string(),
                        timestamp_ms: ts,
                    });
                } else if is_contracting {
                    // Short Entry
                    let sl_price = (Decimal::from_f64_retain(close_price).unwrap_or(Decimal::ZERO)
                        + sl_dist)
                        .to_f64()
                        .unwrap_or(0.0);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: self.config.max_position_size.to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl_price),
                        take_profit: None,
                        reason: "Gator histograms contracting".to_string(),
                        timestamp_ms: ts,
                    });

                    // Long Exit
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // exit long implies selling
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "Gator histograms contracting (Long Exit)".to_string(),
                        timestamp_ms: ts,
                    });
                }"""
new2 = """                // Upper is positive, so expanding means c_up > p_up.
                // Lower is negative, so expanding (abs increasing) means c_low < p_low.
                let is_expanding = c_up > p_up && c_low < p_low;

                // Contracting means abs decreasing.
                // c_up < p_up and c_low > p_low
                let is_contracting = c_up < p_up && c_low > p_low;

                let sl_dist = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO)
                    * Decimal::from_f64_retain(self.config.stop_loss_atr_mult)
                        .unwrap_or(Decimal::ZERO);

                if is_expanding && !was_expanding {
                    // Long Entry
                    let sl_price = (Decimal::from_f64_retain(close_price).unwrap_or(Decimal::ZERO)
                        - sl_dist)
                        .to_f64()
                        .unwrap_or(0.0);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: self.config.max_position_size.to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl_price),
                        take_profit: None,
                        reason: "Gator histograms expanding".to_string(),
                        timestamp_ms: ts,
                    });

                    // Short Exit
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // exit short implies buying
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "Gator histograms expanding (Short Exit)".to_string(),
                        timestamp_ms: ts,
                    });
                } else if is_contracting && !was_contracting {
                    // Short Entry
                    let sl_price = (Decimal::from_f64_retain(close_price).unwrap_or(Decimal::ZERO)
                        + sl_dist)
                        .to_f64()
                        .unwrap_or(0.0);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: self.config.max_position_size.to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl_price),
                        take_profit: None,
                        reason: "Gator histograms contracting".to_string(),
                        timestamp_ms: ts,
                    });

                    // Long Exit
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // exit long implies selling
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "Gator histograms contracting (Long Exit)".to_string(),
                        timestamp_ms: ts,
                    });
                }

                was_expanding = is_expanding;
                was_contracting = is_contracting;"""
content = content.replace(orig2, new2)

with open('crates/strategies/src/gator_oscillator.rs', 'w') as f:
    f.write(content)
