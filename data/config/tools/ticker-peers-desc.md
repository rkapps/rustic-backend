# Ticker Peers

Returns peer stocks for a given ticker across three dimensions: industry peers (same industry), sector peers (same sector), \
 and similar stocks (pre-computed embedding similarity on business description). \
 ALWAYS call this tool first before fetching any peer data. \
 Never use training knowledge to assume peers. \
 Use the returned symbols to decide which stocks to analyse further.
