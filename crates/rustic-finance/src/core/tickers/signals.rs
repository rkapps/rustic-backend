use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;

use crate::domain::tickers::indicator::indicator_type::{
    ATR, BB_LOWER, BB_MIDDLE, BB_UPPER, MACD, MACD_HISTOGRAM, MACD_SIGNAL, RSI, SMA, STOCHASTIC_D,
    STOCHASTIC_K,
};
use crate::domain::tickers::signals::{
    Direction, OverallDirection, OverallSignal, Signal, SignalTier,
};

const ZERO: Decimal = dec!(0);
const RSI_OVERSOLD: Decimal = dec!(30);
const RSI_OVERSOLD_35: Decimal = dec!(35);
const RSI_OVERBOUGHT: Decimal = dec!(70);
const RSI_OVERBOUGHT_65: Decimal = dec!(65);
const RSI_PULLBACK_LOW: Decimal = dec!(40);
const RSI_PULLBACK_HIGH: Decimal = dec!(55);
const RSI_RALLY_LOW: Decimal = dec!(45);
const RSI_RALLY_HIGH: Decimal = dec!(60);
const RSI_MEAN_REVERSION: Decimal = dec!(40);
const STOCHASTIC_MEAN_REVERSION: Decimal = dec!(30);
const STOCHASTIC_D_LOWER: Decimal = dec!(20);
const STOCHASTIC_D_UPPER: Decimal = dec!(80);
const STOCHASTIC_K_LOWER: Decimal = dec!(20);
const STOCHASTIC_K_MID: Decimal = dec!(50);
const STOCHASTIC_K_UPPER: Decimal = dec!(50);

const OVERSOLD_COMBO_SIGNALS: [&str; 4] = [
    "RSI Oversold",
    "BB Breakout Lower",
    "Stochastic Bearish",
    "Below SMA50",
];
const OVERBOUGHT_COMBO_SIGNALS: [&str; 4] = [
    "RSI Overbought",
    "BB Breakout Upper",
    "Stochastic Bullish",
    "Above SMA50",
];

impl Signal {
    fn new(name: &str, direction: Direction, tier: SignalTier, inputs: &[&str]) -> Self {
        Self {
            name: name.to_string(),
            direction,
            tier,
            inputs: inputs.iter().map(|s| s.to_string()).collect(),
        }
    }
}

pub fn signal_magnitude(name: &str) -> Decimal {
    match name {
        "SMA Stack Bullish" | "SMA Stack Bearish" => dec!(2),
        "Golden Cross" | "Death Cross" => dec!(2),
        "Golden Cross Active" => dec!(1),
        "Death Cross Active" => dec!(-1),        
        "Momentum Breakout" => dec!(2),
        "Bullish Trend Exhaustion" | "Bearish Trend Exhaustion" => dec!(2),
        "Oversold Confluence" | "Overbought Confluence" => dec!(2),
        "Deeply Oversold" | "Deeply Overbought" => dec!(2),
        "RSI Multi-period Oversold" | "RSI Multi-period Overbought" => dec!(2),
        "Oversold Reversal Setup" | "Overbought Reversal Setup" => dec!(1),
        "Moderately Oversold" | "Moderately Overbought" => dec!(1),
        "Mean Reversion Candidate" => dec!(1),
        "Above SMA50" | "Below SMA50" => dec!(1),
        "Bullish Pullback" | "Bearish Rally" => dec!(1),
        "MACD Bullish Crossover" | "MACD Bearish Crossover" => dec!(1),
        "MACD Histogram Expanding" | "MACD Histogram Weakening" => dec!(1),
        "BB Breakout Upper" | "BB Breakout Lower" => dec!(1),
        "Stochastic Bullish" | "Stochastic Bearish" => dec!(1),
        "RSI Oversold" | "RSI Overbought" => dec!(1),
        _ => ZERO,
    }
}

pub fn signed_weight(signal: &Signal) -> Decimal {
    match signal.direction {
        Direction::Bullish => signal_magnitude(&signal.name),
        Direction::Bearish => -signal_magnitude(&signal.name),
        Direction::Neutral => ZERO,
    }
}


pub struct SignalsCalculator {}

impl SignalsCalculator {
    /// Entry point: curr/prev are the `values` maps from consecutive TickerIndicator rows.
    pub fn calculate(
        &self,
        curr: &HashMap<String, Decimal>,
        prev: Option<&HashMap<String, Decimal>>,
    ) -> (Vec<Signal>, Option<OverallSignal>) {
        let mut signals = Vec::new();

        signals.extend(self.sma_stack(curr));
        signals.extend(self.sma_50(curr));
        signals.extend(self.sma_trend_state(curr));  
        signals.extend(self.bollinger_bands(curr));
        signals.extend(self.rsi(curr));

        if let Some(prev) = prev {
            signals.extend(self.sma_crossover(curr, prev));
            signals.extend(self.macd_crossover(curr, prev));
            signals.extend(self.macd_histogram_trend(curr, prev));
            signals.extend(self.stochastic(curr, prev));
            signals.extend(self.momentum(curr, prev));
            signals.extend(self.trend_exhaustion(curr, prev));
            signals.extend(self.mean_reversion(curr, prev));
            signals.extend(self.volatility(curr, prev));
        }

        signals.extend(self.confluence(curr));

        // Severity composites read the signals already collected above.
        let severity = self
            .oversold_severity(&signals)
            .into_iter()
            .chain(self.overbought_severity(&signals));
        signals.extend(severity);

        let overall = self.rollup(&signals);
        (signals, overall)
    }

    fn sma_stack(&self, curr: &HashMap<String, Decimal>) -> Vec<Signal> {
        let (Some(&s20), Some(&s50), Some(&s100), Some(&s200)) = (
            curr.get(&format!("{}_20", SMA)),
            curr.get(&format!("{}_50", SMA)),
            curr.get(&format!("{}_100", SMA)),
            curr.get(&format!("{}_200", SMA)),
        ) else {
            return vec![];
        };

        if s20 > s50 && s50 > s100 && s100 > s200 {
            return vec![Signal::new(
                "SMA Stack Bullish",
                Direction::Bullish,
                SignalTier::Composite,
                &["sma_20", "sma_50", "sma_100", "sma_200"],
            )];
        }
        if s20 < s50 && s50 < s100 && s100 < s200 {
            return vec![Signal::new(
                "SMA Stack Bearish",
                Direction::Bearish,
                SignalTier::Composite,
                &["sma_20", "sma_50", "sma_100", "sma_200"],
            )];
        }
        vec![]
    }

    fn sma_50(&self, curr: &HashMap<String, Decimal>) -> Vec<Signal> {
        let (Some(&price), Some(&s50), Some(&s200), Some(&rsi)) = (
            curr.get("price"),
            curr.get(&format!("{}_50", SMA)),
            curr.get(&format!("{}_200", SMA)),
            curr.get(&format!("{}_14", RSI)),
        ) else {
            return vec![];
        };

        let mut out = Vec::new();
        if price > s50 {
            out.push(Signal::new(
                "Above SMA50",
                Direction::Bullish,
                SignalTier::Composite,
                &["price", "sma_50"],
            ));
        } else if price < s50 {
            out.push(Signal::new(
                "Below SMA50",
                Direction::Bearish,
                SignalTier::Composite,
                &["price", "sma_50"],
            ));
        }

        if s50 > s200 && price > s50 && rsi >= RSI_PULLBACK_LOW && rsi <= RSI_PULLBACK_HIGH {
            out.push(Signal::new(
                "Bullish Pullback",
                Direction::Bullish,
                SignalTier::Composite,
                &["sma_50", "sma_200", "price", "rsi_14"],
            ));
        }
        if s50 < s200 && price < s50 && rsi > RSI_RALLY_LOW && rsi <= RSI_RALLY_HIGH {
            out.push(Signal::new(
                "Bearish Rally",
                Direction::Bearish,
                SignalTier::Composite,
                &["sma_50", "sma_200", "price", "rsi_14"],
            ));
        }
        out
    }

    fn sma_crossover(
        &self,
        curr: &HashMap<String, Decimal>,
        prev: &HashMap<String, Decimal>,
    ) -> Vec<Signal> {
        let (Some(&s50), Some(&s200), Some(&ps50), Some(&ps200)) = (
            curr.get(&format!("{}_50", SMA)),
            curr.get(&format!("{}_200", SMA)),
            prev.get(&format!("{}_50", SMA)),
            prev.get(&format!("{}_200", SMA)),
        ) else {
            return vec![];
        };

        if s50 > s200 && ps50 <= ps200 {
            return vec![Signal::new(
                "Golden Cross",
                Direction::Bullish,
                SignalTier::Composite,
                &["sma_50", "sma_200"],
            )];
        }
        if s50 < s200 && ps50 >= ps200 {
            return vec![Signal::new(
                "Death Cross",
                Direction::Bearish,
                SignalTier::Composite,
                &["sma_50", "sma_200"],
            )];
        }
        vec![]
    }

    fn sma_trend_state(&self, curr: &HashMap<String, Decimal>) -> Vec<Signal> {
        let (Some(&s50), Some(&s200)) = (
            curr.get(&format!("{}_50", SMA)),
            curr.get(&format!("{}_200", SMA)),
        ) else {
            return vec![];
        };

        if s50 > s200 {
            vec![Signal::new(
                "Golden Cross Active",
                Direction::Bullish,
                SignalTier::Composite,
                &["sma_50", "sma_200"],
            )]
        } else if s50 < s200 {
            vec![Signal::new(
                "Death Cross Active",
                Direction::Bearish,
                SignalTier::Composite,
                &["sma_50", "sma_200"],
            )]
        } else {
            vec![]
        }
    }

    fn macd_crossover(
        &self,
        curr: &HashMap<String, Decimal>,
        prev: &HashMap<String, Decimal>,
    ) -> Vec<Signal> {
        let (Some(&m), Some(&s), Some(&pm), Some(&ps)) = (
            curr.get(MACD),
            curr.get(MACD_SIGNAL),
            prev.get(MACD),
            prev.get(MACD_SIGNAL),
        ) else {
            return vec![];
        };

        if m > s && pm <= ps {
            return vec![Signal::new(
                "MACD Bullish Crossover",
                Direction::Bullish,
                SignalTier::Composite,
                &["macd", "macd_signal"],
            )];
        }
        if m < s && pm >= ps {
            return vec![Signal::new(
                "MACD Bearish Crossover",
                Direction::Bearish,
                SignalTier::Composite,
                &["macd", "macd_signal"],
            )];
        }
        vec![]
    }

    fn macd_histogram_trend(
        &self,
        curr: &HashMap<String, Decimal>,
        prev: &HashMap<String, Decimal>,
    ) -> Vec<Signal> {
        let (Some(&h), Some(&ph)) = (curr.get(MACD_HISTOGRAM), prev.get(MACD_HISTOGRAM)) else {
            return vec![];
        };

        let same_side = (h > ZERO && ph > ZERO) || (h < ZERO && ph < ZERO);
        if !same_side {
            return vec![];
        }

        let magnitude_growing = h.abs() > ph.abs();

        if magnitude_growing {
            // momentum strengthening in whichever direction it's already pointing
            let dir = if h > ZERO {
                Direction::Bullish
            } else {
                Direction::Bearish
            };
            vec![Signal::new(
                "MACD Histogram Expanding",
                dir,
                SignalTier::Single,
                &["macd_histogram"],
            )]
        } else {
            // momentum fading — opposite sentiment building
            let dir = if h > ZERO {
                Direction::Bearish
            } else {
                Direction::Bullish
            };
            vec![Signal::new(
                "MACD Histogram Weakening",
                dir,
                SignalTier::Single,
                &["macd_histogram"],
            )]
        }
    }

    fn bollinger_bands(&self, curr: &HashMap<String, Decimal>) -> Vec<Signal> {
        let (Some(&price), Some(&upper), Some(&middle), Some(&lower)) = (
            curr.get("price"),
            curr.get(BB_UPPER),
            curr.get(BB_MIDDLE),
            curr.get(BB_LOWER),
        ) else {
            return vec![];
        };

        let mut out = Vec::new();
        if price > upper {
            out.push(Signal::new(
                "BB Breakout Upper",
                Direction::Bullish,
                SignalTier::Single,
                &["price", "bb_upper"],
            ));
        }
        if price < lower {
            out.push(Signal::new(
                "BB Breakout Lower",
                Direction::Bearish,
                SignalTier::Single,
                &["price", "bb_lower"],
            ));
        }
        if middle > ZERO {
            let width = (upper - lower) / middle * dec!(100);
            if width < dec!(4) {
                out.push(Signal::new(
                    "BB Squeeze",
                    Direction::Neutral,
                    SignalTier::Single,
                    &["bb_upper", "bb_lower", "bb_middle"],
                ));
            }
        }
        out
    }

    fn stochastic(
        &self,
        curr: &HashMap<String, Decimal>,
        prev: &HashMap<String, Decimal>,
    ) -> Vec<Signal> {
        let (Some(&k), Some(&d), Some(&pk), Some(&pd)) = (
            curr.get(STOCHASTIC_K),
            curr.get(STOCHASTIC_D),
            prev.get(STOCHASTIC_K),
            prev.get(STOCHASTIC_D),
        ) else {
            return vec![];
        };

        if k > d && pk <= pd {
            return vec![Signal::new(
                "Stochastic Bullish",
                Direction::Bullish,
                SignalTier::Composite,
                &["stochastic_k", "stochastic_d"],
            )];
        }
        if k < d && pk >= pd {
            return vec![Signal::new(
                "Stochastic Bearish",
                Direction::Bearish,
                SignalTier::Composite,
                &["stochastic_k", "stochastic_d"],
            )];
        }
        vec![]
    }

    fn rsi(&self, curr: &HashMap<String, Decimal>) -> Vec<Signal> {
        let Some(&r14) = curr.get(&format!("{}_14", RSI)) else {
            return vec![];
        };
        let r10 = curr.get(&format!("{}_10", RSI)).copied();
        let r26 = curr.get(&format!("{}_26", RSI)).copied();

        let mut out = Vec::new();
        if r14 < RSI_OVERSOLD {
            out.push(Signal::new(
                "RSI Oversold",
                Direction::Bearish,
                SignalTier::Single,
                &["rsi_14"],
            ));
        }
        if r14 > RSI_OVERBOUGHT {
            out.push(Signal::new(
                "RSI Overbought",
                Direction::Bearish,
                SignalTier::Single,
                &["rsi_14"],
            ));
        }
        if let (Some(r10), Some(r26)) = (r10, r26) {
            if r10 < RSI_OVERSOLD_35 && r14 < RSI_OVERSOLD_35 && r26 < RSI_OVERSOLD_35 {
                out.push(Signal::new(
                    "RSI Multi-period Oversold",
                    Direction::Bullish,
                    SignalTier::Composite,
                    &["rsi_10", "rsi_14", "rsi_26"],
                ));
            }
            if r10 > RSI_OVERBOUGHT_65 && r14 > RSI_OVERBOUGHT_65 && r26 > RSI_OVERBOUGHT_65 {
                out.push(Signal::new(
                    "RSI Multi-period Overbought",
                    Direction::Bearish,
                    SignalTier::Composite,
                    &["rsi_10", "rsi_14", "rsi_26"],
                ));
            }
        }
        out
    }

    fn confluence(&self, curr: &HashMap<String, Decimal>) -> Vec<Signal> {
        let (Some(&price), Some(&rsi), Some(&lower), Some(&upper), Some(&k), Some(&d)) = (
            curr.get("price"),
            curr.get(&format!("{}_14", RSI)),
            curr.get(BB_LOWER),
            curr.get(BB_UPPER),
            curr.get(STOCHASTIC_K),
            curr.get(STOCHASTIC_D),
        ) else {
            return vec![];
        };

        let mut out = Vec::new();
        if rsi < RSI_OVERSOLD_35 && price < lower && d < STOCHASTIC_D_LOWER {
            out.push(Signal::new(
                "Oversold Confluence",
                Direction::Bullish,
                SignalTier::Composite,
                &["rsi_14", "price", "bb_lower", "stochastic_d"],
            ));
            if k > d {
                out.push(Signal::new(
                    "Oversold Reversal Setup",
                    Direction::Bullish,
                    SignalTier::Composite,
                    &["stochastic_k", "stochastic_d"],
                ));
            }
        }
        if rsi > RSI_OVERBOUGHT_65 && price > upper && d > STOCHASTIC_D_UPPER {
            out.push(Signal::new(
                "Overbought Confluence",
                Direction::Bearish,
                SignalTier::Composite,
                &["rsi_14", "price", "bb_upper", "stochastic_d"],
            ));
            if k < d {
                out.push(Signal::new(
                    "Overbought Reversal Setup",
                    Direction::Bearish,
                    SignalTier::Composite,
                    &["stochastic_k", "stochastic_d"],
                ));
            }
        }
        out
    }

    fn momentum(
        &self,
        curr: &HashMap<String, Decimal>,
        prev: &HashMap<String, Decimal>,
    ) -> Vec<Signal> {
        let (
            Some(&price),
            Some(&s20),
            Some(&s50),
            Some(&s100),
            Some(&s200),
            Some(&h),
            Some(&ph),
            Some(&k),
            Some(&d),
        ) = (
            curr.get("price"),
            curr.get(&format!("{}_20", SMA)),
            curr.get(&format!("{}_50", SMA)),
            curr.get(&format!("{}_100", SMA)),
            curr.get(&format!("{}_200", SMA)),
            curr.get(MACD_HISTOGRAM),
            prev.get(MACD_HISTOGRAM),
            curr.get(STOCHASTIC_K),
            curr.get(STOCHASTIC_D),
        )
        else {
            return vec![];
        };

        if price > s20
            && price > s50
            && price > s100
            && price > s200
            && h > ZERO
            && h > ph
            && k > STOCHASTIC_K_MID
            && k > d
        {
            return vec![Signal::new(
                "Momentum Breakout",
                Direction::Bullish,
                SignalTier::Composite,
                &[
                    "price",
                    "sma_20",
                    "sma_50",
                    "sma_100",
                    "sma_200",
                    "macd_histogram",
                    "stochastic_k",
                    "stochastic_d",
                ],
            )];
        }
        vec![]
    }

    fn trend_exhaustion(
        &self,
        curr: &HashMap<String, Decimal>,
        prev: &HashMap<String, Decimal>,
    ) -> Vec<Signal> {
        let (
            Some(&price),
            Some(&upper),
            Some(&lower),
            Some(&rsi),
            Some(&h),
            Some(&ph),
            Some(&k),
            Some(&d),
        ) = (
            curr.get("price"),
            curr.get(BB_UPPER),
            curr.get(BB_LOWER),
            curr.get(&format!("{}_14", RSI)),
            curr.get(MACD_HISTOGRAM),
            prev.get(MACD_HISTOGRAM),
            curr.get(STOCHASTIC_K),
            curr.get(STOCHASTIC_D),
        )
        else {
            return vec![];
        };

        if price >= upper
            && rsi > RSI_OVERBOUGHT_65
            && h > ZERO
            && h < ph
            && k > STOCHASTIC_K_UPPER
            && k < d
        {
            return vec![Signal::new(
                "Bullish Trend Exhaustion",
                Direction::Bearish,
                SignalTier::Composite,
                &[
                    "price",
                    "bb_upper",
                    "rsi_14",
                    "macd_histogram",
                    "stochastic_k",
                    "stochastic_d",
                ],
            )];
        }
        if price < lower
            && rsi < RSI_OVERSOLD_35
            && h < ZERO
            && h > ph
            && k < STOCHASTIC_K_LOWER
            && k > d
        {
            return vec![Signal::new(
                "Bearish Trend Exhaustion",
                Direction::Bullish,
                SignalTier::Composite,
                &[
                    "price",
                    "bb_lower",
                    "rsi_14",
                    "macd_histogram",
                    "stochastic_k",
                    "stochastic_d",
                ],
            )];
        }
        vec![]
    }

    fn mean_reversion(
        &self,
        curr: &HashMap<String, Decimal>,
        prev: &HashMap<String, Decimal>,
    ) -> Vec<Signal> {
        let (Some(&rsi), Some(&s20), Some(&price), Some(&k), Some(&atr), Some(&prev_atr)) = (
            curr.get(&format!("{}_14", RSI)),
            curr.get(&format!("{}_20", SMA)),
            curr.get("price"),
            curr.get(STOCHASTIC_K),
            curr.get(ATR),
            prev.get(ATR),
        ) else {
            return vec![];
        };

        if rsi < RSI_MEAN_REVERSION
            && price < s20
            && k < STOCHASTIC_MEAN_REVERSION
            && atr <= prev_atr
        {
            return vec![Signal::new(
                "Mean Reversion Candidate",
                Direction::Bullish,
                SignalTier::Composite,
                &["rsi_14", "sma_20", "price", "stochastic_k", "atr"],
            )];
        }
        vec![]
    }

    fn volatility(
        &self,
        curr: &HashMap<String, Decimal>,
        prev: &HashMap<String, Decimal>,
    ) -> Vec<Signal> {
        let (Some(&atr), Some(&patr)) = (curr.get(ATR), prev.get(ATR)) else {
            return vec![];
        };

        if atr > patr {
            return vec![Signal::new(
                "Volatility Expanding",
                Direction::Neutral,
                SignalTier::Single,
                &["atr"],
            )];
        }

        if atr < patr {
            return vec![Signal::new(
                "Volatility Contracting",
                Direction::Neutral,
                SignalTier::Single,
                &["atr"],
            )];
        }
        vec![]
    }

    fn oversold_severity(&self, signals: &[Signal]) -> Vec<Signal> {
        let names: Vec<&str> = signals.iter().map(|s| s.name.as_str()).collect();
        let count = OVERSOLD_COMBO_SIGNALS
            .iter()
            .filter(|s| names.contains(s))
            .count();

        if count >= 3 {
            vec![Signal::new(
                "Deeply Oversold",
                Direction::Bullish,
                SignalTier::Composite,
                &OVERSOLD_COMBO_SIGNALS,
            )]
        } else if count == 2 {
            vec![Signal::new(
                "Moderately Oversold",
                Direction::Bullish,
                SignalTier::Composite,
                &OVERSOLD_COMBO_SIGNALS,
            )]
        } else {
            vec![]
        }
    }

    fn overbought_severity(&self, signals: &[Signal]) -> Vec<Signal> {
        let names: Vec<&str> = signals.iter().map(|s| s.name.as_str()).collect();
        let count = OVERBOUGHT_COMBO_SIGNALS
            .iter()
            .filter(|s| names.contains(s))
            .count();

        if count >= 3 {
            vec![Signal::new(
                "Deeply Overbought",
                Direction::Bearish,
                SignalTier::Composite,
                &OVERBOUGHT_COMBO_SIGNALS,
            )]
        } else if count == 2 {
            vec![Signal::new(
                "Moderately Overbought",
                Direction::Bearish,
                SignalTier::Composite,
                &OVERBOUGHT_COMBO_SIGNALS,
            )]
        } else {
            vec![]
        }
    }

    fn rollup(&self, signals: &[Signal]) -> Option<OverallSignal> {
        if signals.is_empty() {
            return None;
        }

        let score: Decimal = signals.iter().map(|s| signed_weight(s)).sum();
        let supporting: Vec<&Signal> = signals.iter().filter(|s| signed_weight(s) > ZERO).collect();
        let opposing: Vec<&Signal> = signals.iter().filter(|s| signed_weight(s) < ZERO).collect();
        let conflicting = !supporting.is_empty() && !opposing.is_empty();

        let direction = if score > dec!(1) && !conflicting {
            OverallDirection::Bullish
        } else if score < dec!(-1) && !conflicting {
            OverallDirection::Bearish
        } else if conflicting && score.abs() >= dec!(2) {
            if score > ZERO {
                OverallDirection::Bullish
            } else {
                OverallDirection::Bearish
            }
        } else if conflicting {
            OverallDirection::Mixed
        } else {
            OverallDirection::Neutral
        };

        let summary = match direction {
            OverallDirection::Bullish if conflicting => {
                "Bullish overall, with some conflicting signals".to_string()
            }
            OverallDirection::Bullish => "Bullish trend with supporting momentum".to_string(),
            OverallDirection::Bearish if conflicting => {
                "Bearish overall, with some conflicting signals".to_string()
            }
            OverallDirection::Bearish => "Bearish trend with confirming momentum".to_string(),
            OverallDirection::Mixed => "Mixed signals — trend and momentum disagree".to_string(),
            OverallDirection::Neutral => "No strong directional signal".to_string(),
        };

        Some(OverallSignal {
            direction,
            score,
            summary,
            conflicting,
        })
    }
}
