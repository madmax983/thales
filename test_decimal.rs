use polars::prelude::*;

fn main() -> anyhow::Result<()> {
    let df = df!(
        "close" => &[10.0f64, 11.0f64, 12.0f64]
    )?;

    let expr = col("close").cast(DataType::Decimal(Some(28), Some(14)));
    let df2 = df.clone().lazy().with_column(expr).collect()?;
    println!("{:?}", df2.column("close")?.dtype());
    Ok(())
}
