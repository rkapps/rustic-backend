use chrono::{DateTime, Utc};

/// A composable query descriptor shared by all storage backends.
///
/// Build a `SearchCriteria` incrementally with [`add_condition`],
/// [`add_sort`], and [`add_limit`], then pass it to
/// [`Repository::find`](crate::core::repository::Repository::find) or
/// [`Repository::delete_many`](crate::core::repository::Repository::delete_many).
///
/// An empty `SearchCriteria` (the `Default`) matches every record.
#[derive(Debug, Clone, Default)]
pub struct SearchCriteria {
    pub conditions: Vec<SearchCondition>,
    pub sort_fields: Option<Vec<SortField>>,
    pub limit: Option<usize>,
}

/// A single predicate applied to one document field.
#[derive(Debug, Clone)]
pub struct SearchCondition {
    pub field: String,
    pub operator: SearchOp,
    pub value: SearchValue,
}

/// Comparison operators supported by the query DSL.
///
/// Both the file backend (in-memory) and the MongoDB backend translate these
/// to their respective filter representations, so queries written against
/// `SearchCriteria` are portable across backends.
#[derive(Debug, Clone)]
pub enum SearchOp {
    /// Field equals value (`$eq` in MongoDB).
    Eq,
    /// Field does not equal value (`$ne`).
    Ne,
    /// Field is strictly greater than value (`$gt`).
    Gt,
    /// Field is greater than or equal to value (`$gte`).
    Gte,
    /// Field is strictly less than value (`$lt`).
    Lt,
    /// Field is less than or equal to value (`$lte`).
    Lte,
    /// Field value is contained in the given array (`$in`).
    In,
    /// Array field contains all elements in the given array (`$all`).
    All,
    /// Field value is not contained in the given array (`$nin`).
    NotIn,
    /// Field value matches the given string (case-insensitive regex, `$regex`).
    Contains,
    /// Field exists (or does not exist) in the document (`$exists`).
    Exists,
}

/// The right-hand side of a [`SearchCondition`].
#[derive(Debug, Clone)]
pub enum SearchValue {
    String(String),
    Decimal(rust_decimal::Decimal),
    Int(i64),
    Bool(bool),
    /// Used with [`SearchOp::In`], [`SearchOp::NotIn`], and [`SearchOp::All`].
    Array(Vec<String>),
    DateTime(DateTime<Utc>),
}

/// Specifies a sort key and its direction.
#[derive(Debug, Clone)]
pub struct SortField {
    pub field: String,
    /// `true` = ascending (A → Z, 0 → 9), `false` = descending.
    pub ascending: bool,
}

impl SearchCriteria {
    /// Create an empty criteria that matches all records.
    pub fn new() -> SearchCriteria {
        SearchCriteria {
            conditions: Vec::new(),
            sort_fields: None,
            limit: None,
        }
    }

    /// Append a filter condition.  Multiple conditions are ANDed together.
    pub fn add_condition(&mut self, field: &str, operator: SearchOp, value: SearchValue) {
        self.conditions.push(SearchCondition {
            field: field.to_string(),
            operator,
            value,
        })
    }

    /// Append a sort key.  Multiple sort fields are applied in insertion order.
    pub fn add_sort(&mut self, field: &str, ascending: bool) {
        let sort_field = SortField {
            field: field.to_string(),
            ascending,
        };

        self.sort_fields.get_or_insert(Vec::new()).push(sort_field);
    }

    /// Cap the number of returned records.  Only the first call has any effect;
    /// subsequent calls are ignored to prevent accidental override.
    pub fn add_limit(&mut self, limit: usize) {
        self.limit.get_or_insert(limit);
    }
}
