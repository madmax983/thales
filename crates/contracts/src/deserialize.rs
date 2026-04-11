use serde::{Deserialize, Deserializer};
use serde_json::Value;

pub fn deserialize_f64_nan<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let val: Value = Deserialize::deserialize(deserializer)?;
    match val {
        Value::Number(n) => n
            .as_f64()
            .ok_or_else(|| serde::de::Error::custom("Expected float")),
        Value::String(s) if s.to_lowercase() == "nan" => Ok(f64::NAN),
        Value::Null => Ok(f64::NAN),
        _ => Err(serde::de::Error::custom("Expected float, NaN, or null")),
    }
}
