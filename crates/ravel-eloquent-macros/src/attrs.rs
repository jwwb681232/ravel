//! Parse `#[model(...)]` attributes from struct fields and container.
//!
//! Uses `syn::Meta` for robust parsing (no string matching).
#![allow(dead_code)]

use syn::{Attribute, Fields, Type};

/// Helper: parse comma-separated Meta items inside a `#[model(...)]` attribute.
struct MetaVec(Vec<syn::Meta>);

impl syn::parse::Parse for MetaVec {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut metas = Vec::new();
        while !input.is_empty() {
            let meta = input.parse::<syn::Meta>()?;
            metas.push(meta);
            if input.is_empty() {
                break;
            }
            let _ = input.parse::<syn::Token![,]>()?;
        }
        Ok(MetaVec(metas))
    }
}

/// Column type variants representing database column types.
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnType {
    Id,
    Uuid,
    /// Default VARCHAR length. `None` means use the default (255).
    String(Option<usize>),
    Text,
    Integer,
    BigInt,
    Boolean,
    Float,
    DateTime,
    Json,
}

/// Detected or declared relationship kind.
#[derive(Debug, Clone)]
pub enum RelationKind {
    HasOne {
        entity_type: Type,
    },
    HasMany {
        entity_type: Type,
        via: Option<String>,
    },
    BelongsTo {
        entity_type: Type,
        from: String,
        to: String,
    },
}

/// Parsed attributes for a single struct field.
#[derive(Debug, Clone)]
pub struct FieldAttr {
    /// The Rust field name (e.g. `email`).
    pub field_name: String,
    /// The Rust type of the field (e.g. `String`).
    pub field_type: Type,
    /// The database column type.
    pub col_type: ColumnType,
    /// Actual database column name (defaults to `field_name`).
    pub column_name: String,
    pub is_primary_key: bool,
    pub is_unique: bool,
    pub is_nullable: bool,
    /// Excluded from `to_public()` output.
    pub is_hidden: bool,
    /// Auto-detected or explicitly declared relation.
    pub relation: Option<RelationKind>,
}

/// Parsed model attributes from a `#[derive(Model)]` struct.
#[derive(Debug, Clone)]
pub struct ModelAttrs {
    pub table_name: String,
    pub fields: Vec<FieldAttr>,
    pub has_timestamps: bool,
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Check whether a type is `Option<T>`.
pub fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            return segment.ident == "Option";
        }
    }
    false
}

/// Extract the first generic type argument from a type like `HasMany<Post>`.
///
/// Returns `None` when the type has no angle-bracketed arguments.
pub fn extract_first_generic_arg(ty: &Type) -> Option<Type> {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                if let Some(syn::GenericArgument::Type(ty)) = args.args.first() {
                    return Some(ty.clone());
                }
            }
        }
    }
    None
}

/// Detect `HasMany<T>` or `HasOne<T>` from a field type.
///
/// Returns `None` for any other type.
pub fn detect_relation(ty: &Type) -> Option<RelationKind> {
    let last_segment = match ty {
        Type::Path(type_path) => type_path.path.segments.last()?,
        _ => return None,
    };
    let ident = last_segment.ident.to_string();
    let entity_type = extract_first_generic_arg(ty)?;
    match ident.as_str() {
        "HasMany" => Some(RelationKind::HasMany {
            entity_type,
            via: None,
        }),
        "HasOne" => Some(RelationKind::HasOne { entity_type }),
        _ => None,
    }
}

/// Convert `snake_case` to `PascalCase`.
///
/// ```ignore
/// assert_eq!(to_pascal_case("hello_world"), "HelloWorld");
/// assert_eq!(to_pascal_case("user_id"), "UserId");
/// ```
pub fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut chars = p.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().to_string() + chars.as_str(),
            }
        })
        .collect()
}

// ── Parsing ──────────────────────────────────────────────────────────────────

/// Parse the `#[model(table = "...")]` container attribute.
///
/// Returns `(table_name, has_timestamps)`.
fn parse_container_attrs(attrs: &[Attribute]) -> Result<(String, bool), syn::Error> {
    for attr in attrs {
        if !attr.path().is_ident("model") {
            continue;
        }
        let mut table: Option<String> = None;
        let mut has_timestamps = false;
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("table") {
                let s: syn::LitStr = meta.value()?.parse()?;
                table = Some(s.value());
            } else if meta.path.is_ident("timestamps") {
                has_timestamps = true;
            }
            Ok(())
        })?;
        if let Some(t) = table {
            return Ok((t, has_timestamps));
        }
    }
    Err(syn::Error::new(
        proc_macro2::Span::call_site(),
        "#[model(table = \"...\")] is required on the struct",
    ))
}

/// Process a single `Meta` item from a `#[model(...)]` attribute.
fn apply_meta(
    meta: &syn::Meta,
    col_type: &mut ColumnType,
    column_name: &mut String,
    is_primary_key: &mut bool,
    is_unique: &mut bool,
    is_nullable: &mut bool,
    is_hidden: &mut bool,
    relation: &mut Option<RelationKind>,
    pending_from: &mut Option<String>,
    pending_to: &mut Option<String>,
    field_type: &syn::Type,
) {
    match meta {
        syn::Meta::Path(path) => {
            if path.is_ident("id") {
                *is_primary_key = true;
                *col_type = ColumnType::Id;
            } else if path.is_ident("uuid") {
                *col_type = ColumnType::Uuid;
            } else if path.is_ident("string") {
                // bare #[model(string)] -> default length
                *col_type = ColumnType::String(None);
            } else if path.is_ident("text") {
                *col_type = ColumnType::Text;
            } else if path.is_ident("integer") {
                *col_type = ColumnType::Integer;
            } else if path.is_ident("bigint") {
                *col_type = ColumnType::BigInt;
            } else if path.is_ident("boolean") {
                *col_type = ColumnType::Boolean;
            } else if path.is_ident("float") {
                *col_type = ColumnType::Float;
            } else if path.is_ident("datetime") {
                *col_type = ColumnType::DateTime;
            } else if path.is_ident("json") {
                *col_type = ColumnType::Json;
            } else if path.is_ident("unique") {
                *is_unique = true;
            } else if path.is_ident("nullable") {
                *is_nullable = true;
            } else if path.is_ident("hidden") {
                *is_hidden = true;
            } else if path.is_ident("has_many") {
                if !matches!(relation, Some(RelationKind::HasMany { .. })) {
                    let entity_type = extract_first_generic_arg(field_type)
                        .unwrap_or_else(|| syn::parse_quote! { () });
                    *relation = Some(RelationKind::HasMany {
                        entity_type,
                        via: None,
                    });
                }
            } else if path.is_ident("has_one") {
                if !matches!(relation, Some(RelationKind::HasOne { .. })) {
                    let entity_type = extract_first_generic_arg(field_type)
                        .unwrap_or_else(|| syn::parse_quote! { () });
                    *relation = Some(RelationKind::HasOne { entity_type });
                }
            } else if path.is_ident("belongs_to") {
                let entity_type = extract_first_generic_arg(field_type)
                    .unwrap_or_else(|| syn::parse_quote! { () });
                let from = pending_from.take().unwrap_or_default();
                let to = pending_to.take().unwrap_or_default();
                *relation = Some(RelationKind::BelongsTo {
                    entity_type,
                    from,
                    to,
                });
            }
        }
        syn::Meta::List(list) => {
            if list.path.is_ident("string") {
                // #[model(string(254))]
                if let Ok(lit) = syn::parse2::<syn::LitInt>(list.tokens.clone()) {
                    if let Ok(n) = lit.base10_parse::<usize>() {
                        *col_type = ColumnType::String(Some(n));
                    }
                }
            }
        }
        syn::Meta::NameValue(nv) => {
            if nv.path.is_ident("column") {
                if let syn::Expr::Lit(expr_lit) = &nv.value {
                    if let syn::Lit::Str(s) = &expr_lit.lit {
                        *column_name = s.value();
                    }
                }
            } else if nv.path.is_ident("from") {
                if let syn::Expr::Lit(expr_lit) = &nv.value {
                    if let syn::Lit::Str(s) = &expr_lit.lit {
                        if let Some(RelationKind::BelongsTo { from, .. }) = relation {
                            *from = s.value();
                        } else {
                            *pending_from = Some(s.value());
                        }
                    }
                }
            } else if nv.path.is_ident("to") {
                if let syn::Expr::Lit(expr_lit) = &nv.value {
                    if let syn::Lit::Str(s) = &expr_lit.lit {
                        if let Some(RelationKind::BelongsTo { to, .. }) = relation {
                            *to = s.value();
                        } else {
                            *pending_to = Some(s.value());
                        }
                    }
                }
            }
        }
    }
}

/// Parse `#[model(...)]` field-level attributes and auto-detect from field type.
fn parse_field_attrs(field: &syn::Field) -> FieldAttr {
    let field_name = field.ident.as_ref().unwrap().to_string();
    let field_type = field.ty.clone();

    // Auto-detect from type
    let auto_nullable = is_option_type(&field_type);
    let auto_relation = detect_relation(&field_type);

    // Mutable state populated by the attribute loop below
    let mut col_type = ColumnType::String(None);
    let mut column_name = field_name.clone();
    let mut is_primary_key = false;
    let mut is_unique = false;
    let mut is_nullable = auto_nullable;
    let mut is_hidden = false;
    let mut relation = auto_relation;
    let mut pending_from: Option<String> = None;
    let mut pending_to: Option<String> = None;

    for attr in &field.attrs {
        if !attr.path().is_ident("model") {
            continue;
        }
        // Parse the tokens inside #[model(...)] as comma-separated Meta items.
        if let syn::Meta::List(list) = &attr.meta {
            if let Ok(metas) = syn::parse2::<MetaVec>(list.tokens.clone()) {
                for meta in &metas.0 {
                    apply_meta(
                        meta,
                        &mut col_type,
                        &mut column_name,
                        &mut is_primary_key,
                        &mut is_unique,
                        &mut is_nullable,
                        &mut is_hidden,
                        &mut relation,
                        &mut pending_from,
                        &mut pending_to,
                        &field_type,
                    );
                }
            }
        }
    }

    FieldAttr {
        field_name,
        field_type,
        col_type,
        column_name,
        is_primary_key,
        is_unique,
        is_nullable,
        is_hidden,
        relation,
    }
}

/// Collect all model attributes from a struct definition.
pub fn parse_model(input: &syn::DeriveInput) -> Result<ModelAttrs, syn::Error> {
    let (table_name, mut has_timestamps) = parse_container_attrs(&input.attrs)?;
    let mut fields = Vec::new();

    if let syn::Data::Struct(data) = &input.data
        && let Fields::Named(fields_named) = &data.fields
    {
        for field in &fields_named.named {
            let field_name = field.ident.as_ref().unwrap().to_string();
            let fa = parse_field_attrs(field);

            // Auto-detect timestamps from field names (e.g. created_at, updated_at
            // with a datetime column type).
            if !has_timestamps
                && (field_name == "created_at" || field_name == "updated_at")
                && matches!(fa.col_type, ColumnType::DateTime)
            {
                has_timestamps = true;
            }

            fields.push(fa);
        }
    } else {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[derive(Model)] only supports structs with named fields",
        ));
    }

    Ok(ModelAttrs {
        table_name,
        fields,
        has_timestamps,
    })
}

// ── Tests ────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn parse_struct(s: &str) -> syn::DeriveInput {
        syn::parse_str(s).expect("Failed to parse struct")
    }

    #[test]
    fn test_parse_basic_model() {
        let input = parse_struct(
            r#"
            #[model(table = "users")]
            struct User {
                #[model(id)]
                id: i32,
                name: String,
                #[model(hidden)]
                password: String,
            }
            "#,
        );
        let attrs = parse_model(&input).unwrap();
        assert_eq!(attrs.table_name, "users");
        assert!(!attrs.has_timestamps);
        assert_eq!(attrs.fields.len(), 3);

        // ── id field ──────────────────────────────────────────────────
        let id_field = &attrs.fields[0];
        assert_eq!(id_field.field_name, "id");
        assert!(id_field.is_primary_key);
        assert!(!id_field.is_hidden);
        assert!(!id_field.is_unique);
        assert!(!id_field.is_nullable);
        assert_eq!(id_field.col_type, ColumnType::Id);

        // ── name field (default string) ───────────────────────────────
        let name = &attrs.fields[1];
        assert_eq!(name.field_name, "name");
        assert_eq!(name.col_type, ColumnType::String(None));
        assert!(!name.is_hidden);
        assert!(!name.is_primary_key);

        // ── password field (hidden) ───────────────────────────────────
        let pwd = &attrs.fields[2];
        assert_eq!(pwd.field_name, "password");
        assert!(pwd.is_hidden);
        assert_eq!(pwd.col_type, ColumnType::String(None));
    }

    #[test]
    fn test_parse_string_length() {
        let input = parse_struct(
            r#"
            #[model(table = "users")]
            struct User {
                #[model(string(254), unique)]
                email: String,
            }
            "#,
        );
        let attrs = parse_model(&input).unwrap();
        assert_eq!(attrs.fields.len(), 1);
        let field = &attrs.fields[0];
        assert_eq!(field.col_type, ColumnType::String(Some(254)));
        assert!(field.is_unique);
        assert_eq!(field.field_name, "email");
    }

    #[test]
    fn test_parse_relations() {
        let input = parse_struct(
            r#"
            #[model(table = "users")]
            struct User {
                #[model(id)]
                id: i32,
                posts: HasMany<Post>,
                #[model(belongs_to, from = "user_id", to = "id")]
                team: BelongsTo<Team>,
            }
            "#,
        );
        let attrs = parse_model(&input).unwrap();
        assert_eq!(attrs.fields.len(), 3);

        // ── Auto-detected HasMany ─────────────────────────────────────
        let posts = &attrs.fields[1];
        assert!(posts.relation.is_some());
        match &posts.relation {
            Some(RelationKind::HasMany {
                entity_type: _,
                via,
            }) => {
                assert!(via.is_none(), "HasMany via should be None by default");
            }
            other => panic!("Expected HasMany relation, got {other:?}"),
        }

        // ── BelongsTo with explicit from/to ───────────────────────────
        let team = &attrs.fields[2];
        match &team.relation {
            Some(RelationKind::BelongsTo {
                entity_type: _,
                from,
                to,
            }) => {
                assert_eq!(from, "user_id");
                assert_eq!(to, "id");
            }
            other => panic!("Expected BelongsTo relation, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_timestamps() {
        let input = parse_struct(
            r#"
            #[model(table = "x", timestamps)]
            struct X {
                #[model(id)]
                id: i32,
            }
            "#,
        );
        let attrs = parse_model(&input).unwrap();
        assert!(attrs.has_timestamps);
        assert_eq!(attrs.table_name, "x");
    }

    #[test]
    fn test_parse_nullable_auto_detect() {
        let input = parse_struct(
            r#"
            #[model(table = "items")]
            struct Item {
                #[model(id)]
                id: i32,
                description: Option<String>,
            }
            "#,
        );
        let attrs = parse_model(&input).unwrap();
        assert!(attrs.fields[1].is_nullable);
    }

    #[test]
    fn test_parse_column_name_override() {
        let input = parse_struct(
            r#"
            #[model(table = "users")]
            struct User {
                #[model(id)]
                id: i32,
                #[model(column = "real_name")]
                name: String,
            }
            "#,
        );
        let attrs = parse_model(&input).unwrap();
        assert_eq!(attrs.fields[1].column_name, "real_name");
        assert_eq!(attrs.fields[1].field_name, "name");
    }

    #[test]
    fn test_parse_all_column_types() {
        let input = parse_struct(
            r#"
            #[model(table = "types")]
            struct AllTypes {
                #[model(id)]       a: i32,
                #[model(uuid)]     b: String,
                #[model(string)]   c: String,
                #[model(text)]     d: String,
                #[model(integer)]  e: i32,
                #[model(bigint)]   f: i64,
                #[model(boolean)]  g: bool,
                #[model(float)]    h: f64,
                #[model(datetime)] i: String,
                #[model(json)]     j: serde_json::Value,
            }
            "#,
        );
        let attrs = parse_model(&input).unwrap();
        let expectations = [
            ColumnType::Id,
            ColumnType::Uuid,
            ColumnType::String(None),
            ColumnType::Text,
            ColumnType::Integer,
            ColumnType::BigInt,
            ColumnType::Boolean,
            ColumnType::Float,
            ColumnType::DateTime,
            ColumnType::Json,
        ];
        for (i, expected) in expectations.iter().enumerate() {
            assert_eq!(
                &attrs.fields[i].col_type, expected,
                "Mismatch at field index {i}, field {}",
                attrs.fields[i].field_name
            );
        }
    }

    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("hello_world"), "HelloWorld");
        assert_eq!(to_pascal_case("user_id"), "UserId");
        assert_eq!(to_pascal_case("id"), "Id");
        assert_eq!(to_pascal_case(""), "");
        assert_eq!(to_pascal_case("already_pascal"), "AlreadyPascal");
    }

    #[test]
    fn test_is_option_type() {
        let opt: Type = syn::parse_str("Option<String>").unwrap();
        assert!(is_option_type(&opt));

        let s: Type = syn::parse_str("String").unwrap();
        assert!(!is_option_type(&s));

        let vec: Type = syn::parse_str("Vec<i32>").unwrap();
        assert!(!is_option_type(&vec));
    }

    #[test]
    fn test_extract_first_generic_arg() {
        let hasmany: Type = syn::parse_str("HasMany<Post>").unwrap();
        let inner = extract_first_generic_arg(&hasmany).unwrap();
        assert_eq!(quote::quote!(#inner).to_string(), "Post");

        let plain: Type = syn::parse_str("String").unwrap();
        assert!(extract_first_generic_arg(&plain).is_none());
    }

    #[test]
    fn test_detect_relation() {
        let hasmany: Type = syn::parse_str("HasMany<Post>").unwrap();
        let rel = detect_relation(&hasmany);
        assert!(matches!(rel, Some(RelationKind::HasMany { .. })));

        let hasone: Type = syn::parse_str("HasOne<Profile>").unwrap();
        let rel = detect_relation(&hasone);
        assert!(matches!(rel, Some(RelationKind::HasOne { .. })));

        let not_rel: Type = syn::parse_str("String").unwrap();
        assert!(detect_relation(&not_rel).is_none());
    }
}
