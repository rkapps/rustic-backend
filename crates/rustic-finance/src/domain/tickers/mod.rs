use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
pub mod embedding;
pub mod history;
pub mod indicator;
pub mod sentiment;
pub mod ticker;
pub mod ticker_peer;
pub mod control;
pub mod news;

pub const TICKER_COLLECTION_NAME: &str = "ticker";
pub const TICKER_CONTROL_COLLECTION_NAME: &str = "ticker_control";
pub const TICKER_HISTORY_COLLECTION_NAME: &str = "ticker_history";
pub const TICKER_NEWS_COLLECTION_NAME: &str = "ticker_news";
pub const TICKER_INDICATOR_COLLECTION_NAME: &str = "ticker_indicator";
pub const TICKER_SENTIMENT_COLLECTION_NAME: &str = "ticker_sentiment";
pub const TICKER_EMBEDDING_COLLECTION_NAME: &str = "ticker_embedding";
pub const TICKER_ALPHA_COLLECTION_NAME: &str = "ticker_alpha";

pub const TICKER_PERFORMANCE_PERIODS: [&str; 8] = ["1W", "1M", "3M", "6M", "1Y", "Ytd", "2Y", "5Y"];

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum AssetType {
    #[default]
    Stock,
    Etf,
    Crypto,
}

impl FromStr for AssetType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "STOCK" => Ok(AssetType::Stock),
            "ETF" => Ok(AssetType::Etf),
            "CRYPTO" => Ok(AssetType::Crypto),
            _ => Err(format!("Unknown asset type: {}", s)),
        }
    }
}

pub mod decimal_serde {
    use bson::Decimal128;
    use rust_decimal::Decimal;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::str::FromStr;

    pub fn serialize<S>(decimal: &Decimal, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let d128 = Decimal128::from_str(&decimal.to_string()).map_err(serde::ser::Error::custom)?;
        d128.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Decimal, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;

        // Accept f64, i64, Decimal128, or String
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum NumberVariant {
            Float(f64),
            Int(i64),
            Decimal128(Decimal128),
            String(String),
        }

        let value = NumberVariant::deserialize(deserializer)?;

        match value {
            NumberVariant::Float(f) => Decimal::try_from(f).map_err(Error::custom),
            NumberVariant::Int(i) => Ok(Decimal::from(i)),
            NumberVariant::Decimal128(d) => {
                Decimal::from_str(&d.to_string()).map_err(Error::custom)
            }
            NumberVariant::String(s) => Decimal::from_str(&s).map_err(Error::custom),
        }
    }
}

fn deserialize_flexible_datetime<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum DateTimeVariant {
        BsonDateTime(bson::DateTime),
        String(String),
        // Sometimes MongoDB Extended JSON has: {"$date": "..."}
        ExtendedJson {
            #[serde(rename = "$date")]
            date: String,
        },
    }

    let value = DateTimeVariant::deserialize(deserializer)?;

    match value {
        DateTimeVariant::BsonDateTime(dt) => Ok(dt.to_chrono()),
        DateTimeVariant::String(s) => s
            .parse::<DateTime<Utc>>()
            .map_err(|e| Error::custom(format!("Invalid datetime string: {}", e))),
        DateTimeVariant::ExtendedJson { date } => date
            .parse::<DateTime<Utc>>()
            .map_err(|e| Error::custom(format!("Invalid datetime in $date: {}", e))),
    }
}

pub fn serialize_as_bson_datetime<S>(dt: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let bson_dt = bson::DateTime::from_chrono(*dt);
    bson_dt.serialize(serializer)
}

pub mod performance_serde {
    use rust_decimal::Decimal;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::{collections::HashMap, str::FromStr};

    pub fn serialize<S>(
        map: &HashMap<String, HashMap<String, Decimal>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use std::str::FromStr;
        let converted: HashMap<String, HashMap<String, bson::Decimal128>> = map
            .iter()
            .map(|(k, v)| {
                let inner = v
                    .iter()
                    .map(|(ik, iv)| {
                        let d128 = bson::Decimal128::from_str(&iv.to_string())
                            .unwrap_or_else(|_| bson::Decimal128::from_str("0").unwrap());
                        (ik.clone(), d128)
                    })
                    .collect();
                (k.clone(), inner)
            })
            .collect();
        converted.serialize(serializer)
    }
    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<HashMap<String, HashMap<String, Decimal>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let map = HashMap::<String, HashMap<String, bson::Decimal128>>::deserialize(deserializer)?;
        Ok(map
            .into_iter()
            .map(|(k, v)| {
                let inner = v
                    .into_iter()
                    .map(|(ik, iv)| {
                        let decimal = Decimal::from_str(&iv.to_string()).unwrap_or_default();
                        (ik, decimal)
                    })
                    .collect();
                (k, inner)
            })
            .collect())
    }
}

pub mod indicator_serde {
    use bson::Decimal128;
    use rust_decimal::Decimal;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::HashMap;
    use std::str::FromStr;

    pub fn serialize<S>(map: &HashMap<String, Decimal>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let converted: HashMap<String, Decimal128> = map
            .iter()
            .map(|(k, v)| {
                let d128 = Decimal128::from_str(&v.to_string())
                    .unwrap_or_else(|_| Decimal128::from_str("0").unwrap());
                (k.clone(), d128)
            })
            .collect();
        converted.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<HashMap<String, Decimal>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let map = HashMap::<String, Decimal128>::deserialize(deserializer)?;
        Ok(map
            .into_iter()
            .map(|(k, v)| {
                let decimal = Decimal::from_str(&v.to_string()).unwrap_or_default();
                (k, decimal)
            })
            .collect())
    }
}
