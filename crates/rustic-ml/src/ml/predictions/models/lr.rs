use crate::ml::predictions::{
    helper::{calculate_mean_and_deviation, scale_data}, input::PredictionInput, trainer::TrainingSample, traits::PredictionModel
};
use anyhow::Result;
use tracing::{info, trace};

pub struct LinearRegressionModel {}

impl LinearRegressionModel {
    pub fn train(inputs: Vec<TrainingSample>) -> Result<()> {
        // split into training and test
        let split_idx = (inputs.len() as f64 * 0.8) as usize;
        let (train_slice, test_inputs) = inputs.split_at(split_idx);
        let mut train_inputs = train_slice.to_vec();

        // calculate means and standard deviations
        let (means, sdevs) = calculate_mean_and_deviation(&train_inputs);

        info!(
            target: "ml-lr",
            train_inputs= ?train_inputs.len(),
            test_inputs= ?&test_inputs.len(),
            means= ?means,
            stds= ?sdevs
        );

        // apply scaling
        scale_data(&mut train_inputs, &means, &sdevs);

        trace!(
            target: "ml-lr",
            scaled_data= ?train_inputs
        );

        Ok(())
    }
}

impl PredictionModel for LinearRegressionModel {
    fn predict(_input: &PredictionInput) -> f64 {
        0.0
    }
}
