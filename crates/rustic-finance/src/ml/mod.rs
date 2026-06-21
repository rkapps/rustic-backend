pub mod build;
pub(crate) mod labeller;
pub(crate) mod features;

pub use build::build_ticker_prediction_models;

const ROUND_PRECISION: u32 = 4;
