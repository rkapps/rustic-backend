# rustic-admin

Administrative CLI for schema management and data seeding.

## Commands

```bash
cargo run --bin rustic-admin -- <COMMAND>
```

| Command | Description |
|---------|-------------|
| `update-schema` | Create / update MongoDB indexes for all three databases (platform, finance, economic) |
| `load-tickers --file <path>` | Seed the finance database with ticker master records from a spreadsheet (`.xlsx`) |
| `check-env` | Verify required environment variables are set |

### `update-schema`

Runs `update_rustic_platform`, `update_economic_db`, and `update_finance_db` in sequence. Safe to re-run — uses upsert-style index creation.

```bash
MONGO_URI=... \
RUSTIC_PLATFORM_DB_NAME=rustic_platform \
RUSTIC_ECONOMIC_DB_NAME=rustic_economic \
RUSTIC_FINANCE_DB_NAME=rustic_finance   \
cargo run --bin rustic-admin -- update-schema
```

### `load-tickers`

Reads a `.xlsx` file and upserts ticker master records (symbol, name, sector, industry, exchange, asset type) into the finance database:

```bash
MONGO_URI=... RUSTIC_FINANCE_DB_NAME=rustic_finance \
cargo run --bin rustic-admin -- load-tickers --file tickers.xlsx
```

## Environment variables

| Variable | Purpose |
|----------|---------|
| `MONGO_URI` | MongoDB connection string |
| `RUSTIC_PLATFORM_DB_NAME` | Platform database (conversations, turns) |
| `RUSTIC_FINANCE_DB_NAME` | Finance database |
| `RUSTIC_ECONOMIC_DB_NAME` | Economic database |
| `RUST_LOG` | Log filter |
