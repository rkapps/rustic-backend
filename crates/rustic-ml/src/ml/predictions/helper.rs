use crate::ml::predictions::trainer::TrainingSample;

pub fn calculate_mean_and_deviation(inputs: &[TrainingSample]) -> (Vec<f64>, Vec<f64>) {
    // get samples
    let n_samples = inputs.len();

    // get number of features
    let n_features = inputs.first().unwrap().values.len();

    // default vec with 0.0
    let mut means = vec![0.0f64; n_features];

    for input in inputs {
        for (j, value) in input.values.iter().enumerate() {
            means[j] += value;
        }
    }

    //divide and assign by number of samples
    means.iter_mut().for_each(|m| *m /= n_samples as f64);

    // default vec with 0.0
    let mut stds = vec![0.0f64; n_features];
    for input in inputs {
        for (j, value) in input.values.iter().enumerate() {
            stds[j] += (value - means[j]).powi(2);
        }
    }

    //divide and assign by number of samples
    stds.iter_mut()
        .for_each(|s| *s = (*s / n_samples as f64).sqrt().max(1e-8));

    (means, stds)
}

pub fn scale_data(inputs: &mut [TrainingSample], means: &[f64], sdevs: &[f64]) {
    inputs.iter_mut().enumerate().for_each(|(_i, input)| {
        input.values.iter_mut().enumerate().for_each(|(j, value)| {
            *value = (*value - means[j]) / sdevs[j];
        });
    });
}
