use anyhow::Result;
use std::{collections::HashMap, sync::Arc};
use tracing::debug;

use rustic_ml::{EmbeddingClient, search};

use crate::{
    domain::{
        TickerEmbedding, TickerSentiment, dto::ticker_sentiment_entity::TickerSentimentEntity,
    },
    storage::reader::StorageReader,
};

// "earnings outlook growth guidance"
// "analyst price target upgrade downgrade"
// "AI demand chip supply competition"
// "insider trading institutional holdings"
// "regulatory risk antitrust lawsuit"
// "dividend buyback capital allocation"
// "debt credit rating financial health"
// "product launch market share competition"
pub async fn search_ticker_sentiments(
    reader: Arc<dyn StorageReader>,
    embedding_client: Arc<dyn EmbeddingClient>,
    symbols: Vec<String>,
    query: String,
    limit: usize,
) -> Result<Vec<TickerSentimentEntity>> {
    let query_embeddings = embedding_client.embed_text(&query).await?.into_vec();
    debug!(
        "Query: {} embedding length: {}",
        query,
        query_embeddings.len()
    );
    let embeddings = reader.get_ticker_embeddings(symbols).await?;
    debug!("Screened stocks from initial search: {}", embeddings.len());

    //build the candidates from the embeddings for apple stock
    let candidates: Vec<(TickerEmbedding, Vec<f32>)> = embeddings
        .into_iter()
        .map(|entry| (entry.clone(), entry.vector.clone()))
        .collect();

    if !candidates.is_empty() && !query_embeddings.is_empty() {
        debug!(
            "Candidates embedding length: {}",
            candidates.first().unwrap().1.len()
        );
        // top 5 similarit results from vector_search
        let embedding_results = search::<TickerEmbedding>(&query_embeddings, &candidates, limit);
        debug!("matched results: {}", embedding_results.len());

        // 2. collect sentiment ids
        let sentiment_ids: Vec<String> = embedding_results
            .iter()
            .map(|(e, _)| e.sentiment_id.clone())
            .collect();

        debug!("sentiment Ids: {:?}", sentiment_ids);

        let sentiments = reader.get_ticker_sentiments_by_ids(sentiment_ids).await?;
        debug!("sentiment: {:?}", sentiments.len());
        let entities = join_sentiment_with_similarity(embedding_results, sentiments);
        return Ok(entities);
    }

    Ok(Vec::new())
}

fn join_sentiment_with_similarity(
    embedding_results: Vec<(TickerEmbedding, f32)>,
    sentiments: Vec<TickerSentiment>,
) -> Vec<TickerSentimentEntity> {
    // build lookup map from sentiment_id -> TickerSentiment
    let sentiment_map: HashMap<String, TickerSentiment> =
        sentiments.into_iter().map(|s| (s.id.clone(), s)).collect();

    embedding_results
        .into_iter()
        .filter_map(|(embedding, similarity)| {
            sentiment_map
                .get(&embedding.sentiment_id)
                .map(|sentiment| TickerSentimentEntity {
                    id: sentiment.id.clone(),
                    date: sentiment.date,
                    symbol: embedding.symbol,
                    title: sentiment.title.clone(),
                    score: sentiment.score,
                    label: sentiment.label.clone(),
                    source: sentiment.source.clone(),
                    similarity,
                })
        })
        .collect()
}
