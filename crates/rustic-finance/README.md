# rustic-finance

Finance data domain for the rustic-ai platform. Manages equity, ETF, and crypto ticker data — price history, technical indicators, news, sentiment, and vector embeddings — and exposes seven agent-callable tools so LLMs can query the data at runtime.

## What it does

- **Ticker data storage** — MongoDB collections for tickers, OHLCV history, technical indicators, news, sentiment, and embeddings
- **Data synchronisation** — Pull end-of-day prices, real-time quotes, news, and sentiment from Alpha Vantage, Tiingo, and CoinMarketCap
- **Semantic search** — Ticker embeddings for similarity-based screening
- **ML models** — Logistic regression price-direction prediction models built per-ticker from historical features
- **Agent tools** — Seven `rustic_core::Tool` implementations for LLM access to all of the above
- **MongoDB schema** — `update_finance_db` creates collections, indexes, and time-series collections

## Key types

### `FinanceService`

Top-level service; instantiated in two modes:

```rust
// Read-only mode (used by the API server)
FinanceService::new_reader(mongo_uri, mongo_db, embedding_client).await?;

// Read + write mode (used by the pipeline binary)
FinanceService::new(
    mongo_uri, mongo_db, embedding_client,
    Some(alpha_key),
    Some(tiingo_token),
    Some(coinmarketcap_key),
).await?;
```

### Agent tools

```rust
let tools: Vec<Arc<dyn Tool>> = service.tools();
```

| Tool | Name | Description |
|------|------|-------------|
| `TickerSnapshotTool` | `ticker_snapshot` | Current price, 52-week range, PE, EPS, market cap, PEG, PB, PS ratios |
| `TickerPriceHistoryTool` | `ticker_price_history` | OHLCV candlestick history for a ticker over a date range |
| `TickerIndicatorTool` | `ticker_indicator` | Technical indicators (RSI, MACD, Bollinger Bands, SMA, EMA, etc.) |
| `TickerSentimentTool` | `ticker_sentiment` | News sentiment scores and recent headlines |
| `TickerScreeningTool` | `ticker_screening` | Screen tickers by sector, market cap, fundamentals |
| `TickerPeersTool` | `ticker_peers` | Find peer companies in the same sector |
| `TickerTaxonomyTool` | `ticker_taxonomy` | Sector, industry, and sub-industry classification |

### Data synchronisation (feature `writer`)

```rust
// End-of-day prices for all tracked tickers
service.update_eod_tickers("", true).await?;

// Real-time quotes for stocks and ETFs
service.update_realtime_stocks_etfs("", true).await?;

// Real-time crypto quotes
service.update_realtime_cryptos("", true).await?;

// News articles for all tickers
service.update_tickers_news().await?;

// Sentiment scores and vector embeddings from news
service.update_eod_tickers_sentiments_embeddings("", true).await?;
```

### ML prediction models (feature `writer`)

Builds per-ticker logistic regression models that predict next-day price direction from OHLCV and indicator features:

```rust
service.build_ticker_prediction_models("AAPL,MSFT,TSLA").await?;
// empty string → build models for all tracked tickers
```

### Schema

```rust
use rustic_finance::schema::update_finance_db;

update_finance_db(&mongo_uri, &mongo_db).await?;
```

## Domain modules

```text
domain/tickers/
  ticker.rs        — Ticker master record (symbol, name, sector, fundamentals)
  history.rs       — OHLCV price record
  indicator.rs     — Technical indicator record
  news.rs          — News article
  sentiment.rs     — Sentiment score
  embedding.rs     — Vector embedding for semantic search
  control.rs       — Sync control record (last synced at, etc.)

tools/             — Agent-callable Tool impls (one per tool listed above)
ml/                — Feature engineering, logistic regression trainer/predictor
core/              — Ticker sync orchestration and chart/indicator computation
storage/           — MongoDB reader and writer traits + implementations
```

## Data providers

| Provider | Dependency | Used for |
|----------|------------|---------|
| Alpha Vantage | `ALPHA_API_KEY` | EOD prices, fundamentals, news sentiment |
| Tiingo | `TIINGO_API_TOKEN` | Real-time stock / ETF quotes, news |
| CoinMarketCap | `COINMARKETCAP_API_KEY` | Real-time crypto quotes |

## Dependencies

- `rustic-core` — `Tool` trait
- `rustic-ml` — `EmbeddingClient` for ticker embeddings
- `rustic-providers` — `ProviderService` (Alpha Vantage, Tiingo, CoinMarketCap clients)
- `rustic-storage` — `MongoDatabase`, `Repository` trait
