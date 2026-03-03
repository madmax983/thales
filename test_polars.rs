use polars::prelude::*;

fn main() {
    let df = df!(
        "a" => &[1.0f64, 2.0, 3.0]
    ).unwrap();
    let lazy = df.lazy().with_columns([
        col("a").cast(DataType::Decimal(None, Some(4)))
    ]);
    let res = lazy.collect().unwrap();
    println!("{:?}", res);
}
