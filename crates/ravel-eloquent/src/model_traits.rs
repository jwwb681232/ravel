/// Model metadata — auto-implemented by #[derive(Model)]
pub trait ModelMeta {
    /// JSON-safe public type (hidden fields excluded)
    type Public: serde::Serialize;

    /// Column metadata enum type
    type Columns: Copy;

    /// Database table name
    fn table_name() -> &'static str;

    /// All database column names (excluding relation fields)
    fn columns() -> &'static [&'static str];

    /// Primary key column name
    fn id_column() -> &'static str;

    /// Non-hidden column names (for API serialization)
    fn public_columns() -> &'static [&'static str];
}
