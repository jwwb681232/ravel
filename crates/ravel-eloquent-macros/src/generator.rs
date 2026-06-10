//! Code generator for `#[derive(Model)]`.
//!
//! Generates:
//!
//! 1.  `{StructName}Column` enum — PascalCase variants, `as_str()` method
//! 2.  `{StructName}Public` struct — non-hidden, non-relation fields
//! 3.  `{StructName}Entity` / `{StructName}ActiveModel` / `{StructName}PrimaryKey` — SeaORM types
//! 4.  `ModelMeta` trait impl
//! 5.  Inherent impl block — `to_public()`, `query()`, `where_str()`, CRUD, setters
//! 6.  `ModelExt` / `ActiveModelExt` / `FromQueryResult` / `ModelTrait` impls

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
    let entity_name = format_ident!("{}Entity", struct_name);
    let active_model_name = format_ident!("{}ActiveModel", struct_name);
    let pk_enum = format_ident!("{}PrimaryKey", struct_name);
    let rel_enum = format_ident!("{}Relation", struct_name);

    let columns = generate_column_enum(model, &column_enum, struct_name);
    let public_ = generate_public_struct(model, &public_name);
    let sea_orm_types = generate_sea_orm_types(model, struct_name, &entity_name, &active_model_name, &pk_enum, &rel_enum, &column_enum);
    let meta = generate_model_meta(model, struct_name, &public_name, &column_enum, &entity_name, &active_model_name);
    let methods = generate_inherent_methods(model, struct_name);
    let traits = generate_trait_impls(model, struct_name, &entity_name, &column_enum);

    quote! {
        #columns

        #public_

        #sea_orm_types

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

// ── 2b. SeaORM Entity / ActiveModel / PrimaryKey ─────────────────────────

fn generate_sea_orm_types(
    model: &ModelAttrs,
    struct_name: &syn::Ident,
    entity_name: &syn::Ident,
    active_model_name: &syn::Ident,
    pk_enum: &syn::Ident,
    rel_enum: &syn::Ident,
    column_enum: &syn::Ident,
) -> TokenStream {
    let db_fields: Vec<_> = model.fields.iter().filter(|f| f.relation.is_none()).collect();

    // ActiveModel fields: each field is wrapped in ActiveValue
    let _active_model_fields: Vec<TokenStream> = db_fields.iter().map(|f| {
        let name = format_ident!("{}", f.field_name);
        quote! { pub #name: ::sea_orm::ActiveValue<sea_orm::Value> }
    }).collect();

    let _active_model_notset_fields: Vec<TokenStream> = db_fields.iter().map(|f| {
        let name = format_ident!("{}", f.field_name);
        quote! { #name: ::sea_orm::ActiveValue::NotSet }
    }).collect();

    // PrimaryKey variants — just the pk columns
    let pk_fields: Vec<_> = db_fields.iter().filter(|f| f.is_primary_key || matches!(f.col_type, ColumnType::Id | ColumnType::Uuid)).collect();
    let pk_variants: Vec<TokenStream> = pk_fields.iter().map(|f| {
        let v = format_ident!("{}", to_pascal_case(&f.field_name));
        quote! { #v }
    }).collect();
    let pk_iter_variants: Vec<TokenStream> = pk_variants.iter().map(|v| quote! { Self::#v }).collect();
    let pk_value_type = if pk_fields.len() == 1 {
        let ty = &pk_fields[0].field_type;
        quote! { #ty }
    } else {
        let types: Vec<_> = pk_fields.iter().map(|f| &f.field_type).collect();
        quote! { (#(#types,)*) }
    };
    let pk_auto_increment = pk_fields.iter().any(|f| matches!(f.col_type, ColumnType::Id));
    let pk_auto_inc = if pk_auto_increment { quote! { true } } else { quote! { false } };

    let pk_into_column_arms: Vec<TokenStream> = pk_fields.iter().map(|f| {
        let v = format_ident!("{}", to_pascal_case(&f.field_name));
        let col_v = format_ident!("{}", to_pascal_case(&f.field_name));
        quote! { Self::#v => #column_enum::#col_v }
    }).collect();
    let pk_from_column_arms: Vec<TokenStream> = pk_fields.iter().map(|f| {
        let v = format_ident!("{}", to_pascal_case(&f.field_name));
        let col_v = format_ident!("{}", to_pascal_case(&f.field_name));
        quote! { #column_enum::#col_v => ::core::option::Option::Some(Self::#v) }
    }).collect();

    // IntoActiveModel impl: Model -> ActiveModel (Unchanged fields)
    let into_active_model_fields: Vec<TokenStream> = db_fields.iter().map(|f| {
        let name = format_ident!("{}", f.field_name);
        quote! { #name: ::sea_orm::ActiveValue::Unchanged(::sea_orm::Value::from(self.#name.clone())) }
    }).collect();

    // ActiveModelTrait impl arms
    let am_get_arms: Vec<TokenStream> = db_fields.iter().map(|f| {
        let name = format_ident!("{}", f.field_name);
        let col_v = format_ident!("{}", to_pascal_case(&f.field_name));
        quote! { #column_enum::#col_v => self.#name.clone() }
    }).collect();
    let am_take_arms: Vec<TokenStream> = db_fields.iter().map(|f| {
        let name = format_ident!("{}", f.field_name);
        let col_v = format_ident!("{}", to_pascal_case(&f.field_name));
        quote! { #column_enum::#col_v => ::std::mem::replace(&mut self.#name, ::sea_orm::ActiveValue::NotSet) }
    }).collect();
    let am_set_if_not_equals_arms: Vec<TokenStream> = db_fields.iter().map(|f| {
        let name = format_ident!("{}", f.field_name);
        let col_v = format_ident!("{}", to_pascal_case(&f.field_name));
        quote! { #column_enum::#col_v => { self.#name = ::sea_orm::ActiveValue::Set(v); } }
    }).collect();
    let am_try_set_arms: Vec<TokenStream> = db_fields.iter().map(|f| {
        let name = format_ident!("{}", f.field_name);
        let col_v = format_ident!("{}", to_pascal_case(&f.field_name));
        quote! {
            #column_enum::#col_v => {
                self.#name = ::sea_orm::ActiveValue::Set(v);
                ::core::result::Result::Ok(())
            }
        }
    }).collect();
    let am_not_set_arms: Vec<TokenStream> = db_fields.iter().map(|f| {
        let name = format_ident!("{}", f.field_name);
        let col_v = format_ident!("{}", to_pascal_case(&f.field_name));
        quote! { #column_enum::#col_v => { self.#name = ::sea_orm::ActiveValue::NotSet; } }
    }).collect();
    let am_is_not_set_arms: Vec<TokenStream> = db_fields.iter().map(|f| {
        let name = format_ident!("{}", f.field_name);
        let col_v = format_ident!("{}", to_pascal_case(&f.field_name));
        quote! { #column_enum::#col_v => matches!(self.#name, ::sea_orm::ActiveValue::NotSet) }
    }).collect();
    let am_reset_arms: Vec<TokenStream> = db_fields.iter().map(|f| {
        let name = format_ident!("{}", f.field_name);
        let col_v = format_ident!("{}", to_pascal_case(&f.field_name));
        quote! { #column_enum::#col_v => { if let ::sea_orm::ActiveValue::Unchanged(v) = &self.#name { self.#name = ::sea_orm::ActiveValue::Set(v.clone()); } } }
    }).collect();

    quote! {
        // ── Entity unit struct ──────────────────────────────────────
        #[derive(Copy, Clone, Default, Debug)]
        struct #entity_name;

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

        impl ::sea_orm::EntityTrait for #entity_name {
            type Model = #struct_name;
            type ModelEx = #struct_name;
            type ActiveModel = #active_model_name;
            type ActiveModelEx = #active_model_name;
            type Column = #column_enum;
            type Relation = #rel_enum;
            type PrimaryKey = #pk_enum;
        }

        // ── ActiveModel struct ──────────────────────────────────────
        #[derive(Clone, Debug)]
        struct #active_model_name {
            #(#_active_model_fields,)*
        }

        impl ::sea_orm::ActiveModelTrait for #active_model_name {
            type Entity = #entity_name;

            fn default() -> Self {
                Self {
                    #(#_active_model_notset_fields,)*
                }
            }

            fn take(&mut self, c: <Self::Entity as ::sea_orm::EntityTrait>::Column) -> ::sea_orm::ActiveValue<::sea_orm::Value> {
                match c {
                    #(#am_take_arms,)*
                }
            }

            fn get(&self, c: <Self::Entity as ::sea_orm::EntityTrait>::Column) -> ::sea_orm::ActiveValue<::sea_orm::Value> {
                match c {
                    #(#am_get_arms,)*
                }
            }

            fn set_if_not_equals(&mut self, c: <Self::Entity as ::sea_orm::EntityTrait>::Column, v: ::sea_orm::Value) {
                match c {
                    #(#am_set_if_not_equals_arms,)*
                }
            }

            fn try_set(&mut self, c: <Self::Entity as ::sea_orm::EntityTrait>::Column, v: ::sea_orm::Value) -> ::core::result::Result<(), ::sea_orm::DbErr> {
                match c {
                    #(#am_try_set_arms,)*
                    _ => ::core::result::Result::Ok(())
                }
            }

            fn not_set(&mut self, c: <Self::Entity as ::sea_orm::EntityTrait>::Column) {
                match c {
                    #(#am_not_set_arms,)*
                }
            }

            fn is_not_set(&self, c: <Self::Entity as ::sea_orm::EntityTrait>::Column) -> bool {
                match c {
                    #(#am_is_not_set_arms,)*
                }
            }

            fn default_values() -> Self {
                Self::default()
            }

            fn reset(&mut self, c: <Self::Entity as ::sea_orm::EntityTrait>::Column) {
                match c {
                    #(#am_reset_arms,)*
                }
            }
        }

        impl ::sea_orm::ActiveModelBehavior for #active_model_name {}

        // ── IntoActiveModel (Model -> ActiveModel) ──────────────────
        impl ::sea_orm::IntoActiveModel<#active_model_name> for #struct_name {
            fn into_active_model(self) -> #active_model_name {
                #active_model_name {
                    #(#into_active_model_fields,)*
                }
            }
        }

        // ── PrimaryKey enum ─────────────────────────────────────────
        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
        pub enum #pk_enum {
            #(#pk_variants,)*
        }

        impl ::sea_orm::sea_query::Iden for #pk_enum {
            fn unquoted(&self) -> &str {
                match self {
                    #(#pk_into_column_arms,)*
                }
                .as_str()
            }
        }

        impl ::sea_orm::IdenStatic for #pk_enum {
            fn as_str(&self) -> &'static str {
                match self {
                    #(#pk_into_column_arms,)*
                }
                .as_str()
            }
        }

        impl ::sea_orm::Iterable for #pk_enum {
            type Iterator = ::std::vec::IntoIter<Self>;
            fn iter() -> Self::Iterator {
                ::std::vec![#(#pk_iter_variants,)*].into_iter()
            }
        }

        impl ::sea_orm::PrimaryKeyTrait for #pk_enum {
            type ValueType = #pk_value_type;
            fn auto_increment() -> bool {
                #pk_auto_inc
            }
        }

        impl ::sea_orm::PrimaryKeyToColumn for #pk_enum {
            type Column = #column_enum;
            fn into_column(self) -> Self::Column {
                match self {
                    #(#pk_into_column_arms,)*
                }
            }
            fn from_column(col: Self::Column) -> ::core::option::Option<Self> {
                match col {
                    #(#pk_from_column_arms,)*
                    _ => ::core::option::Option::None,
                }
            }
        }

        // ── Relation enum (dummy, not used by our system) ───────────
        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
        pub enum #rel_enum {
            #[doc(hidden)]
            __Void
        }

        impl ::sea_orm::sea_query::Iden for #rel_enum {
            fn unquoted(&self) -> &str { match self { Self::__Void => "" } }
        }

        impl ::sea_orm::IdenStatic for #rel_enum {
            fn as_str(&self) -> &'static str { match self { Self::__Void => "" } }
        }

        impl ::sea_orm::Iterable for #rel_enum {
            type Iterator = ::std::vec::IntoIter<Self>;
            fn iter() -> Self::Iterator { ::std::vec![Self::__Void].into_iter() }
        }

        impl ::sea_orm::RelationTrait for #rel_enum {
            fn def(&self) -> ::sea_orm::RelationDef {
                match self { Self::__Void => unreachable!() }
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
    entity_name: &syn::Ident,
    active_model_name: &syn::Ident,
) -> TokenStream {
    let table_name = &model.table_name;

    let column_names: Vec<&str> = model
        .fields
        .iter()
        .filter(|f| f.relation.is_none())
        .map(|f| f.column_name.as_str())
        .collect();

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
    if model.has_soft_deletes {
        let has_del = model.fields.iter().any(|f| f.column_name == model.soft_delete_column);
        if !has_del {
            all_columns.push(&model.soft_delete_column);
        }
    }

    let id_col = model
        .fields
        .iter()
        .find(|f| f.is_primary_key || matches!(f.col_type, ColumnType::Id | ColumnType::Uuid))
        .map(|f| f.column_name.as_str())
        .unwrap_or("id");

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

    let rel_fields: Vec<_> = model.fields.iter().filter(|f| f.relation.is_some()).collect();

    let get_columns_fns: Vec<TokenStream> = rel_fields.iter().map(|f| {
        let rel = f.relation.as_ref().unwrap();
        let entity_type = match rel {
            crate::attrs::RelationKind::HasMany { entity_type, .. } => entity_type,
            crate::attrs::RelationKind::HasOne { entity_type, .. } => entity_type,
            crate::attrs::RelationKind::BelongsTo { entity_type, .. } => entity_type,
            crate::attrs::RelationKind::BelongsToMany { entity_type, .. } => entity_type,
        };
        let fn_name = format_ident!("__{}_{}_columns", struct_name, f.field_name);
        quote! {
            fn #fn_name() -> &'static [&'static str] {
                <#entity_type as ravel_eloquent::ModelMeta>::columns()
            }
        }
    }).collect();

    let rel_meta_vars: Vec<TokenStream> = rel_fields.iter().map(|f| {
        let rel = f.relation.as_ref().unwrap();
        let field_name = &f.field_name;
        let fn_name = format_ident!("__{}_{}_columns", struct_name, f.field_name);

        let (kind, rel_table, foreign_key, local_key, pivot_table, pivot_foreign_key, pivot_related_key): (&str, String, String, String, Option<String>, Option<String>, Option<String>) = match rel {
            crate::attrs::RelationKind::HasMany { entity_type, table, .. } => {
                let table = table.as_deref().map(|s| s.to_string()).unwrap_or_else(|| {
                    let type_str = quote::quote!(#entity_type).to_string();
                    to_snake(&type_str)
                });
                let fk_base = table_name.trim_end_matches('s');
                let fk = format!("{}_id", fk_base);
                ("HasMany", table, fk, "id".to_string(), None, None, None)
            }
            crate::attrs::RelationKind::HasOne { entity_type, table } => {
                let table = table.as_deref().map(|s| s.to_string()).unwrap_or_else(|| {
                    let type_str = quote::quote!(#entity_type).to_string();
                    to_snake(&type_str)
                });
                let fk_base = table_name.trim_end_matches('s');
                let fk = format!("{}_id", fk_base);
                ("HasOne", table, fk, "id".to_string(), None, None, None)
            }
            crate::attrs::RelationKind::BelongsTo { entity_type, from, to, table } => {
                let table = table.as_deref().map(|s| s.to_string()).unwrap_or_else(|| {
                    let type_str = quote::quote!(#entity_type).to_string();
                    to_snake(&type_str)
                });
                ("BelongsTo", table, from.clone(), to.clone(), None, None, None)
            }
            crate::attrs::RelationKind::BelongsToMany { entity_type, via, foreign_key: btm_fk, related_key: btm_rk, table } => {
                let table = table.as_deref().map(|s| s.to_string()).unwrap_or_else(|| {
                    let type_str = quote::quote!(#entity_type).to_string();
                    to_snake(&type_str)
                });
                let pivot = via.clone();
                let pk_fk = btm_fk.clone().unwrap_or_else(|| {
                    let base = table_name.trim_end_matches('s');
                    format!("{}_id", base)
                });
                let pk_rk = btm_rk.clone().unwrap_or_else(|| {
                    let base = table.trim_end_matches('s');
                    format!("{}_id", base)
                });
                ("BelongsToMany", table, pk_fk.clone(), "id".to_string(), Some(pivot), Some(pk_fk.clone()), Some(pk_rk))
            }
        };
        let kind_ident = format_ident!("{}", kind);

        let pivot_table_expr = match &pivot_table {
            Some(t) => quote! { ::core::option::Option::Some(#t) },
            None => quote! { ::core::option::Option::None },
        };
        let pivot_fk_expr = match &pivot_foreign_key {
            Some(fk) => quote! { ::core::option::Option::Some(#fk) },
            None => quote! { ::core::option::Option::None },
        };
        let pivot_rk_expr = match &pivot_related_key {
            Some(rk) => quote! { ::core::option::Option::Some(#rk) },
            None => quote! { ::core::option::Option::None },
        };

        quote! {
            ravel_eloquent::RelationMeta {
                field_name: #field_name,
                table_name: #rel_table,
                kind: ravel_eloquent::RelationKind::#kind_ident,
                foreign_key: #foreign_key,
                local_key: #local_key,
                get_columns: #fn_name,
                pivot_table: #pivot_table_expr,
                pivot_foreign_key: #pivot_fk_expr,
                pivot_related_key: #pivot_rk_expr,
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

    let soft_delete_impl = if model.has_soft_deletes {
        let del_col = &model.soft_delete_column;
        quote! {
            fn soft_delete_column() -> ::core::option::Option<&'static str> {
                ::core::option::Option::Some(#del_col)
            }
        }
    } else {
        quote! {}
    };

    // find_column: maps column name string → Column enum variant
    let find_column_arms: Vec<TokenStream> = model.fields.iter()
        .filter(|f| f.relation.is_none())
        .map(|f| {
            let col_name = &f.column_name;
            let variant = format_ident!("{}", to_pascal_case(&f.field_name));
            quote! { #col_name => ::std::option::Option::Some(#column_enum::#variant) }
        })
        .collect();

    quote! {
        impl ravel_eloquent::ModelMeta for #struct_name {
            type Public = #public_name;
            type Columns = #column_enum;
            type Entity = #entity_name;
            type ActiveModel = #active_model_name;

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

            #soft_delete_impl

            fn find_column(name: &str) -> ::std::option::Option<<Self::Entity as ::sea_orm::EntityTrait>::Column> {
                match name {
                    #(#find_column_arms,)*
                    _ => ::std::option::Option::None,
                }
            }
        }

        #(#get_columns_fns)*
    }
}

// ── 5. Inherent methods ────────────────────────────────────────────────────

fn generate_inherent_methods(model: &ModelAttrs, struct_name: &syn::Ident) -> TokenStream {
    let to_public = generate_to_public(model, struct_name);
    let query = generate_query_shorthands(model, struct_name);
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

fn generate_query_shorthands(model: &ModelAttrs, _struct_name: &syn::Ident) -> TokenStream {
    let soft_filter = if model.has_soft_deletes {
        let del_col = &model.soft_delete_column;
        quote! {
            qb = qb.with_soft_delete_filter(#del_col);
        }
    } else {
        quote! {}
    };
    quote! {
        pub fn query() -> ravel_eloquent::QueryBuilder {
            let mut qb = ravel_eloquent::QueryBuilder::new(
                <Self as ravel_eloquent::ModelMeta>::table_name(),
                <Self as ravel_eloquent::ModelMeta>::columns(),
            );
            #soft_filter
            qb
        }

        pub fn query_with_trashed() -> ravel_eloquent::QueryBuilder {
            ravel_eloquent::QueryBuilder::new_with_trashed(
                <Self as ravel_eloquent::ModelMeta>::table_name(),
                <Self as ravel_eloquent::ModelMeta>::columns(),
            )
        }

        pub fn where_str(
            col: &str,
            val: impl Into<sea_orm::Value>,
        ) -> ravel_eloquent::QueryBuilder {
            Self::query().where_str(col, val)
        }
    }
}

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

fn generate_trait_impls(
    model: &ModelAttrs,
    struct_name: &syn::Ident,
    entity_name: &syn::Ident,
    column_enum: &syn::Ident,
) -> TokenStream {
    let public_name = format_ident!("{}Public", struct_name);

    let db_fields: Vec<_> = model.fields.iter().filter(|f| f.relation.is_none()).collect();

    let mut impls: Vec<TokenStream> = Vec::new();

    // ModelExt — auto-implemented by the macro
    impls.push(quote! { impl ravel_eloquent::ModelExt for #struct_name {} });
    impls.push(quote! { impl ravel_eloquent::ActiveModelExt for #struct_name {} });

    // Serializes
    impls.push(quote! {
        impl ravel_eloquent::Serializes for #struct_name {
            fn to_public(&self) -> #public_name {
                self.to_public()
            }
        }
    });

    // Replicates
    impls.push(quote! { impl ravel_eloquent::Replicates for #struct_name {} });

    // Default
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

    // Fillable
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

    // FromQueryResult: serde round-trip
    let col_names: Vec<&str> = model.fields.iter()
        .filter(|f| f.relation.is_none())
        .map(|f| f.column_name.as_str())
        .collect();

    impls.push(quote! {
        impl ::sea_orm::FromQueryResult for #struct_name {
            fn from_query_result(res: &::sea_orm::QueryResult, pre: &str) -> ::std::result::Result<Self, ::sea_orm::DbErr> {
                let cols: &[&str] = &[#(#col_names),*];
                let mut map = ::serde_json::Map::new();
                for col in cols.iter() {
                    use ::sea_orm::TryGetable;
                    let raw: ::std::result::Result<String, _> = res.try_get(pre, col);
                    let json_val = match raw {
                        ::std::result::Result::Ok(s) => ::serde_json::Value::String(s),
                        ::std::result::Result::Err(_) => {
                            let int_val: ::std::result::Result<i64, _> = res.try_get(pre, col);
                            match int_val {
                                ::std::result::Result::Ok(i) => ::serde_json::json!(i),
                                ::std::result::Result::Err(_) => {
                                    let float_val: ::std::result::Result<f64, _> = res.try_get(pre, col);
                                    match float_val {
                                        ::std::result::Result::Ok(f) => ::serde_json::json!(f),
                                        ::std::result::Result::Err(_) => {
                                            let bool_val: ::std::result::Result<bool, _> = res.try_get(pre, col);
                                            match bool_val {
                                                ::std::result::Result::Ok(b) => ::serde_json::json!(b),
                                                ::std::result::Result::Err(_) => ::serde_json::Value::Null,
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    };
                    map.insert(col.to_string(), json_val);
                }
                ::serde_json::from_value(::serde_json::Value::Object(map))
                    .map_err(|e| ::sea_orm::DbErr::Json(e.to_string()))
            }
        }
    });

    // ModelTrait: delegate get/set to column match
    let model_get_arms: Vec<TokenStream> = db_fields.iter().map(|f| {
        let name = format_ident!("{}", f.field_name);
        let col_v = format_ident!("{}", to_pascal_case(&f.field_name));
        quote! { #column_enum::#col_v => ::sea_orm::Value::from(self.#name.clone()) }
    }).collect();
    let model_try_set_arms: Vec<TokenStream> = db_fields.iter().map(|f| {
        let name = format_ident!("{}", f.field_name);
        let col_v = format_ident!("{}", to_pascal_case(&f.field_name));
        let ty = &f.field_type;
        quote! {
            #column_enum::#col_v => {
                let val: #ty = v.unwrap();
                self.#name = val;
                ::std::result::Result::Ok(())
            }
        }
    }).collect();

    impls.push(quote! {
        impl ::sea_orm::ModelTrait for #struct_name {
            type Entity = #entity_name;

            fn get(&self, c: <Self::Entity as ::sea_orm::EntityTrait>::Column) -> ::sea_orm::Value {
                match c {
                    #(#model_get_arms,)*
                }
            }

            fn get_value_type(_c: <Self::Entity as ::sea_orm::EntityTrait>::Column) -> ::sea_orm::sea_query::ArrayType {
                ::sea_orm::sea_query::ArrayType::String
            }

            fn try_set(&mut self, c: <Self::Entity as ::sea_orm::EntityTrait>::Column, v: ::sea_orm::Value) -> ::std::result::Result<(), ::sea_orm::DbErr> {
                match c {
                    #(#model_try_set_arms,)*
                    _ => ::std::result::Result::Err(::sea_orm::DbErr::Type("unknown column".into()))
                }
            }
        }
    });

    quote! { #(#impls)* }
}
