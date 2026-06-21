use rust_decimal::prelude::ToPrimitive;
use rustic_core::utils::float_utils::round_to_precision_2;
use rustic_ml::ml::predictions::trainer::TrainingSample;
use tracing::{debug, info, trace};

use crate::{
    domain::{Ticker, TickerIndicator},
    ml::features::Features,
};

pub fn build_labels(
    ticker: &Ticker,
    indicators: &[TickerIndicator],
    period: usize,
) -> Vec<TrainingSample> {
    // drop last n indicators
    let labelable = &indicators[..indicators.len() - period];
    info!(
        target: "ml",
        "Ticker: {} Period: {} Labelable: {:?}",
        ticker.symbol,
        period,
        labelable.len()
    );
    labelable
        .iter()
        .enumerate()
        .filter_map(|(i, indicator)| {

            let prev_indicator = indicators.get(i + 1).unwrap();
            let current_price = indicator.values.get("price");
            let future_price = indicators[i + period].values.get("price");

            trace!(
                target: "ml",
                index= %i,
                date= ?indicator.date,
                price= ?current_price,
                future_price = ?future_price,
                sma_20=?&indicator.values.get("sma_20"),
                sma_100=?&indicator.values.get("sma_50"),
                sma_200=?&indicator.values.get("sma_200"),
            );


            if let Some(current_price) = current_price
                && let Some(future_price) = future_price
            {
                let features = Features::new(indicator.clone(), prev_indicator.clone());
                let return_pct = ((future_price - current_price) / current_price)
                    .to_f64()
                    .unwrap_or(0.0)
                    * 100.0
                    ;
                let return_pct = round_to_precision_2(return_pct);

                let values = features.values();
                debug!(
                    target: "ml",
                    date= %indicator.date,
                    price= ?indicator.values.get("price"),
                    future_price = ?future_price,
                    return_pct =?return_pct,
                    values= ?values
                );
                Some(TrainingSample::new(
                    ticker.symbol.clone(),
                    values,
                    return_pct,
                ))
            } else {
                None
            }
        })
        .collect()
}
