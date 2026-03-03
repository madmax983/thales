use chrono::{DateTime, Utc};
fn main() {
    let s = "2023-01-01T10:00:00Z";
    let dt = s.parse::<DateTime<Utc>>().ok();
    println!("{:?}", dt);
}
