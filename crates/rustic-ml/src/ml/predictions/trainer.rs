#[derive(Clone, Debug)]
pub struct TrainingSample {
    pub id: String,
    pub values: Vec<f64>, // feature values rsi_14, rsi_divergence [ 32.0, 1.3]
    pub target: f64,      // return percentage
}

impl TrainingSample {
    pub fn new(id: String, values: Vec<f64>, target: f64) -> Self {
        Self { id, values, target }
    }
}
