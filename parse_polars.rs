use polars::prelude::*;

fn main() {
    let df = df!(
        "timestamp" => &["2023-01-01T10:00:00Z", "2023-01-01T11:00:00Z"]
    ).unwrap();
    println!("{:?}", df.column("timestamp").unwrap().cast(&DataType::Date).unwrap());
}
