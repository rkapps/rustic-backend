use crate::core::search::{SearchCriteria, SearchOp, SearchValue};
use bson::{Bson, DateTime as BsonDateTime, Document, doc};
use rust_decimal::prelude::ToPrimitive;

/// Translates the backend-agnostic [`SearchCriteria`] DSL into MongoDB BSON
/// documents suitable for use as query filters and sort specifiers.
pub struct MongoCriteriaBuilder;

impl MongoCriteriaBuilder {
    /// Build a BSON filter document from `criteria`.
    ///
    /// Multiple conditions targeting the same field are merged into a single
    /// subdocument (e.g. `{"price": {"$gte": 100, "$lte": 500}}`), which is
    /// required for MongoDB range queries to work correctly.
    pub fn build_filter(criteria: &SearchCriteria) -> Document {
        let mut filter = doc! {};
        for condition in criteria.conditions.iter() {
            let key = condition.field.clone();
            let bson_value = MongoCriteriaBuilder::to_bson(condition.value.clone());

            match condition.operator {
                // ✅ Equality: insert value directly
                SearchOp::Eq => {
                    filter.insert(key, bson_value);
                }

                SearchOp::Gte | SearchOp::Lte | SearchOp::Gt | SearchOp::Lt | SearchOp::Ne => {
                    let operator_key = match condition.operator {
                        SearchOp::Gte => "$gte",
                        SearchOp::Lte => "$lte",
                        SearchOp::Gt => "$gt",
                        SearchOp::Lt => "$lt",
                        SearchOp::Ne => "$ne",
                        _ => unreachable!(),
                    };

                    // Check if field already has operators
                    if let Some(existing) = filter.get_mut(&key) {
                        // Field exists - merge operator into existing document
                        if let Bson::Document(subdoc) = existing {
                            subdoc.insert(operator_key, bson_value);
                        }
                    } else {
                        // Field doesn't exist - create new subdocument
                        filter.insert(key, doc! { operator_key: bson_value });
                    }
                }
                // // ✅ Other operators: wrap in MongoDB operator syntax
                // SearchOp::Ne => {
                //     filter.insert(key, doc! { "$ne": bson_value });
                // }
                // SearchOp::Gt => {
                //     filter.insert(key, doc! { "$gt": bson_value });
                // }
                // SearchOp::Gte => {
                //     filter.insert(key, doc! { "$gte": bson_value });
                // }
                // SearchOp::Lt => {
                //     filter.insert(key, doc! { "$lt": bson_value });
                // }
                // SearchOp::Lte => {
                //     filter.insert(key, doc! { "$lte": bson_value });
                // }
                SearchOp::In => {
                    filter.insert(key, doc! { "$in": bson_value });
                }
                SearchOp::NotIn => {
                    filter.insert(key, doc! { "$nin": bson_value });
                }
                SearchOp::All => {
                    filter.insert(key, doc! { "$all": bson_value });
                }
                SearchOp::Contains => {
                    // Case-insensitive regex search
                    filter.insert(
                        key,
                        doc! {
                            "$regex": bson_value,
                            "$options": "i"
                        },
                    );
                }
                // ✅ EXISTS - Check if field exists
                SearchOp::Exists => {
                    // Value should be a boolean (true = exists, false = doesn't exist)
                    let exists_value = match bson_value {
                        Bson::Boolean(b) => b,
                        _ => true, // Default to true if not a boolean
                    };
                    filter.insert(key, doc! { "$exists": exists_value });
                }
            }
        }

        filter
    }

    fn to_bson(value: SearchValue) -> Bson {
        match value {
            SearchValue::String(s) => Bson::String(s),
            // Use your string_to_decimal logic or native Decimal128 if configured
            SearchValue::Decimal(d) => Bson::Double(d.to_f64().unwrap()),
            SearchValue::Int(i) => Bson::Int64(i),
            SearchValue::Bool(b) => Bson::Boolean(b),
            SearchValue::DateTime(dt) => Bson::DateTime(BsonDateTime::from_chrono(dt)), // ✅
            SearchValue::Array(arr) => {
                let bson_arr = arr.into_iter().map(Bson::String).collect();
                Bson::Array(bson_arr)
            }
        }
    }

    /// Build a BSON sort document from the `sort_fields` of `criteria`.
    ///
    /// Each field maps to `1` (ascending) or `-1` (descending) as required
    /// by MongoDB's `sort()` option.
    pub fn build_sort(search: &SearchCriteria) -> Document {
        let mut sort = doc! {};
        if let Some(fields) = &search.sort_fields {
            for field in fields {
                let val = if field.ascending { 1 } else { -1 };
                sort.insert(field.field.clone(), val);
            }
        };

        sort
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use anyhow::Result;

    #[test]
    fn test_builder_filter_equality() -> Result<()> {
        let sector = "Technology";
        let mut criteria = SearchCriteria::new();
        criteria.add_condition(
            "sector",
            SearchOp::Eq,
            SearchValue::String(sector.to_string()),
        );
        let filter = MongoCriteriaBuilder::build_filter(&criteria);

        let expected = doc! { "sector": "Technology" };
        assert_eq!(filter, expected);

        Ok(())
    }

    #[test]
    fn test_build_filter_greater_than_or_equal() {
        let min_cap = 100000000;
        let mut criteria = SearchCriteria::new();
        criteria.add_condition("market_cap", SearchOp::Gte, SearchValue::Int(min_cap));
        let filter = MongoCriteriaBuilder::build_filter(&criteria);

        let expected = doc! {
            "market_cap": {
                "$gte": Bson::Int64(100000000)  // Explicit Int64
            }
        };
        assert_eq!(filter, expected);
    }

    #[test]
    fn test_build_filter_range_gte_and_lte() {
        // Arrange: Range query (price between 100 and 500)
        let mut criteria = SearchCriteria::new();

        criteria.add_condition(
            "symbol",
            SearchOp::Eq,
            SearchValue::String("AAPL".to_string()),
        );
        criteria.add_condition("price", SearchOp::Gte, SearchValue::Int(100));
        criteria.add_condition("price", SearchOp::Lte, SearchValue::Int(500));

        // Act
        let filter = MongoCriteriaBuilder::build_filter(&criteria);

        // Assert: Should have BOTH operators
        let expected = doc! {
            "symbol" : "AAPL",
            "price": {
                "$gte": 100_i64,
                "$lte": 500_i64
            }
        };
        println!("{}", expected);

        assert_eq!(filter, expected);
    }
}
