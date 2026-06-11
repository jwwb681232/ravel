use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Posts::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Posts::Id).integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Posts::UserId).integer().not_null())
                    .col(ColumnDef::new(Posts::Title).string().not_null())
                    .col(ColumnDef::new(Posts::Content).text().not_null())
                    .col(ColumnDef::new(Posts::CreatedAt).timestamp().not_null()
                        .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)))
                    .col(ColumnDef::new(Posts::UpdatedAt).timestamp().not_null()
                        .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_posts_user_id")
                            .from(Posts::Table, Posts::UserId)
                            .to(Users::Table, Users::Id)
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Posts::Table).to_owned()).await
    }
}

#[derive(Iden)]
enum Posts {
    Table,
    Id,
    UserId,
    Title,
    Content,
    CreatedAt,
    UpdatedAt,
}
