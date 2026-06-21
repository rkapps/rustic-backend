use crate::ml::predictions::input::PredictionInput;


pub trait PredictionModel {

    fn predict(input: &PredictionInput) -> f64;
}
