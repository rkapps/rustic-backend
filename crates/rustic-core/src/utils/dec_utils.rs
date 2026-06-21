use rust_decimal::{Decimal, prelude::ToPrimitive};

// decimal to f64 conversion with default
pub fn decimal_to_float(dec: Option<Decimal>, default: f64) -> f64 {
    dec.and_then(|d| d.to_f64()).unwrap_or(default)
}
