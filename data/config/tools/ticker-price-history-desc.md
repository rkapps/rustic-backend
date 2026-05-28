# Ticker PriceHistory

Returns daily price history for a stock ticker. \
 Use this when the user asks for price action or daily trading history. \
 Map user requests to periods: 'last week' = 7 days, 'last month' = 30 days, \
 'last 3 months' = 90 days. \
 Do not use this for period returns or performance — \
 use get_ticker_snapshot which has pre-computed performance data.
