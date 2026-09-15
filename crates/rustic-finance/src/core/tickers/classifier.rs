pub struct TickerClassifier {}

impl TickerClassifier {
    pub fn beta_bucket(&self, beta: Option<f64>) -> Option<String> {
        let beta = beta?;
        Some(
            match beta {
                b if b < 0.8 => "Low Beta",
                b if b < 1.2 => "Market Beta",
                b if b < 2.0 => "High Beta",
                _ => "Very High Beta",
            }
            .to_string(),
        )
    }

    pub fn analyst_bucket(&self, consensus: Option<&str>) -> Option<String> {
        Some(
            match consensus? {
                "Strong Buy" => "Analyst Strong Buy",
                "Buy" => "Analyst Buy",
                "Hold" => "Analyst Hold",
                "Sell" => "Analyst Sell",
                "Strong Sell" => "Analyst Strong Sell",
                _ => return None,
            }
            .to_string(),
        )
    }
}
