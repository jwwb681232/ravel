//! Parse `#[model(...)]` attributes from struct fields and container.
#![allow(dead_code)]

use syn::{Attribute, Fields, Meta, Type};

/// Parsed representation of a #[derive(Model)] struct.
pub struct ModelAttrs {
    pub table_name: String,
    pub columns: Vec<ColumnAttr>,
    pub has_timestamps: bool,
}

pub struct ColumnAttr {
    pub field_name: String,
    pub field_type: Type,
    pub col_type: ColumnType,
    pub is_id: bool,
    pub is_hidden: bool,
    pub is_unique: bool,
    pub is_nullable: bool,
}

pub enum ColumnType {
    Id,
    String(usize),
    Text,
    Integer,
    BigInt,
    Boolean,
    Float,
    DateTime,
    Json,
    Uuid,
}

/// Parse the `#[model(table = "...")]` container attribute.
pub fn parse_container_attrs(attrs: &[Attribute]) -> Result<String, syn::Error> {
    for attr in attrs {
        if !attr.path().is_ident("model") {
            continue;
        }
        if let Meta::List(list) = &attr.meta {
            let tokens = list.tokens.to_string();
            if let Some(start) = tokens.find("table")
                && let Some(eq) = tokens[start..].find('=') {
                    let rest = &tokens[start + eq + 1..].trim();
                    if let Some(q) = rest.find('"') {
                        let inner = &rest[q + 1..];
                        if let Some(end) = inner.find('"') {
                            return Ok(inner[..end].to_string());
                        }
                    }
                }
        }
    }
    Err(syn::Error::new(
        proc_macro2::Span::call_site(),
        "#[model(table = \"...\")] is required on the struct",
    ))
}

/// Parse `#[model(...)]` field attributes.
pub fn parse_field_attrs(attrs: &[Attribute]) -> ColumnAttr {
    let mut col_type = ColumnType::String(255);
    let mut is_id = false;
    let mut is_hidden = false;
    let mut is_unique = false;
    let mut is_nullable = false;

    for attr in attrs {
        if !attr.path().is_ident("model") {
            continue;
        }
        let tokens = attr
            .meta
            .require_list()
            .map(|l| l.tokens.to_string())
            .unwrap_or_default();

        if tokens.contains("id") {
            is_id = true;
            col_type = ColumnType::Id;
        }
        if tokens.contains("hidden") {
            is_hidden = true;
        }
        if tokens.contains("unique") {
            is_unique = true;
        }
        if tokens.contains("nullable") {
            is_nullable = true;
        }
        if tokens.contains("timestamps") {
            col_type = ColumnType::DateTime;
        }
        // Parse "string" or "string, 255"
        if tokens.contains("string") {
            let len = if let Some(comma) = tokens.find("string") {
                let rest = &tokens[comma + 6..].trim();
                rest.trim_start_matches(',').trim().parse().unwrap_or(255)
            } else {
                255
            };
            col_type = ColumnType::String(len);
        }
        if tokens.contains("text") {
            col_type = ColumnType::Text;
        }
        if tokens.contains("integer") {
            col_type = ColumnType::Integer;
        }
        if tokens.contains("bigint") {
            col_type = ColumnType::BigInt;
        }
        if tokens.contains("boolean") {
            col_type = ColumnType::Boolean;
        }
        if tokens.contains("float") {
            col_type = ColumnType::Float;
        }
        if tokens.contains("datetime") {
            col_type = ColumnType::DateTime;
        }
        if tokens.contains("json") {
            col_type = ColumnType::Json;
        }
        if tokens.contains("uuid") {
            col_type = ColumnType::Uuid;
        }
    }

    ColumnAttr {
        field_name: String::new(),            // filled by caller
        field_type: syn::parse_quote! { () }, // filled by caller
        col_type,
        is_id,
        is_hidden,
        is_unique,
        is_nullable,
    }
}

/// Collect all model attributes from a struct definition.
pub fn parse_model(input: &syn::DeriveInput) -> Result<ModelAttrs, syn::Error> {
    let table_name = parse_container_attrs(&input.attrs)?;
    let mut columns = Vec::new();
    let mut has_timestamps = false;

    if let syn::Data::Struct(data) = &input.data
        && let Fields::Named(fields) = &data.fields {
            for field in &fields.named {
                let field_name = field.ident.as_ref().unwrap().to_string();
                let mut col = parse_field_attrs(&field.attrs);

                // Check for timestamps attribute special case
                if (field_name == "created_at" || field_name == "updated_at")
                    && matches!(col.col_type, ColumnType::DateTime) {
                        has_timestamps = true;
                    }

                col.field_name = field_name;
                col.field_type = field.ty.clone();
                columns.push(col);
            }
        }

    Ok(ModelAttrs {
        table_name,
        columns,
        has_timestamps,
    })
}
