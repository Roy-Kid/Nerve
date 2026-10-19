//! Offline input-to-wire fixture generator for consumer tests.
use std::io::Read;
fn main() {
    let mut raw = String::new();
    std::io::stdin().read_to_string(&mut raw).unwrap();
    let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
    println!("{}", nerve_hub::projection::project_json(value));
}
