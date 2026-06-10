use anyhow::Result;
use chrono::{DateTime, Utc};
use rustic_core::HttpClient;

use crate::finance::{
    alpha::{
        api::{get_etf, get_stock, get_stock_sentiments},
        model::{AlphaEtf, AlphaTicker, AlphaTickerSentimentFeed},
    },
    cmc::{api::get_crypto, model::CmcCryptoData},
    tiingo::{
        api::{get_crypto_history, get_stock_etf_realtime, get_stock_history, get_ticker_news},
        model::{TiingoTickerHistory, TiingoTickerNews, TiingoTickerRealtime},
    },
};

#[derive(Debug, Clone)]
pub struct ProviderService {
    http_client: HttpClient,
    alpha_key: String,
    tiingo_token: String,
    coinmarketcap_key: String,
}

impl ProviderService {
    pub fn new(alpha_key: &str, tiingo_token: &str, coinmarketcap_key: &str) -> Result<Self> {
        let http_client = HttpClient::new()?;
        Ok(ProviderService {
            http_client,
            alpha_key: alpha_key.to_string(),
            tiingo_token: tiingo_token.to_string(),
            coinmarketcap_key: coinmarketcap_key.to_string(),
        })
    }

    pub async fn get_crypto(&self, symbols: Vec<String>) -> Result<CmcCryptoData> {
        let raw = get_crypto(&self.http_client, symbols, &self.coinmarketcap_key).await?;
        Ok(raw)
    }

    pub async fn get_crypto_history(
        &self,
        symbol: &str,
        start_date: &DateTime<Utc>,
        frequency: &str,
    ) -> Result<Vec<TiingoTickerHistory>> {
        get_crypto_history(
            &self.http_client,
            symbol,
            &self.tiingo_token,
            start_date,
            frequency,
        )
        .await
    }

    pub async fn get_etf(&self, symbol: &str) -> Result<AlphaEtf> {
        let raw = get_etf(&self.http_client, symbol, &self.alpha_key).await?;
        Ok(raw)
    }

    pub async fn get_stock_etf_realtime(&self, symbol: &str) -> Result<TiingoTickerRealtime> {
        let realtime =
            get_stock_etf_realtime(&self.http_client, symbol, &self.tiingo_token).await?;
        Ok(realtime)
    }

    pub async fn get_stock(&self, symbol: &str) -> Result<AlphaTicker> {
        let raw = get_stock(&self.http_client, symbol, &self.alpha_key).await?;
        Ok(raw)
    }

    pub async fn get_stock_history(
        &self,
        symbol: &str,
        start_date: &DateTime<Utc>,
    ) -> Result<Vec<TiingoTickerHistory>> {
        get_stock_history(&self.http_client, symbol, &self.tiingo_token, start_date).await
    }

    pub async fn get_ticker_sentiment(
        &self,
        symbol: &str,
        date_from: &DateTime<Utc>,
    ) -> Result<Vec<AlphaTickerSentimentFeed>> {
        let feeds =
            get_stock_sentiments(&self.http_client, symbol, &self.alpha_key, date_from).await?;
        Ok(feeds)
    }

    pub async fn get_ticker_news(&self, symbol: &str) -> Result<Vec<TiingoTickerNews>> {
        let feeds = get_ticker_news(&self.http_client, symbol, &self.tiingo_token).await?;
        Ok(feeds)
    }
}
