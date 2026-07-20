use num_traits::Float;

pub fn round_to_precision_2<T: Float>(value: T) -> T {
    round_to_precision(value, 2)
}

pub fn round_to_precision_4<T: Float>(value: T) -> T {
    round_to_precision(value, 4)
}

pub fn round_to_precision_6<T: Float>(value: T) -> T {
    round_to_precision(value, 6)
}

pub fn round_to_precision<T: Float>(value: T, precision: u32) -> T {
    // 10.0^precision calculated statically using the correct float type
    let multiplier = T::from(10.0).unwrap().powi(precision as i32);

    // Scale up, round to nearest integer, and scale back down
    (value * multiplier).round() / multiplier
}
