use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use rustic_core::HttpClient;
use tracing::info;

use crate::finance::tiingo::model::{
    TiingoTickerHistory, TiingoTickerNews, TiingoTickerPriceData, TiingoTickerRealtime,
};

const TIINGO_REALTIME_URL: &str = "https://api.tiingo.com/iex/";
const TIINGO_EOD_URL: &str = "https://api.tiingo.com/tiingo/daily/";
const TIINGO_NEWS_URL: &str = "https://api.tiingo.com/tiingo/news/";
const TIINGO_CRYPTO_URL: &str = "https://api.tiingo.com/tiingo/crypto/prices";
const TIINGO_MAX_CHUNK_DAYS: i64 = 365 * 5; // confirm against Tiingo's actual observed cap

pub async fn get_ticker_news(
    http_client: &HttpClient,
    symbol: &str,
    api_token: &str,
) -> Result<Vec<TiingoTickerNews>> {
    let url = format!("{}?tickers={}&token={}", TIINGO_NEWS_URL, symbol, api_token,);
    let headers = reqwest::header::HeaderMap::new();
    let hist = http_client
        .get_request::<Vec<TiingoTickerNews>>(url, Some(headers))
        .await?;

    Ok(hist)
}

pub async fn get_stock_history(
    http_client: &HttpClient,
    symbol: &str,
    api_token: &str,
    start_date: &DateTime<Utc>,
) -> Result<Vec<TiingoTickerHistory>> {
    let now = Utc::now();
    let mut all_history = Vec::new();
    let mut chunk_start = *start_date;

    while chunk_start < now {
        let chunk_end = std::cmp::min(chunk_start + Duration::days(TIINGO_MAX_CHUNK_DAYS), now);

        let url = format!(
            "{}{}/prices?token={}&startDate={}&endDate={}",
            TIINGO_EOD_URL,
            symbol,
            api_token,
            convert_datetime_utc_to_ymd(&chunk_start),
            convert_datetime_utc_to_ymd(&chunk_end)
        );
        info!("Url: {}", url);

        let headers = reqwest::header::HeaderMap::new();
        let chunk = http_client
            .get_request::<Vec<TiingoTickerHistory>>(url, Some(headers))
            .await?;

        if !chunk.is_empty() {
            all_history.extend(chunk);
        }

        chunk_start = chunk_end + Duration::days(1); // advance PAST the last fetched date, not onto it
    }

    Ok(all_history)
}

pub async fn get_crypto_history(
    http_client: &HttpClient,
    symbol: &str,
    api_token: &str,
    start_date: &DateTime<Utc>,
    frequency: &str,
) -> Result<Vec<TiingoTickerHistory>> {
    let now = Utc::now();
    let mut all_history = Vec::new();
    let mut chunk_start = *start_date;

    while chunk_start < now {
        let chunk_end = std::cmp::min(chunk_start + Duration::days(TIINGO_MAX_CHUNK_DAYS), now);

        let url = format!(
            "{}?tickers={}&resampleFreq={}&startDate={}&endDate={}&token={}",
            TIINGO_CRYPTO_URL,
            symbol,
            frequency,
            convert_datetime_utc_to_ymd(&chunk_start),
            convert_datetime_utc_to_ymd(&chunk_end),
            api_token
        );
        info!("Url: {}", url);

        let headers = reqwest::header::HeaderMap::new();
        let data = http_client
            .get_request::<Vec<TiingoTickerPriceData>>(url, Some(headers))
            .await?;

        if let Some(price_data) = data.first() {
            all_history.extend(price_data.price_data.clone());
        }

        chunk_start = chunk_end + Duration::days(1); // advance PAST the last fetched date, not onto it
    }

    if all_history.is_empty() {
        return Err(anyhow::anyhow!("No ticker history available"));
    }

    Ok(all_history)
}

pub async fn get_stock_etf_realtime(
    http_client: &HttpClient,
    symbol: &str,
    api_token: &str,
) -> Result<TiingoTickerRealtime> {
    let url = format!("{}{}?token={}", TIINGO_REALTIME_URL, symbol, api_token,);
    let headers = reqwest::header::HeaderMap::new();
    let realtime = http_client
        .get_request::<Vec<TiingoTickerRealtime>>(url, Some(headers))
        .await?;
    if realtime.is_empty() {
        return Err(anyhow::anyhow!("No ticker realtime available"));
    }
    let first = realtime.first().unwrap();
    Ok(first.clone())
}

fn convert_datetime_utc_to_ymd(now: &DateTime<Utc>) -> String {
    let today_utc = now.naive_utc().date();
    today_utc.format("%Y-%m-%d").to_string()
}
