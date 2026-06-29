# rustic-economic

Economic data domain for the rustic-ai platform. Ingests U.S. macroeconomic time series from FRED, BEA, and Census Bureau into MongoDB, and exposes three agent-callable tools so LLMs can query the data at runtime.

## What it does

- **Data ingestion** — Pipeline functions to fetch and upsert FRED series, BEA NIPA / regional tables, and Census ACS / SAIPE datasets into MongoDB
- **Agent tools** — `FredDataTool`, `BeaDataTool`, `CensusDataTool` implement `rustic_core::Tool` so they can be registered with any agent
- **MongoDB schema** — `update_economic_db` creates the required collections and indexes

## Key types

### `EconomicService`

The top-level service; instantiated in two modes:

```rust
// Read-only mode (used by the API server)
EconomicService::new_reader(mongo_uri, mongo_db).await?;

// Read + write mode (used by the pipeline binary)
EconomicService::new(
    mongo_uri, mongo_db,
    Some(fred_api_key),
    Some(bea_api_key),
    Some(census_api_key),
).await?;
```

### Agent tools (feature `reader`)

```rust
let service = EconomicService::new_reader(mongo_uri, mongo_db).await?;
let tools: Vec<Arc<dyn Tool>> = service.tools();
// Returns: [FredDataTool, BeaDataTool, CensusDataTool]
```

| Tool | Name | Description |
|------|------|-------------|
| `FredDataTool` | `fred_series` | Fetch Federal Reserve time series (CPI, unemployment, interest rates, …) |
| `BeaDataTool` | `bea_data` | Fetch BEA national/regional economic accounts (GDP, personal income, …) |
| `CensusDataTool` | `census_data` | Fetch Census ACS demographic and socioeconomic data |

### Data ingestion (feature `writer`)

```rust
// FRED — upsert a single series
service.update_fred_series("CPIAUCSL", "m", 120, "CPI", "inflation", &["macro"]).await?;

// BEA NIPA — upsert a table
service.update_bea_nipa("T10101", "A", "LAST5").await?;

// BEA Regional — upsert state-level data
service.update_bea_regional(&[("CAINC1", "50")], "STATE", &["2023"]).await?;

// Census — upsert ACS variables
service.update_census("acs1", &["B19013_001E", "B01003_001E"], vec!["2023"]).await?;
```

### Pipeline

`EconomicDataPipeline` wraps `EconomicService` and runs all three data sources in sequence:

```rust
use rustic_economic::pipeline::EconomicDataPipeline;

let pipeline = EconomicDataPipeline::new(Arc::new(service));
pipeline.run_fred(false).await?;
pipeline.run_bea(false).await?;
pipeline.run_census(false).await?;
```

### Schema

```rust
use rustic_economic::schema::update_economic_db;

update_economic_db(&mongo_uri, &mongo_db).await?;
```

## Cargo features

| Feature | Enables |
|---------|---------|
| `reader` | `EconomicService::tools()` and storage reader types |
| `writer` | Ingestion methods on `EconomicService`, `EconomicDataPipeline` |

## Dependencies

- `rustic-core` — `Tool` trait
- `rustic-providers` — `FredClient`, `BeaClient`, `CensusClient`
- `rustic-storage` — `MongoDatabase`, `Repository` trait
