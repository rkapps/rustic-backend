# rustic-pipeline

Data ingestion pipeline CLI. Pulls financial and economic data from external APIs and upserts it into MongoDB.

## Commands

```bash
cargo run --bin rustic-pipeline -- <COMMAND>
```

| Command | Description |
|---------|-------------|
| `update-economic-data` | Run all three economic pipelines: FRED series, BEA NIPA/regional tables, Census ACS data |
| `update-tickers-eod` | End-of-day OHLCV prices for all tracked tickers (stocks, ETFs, crypto) |
| `update-tickers-sentiments-embeddings` | News sentiment scores and vector embeddings for all tickers |
| `update-stocks-etfs-realtime` | Real-time quotes for stocks and ETFs (via Tiingo) |
| `update-cryptos-realtime` | Real-time crypto quotes (via CoinMarketCap) |
| `update-tickers-news` | Latest news articles for all tracked tickers |
| `build-tickers-prediction-models` | Build per-ticker logistic regression price-direction models |
| `check-env` | Verify required environment variables are set |

`build-tickers-prediction-models` accepts an optional `--symbols AAPL,MSFT` flag to limit which tickers are processed.

## Environment variables

| Variable | Purpose |
|----------|---------|
| `MONGO_URI` | MongoDB connection string |
| `RUSTIC_FINANCE_DB_NAME` | Finance data database |
| `RUSTIC_ECONOMIC_DB_NAME` | Economic data database |
| `OPENAI_API_KEY` | Used for ticker embeddings |
| `ALPHA_API_KEY` | Alpha Vantage (EOD prices, fundamentals, news) |
| `TIINGO_API_TOKEN` | Tiingo (real-time stocks / ETFs) |
| `COINMARKETCAP_API_KEY` | CoinMarketCap (real-time crypto) |
| `FRED_API_KEY` | St. Louis Fed FRED API |
| `BEA_API_KEY` | Bureau of Economic Analysis API |
| `CENSUS_API_KEY` | U.S. Census Bureau API |
| `RUST_LOG` | Log filter |

## Running in production

The pipeline is intended to be run on a schedule (cron / Cloud Scheduler). Typical schedules:

| Command | Suggested cadence |
|---------|------------------|
| `update-tickers-eod` | Daily, after market close |
| `update-stocks-etfs-realtime` | Every 5 minutes during market hours |
| `update-cryptos-realtime` | Every 5 minutes |
| `update-tickers-news` | Hourly |
| `update-tickers-sentiments-embeddings` | Daily, after EOD update |
| `update-economic-data` | Weekly |
| `build-tickers-prediction-models` | Weekly |
