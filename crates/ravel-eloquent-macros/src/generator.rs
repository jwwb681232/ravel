//! Code generator for `#[derive(Model)]`.
//!
//! Generates:
//!
//! 1.  `{StructName}Column` enum — PascalCase variants, `as_str()` method
//! 2.  `{StructName}Public` struct — non-hidden, non-relation fields
//! 3.  `ModelMeta` trait impl
//! 4.  Inherent impl block — `to_public()`, `query()`, `r#where()`, CRUD, setters
//! 5.  `ModelExt` trait impl

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use crate::attrs::{ColumnType, ModelAttrs, to_pascal_case};

// ── Public entry point ─────────────────────────────────────────────────────

pub fn generate(input: &syn::DeriveInput) -> syn::Result<TokenStream> {
    let model = crate::attrs::parse_model(input)?;
    let expanded = generate_all(&model, &input.ident);
    Ok(expanded)
}

// ── Top-level composition ──────────────────────────────────────────────────

fn generate_all(
    model: &ModelAttrs,
    struct_name: &syn::Ident,
) -> TokenStream {
    let public_name = format_ident!("{}Public", struct_name);
    let column_enum = format_ident!("{}Column", struct_name);

    let columns     = generate_column_enum(model, &column_enum);
    let public_     = generate_public_struct(model, &public_name);
    let meta        = generate_model_meta(model, struct_name, &public_name, &column_enum);
    let methods     = generate_inherent_methods(model, struct_name);
    let traits      = generate_trait_impls(model, struct_name);

    quote! {
        #columns

        #public_

        #meta

        #methods

        #traits
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

        impl sea_orm::sea_query::Iden for #enum_name {
            fn unquoted(&self) -> &str {
                self.as_str()
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
///
/// Uses `ModelMeta` (already generated) to get table and column names,
/// then creates a `QueryBuilder` that operates on `sea-query` directly
/// without needing a SeaORM entity type.
fn generate_query_shorthands(_struct_name: &syn::Ident) -> TokenStream {
    quote! {
        pub fn query() -> ravel_eloquent::QueryBuilder {
            ravel_eloquent::QueryBuilder::new(<Self as ravel_eloquent::ModelMeta>::table_name(), <Self as ravel_eloquent::ModelMeta>::columns())
        }

        pub fn r#where(
            col: &str,
            val: impl Into<sea_orm::Value>,
        ) -> ravel_eloquent::QueryBuilder {
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

    // Fillable: field-by-field fill from JSON
    // Uses `and_then` + `unwrap_or` pattern — the closure does NOT capture
    // `self` fields, so there is no borrow conflict with struct construction.
    let fill_fields: Vec<TokenStream> = model
        .fields
        .iter()
        .filter(|f| f.relation.is_none())
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

