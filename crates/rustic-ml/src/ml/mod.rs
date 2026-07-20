pub mod predictions;

// rustic-ml/
// ├── Cargo.toml
// └── src/
//     ├── lib.rs
//     ├── error.rs
//     ├── ml/
//     │   ├── mod.rs
//     │   ├── trainer.rs          # Trainer, TrainTestSplit, MAX_SAMPLES (300)
//     │   └── evaluator.rs        # Evaluator — MSE, MAE, R², directional accuracy
//     └── prediction/
//         ├── mod.rs
//         ├── input.rs            # TrainingSample, PredictionInput
//         ├── output.rs           # PredictionOutput, Direction
//         ├── traits.rs           # PredictionModel trait
//         ├── snapshot.rs         # FeatureSnapshot, FeatureSnapshotBuilder, keys
//         ├── ensemble.rs         # EnsemblePredictor, EnsembleBreakdown
//         └── models/
//             ├── mod.rs
//             ├── lr.rs           # LinearRegressionModel  (linfa-linear)
//             ├── gbt.rs          # GbtModel               (linfa-ensemble)
//             └── mlp.rs          # MlpModel               (candle, feature-gated)
