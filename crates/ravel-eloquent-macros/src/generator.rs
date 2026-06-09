//! Code generator for `#[derive(Model)]`.
//!
//! Generates:
//!
//! 1.  SeaORM entity definition — `#[sea_orm::model]` + `DeriveEntityModel`
//! 2.  `{StructName}Column` enum — PascalCase variants, `as_str()` method
//! 3.  `{StructName}Public` struct — non-hidden, non-relation fields
//! 4.  `ModelMeta` trait impl
//! 5.  Inherent impl block — `to_public()`, `query()`, `r#where()`, CRUD, setters
//! 6.  `ModelExt` trait impl

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use crate::attrs::{ColumnType, FieldAttr, ModelAttrs, RelationKind, to_pascal_case};

// ── Public entry point ─────────────────────────────────────────────────────

pub fn generate(input: &syn::DeriveInput) -> syn::Result<TokenStream> {
    let model = crate::attrs::parse_model(input)?;
    let expanded = generate_all(&model, &input.ident, &input.vis, &input.generics);
    Ok(expanded)
}

// ── Top-level composition ──────────────────────────────────────────────────

fn generate_all(
    model: &ModelAttrs,
    struct_name: &syn::Ident,
    vis: &syn::Visibility,
    generics: &syn::Generics,
) -> TokenStream {
    let public_name = format_ident!("{}Public", struct_name);
    let column_enum = format_ident!("{}Column", struct_name);

    let entity      = generate_entity(model, struct_name, vis, generics);
    let columns     = generate_column_enum(model, &column_enum);
    let public_     = generate_public_struct(model, &public_name);
    let meta        = generate_model_meta(model, struct_name, &public_name, &column_enum);
    let methods     = generate_inherent_methods(model, struct_name);
    let traits      = generate_trait_impls(model, struct_name);

    quote! {
        #entity

        #columns

        #public_

        #meta

        #methods

        #traits
    }
}

// ── 1. SeaORM entity definition ────────────────────────────────────────────

fn generate_entity(
    model: &ModelAttrs,
    struct_name: &syn::Ident,
    vis: &syn::Visibility,
    generics: &syn::Generics,
) -> TokenStream {
    let table_name = &model.table_name;

    // Build field list from parsed attributes.
    let mut fields: Vec<TokenStream> = model
        .fields
        .iter()
        .map(|f| {
            let name = format_ident!("{}", f.field_name);
            let ty = &f.field_type;

            if f.relation.is_some() {
                generate_relation_field(f, &name, ty)
            } else {
                let attrs = generate_column_attr(f);
                quote! {
                    #attrs
                    pub #name: #ty
                }
            }
        })
        .collect();

    // Synthesize created_at / updated_at when the user declared
    // `#[model(timestamps)]` without actually writing the fields.
    if model.has_timestamps {
        let has_created = model.fields.iter().any(|f| f.field_name == "created_at");
        let has_updated = model.fields.iter().any(|f| f.field_name == "updated_at");

        if !has_created {
            fields.push(quote! {
                #[sea_orm(column_name = "created_at")]
                pub created_at: chrono::NaiveDateTime
            });
        }
        if !has_updated {
            fields.push(quote! {
                #[sea_orm(column_name = "updated_at")]
                pub updated_at: chrono::NaiveDateTime
            });
        }
    }

    quote! {
        #[sea_orm::model]
        #[derive(Clone, Debug, PartialEq, Eq, sea_orm::DeriveEntityModel)]
        #[sea_orm(table_name = #table_name)]
        #vis struct #struct_name #generics {
            #(#fields,)*
        }
    }
}

/// SeaORM column-level attributes for a non-relation field.
fn generate_column_attr(f: &FieldAttr) -> TokenStream {
    let mut attrs: Vec<TokenStream> = Vec::new();

    match f.col_type {
        ColumnType::Id => {
            attrs.push(quote! { primary_key });
            attrs.push(quote! { auto_increment });
        }
        ColumnType::Uuid => {
            attrs.push(quote! { primary_key });
            attrs.push(quote! { auto_increment = false });
        }
        ColumnType::String(Some(len)) => {
            let ct = format!("String(Some({}))", len);
            attrs.push(quote! { column_type = #ct });
        }
        ColumnType::Text => {
            attrs.push(quote! { column_type = "Text" });
        }
        ColumnType::DateTime => {
            attrs.push(quote! { column_type = "DateTime" });
        }
        // Integer, BigInt, Boolean, Float, Json, String(None)  are
        // inferred by SeaORM from the Rust type — no extra attribute.
        _ => {}
    }

    if f.is_unique {
        attrs.push(quote! { unique });
    }
    if f.is_nullable {
        attrs.push(quote! { nullable });
    }
    // Only emit column_name when it differs from the Rust field name.
    if f.column_name != f.field_name {
        attrs.push(quote! { column_name = #(&f.column_name) });
    }

    if attrs.is_empty() {
        return quote! {};
    }
    quote! { #[sea_orm(#(#attrs),*)] }
}

/// Convert an entity type (e.g. `Post`) to SeaORM's dense attribute path
/// (e.g. `"super::post::Entity"`).
fn entity_type_to_seaorm_string(entity_type: &syn::Type) -> String {
    if let syn::Type::Path(type_path) = entity_type {
        if let Some(segment) = type_path.path.segments.last() {
            let ident = segment.ident.to_string();
            let lower = ident.to_lowercase();
            return format!("super::{}::Entity", lower);
        }
    }
    // Fallback — should not happen for valid SeaORM models.
    "super::entity::Entity".to_string()
}

/// SeaORM relation attribute + field for a relation field.
fn generate_relation_field(
    f: &FieldAttr,
    field_name: &syn::Ident,
    field_type: &syn::Type,
) -> TokenStream {
    let rel = f
        .relation
        .as_ref()
        .expect("generate_relation_field called for a non-relation field");

    match rel {
        RelationKind::HasMany { entity_type, via } => {
            let entity_str = entity_type_to_seaorm_string(entity_type);
            match via {
                Some(v) => {
                    quote! {
                        #[sea_orm(has_many = #entity_str, via = #v)]
                        pub #field_name: #field_type
                    }
                }
                None => {
                    quote! {
                        #[sea_orm(has_many = #entity_str)]
                        pub #field_name: #field_type
                    }
                }
            }
        }
        RelationKind::HasOne { entity_type } => {
            let entity_str = entity_type_to_seaorm_string(entity_type);
            quote! {
                #[sea_orm(has_one = #entity_str)]
                pub #field_name: #field_type
            }
        }
        RelationKind::BelongsTo { entity_type, from, to } => {
            let entity_str = entity_type_to_seaorm_string(entity_type);
            quote! {
                #[sea_orm(belongs_to = #entity_str, from = #from, to = #to)]
                pub #field_name: #field_type
            }
        }
    }
}

// ── 2. Column enum ─────────────────────────────────────────────────────────

fn generate_column_enum(model: &ModelAttrs, enum_name: &syn::Ident) -> TokenStream {
    let variants: Vec<TokenStream> = model
        .fields
        .iter()
        .filter(|f| f.relation.is_none())
        .map(|f| {
            let variant = format_ident!("{}", to_pascal_case(&f.field_name));
            quote! { #variant }
        })
        .collect();

    let as_str_arms: Vec<TokenStream> = model
        .fields
        .iter()
        .filter(|f| f.relation.is_none())
        .map(|f| {
            let variant = format_ident!("{}", to_pascal_case(&f.field_name));
            let col_name = &f.column_name;
            quote! { Self::#variant => #col_name }
        })
        .collect();

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
        .find(|f| {
            f.is_primary_key
                || matches!(f.col_type, ColumnType::Id | ColumnType::Uuid)
        })
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
        }
    }
}

// ── 5. Inherent methods ────────────────────────────────────────────────────

fn generate_inherent_methods(model: &ModelAttrs, struct_name: &syn::Ident) -> TokenStream {
    let to_public  = generate_to_public(model, struct_name);
    let query      = generate_query_shorthands(struct_name);
    let static_crud = generate_static_crud(struct_name);
    let setters    = generate_setters(model);

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

/// `query()` and `r#where()` shorthands.
fn generate_query_shorthands(_struct_name: &syn::Ident) -> TokenStream {
    quote! {
        pub fn query() -> ravel_eloquent::QueryBuilder<Self> {
            ravel_eloquent::QueryBuilder::new()
        }

        pub fn r#where(
            col: &str,
            val: impl Into<sea_orm::Value>,
        ) -> ravel_eloquent::QueryBuilder<Self> {
            Self::query().r#where(col, val)
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

        pub async fn delete_by_id(
            db: &sea_orm::DatabaseConnection,
            id: impl Into<sea_orm::Value> + std::marker::Send,
        ) -> ravel_eloquent::Result<()> {
            <Self as ravel_eloquent::ModelExt>::delete_by_id(db, id).await
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

fn generate_trait_impls(_model: &ModelAttrs, struct_name: &syn::Ident) -> TokenStream {
    quote! {
        impl ravel_eloquent::ModelExt for #struct_name {}
    }
}

