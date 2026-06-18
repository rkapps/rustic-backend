use anyhow::Result;
use chrono::{Months, Utc};
use chrono_tz::US::Eastern;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal_macros::dec;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{sync::Semaphore, time::sleep};
use tracing::{debug, error, info, warn};

use rustic_ml::EmbeddingClient;
use rustic_providers::finance::service::ProviderService;

use crate::{
    core::tickers::{
        BASE_CURRENCY,
        indicators::IndicatorCalculator,
        sync::{
            should_sync_embeddings, should_sync_history, should_sync_indicators,
            should_sync_sentiments,
        },
    },
    domain::{
        Ticker, TickerControl, TickerEmbedding, TickerHistory, TickerIndicator, TickerSentiment,
        tickers::{AssetType, TICKER_PERFORMANCE_PERIODS},
    },
    storage::{
        FinanceMongoStorageReader,
        mongo::writer::FinanceMongoStorageWriter,
        reader::TickerSentimentStorageReader,
        writer::{
            TickerControlStorageWriter, TickerEmbeddingStorageWriter, TickerHistoryStorageWriter,
            TickerIndicatorStorageWriter, TickerSentimentStorageWriter, TickerStorageWriter,
        },
    },
    util::data_utils::{
        assets_cap_label, calculate_performance, get_period_close, get_period_start,
    },
};

pub async fn update_all_tickers(
    writer: Arc<FinanceMongoStorageWriter>,
    provider_service: Arc<ProviderService>,
    all_controls: Vec<TickerControl>,
    all_tickers: Vec<Ticker>,
    update: bool,
) -> Result<Vec<Ticker>> {
    // ticker control
    let mut control_map: HashMap<String, TickerControl> = all_controls
        .into_iter()
        .map(|c| (c.symbol.clone(), c))
        .collect();

    let total = all_tickers.len();
    let semaphore = Arc::new(Semaphore::new(2));
    let delay = Duration::from_millis(5000);

    info!("Processing {} tickers with 3 concurrent workers", total);

    let tasks: Vec<_> = all_tickers
        .into_iter()
        .enumerate()
        .filter_map(|(i, ticker)| {
            let tc = match control_map.remove(&ticker.symbol) {
                Some(tc) => tc,
                None => {
                    warn!("No control record for {}, skipping", ticker.symbol);
                    return None;
                }
            };

            let sem = semaphore.clone();
            let writer = writer.clone();
            let provider_service = provider_service.clone();

            Some(tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                let mut ticker = ticker;
                let mut tc = tc;

                if i % 20 == 0 {
                    info!("Updating Ticker: {} {}/{}", ticker.symbol, i + 1, total);
                }

                let result = update_ticker(
                    writer,
                    provider_service,
                    &mut tc,
                    &mut ticker,
                    update,
                )
                .await;

                sleep(delay).await;

                result.map(|_| (ticker, tc))
            }))
        })
        .collect();

    // collect results
    let results = futures::future::join_all(tasks).await;

    let mut success = 0;
    let mut failed = 0;
    let mut updated_tickers = Vec::new();
    let mut updated_controls = Vec::new();

    for result in results {
        match result {
            Ok(Ok((ticker, tc))) => {
                success += 1;
                updated_tickers.push(ticker);
                updated_controls.push(tc);
            }
            Ok(Err(e)) => {
                error!("{}", e);
                failed += 1;
            }
            Err(e) => {
                error!("Task panicked: {}", e);
                failed += 1;
            }
        }
    }

    debug!("Saving {} tickers", updated_tickers.len());

    // bulk write at the end
    if update && !updated_tickers.is_empty() {
        writer.save_tickers(updated_tickers.clone()).await?;
        writer.save_ticker_controls(updated_controls).await?;
    }

    info!("Completed: {} successful, {} failed", success, failed);
    Ok(updated_tickers)
}

pub async fn update_ticker(
    writer: Arc<FinanceMongoStorageWriter>,
    provider_service: Arc<ProviderService>,
    tc: &mut TickerControl,
    ticker: &mut Ticker,
    update: bool,
) -> Result<()> {
    match update_ticker_details(provider_service.clone(), ticker).await {
        Ok(c) => c,
        Err(e) => {
            let emsg = format!("Ticker update failed for {}: {}", ticker.symbol, e);
            error!(emsg);
            // return Err(anyhow::anyhow!(emsg));
        }
    };

    //Get the history
    let mut histories = Vec::new();

    // update history
    // if !update ||  {
    match update_ticker_history(provider_service.clone(), tc, ticker).await {
        Ok((all_histories, new_histories)) => {
            if !new_histories.is_empty() {
                info!(
                    "Ticker new History for {}: {} ",
                    ticker.symbol,
                    new_histories.len()
                );
                if update && should_sync_history(tc) {
                    tc.last_history_sync_at = Some(Utc::now());
                    writer.save_ticker_control(tc.clone()).await?;
                    writer
                        .save_ticker_history(&ticker.symbol, new_histories)
                        .await?;
                }
            }
            histories = all_histories
        }
        Err(e) => error!("History update failed for {}: {}", ticker.symbol, e),
        // }
    }

    // update technical indicators
    if !update || should_sync_indicators(tc) {
        match update_stock_indicators(tc, ticker, &histories).await {
            Ok(new_indicators) => {
                if !new_indicators.is_empty() {
                    debug!(
                        "Ticker {} New Indicators: {}",
                        ticker.symbol,
                        new_indicators.len()
                    );
                    tc.last_indicator_sync_at = Some(Utc::now());
                    if update {
                        writer.save_ticker_control(tc.clone()).await?;
                        writer
                            .save_ticker_indicators(&ticker.symbol, new_indicators)
                            .await?;
                    }
                }
            }
            Err(e) => error!("Indicators update failed for {}: {}", ticker.symbol, e),
        }
    }

    // sort by descending for updating price history
    histories.sort_by(|a, b| b.date.cmp(&a.date));

    debug!("Histories: {}", histories.len());
    update_ticker_price_history(tc, ticker, &histories).await?;
    //now calculate the performance
    update_ticker_performance(tc, ticker, &histories).await?;

    //update signals
    // update_ticker_signals(storage_service.clone(), ticker).await?;

    tc.last_sync_at = Some(Utc::now());

    Ok(())
}

pub(crate) async fn update_ticker_details(
    provider_service: Arc<ProviderService>,
    ticker: &mut Ticker,
) -> Result<()> {
    match ticker.asset_type {
        AssetType::Stock => {
            let raw = provider_service.get_stock(&ticker.symbol).await?;
            ticker.update_from_alpha(raw);
        }
        AssetType::Etf => {
            let raw = provider_service.get_etf(&ticker.symbol).await?;
            ticker.update_etf_from_alpha(raw);
        }
        AssetType::Crypto => {
            let raw = provider_service
                .get_crypto(vec![ticker.symbol.clone()])
                .await?;
            ticker.update_crypto_from_cmc(raw);
        }
    }
    debug!(
        "Updated ticker details for {} total assets: {:?} eps: {:?}",
        ticker.symbol, ticker.total_assets, ticker.eps
    );

    Ok(())
}

pub(crate) async fn update_ticker_history(
    provider_service: Arc<ProviderService>,
    tc: &mut TickerControl,
    ticker: &mut Ticker,
) -> Result<(Vec<TickerHistory>, Vec<TickerHistory>)> {
    let Some(hist_start_date) = Utc::now().checked_sub_months(Months::new(60)) else {
        return Err(anyhow::anyhow!("Error calcuating start date"));
    };

    let histories = match ticker.asset_type {
        AssetType::Stock => {
            let thist = provider_service
                .get_stock_history(&ticker.symbol, &hist_start_date)
                .await?;

            TickerHistory::from_tiingo_batch(&ticker.symbol, &ticker.exchange, thist)?
        }
        AssetType::Etf => {
            let thist = provider_service
                .get_stock_history(&ticker.symbol, &hist_start_date)
                .await?;

            TickerHistory::from_tiingo_batch(&ticker.symbol, &ticker.exchange, thist)?
        }
        AssetType::Crypto => {
            let symbol = format!("{}{}", ticker.symbol, BASE_CURRENCY);
            let mut thist = provider_service
                .get_crypto_history(&symbol, &hist_start_date, "1day")
                .await
                .inspect_err(|e| {
                    warn!("Crypto Ticker history for '{}' error: {}", ticker.symbol, e)
                })?;

            // update adj_close
            for hist in &mut thist {
                hist.adj_close = hist.close;
            }
            TickerHistory::from_tiingo_batch(&ticker.symbol, &ticker.exchange, thist)?
        }
    };

    let mut new_histories = Vec::new();
    if !histories.is_empty() {
        new_histories = match tc.last_history_sync_at {
            Some(last_sync) => {
                let last_sync_date = last_sync.with_timezone(&Eastern).date_naive();
                histories
                    .iter()
                    .filter(|h| h.date.with_timezone(&Eastern).date_naive() > last_sync_date)
                    .cloned()
                    .collect()
            }
            None => {
                // First sync - insert all
                histories.clone()
            }
        };
    }
    debug!(
        "Ticker {} New History updates: {}",
        ticker.symbol,
        new_histories.len()
    );

    Ok((histories, new_histories))
}

pub(crate) async fn update_ticker_price_history(
    _tc: &mut TickerControl,
    ticker: &mut Ticker,
    histories: &[TickerHistory],
) -> Result<()> {
    if !histories.is_empty() {
        let last_history = histories[0].clone();
        let mut prev_history = None;
        if histories.len() > 1 {
            prev_history = Some(histories[1].clone());
        }
        // update the price
        debug!(
            "Last History: {:?} close: {:?} Prev history: {:?}",
            last_history.date, last_history.close, prev_history
        );

        ticker.update_price_from_history(last_history, prev_history)?;

        debug!(
            "Price date: {:?} close: {:?} prev: {:?}",
            ticker.pr_date, ticker.pr_last, ticker.pr_prev
        );
    }

    Ok(())
}

pub(crate) async fn update_ticker_performance(
    _tc: &mut TickerControl,
    ticker: &mut Ticker,
    histories: &[TickerHistory],
) -> Result<()> {
    ticker.performance.clear();
    ticker.performance_search.clear();

    for period in TICKER_PERFORMANCE_PERIODS {
        if let Some(start_date) = get_period_start(period)
            && let Some(period_close) = get_period_close(histories, start_date)
        {
            let mut period_map = HashMap::new();
            let mut period_map_search = HashMap::new();
            period_map.insert("price".to_string(), period_close);
            period_map.insert(
                "perc".to_string(),
                calculate_performance(ticker.pr_close, period_close),
            );

            period_map_search.insert("price".to_string(), period_close.to_f64().unwrap_or(0.0));
            period_map_search.insert(
                "perc".to_string(),
                calculate_performance(ticker.pr_close, period_close)
                    .to_f64()
                    .unwrap_or(0.0),
            );

            ticker.performance.insert(period.to_string(), period_map);
            ticker
                .performance_search
                .insert(period.to_string(), period_map_search);
        }
    }
    Ok(())
}

pub async fn update_all_ticker_sentiments_embeddings(
    reader: Arc<FinanceMongoStorageReader>,
    writer: Arc<FinanceMongoStorageWriter>,
    provider_service: Arc<ProviderService>,
    embedding_client: Arc<dyn EmbeddingClient>,
    all_controls: Vec<TickerControl>,
    all_tickers: Vec<Ticker>,
    update: bool,
) -> Result<()> {
    let mut control_map: HashMap<String, TickerControl> = all_controls
        .into_iter()
        .map(|c| (c.symbol.clone(), c))
        .collect();

    let total = all_tickers.len();
    let semaphore = Arc::new(Semaphore::new(2));
    let delay = Duration::from_millis(5000);

    info!("Processing {} tickers with 3 concurrent workers", total);

    let tasks: Vec<_> = all_tickers
        .into_iter()
        .enumerate()
        .filter_map(|(i, ticker)| {
            let tc = match control_map.remove(&ticker.symbol) {
                Some(tc) => tc,
                None => {
                    warn!("No control record for {}, skipping", ticker.symbol);
                    return None;
                }
            };

            let sem = semaphore.clone();
            let reader = reader.clone();
            let writer = writer.clone();
            let provider_service = provider_service.clone();
            let embedding_client = embedding_client.clone();
            let update = update.clone();

            Some(tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                let ticker = ticker;
                let mut tc = tc;

                if i % 20 == 0 {
                    info!("Updating Ticker sentiments and embeddings: {} {}/{}", ticker.symbol, i + 1, total);
                }
                match update_ticker_sentiments_embeddings(
                    reader,
                    writer,
                    provider_service,
                    embedding_client,
                    &mut tc,
                    &ticker,
                    update,
                ).await
                {
                    Ok(c) => c,
                    Err(e) => {
                        error!("Ticker {} sentiments and embeddings error: {:?}", ticker.symbol, e);
                    },
                }

                sleep(delay).await;
            }))
        })
        .collect();

    // collect results
    let _ = futures::future::join_all(tasks).await;

    Ok(())
}

pub async fn update_ticker_sentiments_embeddings(
    reader: Arc<FinanceMongoStorageReader>,
    writer: Arc<FinanceMongoStorageWriter>,
    provider_service: Arc<ProviderService>,
    embedding_client: Arc<dyn EmbeddingClient>,
    tc: &mut TickerControl,
    ticker: &Ticker,
    update: bool,
) -> Result<()> {

    // update sentiments
    if !update || should_sync_sentiments(tc) {
        match update_ticker_sentiments(provider_service, &tc, &ticker).await {
            Ok(new_sentiments) => {
                if !new_sentiments.is_empty() {
                    debug!(
                        "Ticker {} New Sentiments: {}",
                        ticker.symbol,
                        new_sentiments.len()
                    );
                    tc.last_sentiment_sync_at = Some(Utc::now());
                    if update {
                        writer.save_ticker_control(tc.clone()).await?;
                        writer
                            .save_ticker_sentiments(&ticker.symbol, new_sentiments)
                            .await?;
                    }
                }
            }
            Err(e) => error!("Sentiments update failed for {}: {}", ticker.symbol, e),
        }
    }

    if !update || should_sync_embeddings(&tc) {
        match update_ticker_embeddings(reader, embedding_client, &tc, &ticker).await {
            Ok(new_embeddings) => {
                if !new_embeddings.is_empty() {
                    debug!(
                        "Ticker {} New Embeddings: {}",
                        ticker.symbol,
                        new_embeddings.len()
                    );
                    tc.last_embedding_sync_at = Some(Utc::now());
                    if update {
                        // storage_service.save_ticker_control(tc.clone()).await?;
                        writer
                            .save_ticker_embeddings(&ticker.symbol, new_embeddings)
                            .await?;
                    }
                }
            }
            Err(e) => error!("Embeddings update failed for {}: {}", ticker.symbol, e),
        }
    }

    Ok(())
}

pub(crate) async fn update_ticker_sentiments(
    provider_service: Arc<ProviderService>,
    tc: &TickerControl,
    ticker: &Ticker,
) -> Result<Vec<TickerSentiment>> {
    let mut new_sentiments = Vec::new();

    let Some(date_from) = Utc::now().checked_sub_months(Months::new(1)) else {
        return Err(anyhow::anyhow!("Error with DateTime"));
    };
    let feeds = provider_service
        .get_ticker_sentiment(&ticker.symbol, &date_from)
        .await?;
    let feeds_len = feeds.len();
    let all_sentiments = TickerSentiment::new_from_alpha_batch(&ticker.symbol, feeds);
    debug!(
        "Ticker {} Feeds: {} Sentiments: {}",
        ticker.symbol,
        feeds_len,
        all_sentiments.len()
    );

    let sentiments: Vec<_> = all_sentiments
        .into_iter()
        .filter(|s| s.relevance_score.abs() > 0.60)
        .collect();

    if !sentiments.is_empty() {
        new_sentiments = match tc.last_sentiment_sync_at {
            Some(last_sync) => sentiments
                .into_iter()
                .filter(|h| h.date > last_sync)
                .collect(),
            None => {
                // First sync - insert all
                sentiments
            }
        };
    }
    Ok(new_sentiments)
}

pub(crate) async fn update_ticker_embeddings(
    reader: Arc<FinanceMongoStorageReader>,
    embedding_client: Arc<dyn EmbeddingClient>,
    tc: &TickerControl,
    ticker: &Ticker,
) -> Result<Vec<TickerEmbedding>> {
    let mut new_embeddings = Vec::new();
    let cmp_score = dec!(0.8);

    let all_sentiments = reader
        .get_ticker_sentiments_with_score(vec![ticker.symbol.clone()], &cmp_score)
        .await?;

    if all_sentiments.is_empty() {
        return Ok(new_embeddings);
    }

    let sentiments: Vec<_> = all_sentiments
        .into_iter()
        .filter(|s| s.date > (Utc::now() - chrono::Duration::days(30)))
        .take(50)
        .collect();

    debug!(
        "Ticker {} Sentiments with score: {} - {}",
        ticker.symbol,
        cmp_score,
        sentiments.len()
    );

    if sentiments.is_empty() {
        return Ok(new_embeddings);
    }

    // Generate embeddings
    // Collect the owned Strings so they stay alive
    let mut embedding_texts: Vec<String> = Vec::new();
    let mut sentimentm = HashMap::new();
    for (index, sentiment) in sentiments.into_iter().enumerate() {
        let embedding_text = sentiment.embedding_text();
        embedding_texts.push(embedding_text);
        sentimentm.insert(index, sentiment);
    }

    // Create the references that point to the owned Strings
    let embedding_refs: Vec<&str> = embedding_texts.iter().map(|s| s.as_str()).collect();
    debug!(
        "Ticker {} embeddings: {}",
        ticker.symbol,
        embedding_refs.len()
    );

    let result = match embedding_client.embed_text_batch(&embedding_refs).await {
        Ok(c) => c,
        Err(e) => {
            return Err(anyhow::anyhow!("Embedding error: {}", e));
        }
    };

    let mut embeddings = Vec::new();
    for successful in result.successful {
        let Some(sentiment) = sentimentm.get(&successful.0) else {
            continue;
        };
        let embedding = TickerEmbedding::new(
            &ticker.symbol.to_uppercase(),
            sentiment.date,
            &sentiment.id,
            &sentiment.embedding_text(),
            successful.1.into_vec(),
        );
        embeddings.push(embedding);
    }
    if !embeddings.is_empty() {
        new_embeddings = match tc.last_embedding_sync_at {
            Some(last_sync) => embeddings
                .into_iter()
                .filter(|h| h.date > last_sync)
                .collect(),
            None => {
                // First sync - insert all
                embeddings
            }
        };
    }

    Ok(new_embeddings)
}

pub async fn update_all_ticker_overview_embeddings(
    writer: Arc<FinanceMongoStorageWriter>,
    embedding_client: Arc<dyn EmbeddingClient>,
    all_tickers: Vec<Ticker>,
) -> Result<()> {
    let total = all_tickers.len();
    let semaphore = Arc::new(Semaphore::new(3));
    // let delay = Duration::from_millis(1000);

    info!("Processing {} tickers with 3 concurrent workers", total);

    let tasks: Vec<_> = all_tickers
        .into_iter()
        .enumerate()
        .filter_map(|(i, ticker)| {
            let sem = semaphore.clone();
            let embedding_client = embedding_client.clone();

            Some(tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                let mut ticker = ticker;

                if i % 20 == 0 {
                    info!("Updating Ticker: {} {}/{}", ticker.symbol, i + 1, total);
                }

                let result = update_ticker_overview_embedding(embedding_client, &mut ticker).await;

                result.map(|_| ticker)
            }))
        })
        .collect();

    // collect results
    let results = futures::future::join_all(tasks).await;

    let mut success = 0;
    let mut failed = 0;
    let mut updated_tickers = Vec::new();

    for result in results {
        match result {
            Ok(Ok(ticker)) => {
                success += 1;
                updated_tickers.push(ticker);
            }
            Ok(Err(e)) => {
                error!("{}", e);
                failed += 1;
            }
            Err(e) => {
                error!("Task panicked: {}", e);
                failed += 1;
            }
        }
    }

    debug!("Saving {} tickers", updated_tickers.len());

    // bulk write at the end
    if !updated_tickers.is_empty() {
        writer.save_tickers(updated_tickers.clone()).await?;
    }

    info!("Completed: {} successful, {} failed", success, failed);
    Ok(())
}

pub async fn update_ticker_overview_embedding(
    embedding_client: Arc<dyn EmbeddingClient>,
    ticker: &mut Ticker,
) -> Result<()> {
    let assets_cap_label = assets_cap_label(ticker.total_assets);
    // adding the industry twice to increase the weight.
    let overview_text = format!(
        "{} {} {} {} {}",
        ticker.name,
        ticker.sector.as_deref().unwrap_or(""),
        ticker.industry.as_deref().unwrap_or(""),
        assets_cap_label,
        ticker.overview
    );

    match embedding_client.embed_text(&overview_text).await {
        Ok(embedding) => {
            ticker.overview_text = Some(overview_text);
            ticker.overview_embedding = Some(embedding.into_vec());
        }
        Err(e) => error!("Embedding failed for {}: {}", ticker.symbol, e),
    }

    let industry_text = ticker.industry.clone().unwrap_or("".to_string());
    match embedding_client.embed_text(&industry_text).await {
        Ok(embedding) => {
            ticker.industry_text = Some(industry_text);
            ticker.industry_embedding = Some(embedding.into_vec());
        }
        Err(e) => error!("Embedding failed for {}: {}", ticker.symbol, e),
    }

    Ok(())
}

pub(crate) async fn update_stock_indicators(
    tc: &mut TickerControl,
    ticker: &mut Ticker,
    histories: &[TickerHistory],
) -> Result<Vec<TickerIndicator>> {
    let mut new_indicators = Vec::new();

    if !histories.is_empty() {
        let sma_periods = &[20, 50, 100, 200];
        let ema_periods = &[12, 26, 50];
        let rsi_periods = &[10, 14, 26];
        let k_period = 14;
        let d_period = 3;
        let bb_period = 20;
        let bb_std_dev = 2.0;
        let atr_period = 14;
        let volume_ratio_period = 20;

        let indicators = IndicatorCalculator::calculate_all_in_one_pass(
            histories,
            sma_periods.to_vec(),
            ema_periods.to_vec(),
            rsi_periods.to_vec(),
            k_period,
            d_period,
            bb_period,
            bb_std_dev,
            atr_period,
            volume_ratio_period,
        )?;

        if let Some(last_indicator) = indicators.clone().last() {
            ticker.indicators_search = HashMap::new();
            debug!("Last indicator: {:?}", last_indicator.date);
            for value in &last_indicator.values {
                ticker
                    .indicators_search
                    .insert(value.0.clone(), value.1.to_f64().unwrap_or_default());
            }

            new_indicators = match tc.last_indicator_sync_at {
                Some(last_sync) => indicators
                    .into_iter()
                    .filter(|h| h.date > last_sync)
                    .collect(),
                None => {
                    // First sync - insert all
                    indicators
                }
            };
        }

        debug!(
            "Ticker {} Indicators updates: {:?} new indicators: {}",
            ticker.symbol,
            ticker.indicators_search,
            new_indicators.len()
        );
    }

    Ok(new_indicators)
}

pub async fn update_stocks_etfs_realtime(
    writer: Arc<FinanceMongoStorageWriter>,
    provider_service: Arc<ProviderService>,
    all_tickers: Vec<Ticker>,
    update: bool,
) -> Result<()> {
    let mut updated_tickers = Vec::new();
    let length = all_tickers.len();

    // rate limit constraints
    for (i, mut ticker) in all_tickers.into_iter().enumerate() {
        if i % 20 == 0 {
            info!(
                "Updating Ticker Realtime: {} {}/{}",
                ticker.symbol,
                i + 1,
                length
            );
        }
        match provider_service
            .get_stock_etf_realtime(&ticker.symbol)
            .await
        {
            Ok(raw) => {
                ticker.update_stock_etf_price_realtime(raw)?;
                debug!("Price: {} prev: {}", ticker.pr_last, ticker.pr_prev);
                updated_tickers.push(ticker);
            }
            Err(e) => error!("Ticker Realtime error {}: {}", ticker.symbol, e),
        };
    }

    info!(
        "Stocks and Etfs Realtime update complete: {}/{} updated",
        updated_tickers.len(),
        length
    );

    // bulk write at the end
    if update && !updated_tickers.is_empty() {
        writer.save_tickers(updated_tickers).await?;
    }

    Ok(())
}

pub async fn update_cryptos_realtime(
    writer: Arc<FinanceMongoStorageWriter>,
    provider_service: Arc<ProviderService>,
    all_tickers: Vec<Ticker>,
    update: bool,
) -> Result<()> {
    let mut updated_tickers = Vec::new();
    let length = all_tickers.len();

    let mut all_tickers_map: HashMap<String, Ticker> = all_tickers
        .iter()
        .map(|t| (t.symbol.clone(), t.clone()))
        .collect();
    let symbols: Vec<String> = all_tickers.iter().map(|t| t.symbol.clone()).collect();

    let raw = match provider_service.get_crypto(symbols).await {
        Ok(raw) => raw,
        Err(e) => {
            return Err(anyhow::anyhow!(format!("Cryptos Realtime error: {}", e)));
        }
    };
    for data in raw.data {
        if let Some(ticker) = all_tickers_map.get_mut(&data.0)
            && !data.1.is_empty()
            && let Some(cdata) = data.1.first()
            && let Some(quote) = cdata.quote.get("USD")
        {
            match ticker.update_crypto_realtime(cdata.last_updated, quote.clone()) {
                Ok(_) => {
                    debug!("Data: {} Price: {}", data.0, ticker.pr_last);
                    updated_tickers.push(ticker.clone())
                }
                Err(e) => error!("Ticker Realtime error {}: {}", ticker.symbol, e),
            };
        }
    }
    info!(
        "Cryptos Realtime update complete: {}/{} updated",
        updated_tickers.len(),
        length
    );

    // bulk write at the end
    if update && !updated_tickers.is_empty() {
        writer.save_tickers(updated_tickers).await?;
    }

    Ok(())
}
