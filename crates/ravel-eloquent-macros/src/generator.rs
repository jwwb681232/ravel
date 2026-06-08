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

    let table_name = &attrs.table_name;

    // Generate Column enum
    let column_variants: Vec<_> = attrs
        .columns
        .iter()
        .map(|c| {
            let name = format_ident!("{}", c.field_name);
            quote! { #name }
        })
        .collect();

    let column_names: Vec<_> = attrs
        .columns
        .iter()
        .map(|c| {
            let variant = format_ident!("{}", c.field_name);
            let name = &c.field_name;
            quote! { Self::#variant => #name }
        })
        .collect();

    let sea_orm_col = format_ident!("{}Column", struct_name);

    // Generate public struct (without hidden fields)
    let public_struct_name = format_ident!("{}Public", struct_name);
    let public_fields: Vec<_> = attrs
        .columns
        .iter()
        .filter(|c| !c.is_hidden)
        .map(|c| {
            let name = format_ident!("{}", c.field_name);
            let ty = &c.field_type;
            quote! { pub #name: #ty }
        })
        .collect();

    let public_field_names: Vec<_> = attrs
        .columns
        .iter()
        .filter(|c| !c.is_hidden)
        .map(|c| format_ident!("{}", c.field_name))
        .collect();

    // Hidden fields for Serialize skip
    let _hidden_skip: Vec<_> = attrs
        .columns
        .iter()
        .filter(|c| c.is_hidden)
        .map(|c| {
            let name = format_ident!("{}", c.field_name);
            quote! { #[serde(skip)] #name }
        })
        .collect();

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

        // ── Eloquent query API ─────────────────────────────────────
        impl #struct_name {
            pub fn to_public(&self) -> #public_struct_name {
                #public_struct_name {
                    #(#public_field_names: self.#public_field_names.clone()),*
                }
            }

            pub fn query() -> ravel_eloquent::ModelQuery<Self> {
                ravel_eloquent::ModelQuery::new().table(#table_name)
            }

            pub fn r#where<V: Into<sea_orm::Value>>(
                col: impl AsRef<str>,
                val: V,
            ) -> ravel_eloquent::ModelQuery<Self> {
                Self::query().r#where(col, val)
            }
        }
    };

    Ok(expanded)
}
