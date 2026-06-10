use std::{cmp, collections::HashMap, ops::Div};

use anyhow::Result;
use rust_decimal::Decimal;
use ta::{
    DataItem, Next,
    indicators::{
        AverageTrueRange, BollingerBands, ExponentialMovingAverage, FastStochastic,
        MovingAverageConvergenceDivergence, RelativeStrengthIndex, SimpleMovingAverage,
    },
};
use tracing::trace;

use crate::domain::{
    TickerHistory, TickerIndicator,
    tickers::indicator::indicator_type::{
        ATR, BB_LOWER, BB_MIDDLE, BB_UPPER, EMA, MACD, MACD_HISTOGRAM, MACD_SIGNAL, RSI, SMA,
        STOCHASTIC_D, STOCHASTIC_K, VOLUME_RATIO,
    },
};

pub struct IndicatorCalculator {}

impl IndicatorCalculator {
    #[allow(clippy::too_many_arguments)]
    // calculate technical indicators while looping through the history once.
    pub(crate) fn calculate_all_in_one_pass(
        history: &[TickerHistory],
        sma_periods: Vec<usize>,
        ema_periods: Vec<usize>,
        rsi_periods: Vec<usize>,
        stoch_k_period: usize,
        stoch_d_period: usize,
        bb_period: usize,
        bb_std_dev: f64,
        atr_period: usize,
        volume_ratio_period: usize,
    ) -> Result<Vec<TickerIndicator>> {
        if history.is_empty() {
            return Ok(Vec::new());
        }

        let mut sorted_history = history.to_vec();
        sorted_history.sort_by_key(|h| h.date);

        let mut sma_calcs: Vec<(usize, SimpleMovingAverage)> = sma_periods
            .iter()
            .map(|&p| (p, SimpleMovingAverage::new(p).unwrap()))
            .collect();

        let mut ema_calcs: Vec<(usize, ExponentialMovingAverage)> = ema_periods
            .iter()
            .map(|&p| (p, ExponentialMovingAverage::new(p).unwrap()))
            .collect();

        let mut rsi_calcs: Vec<(usize, RelativeStrengthIndex)> = rsi_periods
            .iter()
            .map(|&p| (p, RelativeStrengthIndex::new(p).unwrap()))
            .collect();

        // MACD returns multiple values, need to store each
        let mut macd = MovingAverageConvergenceDivergence::new(12, 26, 9)?;
        let mut bb = BollingerBands::new(bb_period, bb_std_dev)?;
        let mut atr = AverageTrueRange::new(atr_period)?;
        let mut stoch = FastStochastic::new(stoch_k_period)?;

        let mut indicators = Vec::new();
        let mut k_window: Vec<f64> = Vec::new();

        // ✅ SINGLE LOOP through history
        for (idx, h) in sorted_history.iter().enumerate() {
            let close_f64 = h.close.to_string().parse::<f64>()?;

            let mut values = HashMap::new();
            values.insert("price".to_string(), h.close);

            trace!("History: {:?}", h);
            // Calculate all SMAs
            trace!("Calculating SMAs");
            for (period, sma) in sma_calcs.iter_mut() {
                let value = sma.next(close_f64);
                if idx >= *period - 1 {
                    if idx == sorted_history.len() - 1 {
                        trace!(
                            "sma value for date: {} period: {} - {}",
                            h.date, period, value
                        )
                    };
                    let dec = Decimal::try_from(value)?.round_dp(2);
                    values.insert(format!("{}_{}", SMA, period), dec);
                }
            }

            trace!("Calculating EMAs");
            // Calculate all EMAs
            for (period, ema) in ema_calcs.iter_mut() {
                let value = ema.next(close_f64);
                if idx >= *period - 1 {
                    // if idx == sorted_history.len() - 1 {
                    //     debug!("ema value for date: {} period: {} - {}", h.date, period, value)
                    // }
                    let dec = Decimal::try_from(value)?.round_dp(2);
                    values.insert(format!("{}_{}", EMA, period), dec);
                }
            }

            trace!("Calculating RSIs");
            // Calculate all RSIs
            for (period, rsi) in rsi_calcs.iter_mut() {
                let value = rsi.next(close_f64);
                if idx >= *period {
                    // if idx == sorted_history.len() - 1 {
                    //     debug!("rsi value for date: {} period: {} - {}", h.date, period, value)
                    // }
                    let dec = Decimal::try_from(value)?.round_dp(2);
                    values.insert(format!("{}_{}", RSI, period), dec);
                }
            }

            trace!("Calculating Stochastic Oscillator");
            //Stochastic Oscillator
            let open_f64 = h.open.to_string().parse::<f64>()?;
            let high_f64 = h.high.to_string().parse::<f64>()?;
            let low_f64 = h.low.to_string().parse::<f64>()?;
            let close_f64 = h.close.to_string().parse::<f64>()?;
            let volume_f64 = h.volume.to_string().parse::<f64>()?;

            let bar = DataItem::builder()
                .open(open_f64)
                .high(high_f64)
                .low(low_f64)
                .close(close_f64)
                .volume(volume_f64)
                .build()?;

            let k_value = stoch.next(&bar);

            if idx >= stoch_k_period - 1 {
                k_window.push(k_value);
                values.insert(
                    format!("{}_{}", STOCHASTIC_K, stoch_k_period),
                    Decimal::try_from(k_value)?,
                );

                // Calculate %D once we have enough %K values
                if k_window.len() >= stoch_d_period {
                    // Keep window size to d_period
                    if k_window.len() > stoch_d_period {
                        k_window.remove(0);
                    }
                    let d_value: f64 = k_window.iter().sum::<f64>() / stoch_d_period as f64;
                    let dec = Decimal::try_from(d_value)?.round_dp(2);
                    values.insert(STOCHASTIC_D.to_string(), dec);
                }
            }

            trace!("Calculating MACD");
            //Calculate MACD
            let output = macd.next(close_f64);
            if idx >= 26 {
                // if idx == sorted_history.len() - 1 {
                //     debug!("macd value for date: {} - {}-{}-{}", h.date, output.macd, output.signal, output.histogram )
                // }
                let dec = Decimal::try_from(output.macd)?.round_dp(2);
                values.insert(MACD.to_string(), dec);

                let dec = Decimal::try_from(output.signal)?.round_dp(2);
                values.insert(MACD_SIGNAL.to_string(), dec);

                let dec = Decimal::try_from(output.histogram)?.round_dp(2);
                values.insert(MACD_HISTOGRAM.to_string(), dec);
            }

            trace!("Calculating Bollinger Bands");
            //Bollinger Bands
            let output = bb.next(close_f64);
            if idx >= bb_period - 1 {
                // if idx == sorted_history.len() - 1 {
                //     debug!("bb value for date: {} - {}-{}-{}", h.date, output.upper, output.average, output.lower )
                // }
                let dec = Decimal::try_from(output.upper)?.round_dp(2);
                values.insert(BB_UPPER.to_string(), dec);

                let dec = Decimal::try_from(output.average)?.round_dp(2);
                values.insert(BB_MIDDLE.to_string(), dec);

                let dec = Decimal::try_from(output.lower)?.round_dp(2);
                values.insert(BB_LOWER.to_string(), dec);
            }

            trace!("Calculating ATR");
            //ATR
            let atr_value = atr.next(&bar);
            if idx >= atr_period {
                // if idx == sorted_history.len() - 1 {
                //     debug!("atr value for date: {} - {}", h.date, atr_value )
                // }
                let dec = Decimal::try_from(atr_value)?.round_dp(2);
                values.insert(ATR.to_string(), dec);
            }

            trace!("Calculating Volume Ratio");
            //Volume ratio
            if idx > volume_ratio_period {
                let mut total_volume = Decimal::ZERO;
                if h.volume > Decimal::ZERO {
                    for hist in sorted_history.iter().take(idx).skip(idx - 20) {
                        total_volume += hist.volume;
                    }
                    // debug!("total volume: {}", total_volume);
                    total_volume = total_volume.div(Decimal::from(volume_ratio_period));
                    // debug!("total volume average: {}", total_volume);
                    if total_volume > Decimal::ZERO {
                        total_volume = (h.volume / total_volume).round_dp(2);
                        total_volume = cmp::min(total_volume, Decimal::from(10));
                    }
                }
                // debug!("total volume ratio: {}", total_volume);
                values.insert(VOLUME_RATIO.to_string(), total_volume);
            }

            if !values.is_empty() {
                let indicator = TickerIndicator::new(
                    h.date,
                    &h.metadata.symbol,
                    // &h.metadata.exchange,
                    // &h.metadata.granularity,
                    values,
                );
                indicators.push(indicator);
            }
        }
        Ok(indicators)
    }
}
