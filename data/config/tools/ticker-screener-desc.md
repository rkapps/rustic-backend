# Ticker Screener

Finds and filters stocks or ETFs based on any combination of semantic query, technical signals, industry, market cap, and asset type.
ALWAYS use this tool when the user asks to find, compare, or screen stocks by theme, category, or condition.
Do not use ticker_peers for theme or category queries — use this tool instead.
Always call ticker_taxonomy before this tool when the query mentions a specific industry or sector.

## Query Guidelines

- Pass the user's original query text unchanged. Never replace with generic terms like "find stocks".
- Only include total_assets_range if the user explicitly mentions a market cap size. Never infer it.
- When the query implies a specific industry, populate both query and industry fields.

## Examples

| User query | tool input |
|---|---|
| "software infrastructure mid cap stocks" | query: "software infrastructure", industry: "Software - Infrastructure", assets_cap_range: "mid" |
| "find oversold cloud security companies" | query: "cloud security", signals: ["RSI Oversold"] |
| "mostly oversold" or "heavily oversold" | signals: ["Deeply Oversold"] |
| "defensive buy rated stocks" | signals: ["Low Beta", "Analyst Buy"] |
| "compare spider ETFs" | query: "SPDR ETFs", asset_type: "etf" |

## Signal Rules

Pass ONE signal per category. Results must match ALL signals provided.

| Category | Signals |
|---|---|
| Trend | Golden Cross, Death Cross, Above SMA50, Below SMA50 |
| Momentum | MACD Bullish Crossover, MACD Bearish Crossover |
| RSI | RSI Oversold, RSI Overbought, Deeply Oversold, Mostly Oversold |
| Bands | BB Breakout Upper, BB Breakout Lower, BB Squeeze |
| Stochastic | Stochastic Bullish, Stochastic Bearish |
| Analyst | Analyst Strong Buy, Analyst Buy, Analyst Hold, Analyst Sell, Analyst Strong Sell |
| Beta | Low Beta, Market Beta, High Beta, Very High Beta |
