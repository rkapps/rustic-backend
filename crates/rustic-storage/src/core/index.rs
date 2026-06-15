/// Index definition — backend-agnostic.
#[derive(Debug, Clone)]
pub struct IndexDefinition {
    /// Fields to index and their direction (1 = asc, -1 = desc).
    pub fields: Vec<(String, i32)>,
    /// Whether the index enforces uniqueness.
    pub unique: bool,
    /// Optional name — backends may ignore this.
    pub name: Option<String>,
    /// Whether to create a sparse index.
    pub sparse: bool,
}

impl IndexDefinition {
    pub fn new(fields: Vec<(&str, i32)>) -> Self {
        Self {
            fields: fields
                .into_iter()
                .map(|(f, d)| (f.to_string(), d))
                .collect(),
            unique: false,
            name: None,
            sparse: false,
        }
    }

    pub fn unique(mut self) -> Self {
        self.unique = true;
        self
    }

    pub fn named(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    pub fn sparse(mut self) -> Self {
        self.sparse = true;
        self
    }
}
