use std::path::PathBuf;

/// Options for schema generation.
#[derive(Debug, Clone, Default)]
pub struct SchemaOptions {
    /// Optional path to WIT source files for doc comment enrichment.
    pub wit_path: Option<PathBuf>,
}
