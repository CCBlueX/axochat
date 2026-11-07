use crate::entity::{punishment, user};
use log::*;

use actix::prelude::*;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, DbErr, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};
use std::net::IpAddr;
use std::time::Duration;
use uuid::Uuid;

/// Serializes all database access: every message finishes before the next one starts,
/// so a read always sees the writes queued before it.
pub struct Store {
    db: DatabaseConnection,
}

impl Store {
    pub fn new(db: DatabaseConnection) -> Store {
        Store { db }
    }
}

impl Actor for Store {
    type Context = Context<Self>;
}

macro_rules! atomic {
    ($actor:expr, $db:ident => $body:expr) => {{
        let $db = $actor.db.clone();
        AtomicResponse::new(Box::pin(async move { $body }.into_actor($actor)))
    }};
}

/// Finds or creates the user behind a Minecraft login and records the visit.
#[derive(Message)]
#[rtype(result = "Result<user::Model, DbErr>")]
pub struct Identify {
    pub minecraft_uuid: Uuid,
    pub name: String,
    pub ip: IpAddr,
    /// The id of the user if it is new.
    pub id: Uuid,
    pub at: i64,
}

impl Handler<Identify> for Store {
    type Result = AtomicResponse<Self, Result<user::Model, DbErr>>;

    fn handle(&mut self, msg: Identify, _ctx: &mut Context<Self>) -> Self::Result {
        atomic!(self, db => {
            let existing = user::Entity::find()
                .filter(user::Column::MinecraftUuid.eq(msg.minecraft_uuid))
                .one(&db)
                .await?;
            match existing {
                Some(model) => {
                    let mut model: user::ActiveModel = model.into();
                    model.name = Set(msg.name);
                    model.last_seen_at = Set(msg.at);
                    model.last_ip = Set(Some(msg.ip.to_string()));
                    model.update(&db).await
                }
                None => {
                    user::ActiveModel {
                        id: Set(msg.id),
                        minecraft_uuid: Set(msg.minecraft_uuid),
                        name: Set(msg.name),
                        created_at: Set(msg.at),
                        last_seen_at: Set(msg.at),
                        last_ip: Set(Some(msg.ip.to_string())),
                    }
                    .insert(&db)
                    .await
                }
            }
        })
    }
}

/// The user of a Minecraft UUID. Punishing an unseen UUID creates its user, as banning
/// players who never connected always worked.
#[derive(Message)]
#[rtype(result = "Result<Option<user::Model>, DbErr>")]
pub struct MinecraftUser {
    pub uuid: Uuid,
    pub create: Option<(Uuid, i64)>,
}

impl Handler<MinecraftUser> for Store {
    type Result = AtomicResponse<Self, Result<Option<user::Model>, DbErr>>;

    fn handle(&mut self, msg: MinecraftUser, _ctx: &mut Context<Self>) -> Self::Result {
        atomic!(self, db => {
            let existing = user::Entity::find()
                .filter(user::Column::MinecraftUuid.eq(msg.uuid))
                .one(&db)
                .await?;
            match (existing, msg.create) {
                (None, Some((id, at))) => {
                    let user = user::ActiveModel {
                        id: Set(id),
                        minecraft_uuid: Set(msg.uuid),
                        name: Set(msg.uuid.hyphenated().to_string()),
                        created_at: Set(at),
                        last_seen_at: Set(at),
                        last_ip: Set(None),
                    }
                    .insert(&db)
                    .await?;
                    Ok(Some(user))
                }
                (existing, _) => Ok(existing),
            }
        })
    }
}

/// Changes made in memory first, written in order in one transaction.
#[derive(Message)]
#[rtype(result = "()")]
pub struct Persist(pub Vec<Write>);

#[derive(Debug, Clone, PartialEq)]
pub enum Write {
    Punish(punishment::Model),
    Revoke { ids: Vec<Uuid>, by: Option<Uuid>, at: i64 },
    ForgetIps { seen_before: i64 },
}

const ATTEMPTS: u32 = 3;

impl Handler<Persist> for Store {
    type Result = AtomicResponse<Self, ()>;

    fn handle(&mut self, Persist(writes): Persist, _ctx: &mut Context<Self>) -> Self::Result {
        atomic!(self, db => {
            for attempt in 1..=ATTEMPTS {
                match apply(&db, &writes).await {
                    Ok(()) => return,
                    Err(err) if attempt < ATTEMPTS => {
                        warn!("Could not write {:?}, retrying: {}", writes, err);
                        actix::clock::sleep(Duration::from_secs(1 << attempt)).await;
                    }
                    Err(err) => error!("Could not write {:?}: {}", writes, err),
                }
            }
        })
    }
}

async fn apply(db: &DatabaseConnection, writes: &[Write]) -> Result<(), DbErr> {
    let txn = db.begin().await?;
    for write in writes {
        match write.clone() {
            Write::Punish(model) => {
                punishment::ActiveModel::from(model).insert(&txn).await?;
            }
            Write::Revoke { ids, by, at } => {
                punishment::Entity::update_many()
                    .col_expr(punishment::Column::RevokedAt, Expr::value(at))
                    .col_expr(punishment::Column::RevokedBy, Expr::value(by))
                    .filter(punishment::Column::Id.is_in(ids))
                    .exec(&txn)
                    .await?;
            }
            Write::ForgetIps { seen_before } => {
                user::Entity::update_many()
                    .col_expr(user::Column::LastIp, Expr::value(Option::<String>::None))
                    .filter(user::Column::LastSeenAt.lt(seen_before))
                    .filter(user::Column::LastIp.is_not_null())
                    .exec(&txn)
                    .await?;
            }
        }
    }
    txn.commit().await
}

#[derive(Message)]
#[rtype(result = "Result<Vec<punishment::Model>, DbErr>")]
pub struct LoadPunishments {
    pub now: i64,
}

impl Handler<LoadPunishments> for Store {
    type Result = AtomicResponse<Self, Result<Vec<punishment::Model>, DbErr>>;

    fn handle(&mut self, msg: LoadPunishments, _ctx: &mut Context<Self>) -> Self::Result {
        atomic!(self, db => {
            punishment::Entity::find()
                .filter(punishment::Column::RevokedAt.is_null())
                .filter(
                    Condition::any()
                        .add(punishment::Column::ExpiresAt.is_null())
                        .add(punishment::Column::ExpiresAt.gt(msg.now)),
                )
                .all(&db)
                .await
        })
    }
}
