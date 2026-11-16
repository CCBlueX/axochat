use crate::entity::{punishment, report, user};
use log::*;

use actix::prelude::*;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, DatabaseTransaction, DbErr, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
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

#[derive(Debug, Clone)]
pub enum IdentityKey {
    /// A LiquidBounce Account id.
    Account(String),
    Minecraft(Uuid),
}

/// Finds or creates the user behind a login and records the visit. The login of an account takes
/// over the user its linked Minecraft account had before the link was known.
#[derive(Message)]
#[rtype(result = "Result<Identified, DbErr>")]
pub struct Identify {
    pub key: IdentityKey,
    pub name: String,
    /// The Minecraft account of an account user.
    pub linked: Option<Uuid>,
    /// `None` when a service session sends for the user, which keeps its address, linked Minecraft
    /// account and last visit.
    pub ip: Option<IpAddr>,
    /// The id of the user if it is new.
    pub id: Uuid,
    pub at: i64,
}

#[derive(Debug)]
pub struct Identified {
    pub user: user::Model,
    /// The Minecraft user that was merged into `user` and no longer exists.
    pub absorbed: Option<Uuid>,
}

impl Handler<Identify> for Store {
    type Result = AtomicResponse<Self, Result<Identified, DbErr>>;

    fn handle(&mut self, msg: Identify, _ctx: &mut Context<Self>) -> Self::Result {
        atomic!(self, db => identify(&db, msg).await)
    }
}

async fn identify(db: &DatabaseConnection, msg: Identify) -> Result<Identified, DbErr> {
    let txn = db.begin().await?;
    let existing = match &msg.key {
        IdentityKey::Account(account) => user::Column::Account.eq(account.as_str()),
        IdentityKey::Minecraft(uuid) => user::Column::MinecraftUuid.eq(*uuid),
    };
    let existing = user::Entity::find().filter(existing).one(&txn).await?;
    let former = match (&msg.key, msg.linked) {
        (IdentityKey::Account(_), Some(uuid)) => {
            user::Entity::find().filter(user::Column::MinecraftUuid.eq(uuid)).one(&txn).await?
        }
        _ => None,
    };
    let mut absorbed = None;
    let existing = match (existing, former) {
        // keeps its id, so its punishments and reports stay with it
        (None, Some(former)) => {
            let mut model: user::ActiveModel = former.into();
            if let IdentityKey::Account(account) = &msg.key {
                model.account = Set(Some(account.clone()));
            }
            model.minecraft_uuid = Set(None);
            Some(model.update(&txn).await?)
        }
        (Some(user), Some(former)) => {
            merge(&txn, former.id, user.id).await?;
            absorbed = Some(former.id);
            Some(user)
        }
        (existing, None) => existing,
    };
    let user = match existing {
        Some(model) => {
            let mut model: user::ActiveModel = model.into();
            model.name = Set(msg.name);
            if let Some(ip) = msg.ip {
                model.last_seen_at = Set(msg.at);
                model.last_ip = Set(Some(ip.to_string()));
                if let IdentityKey::Account(_) = msg.key {
                    model.linked_minecraft_uuid = Set(msg.linked);
                }
            }
            model.update(&txn).await?
        }
        None => {
            let (account, minecraft_uuid) = match msg.key {
                IdentityKey::Account(account) => (Some(account), None),
                IdentityKey::Minecraft(uuid) => (None, Some(uuid)),
            };
            user::ActiveModel {
                id: Set(msg.id),
                account: Set(account),
                minecraft_uuid: Set(minecraft_uuid),
                linked_minecraft_uuid: Set(msg.linked),
                name: Set(msg.name),
                created_at: Set(msg.at),
                last_seen_at: Set(msg.at),
                last_ip: Set(msg.ip.map(|ip| ip.to_string())),
            }
            .insert(&txn)
            .await?
        }
    };
    txn.commit().await?;
    Ok(Identified { user, absorbed })
}

/// Moves the punishments and reports of user `from` to user `to` and deletes `from`.
async fn merge(txn: &DatabaseTransaction, from: Uuid, to: Uuid) -> Result<(), DbErr> {
    punishment::Entity::update_many()
        .col_expr(punishment::Column::UserId, Expr::value(to))
        .filter(punishment::Column::UserId.eq(from))
        .exec(txn)
        .await?;
    let reports = report::Entity::find()
        .filter(Condition::any().add(report::Column::ReporterId.eq(from)).add(report::Column::TargetId.eq(from)))
        .all(txn)
        .await?;
    for model in reports {
        let moved = |id: Uuid| if id == from { to } else { id };
        let (id, reporter, target) = (model.id, moved(model.reporter_id), moved(model.target_id));
        // a report both users made of the same message is kept once
        let twice = report::Entity::find()
            .filter(report::Column::ReporterId.eq(reporter))
            .filter(report::Column::TargetId.eq(target))
            .filter(match model.message_id {
                Some(message) => report::Column::MessageId.eq(message),
                None => report::Column::MessageId.is_null(),
            })
            .filter(report::Column::Id.ne(id))
            .one(txn)
            .await?
            .is_some();
        if twice {
            report::Entity::delete_by_id(id).exec(txn).await?;
        } else {
            let mut model: report::ActiveModel = model.into();
            model.reporter_id = Set(reporter);
            model.target_id = Set(target);
            model.update(txn).await?;
        }
    }
    user::Entity::delete_by_id(from).exec(txn).await?;
    Ok(())
}

/// The user of a Minecraft UUID: the account it is linked to, else the Minecraft user.
/// Punishing an unseen UUID creates its user, as banning players who never connected always worked.
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
            let linked = user::Entity::find()
                .filter(user::Column::LinkedMinecraftUuid.eq(msg.uuid))
                .filter(user::Column::Account.is_not_null())
                .one(&db)
                .await?;
            let existing = match linked {
                Some(account) => Some(account),
                None => user::Entity::find().filter(user::Column::MinecraftUuid.eq(msg.uuid)).one(&db).await?,
            };
            match (existing, msg.create) {
                (None, Some((id, at))) => {
                    let user = user::ActiveModel {
                        id: Set(id),
                        account: Set(None),
                        minecraft_uuid: Set(Some(msg.uuid)),
                        linked_minecraft_uuid: Set(None),
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

/// A user by id or name, for staff: accounts first, then Minecraft users; the oldest of a name wins.
#[derive(Message)]
#[rtype(result = "Result<Option<user::Model>, DbErr>")]
pub struct FindUser(pub String);

impl Handler<FindUser> for Store {
    type Result = AtomicResponse<Self, Result<Option<user::Model>, DbErr>>;

    fn handle(&mut self, FindUser(query): FindUser, _ctx: &mut Context<Self>) -> Self::Result {
        atomic!(self, db => {
            if let Ok(id) = Uuid::parse_str(&query) {
                if let Some(user) = user::Entity::find_by_id(id).one(&db).await? {
                    return Ok(Some(user));
                }
            }
            let named = || user::Entity::find().filter(user::Column::Name.eq(query.as_str())).order_by_asc(user::Column::CreatedAt);
            match named().filter(user::Column::Account.is_not_null()).one(&db).await? {
                Some(account) => Ok(Some(account)),
                None => named().one(&db).await,
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
    Report(report::Model),
    ResolveReport { id: Uuid, by: Uuid, at: i64 },
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
            Write::Report(model) => {
                // reporting the same message twice changes nothing
                report::Entity::insert(report::ActiveModel::from(model))
                    .on_conflict_do_nothing_on([
                        report::Column::ReporterId,
                        report::Column::TargetId,
                        report::Column::MessageId,
                    ])
                    .exec_without_returning(&txn)
                    .await?;
            }
            Write::ResolveReport { id, by, at } => {
                report::Entity::update_many()
                    .col_expr(report::Column::ResolvedBy, Expr::value(by))
                    .col_expr(report::Column::ResolvedAt, Expr::value(at))
                    .filter(report::Column::Id.eq(id))
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

/// The latest unresolved reports with the users they name.
#[derive(Message)]
#[rtype(result = "Result<(Vec<report::Model>, Vec<user::Model>), DbErr>")]
pub struct LoadReports {
    pub limit: u64,
}

impl Handler<LoadReports> for Store {
    type Result = AtomicResponse<Self, Result<(Vec<report::Model>, Vec<user::Model>), DbErr>>;

    fn handle(&mut self, msg: LoadReports, _ctx: &mut Context<Self>) -> Self::Result {
        atomic!(self, db => {
            let reports = report::Entity::find()
                .filter(report::Column::ResolvedAt.is_null())
                .order_by_desc(report::Column::CreatedAt)
                .limit(msg.limit)
                .all(&db)
                .await?;
            let ids: Vec<Uuid> = reports.iter().flat_map(|report| [report.reporter_id, report.target_id]).collect();
            let users = user::Entity::find().filter(user::Column::Id.is_in(ids)).all(&db).await?;
            Ok((reports, users))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::punishment::Kind;
    use sea_orm::PaginatorTrait;

    async fn database() -> DatabaseConnection {
        let mut options = sea_orm::ConnectOptions::new("sqlite::memory:");
        options.max_connections(1);
        let db = sea_orm::Database::connect(options).await.unwrap();
        db.get_schema_registry("axochat::entity::*").sync(&db).await.unwrap();
        db
    }

    fn login(key: IdentityKey, linked: Option<Uuid>, id: u128) -> Identify {
        Identify {
            key,
            name: "Player".into(),
            linked,
            ip: Some("203.0.113.7".parse().unwrap()),
            id: Uuid::from_u128(id),
            at: id as i64,
        }
    }

    async fn mute(db: &DatabaseConnection, id: u128, user: Uuid) {
        let model = punishment::Model {
            id: Uuid::from_u128(id),
            kind: Kind::Mute,
            user_id: Some(user),
            ip: None,
            reason: String::new(),
            issued_by: None,
            created_at: 0,
            expires_at: None,
            revoked_at: None,
            revoked_by: None,
        };
        punishment::ActiveModel::from(model).insert(db).await.unwrap();
    }

    async fn report(db: &DatabaseConnection, id: u128, reporter: Uuid, target: Uuid, message: i64) {
        let model = report::Model {
            id: Uuid::from_u128(id),
            reporter_id: reporter,
            target_id: target,
            channel: None,
            message_id: Some(message),
            content: None,
            reason: String::new(),
            created_at: 0,
            resolved_by: None,
            resolved_at: None,
        };
        report::ActiveModel::from(model).insert(db).await.unwrap();
    }

    #[actix_web::test]
    async fn account_login_takes_over_the_minecraft_user() {
        let db = database().await;
        let minecraft = Uuid::from_u128(0xabc);
        let account = || IdentityKey::Account("uid".into());

        let player = identify(&db, login(IdentityKey::Minecraft(minecraft), None, 1)).await.unwrap().user;
        mute(&db, 10, player.id).await;

        // no user of the account yet: the Minecraft user becomes it
        let first = identify(&db, login(account(), Some(minecraft), 2)).await.unwrap();
        assert_eq!((first.user.id, first.absorbed), (player.id, None));
        assert_eq!(first.user.account.as_deref(), Some("uid"));
        assert_eq!((first.user.minecraft_uuid, first.user.linked_minecraft_uuid), (None, Some(minecraft)));

        // a Minecraft user made while the link could not be looked up is merged into it
        let other = Uuid::from_u128(99);
        let again = identify(&db, login(IdentityKey::Minecraft(minecraft), None, 3)).await.unwrap().user;
        assert_ne!(again.id, player.id);
        mute(&db, 11, again.id).await;
        report(&db, 20, other, again.id, 7).await;
        report(&db, 21, other, player.id, 7).await;
        report(&db, 22, again.id, other, 8).await;

        let merged = identify(&db, login(account(), Some(minecraft), 4)).await.unwrap();
        assert_eq!((merged.user.id, merged.absorbed), (player.id, Some(again.id)));
        assert_eq!(user::Entity::find().count(&db).await.unwrap(), 1);
        let muted = punishment::Entity::find().filter(punishment::Column::UserId.eq(player.id)).count(&db).await.unwrap();
        assert_eq!(muted, 2);
        let reports = report::Entity::find().order_by_asc(report::Column::MessageId).all(&db).await.unwrap();
        let named: Vec<_> = reports.iter().map(|report| (report.reporter_id, report.target_id)).collect();
        assert_eq!(named, [(other, player.id), (player.id, other)], "the doubled report is kept once");

        let later = identify(&db, login(account(), Some(minecraft), 5)).await.unwrap();
        assert_eq!((later.user.id, later.absorbed), (player.id, None));
    }
}
