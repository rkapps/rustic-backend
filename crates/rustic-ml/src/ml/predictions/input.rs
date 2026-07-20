#[derive(Clone, Debug)]
pub struct PredictionInput {
    pub id: String,
    pub values: Vec<f64>, // feature values rsi_14, rsi_divergence [ 32.0, 1.3]
}

impl PredictionInput {
    pub fn new(id: String, values: Vec<f64>) -> Self {
        Self { id, values }
    }
}
