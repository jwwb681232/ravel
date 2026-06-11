use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table({{name}}::Table)
                    .if_not_exists()
                    .col(ColumnDef::new({{name}}::Id).integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new({{name}}::Name).string().not_null())
                    .col(ColumnDef::new({{name}}::CreatedAt).timestamp().not_null()
                        .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)))
                    .col(ColumnDef::new({{name}}::UpdatedAt).timestamp().not_null()
                        .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table({{name}}::Table).to_owned())
            .await
    }
}

// ── Replace with your actual table definition ──────────────────
#[derive(Iden)]
enum {{name}} {
    Table,
    Id,
    Name,
    CreatedAt,
    UpdatedAt,
}
