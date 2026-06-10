use anyhow::Result;
use chrono::{DateTime, Utc};
use rustic_core::HttpClient;

use crate::finance::alpha::model::{
    AlphaEtf, AlphaTicker, AlphaTickerSentiment, AlphaTickerSentimentFeed,
};

const ALPHA_BASE_URL: &str = "https://www.alphavantage.co/";
const ALPHA_FUNCTION_NEWS_SENTIMENT: &str = "NEWS_SENTIMENT";
const ALPHA_FUNCTION_OVERVIEW: &str = "OVERVIEW";
const ALPHA_FUNCTION_ETF_PROFILE: &str = "ETF_PROFILE";

pub async fn get_stock(
    http_client: &HttpClient,
    symbol: &str,
    api_key: &str,
) -> Result<AlphaTicker> {
    let url = format!(
        "{}query?function={}&symbol={}&apikey={}",
        ALPHA_BASE_URL, ALPHA_FUNCTION_OVERVIEW, symbol, api_key
    );
    let headers = reqwest::header::HeaderMap::new();
    let ticker = http_client
        .get_request::<AlphaTicker>(url, Some(headers))
        .await?;

    Ok(ticker)
}

pub async fn get_etf(http_client: &HttpClient, symbol: &str, api_key: &str) -> Result<AlphaEtf> {
    let url = format!(
        "{}query?function={}&symbol={}&apikey={}",
        ALPHA_BASE_URL, ALPHA_FUNCTION_ETF_PROFILE, symbol, api_key
    );
    let headers = reqwest::header::HeaderMap::new();
    let etf = http_client
        .get_request::<AlphaEtf>(url, Some(headers))
        .await?;

    Ok(etf)
}

pub async fn get_stock_sentiments(
    http_client: &HttpClient,
    symbol: &str,
    api_key: &str,
    date_from: &DateTime<Utc>,
) -> Result<Vec<AlphaTickerSentimentFeed>> {
    let date_from = convertdatetime_to_format(date_from);
    let url = format!(
        "{}query?function={}&tickers={}&time_from={}&apikey={}&limit=1000&sort=LATEST",
        ALPHA_BASE_URL, ALPHA_FUNCTION_NEWS_SENTIMENT, symbol, date_from, api_key
    );
    let headers = reqwest::header::HeaderMap::new();
    let tsentiment = http_client
        .get_request::<AlphaTickerSentiment>(url, Some(headers))
        .await?;

    Ok(tsentiment.feed)
}

// convert date to format 20220410T0130
fn convertdatetime_to_format(date: &DateTime<Utc>) -> String {
    date.format("%Y%m%dT%H%M").to_string()
}

#[cfg(test)]
mod tests {

    use anyhow::Result;
    use chrono::Utc;
    use rustic_core::HttpClient;
    use std::env;
    use tracing::Level;
    use tracing_subscriber::{filter, layer::SubscriberExt, util::SubscriberInitExt};

    use crate::finance::alpha::api::get_stock_sentiments;

    #[tokio::test]
    #[ignore]
    async fn test_sentiment() -> Result<()> {
        let filter = filter::Targets::new().with_target("test", Level::DEBUG);

        tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer().compact().pretty()) // Compact format
            .with(filter)
            .init();

        let api_key =
            env::var("ALPHA_API_KEY").expect("ALPHA_API_KEY environment variable not set");

        let http_client = HttpClient::new()?;
        let date_from = &Utc::now();
        let feeds = get_stock_sentiments(&http_client, "aapl", api_key.as_str(), date_from).await?;
        assert!(feeds.len() > 0);
        Ok(())
    }
}
