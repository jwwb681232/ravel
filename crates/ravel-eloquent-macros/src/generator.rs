//! Code generator for #[derive(Model)].
//!
//! Generates:
//! - Column enum (database columns)
//! - Public struct (without hidden fields)
//! - impl blocks with Eloquent-style query methods
//! - to_public() conversion method

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

pub fn generate(input: &syn::DeriveInput) -> Result<TokenStream, syn::Error> {
    let attrs = crate::attrs::parse_model(input)?;
    let struct_name = &input.ident;

    // Generate Column enum
    let column_variants: Vec<_> = attrs
        .fields
        .iter()
        .map(|c| {
            let name = format_ident!("{}", c.field_name);
            quote! { #name }
        })
        .collect();

    let column_names: Vec<_> = attrs
        .fields
        .iter()
        .map(|c| {
            let variant = format_ident!("{}", c.field_name);
            let name = &c.column_name;
            quote! { Self::#variant => #name }
        })
        .collect();

    let sea_orm_col = format_ident!("{}Column", struct_name);

    // Generate public struct (without hidden fields)
    let public_struct_name = format_ident!("{}Public", struct_name);
    let public_fields: Vec<_> = attrs
        .fields
        .iter()
        .filter(|c| !c.is_hidden)
        .map(|c| {
            let name = format_ident!("{}", c.field_name);
            let ty = &c.field_type;
            quote! { pub #name: #ty }
        })
        .collect();

    let public_field_names: Vec<_> = attrs
        .fields
        .iter()
        .filter(|c| !c.is_hidden)
        .map(|c| format_ident!("{}", c.field_name))
        .collect();

    // Find the ID column for find() methods
    let id_field_name = attrs
        .fields
        .iter()
        .find(|c| c.is_primary_key)
        .map(|c| c.column_name.clone())
        .unwrap_or_else(|| "id".to_string());

    // Column name literals for ModelMeta
    let column_name_literals: Vec<_> = attrs.fields.iter().map(|c| &c.column_name).collect();

    let table_name_lit = &attrs.table_name;
    let id_name_lit = &id_field_name;

    let expanded = quote! {
        // ── Column enum ────────────────────────────────────────────
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum #sea_orm_col {
            #(#column_variants),*
        }

        impl #sea_orm_col {
            pub fn as_str(&self) -> &'static str {
                match self {
                    #(#column_names),*
                }
            }
        }

        // ── Public view (no hidden fields) ─────────────────────────
        #[derive(Debug, Clone, serde::Serialize)]
        pub struct #public_struct_name {
            #(#public_fields),*
        }

        // ── ModelMeta ──────────────────────────────────────────────
        impl ravel_eloquent::ModelMeta for #struct_name {
            fn table_name() -> &'static str {
                #table_name_lit
            }
            fn columns() -> &'static [&'static str] {
                &[#(#column_name_literals),*]
            }
            fn id_column() -> &'static str {
                #id_name_lit
            }
        }

        // ── Eloquent query API ─────────────────────────────────────
        impl #struct_name {
            pub fn to_public(&self) -> #public_struct_name {
                #public_struct_name {
                    #(#public_field_names: self.#public_field_names.clone()),*
                }
            }

            /// Start a new fluent query builder.
            pub fn query() -> ravel_eloquent::ModelQuery<Self> {
                ravel_eloquent::ModelQuery::new().table(#table_name_lit)
            }

            /// Add a WHERE equality condition and return a query builder.
            pub fn r#where<V: Into<sea_orm::Value>>(
                col: impl AsRef<str>,
                val: V,
            ) -> ravel_eloquent::ModelQuery<Self> {
                Self::query().r#where(col, val)
            }

            /// Retrieve all records.
            pub async fn all(
                db: &sea_orm::DatabaseConnection,
            ) -> anyhow::Result<Vec<Self>> {
                ravel_eloquent::defaults::all::<Self>(db).await
            }

            /// Find by primary key.
            pub async fn find(
                db: &sea_orm::DatabaseConnection,
                id: impl Into<sea_orm::Value> + std::marker::Send,
            ) -> anyhow::Result<Option<Self>> {
                ravel_eloquent::defaults::find::<Self>(db, id).await
            }

            /// Find or return an error.
            pub async fn find_or_fail(
                db: &sea_orm::DatabaseConnection,
                id: impl Into<sea_orm::Value> + std::marker::Send,
            ) -> anyhow::Result<Self> {
                <Self as ravel_eloquent::ModelExt>::find_or_fail(db, id).await
            }

            /// Create a new record from JSON data.
            pub async fn create(
                data: serde_json::Value,
                db: &sea_orm::DatabaseConnection,
            ) -> anyhow::Result<Self> {
                ravel_eloquent::defaults::create::<Self>(data, db).await
            }
        }

        // ── ModelExt trait impl ────────────────────────────────────
        #[async_trait::async_trait]
        impl ravel_eloquent::ModelExt for #struct_name {
            async fn create(
                data: serde_json::Value,
                db: &sea_orm::DatabaseConnection,
            ) -> anyhow::Result<Self> {
                ravel_eloquent::defaults::create::<Self>(data, db).await
            }

            async fn update_by_id(
                db: &sea_orm::DatabaseConnection,
                id: impl Into<sea_orm::Value> + Send,
                data: serde_json::Value,
            ) -> anyhow::Result<()> {
                ravel_eloquent::defaults::update_by_id::<Self>(db, id, data).await
            }

            async fn delete_by_id(
                db: &sea_orm::DatabaseConnection,
                id: impl Into<sea_orm::Value> + Send,
            ) -> anyhow::Result<()> {
                ravel_eloquent::defaults::delete_by_id::<Self>(db, id).await
            }

            async fn find(
                db: &sea_orm::DatabaseConnection,
                id: impl Into<sea_orm::Value> + Send,
            ) -> anyhow::Result<Option<Self>> {
                ravel_eloquent::defaults::find::<Self>(db, id).await
            }

            async fn all(
                db: &sea_orm::DatabaseConnection,
            ) -> anyhow::Result<Vec<Self>> {
                ravel_eloquent::defaults::all::<Self>(db).await
            }
        }
    };

    Ok(expanded)
}
