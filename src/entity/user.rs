use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[sea_orm(unique)]
    pub minecraft_uuid: Uuid,
    #[sea_orm(indexed, column_type = "String(StringLen::N(64))")]
    pub name: String,
    pub created_at: i64,
    #[sea_orm(indexed)]
    pub last_seen_at: i64,
    #[sea_orm(column_type = "String(StringLen::N(45))", nullable)]
    pub last_ip: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
