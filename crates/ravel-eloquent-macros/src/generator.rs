//! Code generator for `#[derive(Model)]`.
//!
//! Generates:
//!
//! 1.  `{StructName}Column` enum — PascalCase variants, `as_str()` method
//! 2.  `{StructName}Public` struct — non-hidden, non-relation fields
//! 3.  `ModelMeta` trait impl
//! 4.  Inherent impl block — `to_public()`, `query()`, `where_str()`, CRUD, setters
//! 5.  `ModelExt` trait impl

use crate::attrs::{ColumnType, ModelAttrs, to_pascal_case};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

// ── Helpers ────────────────────────────────────────────────────────────────

/// Convert `CamelCase` or `PascalCase` to `snake_case`.
fn to_snake(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    let chars = s.chars().peekable();
    for c in chars {
        if c.is_uppercase() {
            if !out.is_empty() {
                out.push('_');
            }
            out.push(c.to_lowercase().next().unwrap());
        } else {
            out.push(c);
        }
    }
    out
}

// ── Public entry point ─────────────────────────────────────────────────────

pub fn generate(input: &syn::DeriveInput) -> syn::Result<TokenStream> {
    let model = crate::attrs::parse_model(input)?;
    let expanded = generate_all(&model, &input.ident);
    Ok(expanded)
}

// ── Top-level composition ──────────────────────────────────────────────────

fn generate_all(model: &ModelAttrs, struct_name: &syn::Ident) -> TokenStream {
    let public_name = format_ident!("{}Public", struct_name);
    let column_enum = format_ident!("{}Column", struct_name);

    let columns = generate_column_enum(model, &column_enum, struct_name);
    let public_ = generate_public_struct(model, &public_name);
    let meta = generate_model_meta(model, struct_name, &public_name, &column_enum);
    let methods = generate_inherent_methods(model, struct_name);
    let traits = generate_trait_impls(model, struct_name);

    quote! {
        #columns

        #public_

        #meta

        #methods

        #traits
    }
}

// ── 2. Column enum ─────────────────────────────────────────────────────────

fn generate_column_enum(model: &ModelAttrs, enum_name: &syn::Ident, struct_name: &syn::Ident) -> TokenStream {
    let variant_names: Vec<syn::Ident> = model
        .fields.iter().filter(|f| f.relation.is_none())
        .map(|f| format_ident!("{}", to_pascal_case(&f.field_name)))
        .collect();
    let variants: Vec<TokenStream> = variant_names.iter().map(|v| quote! { #v }).collect();

    let as_str_arms: Vec<TokenStream> = model
        .fields.iter().filter(|f| f.relation.is_none())
        .map(|f| {
            let variant = &format_ident!("{}", to_pascal_case(&f.field_name));
            let col_name = &f.column_name;
            quote! { Self::#variant => #col_name }
        })
        .collect();

    let from_str_arms: Vec<TokenStream> = model
        .fields.iter().filter(|f| f.relation.is_none())
        .map(|f| {
            let variant = &format_ident!("{}", to_pascal_case(&f.field_name));
            let col_name = &f.column_name;
            quote! { #col_name => ::core::result::Result::Ok(Self::#variant) }
        })
        .collect();

    let def_arms: Vec<TokenStream> = model
        .fields.iter().filter(|f| f.relation.is_none())
        .map(|f| {
            let variant = &format_ident!("{}", to_pascal_case(&f.field_name));
            let nullable = f.is_nullable;
            let unique = f.is_unique;
            let ct = match f.col_type {
                crate::attrs::ColumnType::Id => quote! { sea_orm::ColumnType::Integer },
                crate::attrs::ColumnType::Uuid => quote! { sea_orm::ColumnType::Uuid },
                crate::attrs::ColumnType::String(None) => quote! { sea_orm::ColumnType::String(sea_orm::sea_query::StringLen::N(255)) },
                crate::attrs::ColumnType::String(Some(n)) => {
                    let n = n as u32;
                    quote! { sea_orm::ColumnType::String(sea_orm::sea_query::StringLen::N(#n)) }
                }
                crate::attrs::ColumnType::Text => quote! { sea_orm::ColumnType::Text },
                crate::attrs::ColumnType::Integer => quote! { sea_orm::ColumnType::Integer },
                crate::attrs::ColumnType::BigInt => quote! { sea_orm::ColumnType::BigInteger },
                crate::attrs::ColumnType::Boolean => quote! { sea_orm::ColumnType::Boolean },
                crate::attrs::ColumnType::Float => quote! { sea_orm::ColumnType::Double },
                crate::attrs::ColumnType::DateTime => quote! { sea_orm::ColumnType::DateTime },
                crate::attrs::ColumnType::Json => quote! { sea_orm::ColumnType::Json },
            };
            let mut body = quote! { sea_orm::ColumnTypeTrait::def(#ct) };
            if nullable {
                body = quote! { sea_orm::ColumnDef::nullable(#body) };
            }
            if unique {
                body = quote! { sea_orm::ColumnDef::unique(#body) };
            }
            quote! { Self::#variant => #body }
        })
        .collect();

    let iter_variants: Vec<TokenStream> = variant_names.iter().map(|v| quote! { Self::#v }).collect();

    let entity_name = format_ident!("__{}EntityName", enum_name);

    quote! {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum #enum_name {
            #(#variants,)*
        }

        impl #enum_name {
            pub fn as_str(&self) -> &'static str {
                match self {
                    #(#as_str_arms,)*
                }
            }
        }

        impl ::core::str::FromStr for #enum_name {
            type Err = ::std::string::String;

            fn from_str(s: &str) -> ::core::result::Result<Self, Self::Err> {
                match s {
                    #(#from_str_arms,)*
                    _ => ::core::result::Result::Err(::std::format!("unknown column: {s}"))
                }
            }
        }

        impl ::sea_orm::sea_query::Iden for #enum_name {
            fn unquoted(&self) -> &str {
                self.as_str()
            }
        }

        impl ::sea_orm::IdenStatic for #enum_name {
            fn as_str(&self) -> &'static str {
                self.as_str()
            }
        }

        impl ::sea_orm::Iterable for #enum_name {
            type Iterator = ::std::vec::IntoIter<Self>;

            fn iter() -> Self::Iterator {
                ::std::vec![#(#iter_variants,)*].into_iter()
            }
        }

        #[derive(Debug, Clone, Copy, Default)]
        pub struct #entity_name;

        impl ::sea_orm::sea_query::Iden for #entity_name {
            fn unquoted(&self) -> &str {
                <#struct_name as ravel_eloquent::ModelMeta>::table_name()
            }
        }

        impl ::sea_orm::IdenStatic for #entity_name {
            fn as_str(&self) -> &'static str {
                <#struct_name as ravel_eloquent::ModelMeta>::table_name()
            }
        }

        impl ::sea_orm::EntityName for #entity_name {
            fn table_name(&self) -> &'static str {
                <#struct_name as ravel_eloquent::ModelMeta>::table_name()
            }
        }

        impl ::sea_orm::ColumnTrait for #enum_name {
            type EntityName = #entity_name;

            fn def(&self) -> ::sea_orm::ColumnDef {
                match self {
                    #(#def_arms,)*
                }
            }
        }
    }
}

// ── 3. Public struct ───────────────────────────────────────────────────────

fn generate_public_struct(model: &ModelAttrs, public_name: &syn::Ident) -> TokenStream {
    let mut fields: Vec<TokenStream> = model
        .fields
        .iter()
        .filter(|f| !f.is_hidden && f.relation.is_none())
        .map(|f| {
            let name = format_ident!("{}", f.field_name);
            let ty = &f.field_type;
            quote! { pub #name: #ty }
        })
        .collect();

    // Include auto-synthesized timestamp fields in the Public struct too.
    if model.has_timestamps {
        let has_created = model.fields.iter().any(|f| f.field_name == "created_at");
        let has_updated = model.fields.iter().any(|f| f.field_name == "updated_at");

        if !has_created {
            fields.push(quote! { pub created_at: chrono::NaiveDateTime });
        }
        if !has_updated {
            fields.push(quote! { pub updated_at: chrono::NaiveDateTime });
        }
    }

    quote! {
        #[derive(Debug, Clone, serde::Serialize)]
        pub struct #public_name {
            #(#fields,)*
        }
    }
}

// ── 4. ModelMeta trait impl ────────────────────────────────────────────────

fn generate_model_meta(
    model: &ModelAttrs,
    struct_name: &syn::Ident,
    public_name: &syn::Ident,
    column_enum: &syn::Ident,
) -> TokenStream {
    let table_name = &model.table_name;

    // All non-relation column names.
    let column_names: Vec<&str> = model
        .fields
        .iter()
        .filter(|f| f.relation.is_none())
        .map(|f| f.column_name.as_str())
        .collect();

    // Include synthetic timestamp column names when applicable.
    let mut all_columns = column_names.clone();
    if model.has_timestamps {
        let has_created = model.fields.iter().any(|f| f.field_name == "created_at");
        let has_updated = model.fields.iter().any(|f| f.field_name == "updated_at");
        if !has_created {
            all_columns.push("created_at");
        }
        if !has_updated {
            all_columns.push("updated_at");
        }
    }

    // ID column: first field that is a primary key or Id/Uuid type.
    let id_col = model
        .fields
        .iter()
        .find(|f| f.is_primary_key || matches!(f.col_type, ColumnType::Id | ColumnType::Uuid))
        .map(|f| f.column_name.as_str())
        .unwrap_or("id");

    // Public columns: non-hidden, non-relation (plus synthetic timestamps).
    let mut public_cols: Vec<&str> = model
        .fields
        .iter()
        .filter(|f| !f.is_hidden && f.relation.is_none())
        .map(|f| f.column_name.as_str())
        .collect();
    if model.has_timestamps {
        let has_created = model.fields.iter().any(|f| f.field_name == "created_at");
        let has_updated = model.fields.iter().any(|f| f.field_name == "updated_at");
        if !has_created {
            public_cols.push("created_at");
        }
        if !has_updated {
            public_cols.push("updated_at");
        }
    }

    // ── Relation metadata ──────────────────────────────────────────────

    let rel_fields: Vec<_> = model.fields.iter().filter(|f| f.relation.is_some()).collect();

    // Generate per-relation get_columns() callbacks.
    let get_columns_fns: Vec<TokenStream> = rel_fields.iter().map(|f| {
        let rel = f.relation.as_ref().unwrap();
        let entity_type = match rel {
            crate::attrs::RelationKind::HasMany { entity_type, .. } => entity_type,
            crate::attrs::RelationKind::HasOne { entity_type, .. } => entity_type,
            crate::attrs::RelationKind::BelongsTo { entity_type, .. } => entity_type,
        };
        let fn_name = format_ident!("__{}_{}_columns", struct_name, f.field_name);
        quote! {
            fn #fn_name() -> &'static [&'static str] {
                <#entity_type as ravel_eloquent::ModelMeta>::columns()
            }
        }
    }).collect();

    // Generate RelationMeta static values.
    let rel_meta_vars: Vec<TokenStream> = rel_fields.iter().map(|f| {
        let rel = f.relation.as_ref().unwrap();
        let field_name = &f.field_name;
        let fn_name = format_ident!("__{}_{}_columns", struct_name, f.field_name);

        let (kind, rel_table, foreign_key, local_key): (&str, String, String, String) = match rel {
            crate::attrs::RelationKind::HasMany { entity_type, table, .. } => {
                let table = table.as_deref().map(|s| s.to_string()).unwrap_or_else(|| {
                    let type_str = quote::quote!(#entity_type).to_string();
                    to_snake(&type_str)
                });
                let fk_base = table_name.trim_end_matches('s');
                let fk = format!("{}_id", fk_base);
                ("HasMany", table, fk, "id".to_string())
            }
            crate::attrs::RelationKind::HasOne { entity_type, table } => {
                let table = table.as_deref().map(|s| s.to_string()).unwrap_or_else(|| {
                    let type_str = quote::quote!(#entity_type).to_string();
                    to_snake(&type_str)
                });
                let fk_base = table_name.trim_end_matches('s');
                let fk = format!("{}_id", fk_base);
                ("HasOne", table, fk, "id".to_string())
            }
            crate::attrs::RelationKind::BelongsTo { entity_type, from, to, table } => {
                let table = table.as_deref().map(|s| s.to_string()).unwrap_or_else(|| {
                    let type_str = quote::quote!(#entity_type).to_string();
                    to_snake(&type_str)
                });
                ("BelongsTo", table, from.clone(), to.clone())
            }
        };
        let kind_ident = format_ident!("{}", kind);

        quote! {
            ravel_eloquent::RelationMeta {
                field_name: #field_name,
                table_name: #rel_table,
                kind: ravel_eloquent::RelationKind::#kind_ident,
                foreign_key: #foreign_key,
                local_key: #local_key,
                get_columns: #fn_name,
            }
        }
    }).collect();

    let rel_count = rel_fields.len();
    let relations_impl = if rel_count > 0 {
        quote! {
            fn relations() -> &'static [ravel_eloquent::RelationMeta] {
                static RELS: [ravel_eloquent::RelationMeta; #rel_count] = [
                    #(#rel_meta_vars),*
                ];
                &RELS
            }
        }
    } else {
        quote! {}
    };

    quote! {
        impl ravel_eloquent::ModelMeta for #struct_name {
            type Public = #public_name;
            type Columns = #column_enum;

            fn table_name() -> &'static str {
                #table_name
            }

            fn columns() -> &'static [&'static str] {
                &[#(#all_columns),*]
            }

            fn id_column() -> &'static str {
                #id_col
            }

            fn public_columns() -> &'static [&'static str] {
                &[#(#public_cols),*]
            }

            #relations_impl
        }

        #(#get_columns_fns)*
    }
}

// ── 5. Inherent methods ────────────────────────────────────────────────────

fn generate_inherent_methods(model: &ModelAttrs, struct_name: &syn::Ident) -> TokenStream {
    let to_public = generate_to_public(model, struct_name);
    let query = generate_query_shorthands(struct_name);
    let static_crud = generate_static_crud(struct_name);
    let setters = generate_setters(model);

    quote! {
        impl #struct_name {
            #to_public
            #query
            #static_crud
            #setters
        }
    }
}

/// `to_public(&self) -> {StructName}Public`
fn generate_to_public(model: &ModelAttrs, struct_name: &syn::Ident) -> TokenStream {
    let public_name = format_ident!("{}Public", struct_name);

    let mut field_mappings: Vec<TokenStream> = model
        .fields
        .iter()
        .filter(|f| !f.is_hidden && f.relation.is_none())
        .map(|f| {
            let name = format_ident!("{}", f.field_name);
            quote! { #name: self.#name.clone() }
        })
        .collect();

    // Synthetic timestamp fields.
    if model.has_timestamps {
        let has_created = model.fields.iter().any(|f| f.field_name == "created_at");
        let has_updated = model.fields.iter().any(|f| f.field_name == "updated_at");

        if !has_created {
            field_mappings.push(quote! { created_at: self.created_at.clone() });
        }
        if !has_updated {
            field_mappings.push(quote! { updated_at: self.updated_at.clone() });
        }
    }

    quote! {
        pub fn to_public(&self) -> #public_name {
            #public_name {
                #(#field_mappings,)*
            }
        }
    }
}

/// `query()` and `where_eq()` shorthands.
///
/// Uses `ModelMeta` (already generated) to get table and column names,
/// then creates a `QueryBuilder` that operates on `sea-query` directly
/// without needing a SeaORM entity type.
fn generate_query_shorthands(_struct_name: &syn::Ident) -> TokenStream {
    quote! {
        pub fn query() -> ravel_eloquent::QueryBuilder {
            ravel_eloquent::QueryBuilder::new(<Self as ravel_eloquent::ModelMeta>::table_name(), <Self as ravel_eloquent::ModelMeta>::columns())
        }

        pub fn where_str(
            col: &str,
            val: impl Into<sea_orm::Value>,
        ) -> ravel_eloquent::QueryBuilder {
            Self::query().where_str(col, val)
        }
    }
}

/// Static CRUD methods that delegate to the `ModelExt` trait.
fn generate_static_crud(_struct_name: &syn::Ident) -> TokenStream {
    quote! {
        pub async fn find(
            db: &sea_orm::DatabaseConnection,
            id: impl Into<sea_orm::Value> + std::marker::Send,
        ) -> ravel_eloquent::Result<Option<Self>> {
            <Self as ravel_eloquent::ModelExt>::find(db, id).await
        }

        pub async fn find_or_fail(
            db: &sea_orm::DatabaseConnection,
            id: impl Into<sea_orm::Value> + std::marker::Send,
        ) -> ravel_eloquent::Result<Self> {
            <Self as ravel_eloquent::ModelExt>::find_or_fail(db, id).await
        }

        pub async fn all(
            db: &sea_orm::DatabaseConnection,
        ) -> ravel_eloquent::Result<Vec<Self>> {
            <Self as ravel_eloquent::ModelExt>::all(db).await
        }

        pub async fn create(
            data: serde_json::Value,
            db: &sea_orm::DatabaseConnection,
        ) -> ravel_eloquent::Result<Self> {
            <Self as ravel_eloquent::ModelExt>::create(data, db).await
        }

        pub async fn destroy(
            db: &sea_orm::DatabaseConnection,
            ids: impl IntoIterator<Item = impl Into<sea_orm::Value>> + std::marker::Send,
        ) -> ravel_eloquent::Result<u64> {
            <Self as ravel_eloquent::ModelExt>::destroy(db, ids).await
        }
    }
}

/// `set_<field>(self, val) -> Self` for each non-hidden, non-relation field.
fn generate_setters(model: &ModelAttrs) -> TokenStream {
    let setters: Vec<TokenStream> = model
        .fields
        .iter()
        .filter(|f| !f.is_hidden && f.relation.is_none())
        .map(|f| {
            let setter = format_ident!("set_{}", f.field_name);
            let name = format_ident!("{}", f.field_name);
            let ty = &f.field_type;
            quote! {
                pub fn #setter(mut self, val: impl Into<#ty>) -> Self {
                    self.#name = val.into();
                    self
                }
            }
        })
        .collect();

    quote! { #(#setters)* }
}

// ── 6. Trait implementations ───────────────────────────────────────────────

fn generate_trait_impls(model: &ModelAttrs, struct_name: &syn::Ident) -> TokenStream {
    let public_name = format_ident!("{}Public", struct_name);

    let mut impls: Vec<TokenStream> = Vec::new();

    // ModelExt — auto-implemented by the macro
    impls.push(quote! { impl ravel_eloquent::ModelExt for #struct_name {} });

    // ActiveModelExt — provides save/delete/update/refresh with default impls
    impls.push(quote! { impl ravel_eloquent::ActiveModelExt for #struct_name {} });

    // Serializes — the required `to_public()` method delegates to the inherent
    // method (which takes precedence over the trait method, avoiding recursion).
    impls.push(quote! {
        impl ravel_eloquent::Serializes for #struct_name {
            fn to_public(&self) -> #public_name {
                self.to_public()
            }
        }
    });

    // Replicates — has a default impl (just clone), so a blank impl suffices.
    impls.push(quote! { impl ravel_eloquent::Replicates for #struct_name {} });

    // Default — all fields default to their type's default (HasMany/HasOne/BelongsTo are empty).
    let default_fields: Vec<TokenStream> = model.fields.iter().map(|f| {
        let name = format_ident!("{}", f.field_name);
        quote! { #name: Default::default() }
    }).collect();

    impls.push(quote! {
        impl Default for #struct_name {
            fn default() -> Self {
                Self {
                    #(#default_fields,)*
                }
            }
        }
    });

    // Fillable: field-by-field fill from JSON
    // Uses `and_then` + `unwrap_or` pattern — the closure does NOT capture
    // `self` fields, so there is no borrow conflict with struct construction.
    let fill_fields: Vec<TokenStream> = model
        .fields
        .iter()
        .map(|f| {
            let name = format_ident!("{}", f.field_name);
            quote! {
                #name: data.get(stringify!(#name))
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or(self.#name)
            }
        })
        .collect();

    impls.push(quote! {
        impl ravel_eloquent::Fillable for #struct_name {
            fn fill(self, data: serde_json::Value) -> Self {
                Self {
                    #(#fill_fields,)*
                }
            }
        }
    });

    quote! { #(#impls)* }
}
