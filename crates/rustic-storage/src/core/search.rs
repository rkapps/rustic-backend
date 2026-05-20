use chrono::{DateTime, Utc};

/// A composable query descriptor shared by all storage backends.
///
/// Built with a fluent builder — each method consumes `self` and returns `Self`
/// so conditions can be chained:
///
/// ```
/// # use rustic_storage::core::search::{SearchCriteria, SearchOp, SearchValue};
/// let criteria = SearchCriteria::new()
///     .with_condition("uid", SearchOp::Eq, SearchValue::String("u1".into()))
///     .with_sort("created_at", false)
///     .with_limit(20);
/// ```
///
/// For optional conditions, reassign the builder:
///
/// ```
/// # use rustic_storage::core::search::{SearchCriteria, SearchOp, SearchValue};
/// # let llm: Option<String> = None;
/// let mut criteria = SearchCriteria::new()
///     .with_condition("uid", SearchOp::Eq, SearchValue::String("u1".into()));
/// if let Some(llm) = llm {
///     criteria = criteria.with_condition("llm", SearchOp::Eq, SearchValue::String(llm));
/// }
/// ```
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

// --- From impls so callers pass plain Rust values instead of SearchValue variants ---

impl From<String> for SearchValue {
    fn from(s: String) -> Self { SearchValue::String(s) }
}
impl From<&str> for SearchValue {
    fn from(s: &str) -> Self { SearchValue::String(s.to_string()) }
}
impl From<i64> for SearchValue {
    fn from(i: i64) -> Self { SearchValue::Int(i) }
}
impl From<bool> for SearchValue {
    fn from(b: bool) -> Self { SearchValue::Bool(b) }
}
impl From<DateTime<Utc>> for SearchValue {
    fn from(dt: DateTime<Utc>) -> Self { SearchValue::DateTime(dt) }
}
impl From<Vec<String>> for SearchValue {
    fn from(v: Vec<String>) -> Self { SearchValue::Array(v) }
}
impl From<rust_decimal::Decimal> for SearchValue {
    fn from(d: rust_decimal::Decimal) -> Self { SearchValue::Decimal(d) }
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

    // --- filter methods ---

    /// Field equals `value`.
    pub fn eq(self, field: &str, value: impl Into<SearchValue>) -> Self {
        self.push(field, SearchOp::Eq, value.into())
    }

    /// Field does not equal `value`.
    pub fn ne(self, field: &str, value: impl Into<SearchValue>) -> Self {
        self.push(field, SearchOp::Ne, value.into())
    }

    /// Field is strictly greater than `value`.
    pub fn gt(self, field: &str, value: impl Into<SearchValue>) -> Self {
        self.push(field, SearchOp::Gt, value.into())
    }

    /// Field is greater than or equal to `value`.
    pub fn gte(self, field: &str, value: impl Into<SearchValue>) -> Self {
        self.push(field, SearchOp::Gte, value.into())
    }

    /// Field is strictly less than `value`.
    pub fn lt(self, field: &str, value: impl Into<SearchValue>) -> Self {
        self.push(field, SearchOp::Lt, value.into())
    }

    /// Field is less than or equal to `value`.
    pub fn lte(self, field: &str, value: impl Into<SearchValue>) -> Self {
        self.push(field, SearchOp::Lte, value.into())
    }

    /// Field value contains `value` (case-insensitive regex).
    pub fn contains(self, field: &str, value: impl Into<String>) -> Self {
        self.push(field, SearchOp::Contains, SearchValue::String(value.into()))
    }

    /// Field value is one of `values`.
    pub fn in_values(self, field: &str, values: Vec<String>) -> Self {
        self.push(field, SearchOp::In, SearchValue::Array(values))
    }

    /// Field value is not one of `values`.
    pub fn not_in(self, field: &str, values: Vec<String>) -> Self {
        self.push(field, SearchOp::NotIn, SearchValue::Array(values))
    }

    /// Array field contains all of `values`.
    pub fn all_of(self, field: &str, values: Vec<String>) -> Self {
        self.push(field, SearchOp::All, SearchValue::Array(values))
    }

    /// Field exists (`true`) or does not exist (`false`) in the document.
    pub fn exists(self, field: &str, exists: bool) -> Self {
        self.push(field, SearchOp::Exists, SearchValue::Bool(exists))
    }

    // --- sort and limit ---

    /// Sort by `field` ascending.
    pub fn sort_asc(self, field: &str) -> Self {
        self.push_sort(field, true)
    }

    /// Sort by `field` descending.
    pub fn sort_desc(self, field: &str) -> Self {
        self.push_sort(field, false)
    }

    /// Cap the number of returned records.
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    // --- internal helpers ---

    fn push(mut self, field: &str, operator: SearchOp, value: SearchValue) -> Self {
        self.conditions.push(SearchCondition {
            field: field.to_string(),
            operator,
            value,
        });
        self
    }

    fn push_sort(mut self, field: &str, ascending: bool) -> Self {
        self.sort_fields
            .get_or_insert_with(Vec::new)
            .push(SortField { field: field.to_string(), ascending });
        self
    }
}
