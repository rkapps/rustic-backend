use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Direction {
    Bullish,
    Bearish,
    Neutral,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SignalTier {
    Single,
    Composite,
    Model,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    pub name: String, // "RSI Overbought", "MACD Bullish Crossover"
    pub direction: Direction,
    pub tier: SignalTier,
    pub inputs: Vec<String>, // ["rsi_14"], ["macd","macd_signal"], etc.
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum OverallDirection {
    Bullish,
    Bearish,
    Neutral,
    Mixed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverallSignal {
    pub direction: OverallDirection,
    pub score: Decimal,
    pub summary: String, // deterministic template, not LLM-authored
    pub conflicting: bool,
}
