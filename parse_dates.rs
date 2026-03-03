use polars::prelude::*;
use chrono::NaiveDate;

fn main() {
    let _ = Series::new("test", &[1i64]).datetime().unwrap().as_date();
}
