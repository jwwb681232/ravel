//! Parse `#[model(...)]` attributes from struct fields and container.
//!
//! Uses `syn::Meta` for robust parsing (no string matching).
#![allow(dead_code)]

use syn::{Attribute, Fields, Type};

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

/// Parse the `#[model(table = "name")]` container attribute.
pub fn parse_container_attrs(attrs: &[Attribute]) -> Result<String, syn::Error> {
    for attr in attrs {
        if !attr.path().is_ident("model") {
            continue;
        }
        let mut table: Option<String> = None;
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("table") {
                let s: syn::LitStr = meta.value()?.parse()?;
                table = Some(s.value());
            }
            Ok(())
        })?;
        if let Some(t) = table {
            return Ok(t);
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
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("id") {
                is_id = true;
                col_type = ColumnType::Id;
            } else if meta.path.is_ident("hidden") {
                is_hidden = true;
            } else if meta.path.is_ident("unique") {
                is_unique = true;
            } else if meta.path.is_ident("nullable") {
                is_nullable = true;
            } else if meta.path.is_ident("timestamps") {
                col_type = ColumnType::DateTime;
            } else if meta.path.is_ident("string") {
                // Optional length: #[model(string, 255)]
                let len: Option<syn::LitInt> = meta.value().ok().and_then(|v| v.parse().ok());
                col_type = ColumnType::String(len.map_or(255, |l| l.base10_parse().unwrap_or(255)));
            } else if meta.path.is_ident("text") {
                col_type = ColumnType::Text;
            } else if meta.path.is_ident("integer") {
                col_type = ColumnType::Integer;
            } else if meta.path.is_ident("bigint") {
                col_type = ColumnType::BigInt;
            } else if meta.path.is_ident("boolean") {
                col_type = ColumnType::Boolean;
            } else if meta.path.is_ident("float") {
                col_type = ColumnType::Float;
            } else if meta.path.is_ident("datetime") {
                col_type = ColumnType::DateTime;
            } else if meta.path.is_ident("json") {
                col_type = ColumnType::Json;
            } else if meta.path.is_ident("uuid") {
                col_type = ColumnType::Uuid;
            }
            Ok(())
        });
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
        && let Fields::Named(fields) = &data.fields
    {
        for field in &fields.named {
            let field_name = field.ident.as_ref().unwrap().to_string();
            let mut col = parse_field_attrs(&field.attrs);

            // Check for timestamps attribute special case
            if (field_name == "created_at" || field_name == "updated_at")
                && matches!(col.col_type, ColumnType::DateTime)
            {
                has_timestamps = true;
            }

            col.field_name = field_name;
            col.field_type = field.ty.clone();
            columns.push(col);
        }
    } else {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[derive(Model)] only supports structs with named fields",
        ));
    }

    Ok(ModelAttrs {
        table_name,
        columns,
        has_timestamps,
    })
}
