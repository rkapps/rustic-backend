use chrono::{Duration, Utc};

use crate::domain::TickerControl;


// sync_sentiments return true if not updated in 24 hours
pub(crate) fn should_sync_sentiments(tc: &TickerControl) -> bool {
    if let Some(last_sync) = tc.last_sentiment_sync_at {
        return Utc::now() - last_sync > Duration::hours(24);
    }
    true
}

// sync_embeddings return true if not updated in 24 hours
pub(crate) fn should_sync_embeddings(tc: &TickerControl) -> bool {
    if let Some(last_sync) = tc.last_embedding_sync_at {
        return Utc::now() - last_sync > Duration::hours(24);
    }
    true
}
