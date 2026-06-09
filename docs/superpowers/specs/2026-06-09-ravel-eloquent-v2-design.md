# Ravel Eloquent v2 — 设计文档

**日期**：2026-06-09  
**状态**：已确认，待实施  
**目标**：将 Ravel 的 ORM 深度对标 Laravel Eloquent，聚焦核心体验——实例方法、关系执行 + eager loading、查询构建器补全

---

## 0. 设计决策总览

| 决策点 | 选择 |
|--------|------|
| 范围 | 核心体验优先（实例方法、关系、查询构建器），其余后续迭代 |
| 集成方式 | 深度集成 SeaORM 2.0 类型系统（路线 A） |
| 实例方法所有权 | 消耗式（take ownership），借助 Rust 变量遮蔽保持使用流畅 |
| 关系定义 | 内联到模型字段，直接复用 SeaORM 2.0 dense entity format |
| Eager loading | 复用 SeaORM 2.0 Entity Loader（`.with()`），类型安全 |
| 扩展性 | Trait 组合架构，后期加特性不破坏已有代码 |
| 查询构建器 | 对标 Laravel 条件能力，底层翻译为 SeaORM 类型安全查询 |

---

## 1. 总体架构

```
用户写的模型定义
        │
        ▼
┌─────────────────────────────────────────────────┐
│  #[derive(Model)] 宏                           │
│                                                │
│ 输入: #[model(table/field/relation)]           │
│ 输出:                                          │
│ ① SeaORM 实体定义 (DeriveEntityModel)          │
│ ② #[sea_orm::model] 属性宏                    │
│ ③ Column 枚举                                  │
│ ④ Public 结构体 + to_public()                  │
│ ⑤ Trait 实现 (ActiveModelExt, ModelExt, ...)   │
│ ⑥ 实例方法 (save/delete/setters)              │
│ ⑦ 关系查询方法 (posts().get())                 │
│ ⑧ 查询入口 (query/r#where)                    │
└──────┬──────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────────┐
│  ravel-eloquent (traits + extension methods)    │
│                                                │
│ ModelMeta       — table_name(), columns()       │
│ ActiveModelExt  — save(), insert(), update()    │
│ ModelExt        — find(), all(), create()       │
│ HasTimestamps   — touch()                       │
│ Replicates      — replicate()                   │
│ Serializes      — to_json(), to_public()        │
│ Fillable        — fill(), set_xxx()             │
│ HasRelations    — posts().get(), team().first() │
│ Queryable       — query(), r#where()            │
└──────┬──────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────────┐
│  SeaORM 2.0 (底层引擎)                          │
│                                                │
│ Entity::load().with().one()                    │
│ ActiveModel::save() / update() / delete()      │
│ Select::filter() / order_by() / join()          │
│ ConnectionTrait                                 │
└─────────────────────────────────────────────────┘
```

**核心原则**：`#[derive(Model)]` 生成的 `User` **就是** 标准 `sea_orm::entity::Model`，不是包装类型。所有 Eloquent 方法通过 trait extension 加上去。用户随时可以用 SeaORM 原生 API。

---

## 2. 模型定义语法

### 2.1 表级别属性

```rust
#[derive(Model)]
#[model(table = "users")]
struct User { ... }
```

`table` 为唯一必填属性。

### 2.2 列属性完整参考

| 属性 | 展开效果 |
|------|---------|
| `#[model(id)]` | `#[sea_orm(primary_key, auto_increment)]` |
| `#[model(uuid)]` | `#[sea_orm(primary_key, auto_increment = false)]` + `Uuid` 类型 |
| `#[model(string)]` | `String`，默认 `VARCHAR(255)` |
| `#[model(string, 100)]` | `String` + `#[sea_orm(column_type = "String(Some(100))")]` |
| `#[model(text)]` | `String` + `#[sea_orm(column_type = "Text")]` |
| `#[model(integer)]` | `i32` |
| `#[model(bigint)]` | `i64` |
| `#[model(boolean)]` | `bool` |
| `#[model(float)]` | `f64` |
| `#[model(datetime)]` | `DateTime` |
| `#[model(json)]` | `Json` |
| `#[model(nullable)]` | 字段类型包裹为 `Option<T>` |
| `#[model(unique)]` | `#[sea_orm(unique)]` |
| `#[model(hidden)]` | 字段从 `to_public()` 和 `fill_public()` 中排除 |
| `#[model(column = "real_col")]` | `#[sea_orm(column_name = "real_col")]` |
| `#[model(timestamps)]` | 自动添加 `created_at` + `updated_at` 字段 |

### 2.3 关系属性

| 属性 | 展开效果 |
|------|---------|
| `#[model(has_many)]` | `#[sea_orm(has_many)]` + `HasMany<Entity>` |
| `#[model(has_one)]` | `#[sea_orm(has_one)]` + `HasOne<Entity>` |
| `#[model(belongs_to, from = "fk", to = "id")]` | `#[sea_orm(belongs_to, from = "fk", to = "id")]` + `HasOne<Entity>` |
| `#[model(has_many, via = "junction")]` | `#[sea_orm(has_many, via = "junction")]` — 多对多 |

关系字段的类型参数（`Post`、`Team` 等）由宏从 `HasMany<T>` / `HasOne<T>` 字段签名中提取。

### 2.4 完整示例

```rust
use ravel_eloquent::Model;

#[derive(Model, serde::Serialize, serde::Deserialize)]
#[model(table = "users")]
struct User {
    #[model(id)]
    pub id: i32,

    pub name: String,

    #[model(string, 254, unique)]
    pub email: String,

    #[model(hidden)]
    pub password: String,

    #[model(nullable, string, 500)]
    pub bio: Option<String>,

    #[model(integer)]
    pub team_id: i32,

    #[model(timestamps)]

    // --- 关系 (不回填为数据库列) ---
    #[model(has_many)]
    pub posts: HasMany<Post>,

    #[model(belongs_to, from = "team_id", to = "id")]
    pub team: HasOne<Team>,
}
```

---

## 3. Trait 架构

所有实例方法通过 trait 组合实现，`#[derive(Model)]` 自动为模型实现。

### 3.1 `ModelMeta` — 模型元数据

```rust
pub trait ModelMeta {
    fn table_name() -> &'static str;
    fn columns() -> &'static [&'static str];
    fn id_column() -> &'static str;
    fn public_columns() -> &'static [&'static str];  // 排除 hidden
}
```

### 3.2 `ActiveModelExt` — 写操作

```rust
pub trait ActiveModelExt: ModelMeta + Sized {
    /// id==0 → INSERT，id!=0 → UPDATE。ActiveModel 返回值直接回填。
    async fn save(self, db: &impl ConnectionTrait) -> Result<Self>;

    /// 强制 INSERT，忽略当前 id
    async fn insert(self, db: &impl ConnectionTrait) -> Result<Self>;

    /// 强制 UPDATE，id==0 时报错
    async fn update(self, db: &impl ConnectionTrait) -> Result<Self>;

    /// DELETE，消费实例
    async fn delete(self, db: &impl ConnectionTrait) -> Result<()>;

    /// 从数据库重新查询最新数据
    async fn refresh(self, db: &impl ConnectionTrait) -> Result<Self>;
}
```

`save()` 内部逻辑：
```
                   ┌─ id == 0 ──▶ ActiveModel::insert(db) → 回填 id + timestamps → 返回 Self
                   │
self → ActiveModel ─┤
                   │
                   └─ id != 0 ──▶ ActiveModel::update(db) → 回填 updated_at → 返回 Self
```

### 3.3 `ModelExt` — 静态 CRUD 方法

```rust
pub trait ModelExt: ModelMeta + Sized {
    async fn find(db, id: impl Into<Value>) -> Result<Option<Self>>;
    async fn find_or_fail(db, id: impl Into<Value>) -> Result<Self>;
    async fn all(db) -> Result<Vec<Self>>;
    async fn create(data: serde_json::Value, db) -> Result<Self>;
    async fn destroy(db, id: impl Into<Value>) -> Result<u64>;
    async fn destroy_many(db, ids: &[impl Into<Value>]) -> Result<u64>;
}
```

### 3.4 `Replicates` — 复制

```rust
pub trait Replicates: Clone {
    fn replicate(&self) -> Self;  // clone + id=0
}
```

### 3.5 `HasTimestamps` — 时间戳

```rust
pub trait HasTimestamps: ActiveModelExt {
    async fn touch(self, db: &impl ConnectionTrait) -> Result<Self>;
    // 只更新 updated_at，不碰其他字段
}
```

### 3.6 `Fillable` — 批量赋值

```rust
pub trait Fillable: Sized {
    fn fill(self, data: serde_json::Value) -> Self;
    fn fill_public(self, data: serde_json::Value) -> Self;  // 只填非 hidden
    // 每个字段自动生成 set_xxx(self, val) -> Self
}
```

Setter 示例（宏自动生成）：

```rust
impl Fillable for User {
    fn fill(mut self, data: Value) -> Self {
        if let Some(v) = data.get("name")  { self.name = v.into(); }
        if let Some(v) = data.get("email") { self.email = v.into(); }
        // ... 自动跳过 hidden 字段 password
        self
    }

    pub fn set_name(mut self, val: impl Into<String>) -> Self {
        self.name = val.into();
        self
    }
    pub fn set_email(mut self, val: impl Into<String>) -> Self {
        self.email = val.into();
        self
    }
}
```

### 3.7 `Serializes` — 序列化

```rust
pub trait Serializes {
    type Public: Serialize;

    fn to_public(&self) -> Self::Public;
    fn to_json(&self) -> serde_json::Value;
    fn to_public_json(&self) -> serde_json::Value {
        serde_json::to_value(self.to_public()).unwrap()
    }
}
```

### 3.8 `Queryable` — 查询入口

```rust
pub trait Queryable: ModelMeta {
    fn query() -> Select<Self>;
    fn r#where(col: &str, val: impl Into<Value>) -> Select<Self>;
}
```

### 3.9 扩展性——留给后期迭代

后期可通过新 trait 添加而不破坏已有代码：

```rust
// 未来可加
pub trait SoftDeletes: Sized { ... }
pub trait HasEvents: Sized { ... }
```

`#[derive(Model)]` 检测模型上的属性（如 `#[model(soft_deletes)]`），有则自动实现对应 trait。

---

## 4. 实例方法速查

### 4.1 写入

| 方法 | 签名 | 行为 |
|------|------|------|
| `save()` | `save(self, db) -> Result<Self>` | id==0 INSERT，id!=0 UPDATE |
| `insert()` | `insert(self, db) -> Result<Self>` | 强制 INSERT |
| `update()` | `update(self, db) -> Result<Self>` | 强制 UPDATE，id==0 报错 |
| `delete()` | `delete(self, db) -> Result<()>` | DELETE，消费实例 |

### 4.2 读取

| 方法 | 签名 | 行为 |
|------|------|------|
| `refresh()` | `refresh(self, db) -> Result<Self>` | 重新查一行覆盖 |

### 4.3 转换

| 方法 | 签名 | 行为 |
|------|------|------|
| `replicate()` | `replicate(&self) -> Self` | 克隆 + id 置 0 |
| `to_public()` | `to_public(&self) -> Self::Public` | 排除 hidden |
| `to_json()` | `to_json(&self) -> Value` | 全字段 JSON |
| `to_public_json()` | `to_public_json(&self) -> Value` | 安全 JSON |

### 4.4 时间戳

| 方法 | 签名 | 行为 |
|------|------|------|
| `touch()` | `touch(self, db) -> Result<Self>` | 只更新 updated_at |

### 4.5 Setter

每个非 hidden、非关系字段自动生成：

```rust
pub fn set_<field>(mut self, val: impl Into<FieldType>) -> Self
```

### 4.6 静态方法

| 方法 | 签名 | 行为 |
|------|------|------|
| `find()` | `find(db, id) -> Result<Option<Self>>` | 按 ID 查 |
| `find_or_fail()` | `find_or_fail(db, id) -> Result<Self>` | 不存在则报错 |
| `all()` | `all(db) -> Result<Vec<Self>>` | 全表 |
| `create()` | `create(data: Value, db) -> Result<Self>` | 从 JSON 创建 |
| `destroy()` | `destroy(db, id) -> Result<u64>` | 按 ID 删（无需加载实例） |
| `destroy_many()` | `destroy_many(db, &[id]) -> Result<u64>` | 批量按 ID 删 |

### 4.7 使用示例

```rust
// --- 创建 ---
let user = User {
    id: 0,
    name: "Alice".into(),
    email: "alice@e.com".into(),
    password: "hashed".into(),
    bio: None,
    team_id: 42,
};
let user = user.save(&db).await?;  // INSERT → id 回填
println!("新用户 id: {}", user.id);

// --- 更新 ---
let user = user
    .set_name("Alice Updated")
    .set_email("alice@new.com")
    .save(&db).await?;             // UPDATE

// --- 强制 INSERT（已知要插入时） ---
let user = user.insert(&db).await?;

// --- 时间戳 ---
let user = user.touch(&db).await?; // 只更新 updated_at

// --- 复制 ---
let mut dup = user.replicate();    // clone, id=0
dup.name = "Alice Clone".into();
let dup = dup.save(&db).await?;    // 新记录

// --- 删除（静态，无需加载实例） ---
User::destroy(&db, 42).await?;
User::destroy_many(&db, &[1, 2, 3]).await?;

// --- 删除（实例，已加载） ---
user.delete(&db).await?;

// --- API 响应 ---
let json = user.to_public_json();
// {"id":1,"name":"Alice Updated","email":"alice@new.com","bio":null,...}
// password 不在其中
```

---

## 5. 关系查询

### 5.1 关系定义

关系作为模型字段内联定义：

```rust
#[derive(Model)]
#[model(table = "users")]
struct User {
    #[model(id)]          id: i32,
    #[model(has_many)]    posts: HasMany<Post>,
    #[model(belongs_to, from = "team_id", to = "id")]
                          team: HasOne<Team>,
    #[model(has_one)]     profile: HasOne<Profile>,
    #[model(has_many, via = "role_user")]
                          roles: HasMany<Role>,
}
```

宏生成为这些关系字段标注 `#[sea_orm(has_many)]` / `#[sea_orm(belongs_to)]` 等，同时生成关系查询方法。

### 5.2 `RelationQuery<R>` 类型

```rust
pub struct RelationQuery<R> {
    select: Select<R>,
}

impl<R: EntityTrait + ModelMeta> RelationQuery<R> {
    // 链式过滤
    pub fn r#where(mut self, col: &str, val: impl Into<Value>) -> Self;
    pub fn r#where_gt(self, col: &str, val: impl Into<Value>) -> Self;
    pub fn r#where_in(self, col: &str, vals: Vec<impl Into<Value>>) -> Self;
    pub fn r#where_null(self, col: &str) -> Self;
    pub fn r#where_not_null(self, col: &str) -> Self;
    pub fn or_where(self, col: &str, val: impl Into<Value>) -> Self;

    // 排序
    pub fn order_by(self, col: &str, dir: &str) -> Self;
    pub fn latest(self, col: &str) -> Self;
    pub fn oldest(self, col: &str) -> Self;

    // 分页
    pub fn limit(self, n: u64) -> Self;
    pub fn offset(self, n: u64) -> Self;

    // 执行
    pub async fn get(self, db: &impl ConnectionTrait) -> Result<Vec<R>>;
    pub async fn first(self, db: &impl ConnectionTrait) -> Result<Option<R>>;
    pub async fn count(self, db: &impl ConnectionTrait) -> Result<u64>;
    pub async fn exists(self, db: &impl ConnectionTrait) -> Result<bool>;
    pub async fn paginate(self, db, page: u64, per_page: u64) -> Result<Page<R>>;
}
```

### 5.3 Lazy loading 用法

```rust
let user = User::find(&db, 1).await?;

// 按需查，支持过滤排序
let posts = user.posts()
    .r#where("published", true)
    .latest("created_at")
    .limit(10)
    .get(&db).await?;

let team = user.team().first(&db).await?;

// 聚合
let draft_count = user.posts()
    .r#where("published", false)
    .count(&db).await?;

let has_posts = user.posts().exists(&db).await?;
```

### 5.4 Eager loading — `.with()`

基于 SeaORM 2.0 Entity Loader。宏生成类型安全的 `.with()` 方法：

```rust
// 1-1 关系 → LEFT JOIN
// 1-N 关系 → batched IN 查询（避免行膨胀）
let users = User::query()
    .r#where("active", true)
    .with(Team)                       // belongs_to → LEFT JOIN
    .with(Post)                       // has_many  → batched IN
    .with_nested(Post, Comment)       // posts 下嵌套 comments
    .get(&db).await?;

// 返回 ModelEx 类型（SeaORM 2.0 自动生成），关系已填充
for user in users {
    println!("{} ({})", user.name, user.team.unwrap().name);
    for post in user.posts.iter() {
        println!("  {}", post.title);
        for comment in post.comments.iter() {
            println!("    {}", comment.body);
        }
    }
}
```

### 5.5 关系类型支持矩阵

| 关系类型 | 模型属性 | 查询方法 | Eager loading |
|----------|---------|---------|---------------|
| HasMany | `#[model(has_many)]` | `.posts().get()` | `.with(Post)` → batched IN |
| BelongsTo | `#[model(belongs_to, from, to)]` | `.team().first()` | `.with(Team)` → LEFT JOIN |
| HasOne | `#[model(has_one)]` | `.profile().first()` | `.with(Profile)` → LEFT JOIN |
| BelongsToMany | `#[model(has_many, via)]` | `.roles().get()` | `.with(Role)` → junction + batched IN |
| HasManyThrough | `#[model(has_many, through)]` | 方法链 | `.with_nested(...)` |

---

## 6. 查询构建器

### 6.1 查询入口

```rust
User::query()                        // SELECT * FROM "users"
User::r#where("active", true)        // query() + WHERE
User::query().r#where("active", true) // 等价
```

### 6.2 WHERE 条件

```rust
// ==
.r#where("age", 18)

// 两参数版本（操作符 + 值）
.r#where("age", ">=", 18)

// 类型专用方法
.r#where_gt("age", 18)
.r#where_gte("age", 18)
.r#where_lt("age", 65)
.r#where_lte("age", 65)
.r#where_ne("status", "deleted")
.r#where_like("name", "%Alice%")
.r#where_not_like("name", "%test%")
.r#where_in("id", &[1, 2, 3])
.r#where_not_in("id", &[4, 5])
.r#where_between("age", 18, 65)
.r#where_not_between("age", 0, 17)
.r#where_null("deleted_at")
.r#where_not_null("email")
```

### 6.3 复合条件

```rust
// AND — 链式就是 AND
User::query()
    .r#where("status", "active")
    .r#where_gt("age", 18)

// OR
User::query()
    .r#where("status", "active")
    .or_where("role", "admin")

// 分组
User::query()
    .r#where("status", "active")
    .r#where_group(|q| {
        q.r#where("role", "admin")
         .or_where("role", "moderator")
    })
```

### 6.4 排序、分页

```rust
.order_by("created_at", "DESC")
.order_by("name", "ASC")
.latest()                    // = order_by("created_at", "DESC")
.oldest()                    // = order_by("created_at", "ASC")
.in_random_order()           // ORDER BY RANDOM()
.limit(15)
.offset(30)
.skip(30).take(15)           // 等价
.paginate(&db, 1, 15).await? // → Page<User>
```

### 6.5 聚合

```rust
.count(&db).await?             // → u64
.max("age", &db).await?        // → Option<i32>
.min("age", &db).await?        // → Option<i32>
.avg("age", &db).await?        // → Option<f64>
.sum("score", &db).await?      // → Option<i64>
.exists(&db).await?            // → bool
```

### 6.6 列选择

```rust
.select(&["id", "name", "email"])       // SELECT id, name, email
.select_except(&["password", "token"])  // SELECT 除外的所有列
```

部分列选择返回 `Vec<Value>` 而非完整 Model。

### 6.7 JOIN

```rust
.join(Team, "team_id", "id")           // INNER JOIN
.left_join(Post, "id", "user_id")     // LEFT JOIN
.right_join(Post, "id", "user_id")    // RIGHT JOIN
```

### 6.8 DISTINCT、GROUP BY、HAVING

```rust
User::query()
    .select(&["team_id"])
    .distinct()
    .group_by("team_id")
    .having("count(*)", ">", 5)
    .get(&db).await?;
```

### 6.9 底层穿透

所有方法内部翻译为 SeaORM 2.0 的类型安全 API。用户可随时绕过 Eloquent 封装直接操作底层：

```rust
use sea_orm::*;

User::query()
    .into_select()                     // 拿到 SeaORM Select<Entity>
    .filter(users::Column::Age.gte(18))
    .left_join(teams::Entity)
    .all(&db).await?;
```

---

## 7. 实施阶段划分

| 阶段 | 内容 | 依赖 |
|------|------|------|
| **P0** | 宏改造——生成 SeaORM 2.0 实体 + `ModelMeta` + `ModelExt` + `Queryable` | 重写 `ravel-eloquent-macros` |
| **P1** | `ActiveModelExt` trait + `Fillable` trait（实例方法 save/delete/setters） | P0 |
| **P2** | 查询构建器补全（所有 WHERE 条件、聚合、JOIN、分组） | P0 |
| **P3** | 关系——`RelationQuery<R>` lazy loading | P0 |
| **P4** | 关系——`.with()` eager loading（复用 Entity Loader） | P0, P3 |
| **P5** | `Serializes` / `Replicates` / `HasTimestamps` traits | P0, P1 |
| **P6** | 文档更新 + `testblog` 示例应用 | P0-P5 |

---

## 8. 已确认的范围外（后续迭代）

以下特性本次**不做**，留给后续设计文档：

- DB Facade（`DB::table()`, `DB::transaction()`, `DB::raw()`）
- 模型事件 / Observer（`creating`、`created`、`deleting` 等钩子）
- Soft Deletes
- Query Scopes（局部 + 全局）
- Accessor / Mutator
- 属性 Casting
- `whereHas` / `whereDoesntHave` 过滤
- 子查询
- `save_many` / `create_many` 批量写入

---

## 9. 关键 crates 影响范围

| Crate | 变更性质 |
|-------|---------|
| `ravel-eloquent-macros` | **重度重写**——新属性语法，生成 SeaORM 2.0 实体 + 全部 trait |
| `ravel-eloquent` | **重度扩展**——新增 trait 定义、`RelationQuery<R>`、扩展查询构建器 |
| `ravel-db-core` | 可能微调——`ModelMeta` 相关 trait |
| `ravel-db-seaorm` | 大概率不改——eloquent 层直接调 SeaORM API |
| `ravel-test` / `testblog` | 新增集成测试 + 示例代码 |
