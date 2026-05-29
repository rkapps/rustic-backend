# rustic-providers

Async Rust clients for U.S. economic data APIs, unified behind a single trait and composable via a service facade.

## Providers

| Client | Source | Series ID format |
|---|---|---|
| `FredClient` | [St. Louis Fed (FRED)](https://fred.stlouisfed.org) | `"CPIAUCSL"` |
| `BeaClient` | [Bureau of Economic Analysis](https://apps.bea.gov) | `"TABLE:SERIES_CODE"` |
| `CensusClient` | [U.S. Census Bureau](https://www.census.gov/data/developers) | `"YEAR/DATASET/VARIABLE/GEO"` |

All three implement the `EconomicProvider` trait and return the same `SeriesData` type, so they are interchangeable in generic code.

## Quick start

```rust
use std::sync::Arc;
use rustic_providers::{EconomicDataService, FredClient, BeaClient, CensusClient};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let service = EconomicDataService::builder()
        .with_fred(Arc::new(FredClient::new(std::env::var("FRED_API_KEY")?)?))
        .with_bea(Arc::new(BeaClient::new(std::env::var("BEA_API_KEY")?)?))
        .with_census(Arc::new(CensusClient::new(std::env::var("CENSUS_API_KEY")?)?))
        .build();

    // Last 12 months of CPI (monthly)
    let cpi = service.fred_series("CPIAUCSL", Some("m"), Some(12)).await?;

    // Personal Income from NIPA table T20100
    let income = service.bea_series("T20100:A065RC", Some("A"), None).await?;

    // Median household income by state (ACS 1-year, 2023)
    let state_income = service.census_series("2023/acs1/B19013_001E/state:*", None, None).await?;

    println!("{} CPI points, {} BEA points, {} Census records",
        cpi.data_points.len(), income.data_points.len(), state_income.data_points.len());
    Ok(())
}
```

## API keys

| Provider | Signup |
|---|---|
| FRED | <https://fred.stlouisfed.org/docs/api/api_key.html> |
| BEA | <https://apps.bea.gov/API/signup/> |
| Census | <https://api.census.gov/data/key_signup.html> |

Set keys as environment variables: `FRED_API_KEY`, `BEA_API_KEY`, `CENSUS_API_KEY`.

## FRED

`FredClient` fetches observations and series metadata concurrently.

```rust
use rustic_providers::FredClient;

let client = FredClient::new(api_key)?;

// Observations + metadata together
let data = client.get_series("UNRATE", Some("m"), Some(24)).await?;

// Observations only
let obs = client.get_observations("CPIAUCSL", Some("m"), Some(12)).await?;

// Metadata only
let info = client.get_series_info("GDP").await?;
```

Frequency codes: `"d"` daily, `"w"` weekly, `"m"` monthly, `"q"` quarterly, `"a"` annual.

## BEA

`BeaClient` wraps the NIPA (national accounts) and Regional (state/county) datasets.

```rust
use rustic_providers::BeaClient;

let client = BeaClient::new(api_key)?;

// NIPA table — annual GDP rows
let rows = client.get_nipa("T10101", "A", "2024").await?;

// Regional — personal income for all states
let regional = client.get_regional("CAINC1", "1", "STATE", "2023").await?;

// Via EconomicProvider trait — TABLE:SERIES_CODE format
let data = client.get_series("T20100:A065RC", Some("A"), Some(5)).await?;
```

Common NIPA tables: `T10101` (GDP), `T20100` (Personal Income and Outlays).  
Common Regional tables: `CAINC1` (state income), `CAINC4` (county income).

## Census

`CensusClient` wraps the ACS, SAIPE poverty, and International Trade endpoints.

```rust
use rustic_providers::CensusClient;

let client = CensusClient::new(api_key)?;

// ACS 1-year — median household income by state
let records = client.get_acs("2023", "acs1", &["NAME", "B19013_001E"], "state:*").await?;

// SAIPE poverty estimates
let poverty = client.get_cps("2022", &["NAME", "SAEPOVRTALL_PT"], "state:*").await?;

// Via EconomicProvider trait — YEAR/DATASET/VARIABLE/GEO format
let data = client.get_series("2023/acs1/B19013_001E/state:*", None, Some(10)).await?;
```

Common ACS variables: `B19013_001E` (median income), `B01003_001E` (population),
`B17001_002E` (poverty), `B23025_005E` (unemployed), `B25077_001E` (home value).

## Data types

```rust
pub struct SeriesData {
    pub series_id: String,      // provider-specific ID
    pub title: Option<String>,  // populated for FRED
    pub frequency: String,      // echoed from request
    pub units: Option<String>,  // populated for FRED
    pub data_points: Vec<DataPoint>,
    pub provider: String,       // "fred" | "bea" | "census"
}

pub struct DataPoint {
    pub date: String,   // ISO-8601 or provider-specific ("2024-01", "2023", etc.)
    pub value: f64,
}
```

## Running tests

Integration tests require real API keys:

```bash
FRED_API_KEY=... BEA_API_KEY=... CENSUS_API_KEY=... \
  cargo test -p rustic-providers -- --ignored
```
