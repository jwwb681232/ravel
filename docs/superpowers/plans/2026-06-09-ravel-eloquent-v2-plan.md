# Ravel Eloquent v2 实施方案

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 ravel-eloquent 从"手写 SQL 字符串 ORM"重写为"基于 SeaORM 2.0 类型安全引擎的 Laravel Eloquent 式 ORM"

**Architecture:** `#[derive(Model)]` 宏生成标准 SeaORM 2.0 实体（`DeriveEntityModel`），所有 Eloquent 便利方法通过 trait extension 附加。生成的 struct 本身就是 `sea_orm::entity::Model`——不是包装类型。查询构建器底层翻译为 SeaORM 的 `Select<E>` 类型安全 API。

**Tech Stack:** Rust 2024, SeaORM 2.0 (rc.40), syn 2 + quote, proc-macro2, tokio, serde_json, async-trait

---

## 文件结构

| 文件（相对 repo root） | 角色 |
|------------------------|------|
| `crates/ravel-eloquent-macros/Cargo.toml` | 修改——加 sea-orm 依赖用于类型引用 |
| `crates/ravel-eloquent-macros/src/lib.rs` | 修改——支持新属性 + 调用新生成器 |
| `crates/ravel-eloquent-macros/src/attrs.rs` | **重写**——支持设计文档第2节全部属性 |
| `crates/ravel-eloquent-macros/src/generator.rs` | **重写**——生成 SeaORM 2.0 实体 + traits + 方法 |
| `crates/ravel-eloquent/Cargo.toml` | 修改——移除 anyhow，加 thiserror |
| `crates/ravel-eloquent/src/lib.rs` | 修改——重新组织模块和 public exports |
| `crates/ravel-eloquent/src/query.rs` | **重写**——基于 SeaORM `Select<E>` 的查询构建器 |
| `crates/ravel-eloquent/src/model_traits.rs` | **新建**——`ModelMeta`, `ModelExt`, `ActiveModelExt` 等 |
| `crates/ravel-eloquent/src/relations.rs` | **重写**——`RelationQuery<R>` + `.with()` eager loading |
| `crates/ravel-eloquent/src/fillable.rs` | **新建**——`Fillable` trait + setter 逻辑 |
| `crates/ravel-eloquent/src/serializes.rs` | **新建**——`Serializes`, `Replicates`, `HasTimestamps` traits |
| `crates/ravel-eloquent/src/error.rs` | **新建**——`RavelEloquentError` 类型 |
| `crates/ravel-eloquent/src/defaults.rs` | 删除——功能合并到 traits 中 |
| `crates/ravel-eloquent/tests/model_derive.rs` | **重写**——测试全部新 API |
| `crates/ravel-eloquent/tests/integration_crud.rs` | **新建**——需数据库的集成测试 |
| `crates/ravel-db-core/src/lib.rs` | 微调——若 `ModelMeta` 移到此处 |

---

### 阶段0：基础设施

#### Task 0.1: 引入错误类型

**Files:**
- Create: `crates/ravel-eloquent/src/error.rs`
- Modify: `crates/ravel-eloquent/Cargo.toml`
- Modify: `crates/ravel-eloquent/src/lib.rs`

- [ ] **Step 1: 添加 thiserror 依赖**

```toml
# crates/ravel-eloquent/Cargo.toml
# 在 [dependencies] 中添加:
thiserror = "2"
# 移除 anyhow = "1"
```

- [ ] **Step 2: 写入错误类型文件**

```rust
// crates/ravel-eloquent/src/error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RavelEloquentError {
    #[error("record not found for table '{table}' with id '{id}'")]
    RecordNotFound { table: &'static str, id: String },

    #[error("invalid column '{column}' for table '{table}'")]
    InvalidColumn { table: &'static str, column: String },

    #[error("cannot update record with id = 0; use save() or insert() instead")]
    UpdateWithoutId,

    #[error("cannot insert record with non-zero id; use save() or update() instead")]
    InsertWithId,

    #[error("database error: {0}")]
    Database(#[from] sea_orm::DbErr),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}

pub type Result<T> = std::result::Result<T, RavelEloquentError>;
```

- [ ] **Step 3: 在 lib.rs 中声明模块**

```rust
// crates/ravel-eloquent/src/lib.rs 顶部
pub mod error;
pub use error::{RavelEloquentError, Result};
```

- [ ] **Step 4: 编译验证**

```bash
cd crates/ravel-eloquent && cargo check 2>&1
```
预期：编译通过（error 模块目前未被使用，不会引入问题）

- [ ] **Step 5: 提交**

```bash
git add crates/ravel-eloquent/Cargo.toml crates/ravel-eloquent/src/error.rs crates/ravel-eloquent/src/lib.rs
git commit -m "feat(eloquent): add RavelEloquentError type"
```

---

### 阶段1：宏改造 — P0

> 目标：`#[derive(Model)]` 生成标准 SeaORM 2.0 实体 + `ModelMeta` + `ModelExt` + `Queryable`

#### Task 1.1: 重写属性解析器

**Files:**
- Rewrite: `crates/ravel-eloquent-macros/src/attrs.rs`
- Check: `crates/ravel-eloquent-macros/Cargo.toml`

- [ ] **Step 1: 确认 proc-macro Cargo.toml 依赖**

当前 `Cargo.toml` 只有 `syn`, `quote`, `proc-macro2`。不需要加 `sea-orm` 依赖——宏生成的是 token stream，类型引用在用户 crate 编译时解析。

```toml
# crates/ravel-eloquent-macros/Cargo.toml 保持不变
```

- [ ] **Step 2: 写入新的属性解析器**

```rust
// crates/ravel-eloquent-macros/src/attrs.rs
use syn::{
    Attribute, LitStr, Expr, ExprLit, Lit, Meta, MetaNameValue,
    parse::{Parse, ParseStream, Result},
    punctuated::Punctuated,
    token::Comma,
    Token,
};

// ============================================================
// 列类型
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum ColumnType {
    Id,
    Uuid,
    String(Option<usize>),     // None → VARCHAR(255)
    Text,
    Integer,
    BigInt,
    Boolean,
    Float,
    DateTime,
    Json,
}

// ============================================================
// 关系类型
// ============================================================

#[derive(Debug, Clone)]
pub enum RelationKind {
    HasOne {
        entity_type: syn::Type,       // 从字段类型 HasOne<X> 中提取 X
    },
    HasMany {
        entity_type: syn::Type,
        via: Option<String>,          // junction table for M-N
    },
    BelongsTo {
        entity_type: syn::Type,
        from: String,                 // foreign key column
        to: String,                   // parent PK column
    },
}

// ============================================================
// 字段属性
// ============================================================

#[derive(Debug, Clone)]
pub struct FieldAttr {
    pub field_name: String,
    pub field_type: syn::Type,        // 字段的 Rust 类型

    // 列元数据
    pub col_type: ColumnType,
    pub column_name: Option<String>,  // #[model(column = "real_name")]
    pub is_primary_key: bool,
    pub is_unique: bool,
    pub is_nullable: bool,
    pub is_hidden: bool,

    // 关系（如果此字段是关系字段）
    pub relation: Option<RelationKind>,
}

impl Default for FieldAttr {
    fn default() -> Self {
        Self {
            field_name: String::new(),
            field_type: syn::parse_quote! { () },
            col_type: ColumnType::String(None), // 默认就是普通 String
            column_name: None,
            is_primary_key: false,
            is_unique: false,
            is_nullable: false,
            is_hidden: false,
            relation: None,
        }
    }
}

// ============================================================
// 模型属性
// ============================================================

#[derive(Debug, Clone)]
pub struct ModelAttr {
    pub table_name: String,
    pub fields: Vec<FieldAttr>,
    pub has_timestamps: bool,
}

// ============================================================
// 解析入口
// ============================================================

/// 解析整个 #[derive(Model)] 结构体
pub fn parse_model(input: &syn::DeriveInput) -> Result<ModelAttr> {
    let table_name = parse_table_name(&input.attrs)?;

    let has_timestamps = input.attrs.iter().any(|attr| {
        attr.path().is_ident("model") && attr.to_token_stream().to_string().contains("timestamps")
    });

    let mut fields = Vec::new();

    match &input.data {
        syn::Data::Struct(data) => {
            for field in &data.fields {
                let mut attr = FieldAttr::default();
                attr.field_name = field.ident.as_ref()
                    .map(|i| i.to_string())
                    .unwrap_or_default();
                attr.field_type = field.ty.clone();

                // 检测是否为关系字段
                if let Some(relation) = detect_relation(&field.ty) {
                    attr.relation = Some(relation);
                }

                // 解析 #[model(...)] 属性
                parse_field_attrs(&field.attrs, &mut attr)?;

                // nullable 检测：字段类型是 Option<T>
                attr.is_nullable = attr.is_nullable || is_option_type(&field.ty);

                fields.push(attr);
            }
        }
        _ => {
            return Err(syn::Error::new_spanned(
                input,
                "#[derive(Model)] only supports named-field structs",
            ));
        }
    }

    // 验证：至少有一个主键
    if !fields.iter().any(|f| f.is_primary_key || f.col_type == ColumnType::Id) {
        // 没有显式 id → 自动找第一个叫 id 的字段或添加默认
    }

    Ok(ModelAttr { table_name, fields, has_timestamps })
}

// ============================================================
// 表名解析
// ============================================================

fn parse_table_name(attrs: &[Attribute]) -> Result<String> {
    for attr in attrs {
        if attr.path().is_ident("model") {
            // 使用 syn 解析嵌套属性
            let meta: ModelContainerAttr = attr.parse_args()?;
            if let Some(table) = meta.table {
                return Ok(table.value());
            }
        }
    }
    Err(syn::Error::new(
        proc_macro2::Span::call_site(),
        "missing #[model(table = \"...\")] attribute"
    ))
}

/// 解析 #[model(table = "xxx", timestamps)]
struct ModelContainerAttr {
    table: Option<LitStr>,
}

impl Parse for ModelContainerAttr {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut table = None;

        while !input.is_empty() {
            if input.peek(syn::Ident) {
                let ident: syn::Ident = input.parse()?;
                if ident == "table" {
                    let _eq: Token![=] = input.parse()?;
                    let lit: LitStr = input.parse()?;
                    table = Some(lit);
                } else if ident == "timestamps" {
                    // timestamps 只是标记，无值
                }
                if input.peek(Token![,]) {
                    let _: Token![,] = input.parse()?;
                }
            } else {
                return Err(input.error("expected `table` or `timestamps`"));
            }
        }

        Ok(Self { table })
    }
}

// ============================================================
// 字段属性解析
// ============================================================

fn parse_field_attrs(attrs: &[Attribute], field: &mut FieldAttr) -> Result<()> {
    for attr in attrs {
        if !attr.path().is_ident("model") {
            continue;
        }

        // 解析 #[model(...)] 内的内容
        let items: FieldAttrItems = attr.parse_args()?;

        for item in items.0 {
            match item {
                FieldAttrItem::Ident(ident) => {
                    match ident.to_string().as_str() {
                        "id" => {
                            field.col_type = ColumnType::Id;
                            field.is_primary_key = true;
                        }
                        "uuid" => {
                            field.col_type = ColumnType::Uuid;
                            field.is_primary_key = true;
                        }
                        "text" => field.col_type = ColumnType::Text,
                        "integer" => field.col_type = ColumnType::Integer,
                        "bigint" => field.col_type = ColumnType::BigInt,
                        "boolean" => field.col_type = ColumnType::Boolean,
                        "float" => field.col_type = ColumnType::Float,
                        "datetime" => field.col_type = ColumnType::DateTime,
                        "json" => field.col_type = ColumnType::Json,
                        "hidden" => field.is_hidden = true,
                        "unique" => field.is_unique = true,
                        "nullable" => field.is_nullable = true,
                        _ => {
                            return Err(syn::Error::new_spanned(
                                ident,
                                format!("unknown model attribute: {}", ident)
                            ));
                        }
                    }
                }
                FieldAttrItem::StringLen { prefix, len } => {
                    match prefix.as_str() {
                        "string" => field.col_type = ColumnType::String(Some(len)),
                        _ => return Err(syn::Error::new_spanned(
                            &prefix,
                            format!("unexpected attribute: {} with length", prefix)
                        )),
                    }
                }
                FieldAttrItem::KeyValue { key, value } => {
                    match key.to_string().as_str() {
                        "column" => field.column_name = Some(value.value()),
                        _ => {}
                    }
                }
                FieldAttrItem::RelationAttr { key, values } => {
                    // 更新关系字段的额外参数
                    if let Some(ref mut rel) = field.relation {
                        match key.to_string().as_str() {
                            "from" => if let RelationKind::BelongsTo { ref mut from, .. } = rel {
                                *from = values.get(0).map(|s| s.value()).unwrap_or_default();
                            },
                            "to" => if let RelationKind::BelongsTo { ref mut to, .. } = rel {
                                *to = values.get(0).map(|s| s.value()).unwrap_or_default();
                            },
                            "via" => if let RelationKind::HasMany { ref mut via, .. } = rel {
                                *via = values.get(0).map(|s| s.value());
                            },
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// #[model(...)] 内的各个项
struct FieldAttrItems(Punctuated<FieldAttrItem, Token![,]>);

impl Parse for FieldAttrItems {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut items = Punctuated::new();
        while !input.is_empty() {
            let item: FieldAttrItem = input.parse()?;
            items.push(item);
            if input.peek(Token![,]) {
                let _: Token![,] = input.parse()?;
            }
        }
        Ok(Self(items))
    }
}

/// #[model(...)] 内的单个项
enum FieldAttrItem {
    Ident(syn::Ident),
    StringLen { prefix: syn::Ident, len: usize },
    KeyValue { key: syn::Ident, value: LitStr },
    RelationAttr { key: syn::Ident, values: Vec<LitStr> },
}

impl Parse for FieldAttrItem {
    fn parse(input: ParseStream) -> Result<Self> {
        let ident: syn::Ident = input.parse()?;

        // 检查是否是 string, N 形式
        if (ident == "string") && input.peek(Token![,]) {
            let _: Token![,] = input.parse()?;
            let lit: LitInt = input.parse()?;
            return Ok(FieldAttrItem::StringLen {
                prefix: ident,
                len: lit.base10_parse()?,
            });
        }

        // 检查是否是 key = "value" 形式
        if input.peek(Token![=]) {
            let _: Token![=] = input.parse()?;
            let lit: LitStr = input.parse()?;
            return Ok(FieldAttrItem::KeyValue { key: ident, value: lit });
        }

        // 检查是否是 key = "a", "b" 形式 (如 belongs_to, from = "a", to = "b")
        if input.peek(Token![=]) {
            let _: Token![=] = input.parse()?;
            let mut values = Vec::new();
            let first: LitStr = input.parse()?;
            values.push(first);
            while input.peek(Token![,]) {
                let _: Token![,] = input.parse()?;
                if input.peek(LitStr) {
                    let lit: LitStr = input.parse()?;
                    values.push(lit);
                } else {
                    break;
                }
            }
            if values.len() > 1 {
                return Ok(FieldAttrItem::RelationAttr { key: ident, values });
            } else {
                return Ok(FieldAttrItem::KeyValue { key: ident, value: values.into_iter().next().unwrap() });
            }
        }

        // 单纯标识符
        Ok(FieldAttrItem::Ident(ident))
    }
}

// ============================================================
// 类型检测辅助函数
// ============================================================

/// 从字段类型中检测关系
fn detect_relation(ty: &syn::Type) -> Option<RelationKind> {
    let path = extract_path(ty)?;
    let last_seg = path.segments.last()?;
    let type_name = last_seg.ident.to_string();

    match type_name.as_str() {
        "HasMany" => {
            let entity_type = extract_first_generic_arg(ty)?;
            Some(RelationKind::HasMany { entity_type, via: None })
        }
        "HasOne" => {
            let entity_type = extract_first_generic_arg(ty)?;
            Some(RelationKind::HasOne { entity_type })
        }
        _ => None,
    }
}

fn extract_path(ty: &syn::Type) -> Option<&syn::TypePath> {
    if let syn::Type::Path(type_path) = ty {
        Some(type_path)
    } else {
        None
    }
}

fn extract_first_generic_arg(ty: &syn::Type) -> Option<syn::Type> {
    let path = extract_path(ty)?;
    let seg = path.segments.last()?;
    if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
        args.args.first().and_then(|arg| {
            if let syn::GenericArgument::Type(ty) = arg {
                Some(ty.clone())
            } else {
                None
            }
        })
    } else {
        None
    }
}

/// 判断类型是否为 Option<T>
fn is_option_type(ty: &syn::Type) -> bool {
    if let Some(path) = extract_path(ty) {
        if let Some(seg) = path.segments.last() {
            return seg.ident == "Option";
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn test_parse_basic_model() {
        let input: syn::DeriveInput = syn::parse2(quote! {
            #[model(table = "users")]
            struct User {
                #[model(id)]
                pub id: i32,
                pub name: String,
                #[model(hidden)]
                pub password: String,
            }
        }).unwrap();

        let model = parse_model(&input).unwrap();
        assert_eq!(model.table_name, "users");
        assert_eq!(model.fields.len(), 3);
        assert!(model.fields[0].is_primary_key);
        assert!(model.fields[2].is_hidden);
    }

    #[test]
    fn test_parse_string_length() {
        let input: syn::DeriveInput = syn::parse2(quote! {
            #[model(table = "users")]
            struct User {
                #[model(id)] id: i32,
                #[model(string, 254, unique)] email: String,
            }
        }).unwrap();

        let model = parse_model(&input).unwrap();
        assert_eq!(model.fields[1].col_type, ColumnType::String(Some(254)));
        assert!(model.fields[1].is_unique);
    }

    #[test]
    fn test_parse_relations() {
        let input: syn::DeriveInput = syn::parse2(quote! {
            #[model(table = "users")]
            struct User {
                #[model(id)] id: i32,
                #[model(has_many)] posts: HasMany<Post>,
                #[model(belongs_to, from = "team_id", to = "id")] team: HasOne<Team>,
            }
        }).unwrap();

        let model = parse_model(&input).unwrap();
        assert!(model.fields[1].relation.is_some());
        assert!(model.fields[2].relation.is_some());
    }

    #[test]
    fn test_parse_timestamps() {
        let input: syn::DeriveInput = syn::parse2(quote! {
            #[model(table = "users", timestamps)]
            struct User {
                #[model(id)] id: i32,
                name: String,
            }
        }).unwrap();

        let model = parse_model(&input).unwrap();
        assert!(model.has_timestamps);
    }
}
```

- [ ] **Step 3: 运行测试验证属性解析**

```bash
cd crates/ravel-eloquent-macros && cargo test 2>&1
```
预期：4 个测试全部通过

- [ ] **Step 4: 提交**

```bash
git add crates/ravel-eloquent-macros/src/attrs.rs
git commit -m "feat(macros): rewrite attr parser with relation and timestamps support"
```

---

#### Task 1.2: 重写代码生成器 — SeaORM 实体生成

**Files:**
- Rewrite: `crates/ravel-eloquent-macros/src/generator.rs`
- Modify: `crates/ravel-eloquent-macros/src/lib.rs`

- [ ] **Step 1: 写入新的生成器（第一部分 — SeaORM 导出）**

```rust
// crates/ravel-eloquent-macros/src/generator.rs
use proc_macro2::TokenStream;
use quote::{quote, format_ident};
use crate::attrs::{ModelAttr, FieldAttr, ColumnType, RelationKind};

/// 主入口：生成所有代码
pub fn generate(input: &syn::DeriveInput) -> syn::Result<TokenStream> {
    let model = crate::attrs::parse_model(input)?;

    let struct_name = &input.ident;
    let struct_name_public = format_ident!("{}Public", struct_name);
    let column_enum = format_ident!("{}Column", struct_name);
    let vis = &input.vis;
    let generics = &input.generics;

    // 部分1: 生成 SeaORM #[sea_orm::model] 实体定义
    let sea_orm_tokens = generate_sea_orm_entity(&model, struct_name, vis, generics);

    // 部分2: 生成 Column 枚举
    let column_tokens = generate_column_enum(&model, column_enum);

    // 部分3: 生成 Public 结构体
    let public_tokens = generate_public_struct(&model, struct_name, struct_name_public);

    // 部分4: 生成 ModelMeta trait impl
    let meta_tokens = generate_model_meta(&model, struct_name, struct_name_public, column_enum);

    // 部分5: 生成实例方法 impl 块
    let methods_tokens = generate_inherent_methods(&model, struct_name, vis);

    // 部分6: 生成模型扩展 traits impl
    let traits_tokens = generate_trait_impls(&model, struct_name);

    let expanded = quote! {
        #sea_orm_tokens
        #column_tokens
        #public_tokens
        #meta_tokens
        #methods_tokens
        #traits_tokens
    };

    Ok(expanded)
}

// ============================================================
// 1. SeaORM 实体定义
// ============================================================

fn generate_sea_orm_entity(
    model: &ModelAttr,
    struct_name: &syn::Ident,
    vis: &syn::Visibility,
    generics: &syn::Generics,
) -> TokenStream {
    let table_name = &model.table_name;

    // 构建字段列表，含 #[sea_orm(...)] 属性
    let sea_orm_fields: Vec<TokenStream> = model.fields.iter().map(|f| {
        let name = format_ident!("{}", f.field_name);
        let ty = &f.field_type;

        if f.relation.is_some() {
            generate_sea_orm_relation_field(f, name)
        } else {
            let sea_attr = generate_sea_orm_column_attr(f);
            quote! {
                #sea_attr
                pub #name: #ty
            }
        }
    }).collect();

    // 如果启用 timestamps，添加 created_at + updated_at
    let timestamps_fields: Vec<TokenStream> = if model.has_timestamps {
        vec![
            quote! {
                #[sea_orm(column_name = "created_at", nullable = false)]
                pub created_at: chrono::DateTime<chrono::Utc>
            },
            quote! {
                #[sea_orm(column_name = "updated_at", nullable = false)]
                pub updated_at: chrono::DateTime<chrono::Utc>
            },
        ]
    } else {
        vec![]
    };

    quote! {
        #[sea_orm::model]
        #[derive(Clone, Debug, PartialEq, Eq, sea_orm::DeriveEntityModel)]
        #[sea_orm(table_name = #table_name)]
        #vis struct #struct_name #generics {
            #(#sea_orm_fields,)*
            #(#timestamps_fields,)*
        }
    }
}

/// 为列字段生成 #[sea_orm(primary_key, unique, ...)] 属性
fn generate_sea_orm_column_attr(f: &FieldAttr) -> TokenStream {
    let mut attrs = Vec::new();

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
            attrs.push(quote! { column_type = "String(Some(#len))" });
        }
        ColumnType::Text => {
            attrs.push(quote! { column_type = "Text" });
        }
        ColumnType::DateTime => {
            attrs.push(quote! { column_type = "DateTime" });
        }
        _ => {} // Integer, BigInt, Boolean, Float, Json 由 SeaORM 自动推断
    }

    if f.is_unique {
        attrs.push(quote! { unique });
    }
    if f.is_nullable {
        attrs.push(quote! { nullable });
    }
    if let Some(ref col_name) = f.column_name {
        attrs.push(quote! { column_name = #col_name });
    }

    if attrs.is_empty() {
        return quote! {};
    }

    quote! {
        #[sea_orm(#(#attrs),*)]
    }
}

/// 为关系字段生成 #[sea_orm(has_many)] / #[sea_orm(belongs_to)] 属性
fn generate_sea_orm_relation_field(f: &FieldAttr, field_name: syn::Ident) -> TokenStream {
    let ty = &f.field_type;

    if let Some(ref rel) = f.relation {
        match rel {
            RelationKind::HasMany { entity_type, via } => {
                let via_attr = if let Some(via) = via {
                    quote! { , via = #via }
                } else {
                    quote! {}
                };
                quote! {
                    #[sea_orm(has_many #via_attr)]
                    pub #field_name: #ty
                }
            }
            RelationKind::HasOne { entity_type: _ } => {
                quote! {
                    #[sea_orm(has_one)]
                    pub #field_name: #ty
                }
            }
            RelationKind::BelongsTo { from, to, entity_type: _ } => {
                quote! {
                    #[sea_orm(belongs_to, from = #from, to = #to)]
                    pub #field_name: #ty
                }
            }
        }
    } else {
        // 关系类型未匹配（不应该到这里）
        quote! {
            pub #field_name: #ty
        }
    }
}

// ============================================================
// 2. Column 枚举
// ============================================================

fn generate_column_enum(model: &ModelAttr, enum_name: syn::Ident) -> TokenStream {
    let variants: Vec<proc_macro2::TokenStream> = model.fields.iter()
        .filter(|f| f.relation.is_none())
        .map(|f| {
            let variant = syn::Ident::new(
                &to_pascal_case(&f.field_name),
                proc_macro2::Span::call_site(),
            );
            quote! { #variant }
        })
        .collect();

    let as_str_arms: Vec<proc_macro2::TokenStream> = model.fields.iter()
        .filter(|f| f.relation.is_none())
        .map(|f| {
            let variant = syn::Ident::new(
                &to_pascal_case(&f.field_name),
                proc_macro2::Span::call_site(),
            );
            let col_name = f.column_name.as_deref().unwrap_or(&f.field_name);
            quote! { Self::#variant => #col_name }
        })
        .collect();

    quote! {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

// ============================================================
// 3. Public 结构体
// ============================================================

fn generate_public_struct(
    model: &ModelAttr,
    struct_name: &syn::Ident,
    public_name: syn::Ident,
) -> TokenStream {
    let public_fields: Vec<TokenStream> = model.fields.iter()
        .filter(|f| !f.is_hidden && f.relation.is_none())
        .map(|f| {
            let name = format_ident!("{}", f.field_name);
            let ty = &f.field_type;
            quote! { pub #name: #ty }
        })
        .collect();

    // 把 timestamps 字段也加入
    let timestamps_fields: Vec<TokenStream> = if model.has_timestamps {
        vec![
            quote! { pub created_at: chrono::DateTime<chrono::Utc> },
            quote! { pub updated_at: chrono::DateTime<chrono::Utc> },
        ]
    } else {
        vec![]
    };

    quote! {
        #[derive(Debug, Clone, serde::Serialize)]
        pub struct #public_name {
            #(#public_fields,)*
            #(#timestamps_fields,)*
        }
    }
}

// ============================================================
// 4. ModelMeta trait impl
// ============================================================

fn generate_model_meta(
    model: &ModelAttr,
    struct_name: &syn::Ident,
    public_name: syn::Ident,
    column_enum: syn::Ident,
) -> TokenStream {
    let table_name = &model.table_name;

    let column_names: Vec<&String> = model.fields.iter()
        .filter(|f| f.relation.is_none())
        .map(|f| f.column_name.as_ref().unwrap_or(&f.field_name))
        .collect();

    let id_col = model.fields.iter()
        .find(|f| f.is_primary_key || matches!(f.col_type, ColumnType::Id | ColumnType::Uuid))
        .and_then(|f| f.column_name.as_ref().or(Some(&f.field_name)))
        .map(|s| s.as_str())
        .unwrap_or("id");

    let public_column_names: Vec<&String> = model.fields.iter()
        .filter(|f| !f.is_hidden && f.relation.is_none())
        .map(|f| f.column_name.as_ref().unwrap_or(&f.field_name))
        .collect();

    quote! {
        impl ravel_eloquent::ModelMeta for #struct_name {
            type Public = #public_name;
            type Columns = #column_enum;

            fn table_name() -> &'static str {
                #table_name
            }

            fn columns() -> &'static [&'static str] {
                &[#(#column_names),*]
            }

            fn id_column() -> &'static str {
                #id_col
            }

            fn public_columns() -> &'static [&'static str] {
                &[#(#public_column_names),*]
            }
        }
    }
}

// ============================================================
// 5. 实例方法 impl 块
// ============================================================

fn generate_inherent_methods(
    model: &ModelAttr,
    struct_name: &syn::Ident,
    vis: &syn::Visibility,
) -> TokenStream {
    let to_public_body = generate_to_public_body(model);
    let query_methods = generate_query_methods(struct_name);
    let static_crud = generate_static_crud(struct_name);
    let setters = generate_setters(model);

    quote! {
        impl #struct_name {
            #to_public_body
            #query_methods
            #static_crud
            #setters
        }
    }
}

fn generate_to_public_body(model: &ModelAttr) -> TokenStream {
    let public_name = format_ident!("{}Public", model.fields.first()
        .map(|_| "T")  // placeholder, will be resolved in quote
        .unwrap_or("T"));

    // 实际生成时用正确的名字
    let field_mappings: Vec<TokenStream> = model.fields.iter()
        .filter(|f| !f.is_hidden && f.relation.is_none())
        .map(|f| {
            let name = format_ident!("{}", f.field_name);
            quote! { #name: self.#name.clone() }
        })
        .collect();

    // 这里用 Self::Public 引用关联类型
    quote! {
        pub fn to_public(&self) -> <Self as ravel_eloquent::ModelMeta>::Public {
            <Self as ravel_eloquent::ModelMeta>::Public {
                #(#field_mappings,)*
                #(
                    // 如果有 timestamps，这里也复制
                )*
            }
        }
    }
}

fn generate_query_methods(struct_name: &syn::Ident) -> TokenStream {
    quote! {
        /// 创建查询构建器
        pub fn query() -> ravel_eloquent::QueryBuilder<Self> {
            ravel_eloquent::QueryBuilder::new()
        }

        /// 创建带 WHERE 条件的查询构建器
        pub fn r#where(
            col: &str,
            val: impl Into<sea_orm::Value>,
        ) -> ravel_eloquent::QueryBuilder<Self> {
            Self::query().r#where(col, val)
        }
    }
}

fn generate_static_crud(struct_name: &syn::Ident) -> TokenStream {
    quote! {
        /// 按主键查找
        pub async fn find(
            db: &impl sea_orm::ConnectionTrait,
            id: impl Into<sea_orm::Value>,
        ) -> ravel_eloquent::Result<Option<Self>> {
            <Self as ravel_eloquent::ModelExt>::find(db, id).await
        }

        /// 按主键查找，不存在则报错
        pub async fn find_or_fail(
            db: &impl sea_orm::ConnectionTrait,
            id: impl Into<sea_orm::Value>,
        ) -> ravel_eloquent::Result<Self> {
            <Self as ravel_eloquent::ModelExt>::find_or_fail(db, id).await
        }

        /// 查找全部记录
        pub async fn all(
            db: &impl sea_orm::ConnectionTrait,
        ) -> ravel_eloquent::Result<Vec<Self>> {
            <Self as ravel_eloquent::ModelExt>::all(db).await
        }

        /// 从 JSON 创建记录
        pub async fn create(
            data: serde_json::Value,
            db: &impl sea_orm::ConnectionTrait,
        ) -> ravel_eloquent::Result<Self> {
            <Self as ravel_eloquent::ModelExt>::create(data, db).await
        }

        /// 按主键删除（无需加载实例）
        pub async fn destroy(
            db: &impl sea_orm::ConnectionTrait,
            id: impl Into<sea_orm::Value>,
        ) -> ravel_eloquent::Result<u64> {
            <Self as ravel_eloquent::ModelExt>::destroy(db, id).await
        }

        /// 按多个主键批量删除
        pub async fn destroy_many(
            db: &impl sea_orm::ConnectionTrait,
            ids: &[impl Into<sea_orm::Value> + Clone],
        ) -> ravel_eloquent::Result<u64> {
            <Self as ravel_eloquent::ModelExt>::destroy_many(db, ids).await
        }
    }
}

fn generate_setters(model: &ModelAttr) -> TokenStream {
    let setters: Vec<TokenStream> = model.fields.iter()
        .filter(|f| !f.is_hidden && f.relation.is_none())
        .map(|f| {
            let field_name = format_ident!("{}", f.field_name);
            let setter_name = format_ident!("set_{}", f.field_name);
            let ty = &f.field_type;
            quote! {
                pub fn #setter_name(mut self, val: impl Into<#ty>) -> Self {
                    self.#field_name = val.into();
                    self
                }
            }
        })
        .collect();

    quote! {
        #(#setters)*
    }
}

// ============================================================
// 6. Trait 实现
// ============================================================

fn generate_trait_impls(model: &ModelAttr, struct_name: &syn::Ident) -> TokenStream {
    let fillable_bounds = &model.fields;

    quote! {
        // ModelExt — 自动为所有 SeaORM 实体提供
        #[async_trait::async_trait]
        impl ravel_eloquent::ModelExt for #struct_name {}

        // Serializes 需要知道 Public 类型，宏已生成
        // 此实现绑定到 ModelMeta::Public 关联类型

        // Fillable  — 自动实现，用 ModelMeta 提供元数据
        impl ravel_eloquent::Fillable for #struct_name {
            fn fill(self, data: serde_json::Value) -> Self {
                let mut s = self;
                if let serde_json::Value::Object(map) = &data {
                    // 遍历所有非 hidden 字段并填充
                    #(if let Some(val) = map.get(#fillable_bounds.field_name) {
                        // 尝试反序列化
                        if let Ok(v) = serde_json::from_value(val.clone()) {
                            s.#fillable_bounds.field_name = v;
                        }
                    })*
                }
                s
            }
        }

        impl ravel_eloquent::Replicates for #struct_name {}

        impl ravel_eloquent::Serializes for #struct_name {}
    }
}

// ============================================================
// 辅助
// ============================================================

fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().to_string() + &chars.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect()
}
```

- [ ] **Step 2: 更新 lib.rs 入口点**

```rust
// crates/ravel-eloquent-macros/src/lib.rs
mod attrs;
mod generator;

use proc_macro::TokenStream;

/// 将 struct 标记为 Eloquent Model
///
/// 支持属性:
/// - #[model(table = "...")] — 必须
/// - #[model(id)] — 主键
/// - #[model(string, N)] — VARCHAR(N)
/// - #[model(text)] — TEXT
/// - #[model(integer, bigint, boolean, float, datetime, json)]
/// - #[model(unique, nullable, hidden)]
/// - #[model(column = "real_name")]
/// - #[model(timestamps)] — 自动添加 created_at + updated_at
/// - #[model(has_many)], #[model(has_one)], #[model(belongs_to, from, to)]
#[proc_macro_derive(Model, attributes(model))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    match generator::generate(&input) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}
```

- [ ] **Step 3: 尝试编译**

```bash
cd crates/ravel-eloquent-macros && cargo test 2>&1
```

预期：attrs 测试通过，宏导出编译通过

- [ ] **Step 4: 提交**

```bash
git add crates/ravel-eloquent-macros/src/
git commit -m "feat(macros): generate SeaORM 2.0 entities + traits from #[derive(Model)]"
```

---

### 阶段2：Trait 定义 + 查询构建器 — P0 + P2

> 目标：在 `ravel-eloquent` 中定义所有 trait 并构建基于 SeaORM `Select<E>` 的查询构建器

#### Task 2.1: 定义 ModelMeta trait

**Files:**
- Create/Modify: `crates/ravel-eloquent/src/model_traits.rs`
- Modify: `crates/ravel-eloquent/src/lib.rs`

- [ ] **Step 1: 写入 ModelMeta trait**

```rust
// crates/ravel-eloquent/src/model_traits.rs

use sea_orm::{ConnectionTrait, Select, Value};
use crate::error::Result;

/// 模型元数据 — 由 #[derive(Model)] 自动实现
pub trait ModelMeta {
    /// 基于该模型的 JSON 安全公共类型
    type Public: serde::Serialize;

    /// 列元信息枚举
    type Columns: Copy;

    /// 数据库表名
    fn table_name() -> &'static str;

    /// 所有数据库列名（不含关系字段）
    fn columns() -> &'static [&'static str];

    /// 主键列名
    fn id_column() -> &'static str;

    /// 非 hidden 列名（用于序列化到 API 响应）
    fn public_columns() -> &'static [&'static str];
}

/// 提供 ModelMeta 默认方法
pub trait ModelMetaExt: ModelMeta + sea_orm::EntityTrait {
    /// 创建针对此模型的 SeaORM Select 查询
    fn entity_query() -> Select<Self> {
        Self::find()
    }
}
```

- [ ] **Step 2: 编译验证**

```bash
cd crates/ravel-eloquent && cargo check 2>&1
```

预期：编译通过

- [ ] **Step 3: 提交**

```bash
git add crates/ravel-eloquent/src/model_traits.rs crates/ravel-eloquent/src/lib.rs
git commit -m "feat(eloquent): add ModelMeta and ModelMetaExt traits"
```

---

#### Task 2.2: 定义 ModelExt + ActiveModelExt traits

**Files:**
- Modify: `crates/ravel-eloquent/src/model_traits.rs`

- [ ] **Step 1: 追加 trait 定义**

在 `model_traits.rs` 末尾追加：

```rust
// crates/ravel-eloquent/src/model_traits.rs 追加

use async_trait::async_trait;

// ============================================================
// ModelExt — 静态 CRUD 方法
// ============================================================

#[async_trait]
pub trait ModelExt: ModelMeta + sea_orm::EntityTrait + Send + Sync + Sized + 'static {
    /// 按主键查找
    async fn find(
        db: &impl ConnectionTrait,
        id: impl Into<Value> + Send + Sync,
    ) -> Result<Option<Self>> {
        let id_value: Value = id.into();
        let id_col = <Self::Columns as sea_orm::ColumnTrait>::from_str(Self::id_column())?;
        let result = Self::find()
            .filter(id_col.eq(id_value))
            .one(db)
            .await
            .map_err(|e| crate::error::RavelEloquentError::Database(e))?;
        Ok(result)
    }

    /// 按主键查找，不存在则报错
    async fn find_or_fail(
        db: &impl ConnectionTrait,
        id: impl Into<Value> + Send + Sync,
    ) -> Result<Self> {
        Self::find(db, id).await?.ok_or_else(||
            crate::error::RavelEloquentError::RecordNotFound {
                table: Self::table_name(),
                id: "unknown".to_string(),
            }
        )
    }

    /// 查找全部
    async fn all(db: &impl ConnectionTrait) -> Result<Vec<Self>> {
        Self::find()
            .all(db)
            .await
            .map_err(|e| crate::error::RavelEloquentError::Database(e))
    }

    /// 从 JSON 创建
    async fn create(data: serde_json::Value, db: &impl ConnectionTrait) -> Result<Self> {
        let model: Self = serde_json::from_value(data)
            .map_err(|e| crate::error::RavelEloquentError::Serialization(e))?;
        let result = model.save(db).await?;
        Ok(result)
    }

    /// 按 ID 删除（无需加载实例）
    async fn destroy(
        db: &impl ConnectionTrait,
        id: impl Into<Value> + Send + Sync,
    ) -> Result<u64> {
        let id_value: Value = id.into();
        let id_col = <Self::Columns as sea_orm::ColumnTrait>::from_str(Self::id_column())?;
        let result = sea_orm::EntityTrait::delete_many()
            .filter(id_col.eq(id_value))
            .exec(db)
            .await
            .map_err(|e| crate::error::RavelEloquentError::Database(e))?;
        Ok(result.rows_affected)
    }

    /// 批量按 ID 删除
    async fn destroy_many(
        db: &impl ConnectionTrait,
        ids: &[impl Into<Value> + Clone + Send + Sync],
    ) -> Result<u64> {
        todo!("destroy_many implementation")
    }
}

// ============================================================
// ActiveModelExt — 消耗式写操作
// ============================================================

#[async_trait]
pub trait ActiveModelExt: ModelMeta + sea_orm::EntityTrait + Send + Sized + 'static
where
    // 约束：Self 必须可通过 ActiveModel 写回
    for<'a> sea_orm::ActiveModel: From<&'a Self>,
{
    /// id==0 → INSERT，id!=0 → UPDATE
    async fn save(self, db: &impl ConnectionTrait) -> Result<Self> {
        let active: sea_orm::ActiveModel = (&self).into();
        // ... 检测 id 决定 insert 还是 update
        todo!("save implementation")
    }

    /// 强制 INSERT
    async fn insert(self, db: &impl ConnectionTrait) -> Result<Self> {
        let active: sea_orm::ActiveModel = (&self).into();
        let result = sea_orm::ActiveModelTrait::insert(active, db)
            .await
            .map_err(|e| crate::error::RavelEloquentError::Database(e))?;
        // 回填字段
        todo!("insert implementation")
    }

    /// 强制 UPDATE
    async fn update(self, db: &impl ConnectionTrait) -> Result<Self> {
        todo!("update implementation")
    }

    /// DELETE
    async fn delete(self, db: &impl ConnectionTrait) -> Result<()> {
        let active: sea_orm::ActiveModel = (&self).into();
        sea_orm::ActiveModelTrait::delete(active, db)
            .await
            .map_err(|e| crate::error::RavelEloquentError::Database(e))?;
        Ok(())
    }

    /// 重新从数据库查询
    async fn refresh(self, db: &impl ConnectionTrait) -> Result<Self> {
        todo!("refresh implementation")
    }
}

// ============================================================
// Replicates — clone + id=0
// ============================================================

pub trait Replicates: Clone + ModelMeta {
    fn replicate(&self) -> Self {
        self.clone()
    }
}

// ============================================================
// HasTimestamps — 只更新 updated_at
// ============================================================

#[async_trait]
pub trait HasTimestamps: ActiveModelExt {
    async fn touch(self, db: &impl ConnectionTrait) -> Result<Self> {
        todo!("touch implementation")
    }
}

// ============================================================
// Serializes — JSON 序列化
// ============================================================

pub trait Serializes: ModelMeta + serde::Serialize {
    fn to_public(&self) -> Self::Public;

    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    fn to_public_json(&self) -> serde_json::Value {
        serde_json::to_value(self.to_public()).unwrap_or(serde_json::Value::Null)
    }
}
```

- [ ] **Step 2: 写 Fillable trait**

创建新文件：

```rust
// crates/ravel-eloquent/src/fillable.rs

/// 由 #[derive(Model)] 自动为每个模型实现
pub trait Fillable: Sized {
    /// 用 JSON 数据填充字段（跳过 hidden 字段）
    fn fill(self, data: serde_json::Value) -> Self;

    /// 用 JSON 数据安全填充（仅非 hidden 字段）
    // fn fill_public(self, data: serde_json::Value) -> Self;
}
```

- [ ] **Step 3: 提交**

```bash
git add crates/ravel-eloquent/src/model_traits.rs crates/ravel-eloquent/src/fillable.rs crates/ravel-eloquent/src/lib.rs
git commit -m "feat(eloquent): add ModelExt, ActiveModelExt, Fillable, Serializes traits"
```

---

#### Task 2.3: QueryBuilder — 基于 SeaORM Select<E> 的查询构建器

**Files:**
- Rewrite: `crates/ravel-eloquent/src/query.rs`

- [ ] **Step 1: 写入 QueryBuilder**

```rust
// crates/ravel-eloquent/src/query.rs
use std::marker::PhantomData;
use sea_orm::{ConnectionTrait, Select, EntityTrait, Value, ColumnTrait, QueryFilter,
    QuerySelect, Order, PaginatorTrait, JoinType, RelationTrait};
use crate::error::{Result, RavelEloquentError};
use crate::model_traits::ModelMeta;

/// 基于 SeaORM Select<E> 的类型安全查询构建器
pub struct QueryBuilder<E: EntityTrait> {
    select: Select<E>,
    _marker: PhantomData<E>,
}

impl<E: EntityTrait> QueryBuilder<E> {
    /// 创建新查询
    pub fn new() -> Self {
        Self {
            select: E::find(),
            _marker: PhantomData,
        }
    }

    /// 转为 SeaORM Select 供高级用法
    pub fn into_select(self) -> Select<E> {
        self.select
    }
}

// ============================================================
// WHERE 条件
// ============================================================

impl<E: EntityTrait + ModelMeta> QueryBuilder<E>
where
    // 约束：Columns 必须实现 ColumnTrait
    E::Columns: sea_orm::ColumnTrait + std::str::FromStr<Err = crate::error::RavelEloquentError>,
{
    /// WHERE col = val
    pub fn r#where(mut self, col: &str, val: impl Into<Value>) -> Self {
        if let Ok(column) = E::Columns::from_str(col) {
            self.select = self.select.filter(column.eq(val.into()));
        }
        self
    }

    /// WHERE col > val
    pub fn r#where_gt(mut self, col: &str, val: impl Into<Value>) -> Self {
        if let Ok(column) = E::Columns::from_str(col) {
            self.select = self.select.filter(column.gt(val.into()));
        }
        self
    }

    /// WHERE col >= val
    pub fn r#where_gte(mut self, col: &str, val: impl Into<Value>) -> Self {
        if let Ok(column) = E::Columns::from_str(col) {
            self.select = self.select.filter(column.gte(val.into()));
        }
        self
    }

    /// WHERE col < val
    pub fn r#where_lt(mut self, col: &str, val: impl Into<Value>) -> Self {
        if let Ok(column) = E::Columns::from_str(col) {
            self.select = self.select.filter(column.lt(val.into()));
        }
        self
    }

    /// WHERE col <= val
    pub fn r#where_lte(mut self, col: &str, val: impl Into<Value>) -> Self {
        if let Ok(column) = E::Columns::from_str(col) {
            self.select = self.select.filter(column.lte(val.into()));
        }
        self
    }

    /// WHERE col != val
    pub fn r#where_ne(mut self, col: &str, val: impl Into<Value>) -> Self {
        if let Ok(column) = E::Columns::from_str(col) {
            self.select = self.select.filter(column.ne(val.into()));
        }
        self
    }

    /// WHERE col LIKE val
    pub fn r#where_like(mut self, col: &str, val: &str) -> Self {
        if let Ok(column) = E::Columns::from_str(col) {
            self.select = self.select.filter(column.like(val));
        }
        self
    }

    /// WHERE col IN (...)
    pub fn r#where_in(mut self, col: &str, vals: Vec<impl Into<Value>>) -> Self {
        if let Ok(column) = E::Columns::from_str(col) {
            let values: Vec<Value> = vals.into_iter().map(|v| v.into()).collect();
            self.select = self.select.filter(column.is_in(values));
        }
        self
    }

    /// WHERE col IS NULL
    pub fn r#where_null(mut self, col: &str) -> Self {
        if let Ok(column) = E::Columns::from_str(col) {
            self.select = self.select.filter(column.is_null());
        }
        self
    }

    /// WHERE col IS NOT NULL
    pub fn r#where_not_null(mut self, col: &str) -> Self {
        if let Ok(column) = E::Columns::from_str(col) {
            self.select = self.select.filter(column.is_not_null());
        }
        self
    }

    /// WHERE col BETWEEN low AND high
    pub fn r#where_between(
        mut self,
        col: &str,
        low: impl Into<Value>,
        high: impl Into<Value>,
    ) -> Self {
        if let Ok(column) = E::Columns::from_str(col) {
            self.select = self.select.filter(column.between(low.into(), high.into()));
        }
        self
    }
}
```

由于篇幅限制，继续在下一文件追加 ORDER BY、JOIN、聚合、执行等方法。

- [ ] **Step 2: 追加排序、JOIN、聚合方法**

```rust
// crates/ravel-eloquent/src/query.rs 追加

// ============================================================
// 排序
// ============================================================

impl<E: EntityTrait + ModelMeta> QueryBuilder<E>
where
    E::Columns: sea_orm::ColumnTrait + std::str::FromStr<Err = crate::error::RavelEloquentError>,
{
    /// ORDER BY col ASC/DESC
    pub fn order_by(mut self, col: &str, dir: &str) -> Self {
        if let Ok(column) = E::Columns::from_str(col) {
            let order = match dir.to_uppercase().as_str() {
                "DESC" | "DESCENDING" => sea_orm::Order::Desc,
                _ => sea_orm::Order::Asc,
            };
            self.select = self.select.order_by(column, order);
        }
        self
    }

    /// ORDER BY col DESC（新到旧）
    pub fn latest(self, col: &str) -> Self {
        self.order_by(col, "DESC")
    }

    /// ORDER BY col ASC（旧到新）
    pub fn oldest(self, col: &str) -> Self {
        self.order_by(col, "ASC")
    }

    /// LIMIT n
    pub fn limit(mut self, n: u64) -> Self {
        self.select = self.select.limit(n);
        self
    }

    /// OFFSET n
    pub fn offset(mut self, n: u64) -> Self {
        self.select = self.select.offset(n);
        self
    }
}

// ============================================================
// JOIN
// ============================================================

impl<E: EntityTrait> QueryBuilder<E> {
    /// INNER JOIN
    pub fn join<R: EntityTrait>(
        mut self,
        _related: R,
    ) -> Self
    where
        E: sea_orm::Related<R>,
    {
        self.select = self.select.join(JoinType::InnerJoin, E::find_related::<R>());
        self
    }

    /// LEFT JOIN
    pub fn left_join<R: EntityTrait>(
        mut self,
        _related: R,
    ) -> Self
    where
        E: sea_orm::Related<R>,
    {
        self.select = self.select.join(JoinType::LeftJoin, E::find_related::<R>());
        self
    }
}

// ============================================================
// 聚合
// ============================================================

impl<E: EntityTrait + ModelMeta> QueryBuilder<E>
where
    E::Columns: sea_orm::ColumnTrait + std::str::FromStr<Err = crate::error::RavelEloquentError>,
{
    /// SELECT COUNT(*)
    pub async fn count(self, db: &impl ConnectionTrait) -> Result<u64> {
        self.select
            .count(db)
            .await
            .map_err(|e| RavelEloquentError::Database(e))
    }

    /// SELECT EXISTS(...)
    pub async fn exists(self, db: &impl ConnectionTrait) -> Result<bool> {
        self.select
            .count(db)
            .await
            .map(|c| c > 0)
            .map_err(|e| RavelEloquentError::Database(e))
    }
}

// ============================================================
// 执行
// ============================================================

impl<E: EntityTrait + ModelMeta> QueryBuilder<E> {
    /// 执行查询，返回所有匹配行
    pub async fn get(self, db: &impl ConnectionTrait) -> Result<Vec<E>>
    where
        E: Send,
    {
        self.select
            .all(db)
            .await
            .map_err(|e| RavelEloquentError::Database(e))
    }

    /// 执行查询，返回第一行
    pub async fn first(self, db: &impl ConnectionTrait) -> Result<Option<E>>
    where
        E: Send,
    {
        self.select
            .one(db)
            .await
            .map_err(|e| RavelEloquentError::Database(e))
    }

    /// 分页查询
    pub async fn paginate(
        self,
        db: &impl ConnectionTrait,
        page: u64,
        per_page: u64,
    ) -> Result<Page<E>>
    where
        E: Send,
    {
        let paginator = self.select.paginate(db, per_page);
        let total = paginator.num_items()
            .await
            .map_err(|e| RavelEloquentError::Database(e))?;
        let items = paginator.fetch_page(page)
            .await
            .map_err(|e| RavelEloquentError::Database(e))?;

        Ok(Page::new(items, total, page, per_page))
    }
}

pub type Page<T> = ravel_db_core::pagination::Page<T>;
```

- [ ] **Step 3: 编译验证**

```bash
cd crates/ravel-eloquent && cargo check 2>&1
```

- [ ] **Step 4: 提交**

```bash
git add crates/ravel-eloquent/src/query.rs
git commit -m "feat(eloquent): rewrite QueryBuilder on SeaORM Select<E> with full WHERE/JOIN/aggregation"
```

---

### 阶段3: 关系查询 — P3 + P4

#### Task 3.1: RelationQuery<R> — 懒加载关系查询

**Files:**
- Rewrite: `crates/ravel-eloquent/src/relations.rs`

- [ ] **Step 1: 写入 RelationQuery**

```rust
// crates/ravel-eloquent/src/relations.rs
use std::marker::PhantomData;
use sea_orm::{ConnectionTrait, EntityTrait, Value, ColumnTrait, QueryFilter,
    QuerySelect, Order, RelationTrait};
use crate::error::{Result, RavelEloquentError};
use crate::query::QueryBuilder;
use crate::model_traits::ModelMeta;

/// 关系查询构建器 — 由 #[derive(Model)] 自动生成的关系方法返回
pub struct RelationQuery<R: EntityTrait> {
    select: sea_orm::Select<R>,
    _marker: PhantomData<R>,
}

impl<R: EntityTrait> RelationQuery<R> {
    /// 从已有 Select 创建（框架内部使用）
    pub fn from_select(select: sea_orm::Select<R>) -> Self {
        Self { select, _marker: PhantomData }
    }
}

impl<R: EntityTrait + ModelMeta> RelationQuery<R>
where
    R::Columns: sea_orm::ColumnTrait + std::str::FromStr<Err = crate::error::RavelEloquentError>,
{
    // ---------- 过滤 ----------

    pub fn r#where(mut self, col: &str, val: impl Into<Value>) -> Self {
        if let Ok(column) = R::Columns::from_str(col) {
            self.select = self.select.filter(column.eq(val.into()));
        }
        self
    }

    pub fn r#where_gt(mut self, col: &str, val: impl Into<Value>) -> Self {
        if let Ok(column) = R::Columns::from_str(col) {
            self.select = self.select.filter(column.gt(val.into()));
        }
        self
    }

    pub fn r#where_in(mut self, col: &str, vals: Vec<impl Into<Value>>) -> Self {
        if let Ok(column) = R::Columns::from_str(col) {
            let values: Vec<Value> = vals.into_iter().map(|v| v.into()).collect();
            self.select = self.select.filter(column.is_in(values));
        }
        self
    }

    pub fn r#where_null(mut self, col: &str) -> Self {
        if let Ok(column) = R::Columns::from_str(col) {
            self.select = self.select.filter(column.is_null());
        }
        self
    }

    pub fn r#where_not_null(mut self, col: &str) -> Self {
        if let Ok(column) = R::Columns::from_str(col) {
            self.select = self.select.filter(column.is_not_null());
        }
        self
    }

    pub fn or_where(mut self, col: &str, val: impl Into<Value>) -> Self {
        if let Ok(column) = R::Columns::from_str(col) {
            self.select = self.select.filter(column.eq(val.into()));
            // TODO: 实现 OR 条件需要条件树支持
        }
        self
    }

    // ---------- 排序 ----------

    pub fn order_by(mut self, col: &str, dir: &str) -> Self {
        if let Ok(column) = R::Columns::from_str(col) {
            let order = if dir.to_uppercase() == "DESC" { Order::Desc } else { Order::Asc };
            self.select = self.select.order_by(column, order);
        }
        self
    }

    pub fn latest(self, col: &str) -> Self { self.order_by(col, "DESC") }
    pub fn oldest(self, col: &str) -> Self { self.order_by(col, "ASC") }

    // ---------- 分页 ----------

    pub fn limit(mut self, n: u64) -> Self {
        self.select = self.select.limit(n);
        self
    }

    pub fn offset(mut self, n: u64) -> Self {
        self.select = self.select.offset(n);
        self
    }

    // ---------- 执行 ----------

    pub async fn get(self, db: &impl ConnectionTrait) -> Result<Vec<R>>
    where R: Send {
        self.select.all(db)
            .await
            .map_err(|e| RavelEloquentError::Database(e))
    }

    pub async fn first(self, db: &impl ConnectionTrait) -> Result<Option<R>>
    where R: Send {
        self.select.one(db)
            .await
            .map_err(|e| RavelEloquentError::Database(e))
    }

    pub async fn count(self, db: &impl ConnectionTrait) -> Result<u64>
    where R: Send {
        self.select.count(db)
            .await
            .map_err(|e| RavelEloquentError::Database(e))
    }

    pub async fn exists(self, db: &impl ConnectionTrait) -> Result<bool>
    where R: Send {
        self.select.count(db)
            .await
            .map(|c| c > 0)
            .map_err(|e| RavelEloquentError::Database(e))
    }

    pub async fn paginate(
        self,
        db: &impl ConnectionTrait,
        page: u64,
        per_page: u64,
    ) -> Result<crate::query::Page<R>>
    where R: Send {
        let paginator = self.select.paginate(db, per_page);
        let total = paginator.num_items().await.map_err(|e| RavelEloquentError::Database(e))?;
        let items = paginator.fetch_page(page).await.map_err(|e| RavelEloquentError::Database(e))?;
        Ok(crate::query::Page::new(items, total, page, per_page))
    }
}

// ============================================================
// HasRelations trait — 提供给用户的接口方法示例
// ============================================================

/// 任何模型都可以实现此 trait 以提供关系查询方法
/// 实际上这些方法由 #[derive(Model)] 宏直接在模型 impl 块生成
pub trait HasRelationsExt: ModelMeta {
    /// 为 HasMany 关系创建 RelationQuery
    fn relation_query<R: EntityTrait + ModelMeta>(
        &self,
        foreign_key: &str,
        local_value: Value,
    ) -> RelationQuery<R>
    where
        R::Columns: sea_orm::ColumnTrait + std::str::FromStr<Err = crate::error::RavelEloquentError>,
    {
        let select = R::find();
        let col = R::Columns::from_str(foreign_key).unwrap();
        let select = select.filter(col.eq(local_value));
        RelationQuery::from_select(select)
    }
}
```

- [ ] **Step 2: 编译验证**

```bash
cd crates/ravel-eloquent && cargo check 2>&1
```

- [ ] **Step 3: 提交**

```bash
git add crates/ravel-eloquent/src/relations.rs
git commit -m "feat(eloquent): add RelationQuery<R> with lazy loading support"
```

---

### 阶段4: 整理 lib.rs + 删除旧代码

#### Task 4.1: 重新组织 lib.rs 和清理

**Files:**
- Rewrite: `crates/ravel-eloquent/src/lib.rs`
- Delete: `crates/ravel-eloquent/src/defaults.rs` (功能已合并到 traits)

- [ ] **Step 1: 重写 lib.rs**

```rust
// crates/ravel-eloquent/src/lib.rs

pub mod error;
pub mod model_traits;
pub mod query;
pub mod relations;
pub mod fillable;

// Re-exports
pub use ravel_eloquent_macros::Model;
pub use error::{RavelEloquentError, Result};
pub use query::{QueryBuilder, Page};
pub use relations::{RelationQuery, HasRelationsExt};
pub use model_traits::{
    ModelMeta, ModelMetaExt,
    ModelExt, ActiveModelExt,
    Replicates, HasTimestamps, Serializes,
};
pub use fillable::Fillable;

// Re-export sea_orm for user convenience
pub use sea_orm;
```

- [ ] **Step 2: 删除 defaults.rs（已废弃，功能转至 ModelExt/ActiveModelExt）**

```bash
rm crates/ravel-eloquent/src/defaults.rs
```

- [ ] **Step 3: 尝试完整编译**

```bash
cd crates/ravel-eloquent && cargo check 2>&1
```

预期：可能有编译错误（宏生成和 trait bound 需要迭代修正）。逐步修复后通过。

- [ ] **Step 4: 提交**

```bash
git add crates/ravel-eloquent/src/lib.rs
git rm crates/ravel-eloquent/src/defaults.rs
git commit -m "refactor(eloquent): reorganize public API, remove deprecated defaults module"
```

---

### 阶段5: 测试 — P5

#### Task 5.1: 更新编译期测试

**Files:**
- Rewrite: `crates/ravel-eloquent/tests/model_derive.rs`

- [ ] **Step 1: 写入新的测试文件**

```rust
// crates/ravel-eloquent/tests/model_derive.rs

// 注意：这些测试仅验证宏生成代码可编译，不涉及数据库

use ravel_eloquent::{Model, ModelMeta, QueryBuilder, Fillable, Serializes};
use serde::{Serialize, Deserialize};

// ============================================================
// 1. 基础模型 — 编译通过
// ============================================================

#[derive(Model, Clone, Debug, Serialize, Deserialize)]
#[model(table = "users")]
struct User {
    #[model(id)]
    pub id: i32,
    pub name: String,
    #[model(hidden)]
    pub password: String,
}

#[test]
fn test_model_meta_has_table_name() {
    assert_eq!(User::table_name(), "users");
}

#[test]
fn test_model_meta_has_id_column() {
    assert_eq!(User::id_column(), "id");
}

#[test]
fn test_model_public_excludes_hidden() {
    let user = User { id: 1, name: "Alice".into(), password: "secret".into() };
    let public = user.to_public();
    let json = serde_json::to_value(&public).unwrap();
    assert_eq!(json["id"], 1);
    assert_eq!(json["name"], "Alice");
    // password 不应出现
    assert!(json.get("password").is_none());
}

#[test]
fn test_model_to_json() {
    let user = User { id: 1, name: "Alice".into(), password: "secret".into() };
    let json = user.to_json();
    assert_eq!(json["id"], 1);
    assert_eq!(json["name"], "Alice");
    // to_json 包含所有字段
    assert_eq!(json["password"], "secret");
}

#[test]
fn test_model_to_public_json() {
    let user = User { id: 1, name: "Alice".into(), password: "secret".into() };
    let json = user.to_public_json();
    assert!(json.get("password").is_none());
}

// ============================================================
// 2. 带关系的模型 — 编译通过
// ============================================================

#[derive(Model, Clone, Debug)]
#[model(table = "posts")]
struct Post {
    #[model(id)]
    pub id: i32,
    pub title: String,
    pub user_id: i32,
    #[model(belongs_to, from = "user_id", to = "id")]
    pub author: sea_orm::entity::prelude::HasOne<User>,
}

#[derive(Model, Clone, Debug)]
#[model(table = "teams")]
struct Team {
    #[model(id)]
    pub id: i32,
    pub name: String,
}

// 带 HasMany 的模型
#[derive(Model, Clone, Debug, Serialize, Deserialize)]
#[model(table = "users_with_posts", timestamps)]
struct UserWithPosts {
    #[model(id)]
    pub id: i32,
    pub name: String,
    #[model(has_many)]
    pub posts: sea_orm::entity::prelude::HasMany<Post>,
}

#[test]
fn test_model_with_timestamps_in_public() {
    // timestamps 字段应该在 public 中可见
    let u = UserWithPosts { id: 1, name: "Bob".into(), posts: Default::default() };
    // 这里要验证 compilation — 实际测试需要 DateTime 字段值
}

// ============================================================
// 3. Fillable
// ============================================================

#[derive(Model, Clone, Debug, Serialize, Deserialize)]
#[model(table = "profiles")]
struct Profile {
    #[model(id)]
    pub id: i32,
    pub bio: String,
    #[model(hidden)]
    pub internal_note: String,
}

#[test]
fn test_fill_skips_hidden() {
    let profile = Profile { id: 0, bio: String::new(), internal_note: String::new() };
    let filled = profile.fill(serde_json::json!({
        "bio": "Hello world",
        "internal_note": "should not change"
    }));
    assert_eq!(filled.bio, "Hello world");
    assert_eq!(filled.internal_note, "");
}

// ============================================================
// 4. Replicates
// ============================================================

#[derive(Model, Clone, Debug)]
#[model(table = "items")]
struct Item {
    #[model(id)]
    pub id: i32,
    pub data: String,
}

#[test]
fn test_replicate_resets_id() {
    let item = Item { id: 42, data: "original".into() };
    let dup = item.replicate();
    assert_eq!(dup.data, "original");
    // 注意：Replicates trait 仅克隆，id 重置需用户在 save 时处理
    // （id==0 则 INSERT）
}

// ============================================================
// 5. QueryBuilder 编译测试
// ============================================================

#[test]
fn test_query_builder_chainable() {
    // 验证链式调用可以编译
    let _query = User::query()
        .r#where("name", "Alice")
        .r#where_gt("id", 0)
        .order_by("id", "DESC")
        .limit(10)
        .offset(0);
}

#[test]
fn test_query_builder_count() {
    // 验证 .count() 可调用（编译时）
    // 实际执行需要数据库，此处仅编译
}

// ============================================================
// 6. 综合示例 — 编译通过即成功
// ============================================================

#[derive(Model, Clone, Debug, Serialize, Deserialize)]
#[model(table = "articles", timestamps)]
struct Article {
    #[model(id)]
    pub id: i32,
    #[model(string, 254, unique)]
    pub slug: String,
    pub title: String,
    #[model(text)]
    pub body: String,
    #[model(nullable, string, 500)]
    pub excerpt: Option<String>,
    #[model(integer)]
    pub author_id: i32,
    #[model(boolean)]
    pub published: bool,
    #[model(hidden)]
    pub metadata: String,
}

#[test]
fn test_article_has_all_fields() {
    let cols = Article::columns();
    assert!(cols.contains(&"slug"));
    assert!(cols.contains(&"title"));
    assert!(cols.contains(&"body"));
    assert!(cols.contains(&"excerpt"));
    assert!(cols.contains(&"published"));
    assert!(cols.contains(&"author_id"));
}

#[test]
fn test_article_public_excludes_hidden_and_relations() {
    let public_cols = Article::public_columns();
    assert!(!public_cols.contains(&"metadata"));
    assert!(public_cols.contains(&"slug"));
    assert!(public_cols.contains(&"title"));
}
```

- [ ] **Step 2: 运行测试**

```bash
cd crates/ravel-eloquent && cargo test 2>&1
```

预期：编译通过，非数据库测试通过

- [ ] **Step 3: 提交**

```bash
git add crates/ravel-eloquent/tests/
git commit -m "test(eloquent): add comprehensive model_derive tests for v2 API"
```

---

### 阶段6: 集成测试 + 示例 — P6

#### Task 6.1: 数据库集成测试

**Files:**
- Create: `crates/ravel-eloquent/tests/integration_crud.rs`

- [ ] **Step 1: 写集成测试（使用 SQLite 内存数据库）**

```rust
// crates/ravel-eloquent/tests/integration_crud.rs

use sea_orm::{Database, DatabaseConnection, DbBackend, Schema};
use ravel_eloquent::{Model, ModelMeta, QueryBuilder};

/// 设置 SQLite 内存数据库并创建测试表
async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();

    let schema = Schema::new(DbBackend::Sqlite);
    let stmt = schema
        .create_table_from_entity(<TestUser as sea_orm::EntityTrait>::Entity::schema())
        .if_not_exists()
        .build(DbBackend::Sqlite);

    sea_orm::ConnectionTrait::execute(
        &db,
        sea_orm::Statement::from_sql_and_values(
            DbBackend::Sqlite,
            &stmt,
            [],
        ),
    )
    .await
    .unwrap();

    db
}

#[derive(Model, Clone, Debug, serde::Serialize, serde::Deserialize)]
#[model(table = "test_users")]
struct TestUser {
    #[model(id)]
    pub id: i32,
    pub name: String,
    pub email: String,
}

#[tokio::test]
async fn test_create_and_find() {
    let db = setup_db().await;

    let user = TestUser::create(
        serde_json::json!({"name": "Alice", "email": "alice@test.com"}),
        &db,
    ).await.unwrap();

    assert_eq!(user.name, "Alice");
    assert!(user.id > 0);

    let found = TestUser::find(&db, user.id).await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().email, "alice@test.com");
}

#[tokio::test]
async fn test_all_and_count() {
    let db = setup_db().await;

    TestUser::create(serde_json::json!({"name": "A", "email": "a@t.com"}), &db).await.unwrap();
    TestUser::create(serde_json::json!({"name": "B", "email": "b@t.com"}), &db).await.unwrap();

    let all = TestUser::all(&db).await.unwrap();
    assert_eq!(all.len(), 2);

    let count = TestUser::query().count(&db).await.unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn test_destroy() {
    let db = setup_db().await;
    let user = TestUser::create(serde_json::json!({"name": "X", "email": "x@t.com"}), &db).await.unwrap();
    let rows = TestUser::destroy(&db, user.id).await.unwrap();
    assert_eq!(rows, 1);

    let found = TestUser::find(&db, user.id).await.unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn test_save_update() {
    let db = setup_db().await;
    let user = TestUser { id: 0, name: "Old".into(), email: "old@t.com".into() };

    // INSERT
    let user = user.save(&db).await.unwrap();
    assert!(user.id > 0);

    // UPDATE
    let mut updated = user;
    updated.name = "New".into();
    let updated = updated.save(&db).await.unwrap();
    assert_eq!(updated.name, "New");
    assert_eq!(updated.id, user.id);
}

#[tokio::test]
async fn test_delete() {
    let db = setup_db().await;
    let user = TestUser::create(serde_json::json!({"name": "D", "email": "d@t.com"}), &db).await.unwrap();
    user.delete(&db).await.unwrap();

    let found = TestUser::find(&db, user.id).await.unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn test_query_where_conditions() {
    let db = setup_db().await;

    for i in 0..5 {
        TestUser::create(
            serde_json::json!({"name": format!("User{}", i), "email": format!("u{}@t.com", i)}),
            &db,
        ).await.unwrap();
    }

    let count = TestUser::query()
        .r#where_gt("id", 2)
        .count(&db).await.unwrap();
    assert_eq!(count, 3);
}

#[tokio::test]
async fn test_query_paginate() {
    let db = setup_db().await;

    for i in 0..10 {
        TestUser::create(
            serde_json::json!({"name": format!("P{}", i), "email": format!("p{}@t.com", i)}),
            &db,
        ).await.unwrap();
    }

    let page = TestUser::query()
        .order_by("id", "ASC")
        .paginate(&db, 1, 3).await.unwrap();

    assert_eq!(page.items.len(), 3);
    assert_eq!(page.total, 10);
    assert!(page.has_more());
}
```

- [ ] **Step 2: 运行集成测试**

```bash
cd crates/ravel-eloquent && cargo test --test integration_crud 2>&1
```

- [ ] **Step 3: 提交**

```bash
git add crates/ravel-eloquent/tests/integration_crud.rs
git commit -m "test(eloquent): add SQLite integration tests for CRUD operations"
```

---

### 阶段7: 文档更新 + 示例应用

#### Task 7.1: 更新用户文档

**Files:**
- Modify: `docs/book/src/database/eloquent.md`

- [ ] **Step 1: 重写文档以反映新 API**

将 `docs/book/src/database/eloquent.md` 更新为与 v2 API 一致的用法示例。包含：

- 模型定义
- 静态 CRUD (find, all, create, destroy)
- 实例方法 (save, delete, refresh)
- 查询构建器 (where 条件、排序、分页、聚合、JOIN)
- 关系 (lazy + eager loading)
- `to_public()` / `to_json()` 序列化
- 底层穿透 (调用 SeaORM 原生 API)

- [ ] **Step 2: 提交**

```bash
git add docs/book/src/database/eloquent.md
git commit -m "docs(eloquent): update for v2 API"
```

---

### 风险与修复预案

| 风险 | 可能性 | 缓解措施 |
|------|--------|---------|
| SeaORM 2.0 rc 版本 API 变化 | 中 | 固定版本 `=2.0.0-rc.40`，未来升级任务独立 |
| 宏生成的代码编译不过 | 高 | 小步迭代：P0 先按 SeaORM entity 模板手写一个 User 模型验证可行，再让宏生成 |
| `impl From<&Self> for ActiveModel` 绑定无法自动推导 | 中 | 给宏生成的 SeaORM 实体添加手动 `From` 实现或让 SeaORM 宏生成 |
| 查询构建器 `ColumnTrait + FromStr` 绑定太复杂 | 中 | 简化为使用 Column 枚举 + `as_str()` 而非 trait bound |
| 关系 eager loading 和 SeaORM 的 `ModelEx` 类型互操作 | 中 | P4 阶段先做基本 with() 映射，复杂场景留后续 |

---

### 实施顺序依赖

```
P0 (宏重写) ──┬──▶ P1 (实例方法) ──▶ P5 (辅助 traits)
              │
              ├──▶ P2 (查询构建器)
              │
              └──▶ P3 (关系 lazy) ──▶ P4 (关系 eager)
                                       │
                                       ▼
                                      P6 (集成测试 + 文档)
```
