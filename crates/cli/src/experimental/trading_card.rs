use anyhow::Result;
use contracts::BarSeries;
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub struct CardStats {
    pub power: i32,   // Based on Volatility
    pub agility: i32, // Based on Momentum
    pub stamina: i32, // Based on Volume
}

pub fn calculate_stats(series: &BarSeries) -> CardStats {
    if series.bars.is_empty() {
        return CardStats {
            power: 0,
            agility: 0,
            stamina: 0,
        };
    }

    let mut total_volatility = 0.0;
    let mut total_momentum = 0.0;
    let mut max_volume = 0.0;

    for bar in &series.bars {
        let volatility = if bar.low > 0.0 {
            (bar.high - bar.low) / bar.low
        } else {
            0.0
        };
        total_volatility += volatility;

        let momentum = if bar.open > 0.0 {
            (bar.close - bar.open).abs() / bar.open
        } else {
            0.0
        };
        total_momentum += momentum;

        if bar.volume > max_volume {
            max_volume = bar.volume;
        }
    }

    let n = series.bars.len() as f64;
    let avg_volatility = total_volatility / n;
    let avg_momentum = total_momentum / n;

    // Normalize to 1-99 RPG stats
    let power = (avg_volatility * 1000.0).clamp(1.0, 99.0) as i32;
    let agility = (avg_momentum * 1000.0).clamp(1.0, 99.0) as i32;

    // Simplistic volume normalization
    let stamina = ((max_volume.log10() / 10.0) * 99.0).clamp(1.0, 99.0) as i32;

    CardStats {
        power,
        agility,
        stamina,
    }
}

pub fn export_trading_card_svg(
    series: &BarSeries,
    title: &str,
    path: impl AsRef<Path>,
) -> Result<()> {
    let stats = calculate_stats(series);

    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 300 400" width="300" height="400">
  <rect width="100%" height="100%" fill="rgb(43, 43, 43)" rx="15" ry="15"/>
  <rect x="10" y="10" width="280" height="380" fill="rgb(30, 30, 30)" rx="10" ry="10" stroke="rgb(243, 156, 18)" stroke-width="4"/>
  <text x="150" y="40" font-family="Arial" font-size="24" font-weight="bold" fill="rgb(243, 156, 18)" text-anchor="middle">{}</text>

  <rect x="20" y="60" width="260" height="150" fill="rgb(58, 58, 58)" rx="5" ry="5" stroke="rgb(243, 156, 18)" stroke-width="2"/>
  <text x="150" y="140" font-family="Arial" font-size="60" fill="rgb(85, 85, 85)" text-anchor="middle">{}</text>

  <rect x="20" y="230" width="260" height="140" fill="rgb(58, 58, 58)" rx="5" ry="5"/>
  <text x="30" y="260" font-family="Arial" font-size="18" font-weight="bold" fill="rgb(255, 255, 255)">STATS</text>

  <text x="30" y="290" font-family="Arial" font-size="16" fill="rgb(204, 204, 204)">Power (Vol):</text>
  <text x="270" y="290" font-family="Arial" font-size="16" font-weight="bold" fill="rgb(231, 76, 60)" text-anchor="end">{}</text>

  <text x="30" y="320" font-family="Arial" font-size="16" fill="rgb(204, 204, 204)">Agility (Mom):</text>
  <text x="270" y="320" font-family="Arial" font-size="16" font-weight="bold" fill="rgb(52, 152, 219)" text-anchor="end">{}</text>

  <text x="30" y="350" font-family="Arial" font-size="16" fill="rgb(204, 204, 204)">Stamina (Vol):</text>
  <text x="270" y="350" font-family="Arial" font-size="16" font-weight="bold" fill="rgb(46, 204, 113)" text-anchor="end">{}</text>
</svg>"#,
        title,
        series
            .bars
            .first()
            .map(|b| b.symbol.as_str())
            .unwrap_or("???"),
        stats.power,
        stats.agility,
        stats.stamina
    );

    let mut file = File::create(path)?;
    file.write_all(svg.as_bytes())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    #[test]
    fn test_export_trading_card_svg() {
        let bars = vec![
            Bar {
                symbol: "AAPL".to_string(),
                market: "equities".to_string(),
                timeframe: "1d".to_string(),
                timestamp_unix_ms: 1600000000000,
                open: 100.0,
                high: 110.0,
                low: 90.0,
                close: 105.0,
                volume: 1000.0,
            },
            Bar {
                symbol: "AAPL".to_string(),
                market: "equities".to_string(),
                timeframe: "1d".to_string(),
                timestamp_unix_ms: 1600086400000,
                open: 105.0,
                high: 115.0,
                low: 100.0,
                close: 112.0,
                volume: 2000.0,
            },
        ];
        let series = BarSeries {
            schema_version: "v1".to_string(),
            bars,
        };

        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("card_test.svg");

        let result = export_trading_card_svg(&series, "Apple Dragon", &file_path);
        assert!(result.is_ok(), "SVG export should succeed");

        let contents = std::fs::read_to_string(&file_path).unwrap();
        assert!(contents.contains("<svg"), "Output must be valid SVG");
        assert!(
            contents.contains("Apple Dragon"),
            "Output must contain the title"
        );
    }
}
