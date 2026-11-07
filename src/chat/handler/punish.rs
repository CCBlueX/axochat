use super::{ChatServer, ClientPacket};
use crate::chat::{new_id, now_ms, InternalId, Kind as UserKind, PunishmentView, SuccessReason, UserRef};
use crate::entity::punishment::Kind;
use crate::error::*;
use crate::ip::Cidr;
use crate::moderation::Punishment;
use crate::store::{FindUser, Write};
use log::*;

use actix::*;
use std::net::IpAddr;
use uuid::Uuid;

const MAX_REASON: usize = 256;
const NO_TARGET: &str = "a user or an ip is required";

pub(super) enum Target {
    User { query: String, include_ip: bool },
    Ip(String),
    Nobody,
}

/// A user found by id or name, who may be offline.
pub(super) struct Found {
    pub user: UserRef,
    last_ip: Option<IpAddr>,
    staff: bool,
}

impl ChatServer {
    pub(super) fn handle_punish(
        &mut self,
        user_id: InternalId,
        target: Target,
        kind: Kind,
        duration: Option<u64>,
        reason: String,
        ctx: &mut Context<Self>,
    ) {
        let Some(staff) = self.moderator(user_id) else { return };
        let created_at = now_ms();
        let mut punishment = Punishment {
            id: new_id(&mut self.rng),
            kind,
            user: None,
            ip: None,
            reason: reason.chars().take(MAX_REASON).collect(),
            issued_by: Some(staff),
            created_at,
            expires_at: duration.map(|seconds| created_at.saturating_add((seconds as i64).saturating_mul(1000))),
        };

        match target {
            Target::User { query, include_ip } => self.find_user(ctx, user_id, query, move |actor, found| {
                if found.staff {
                    actor.send_error(user_id, ClientError::NotPermitted);
                    return;
                }
                punishment.user = Some(found.user.id);
                punishment.ip = found.last_ip.filter(|_| include_ip).map(Cidr::ban_range);
                info!("`{}` punished `{}` ({:?}).", staff, found.user.name, kind);
                actor.punish(punishment);
                actor.send(user_id, ClientPacket::Success { reason: SuccessReason::Punish });
            }),
            Target::Ip(ip) => {
                let Ok(ip) = ip.parse::<Cidr>() else {
                    self.send_error(user_id, ClientError::InvalidPacket(format!("invalid ip `{}`", ip)));
                    return;
                };
                punishment.ip = Some(ip);
                info!("`{}` punished {} ({:?}).", staff, ip, kind);
                self.punish(punishment);
                self.send(user_id, ClientPacket::Success { reason: SuccessReason::Punish });
            }
            Target::Nobody => self.send_error(user_id, ClientError::InvalidPacket(NO_TARGET.into())),
        }
    }

    pub(super) fn handle_pardon(
        &mut self,
        user_id: InternalId,
        user: Option<String>,
        ip: Option<String>,
        ctx: &mut Context<Self>,
    ) {
        let Some(staff) = self.moderator(user_id) else { return };
        match (user, ip) {
            (Some(query), _) => self.find_user(ctx, user_id, query, move |actor, found| {
                actor.pardon(user_id, staff, Some(found.user.id), None);
            }),
            (None, Some(ip)) => match ip.parse::<Cidr>() {
                Ok(ip) => self.pardon(user_id, staff, None, Some(ip)),
                Err(_) => self.send_error(user_id, ClientError::InvalidPacket(format!("invalid ip `{}`", ip))),
            },
            (None, None) => self.send_error(user_id, ClientError::InvalidPacket(NO_TARGET.into())),
        }
    }

    pub(super) fn handle_request_punishments(&mut self, user_id: InternalId, user: String, ctx: &mut Context<Self>) {
        if self.moderator(user_id).is_none() {
            return;
        }
        self.find_user(ctx, user_id, user, move |actor, found| {
            let punishments = actor
                .moderation
                .of_user(found.user.id, now_ms())
                .into_iter()
                .map(|punishment| PunishmentView {
                    id: punishment.id,
                    kind: punishment.kind,
                    ip: punishment.ip.map(|ip| ip.to_string()),
                    reason: punishment.reason.clone(),
                    issued_by: punishment.issued_by,
                    created: punishment.created_at,
                    expires: punishment.expires_at,
                })
                .collect();
            actor.send(user_id, ClientPacket::Punishments { user: found.user, punishments });
        });
    }

    /// Stores the punishment and applies it to the connections it covers.
    pub(super) fn punish(&mut self, punishment: Punishment) {
        self.persist(vec![Write::Punish(punishment.to_model())]);

        let covered: Vec<InternalId> = self
            .connections
            .iter()
            .filter(|(id, session)| {
                session.is_logged_in() && !self.is_staff(**id) && punishment.covers(session.stored_id, Some(session.ip))
            })
            .map(|(id, _)| *id)
            .collect();
        for id in covered {
            if self.connections.get(&id).is_some_and(|session| session.hello) {
                self.send(
                    id,
                    ClientPacket::Punished {
                        kind: punishment.kind,
                        reason: punishment.reason.clone(),
                        expires: punishment.expires_at,
                    },
                );
            }
            if punishment.kind == Kind::Ban {
                self.send_error(id, ClientError::Banned);
                self.logout(id);
            }
        }

        self.moderation.add(punishment);
    }

    fn pardon(&mut self, user_id: InternalId, staff: Uuid, user: Option<Uuid>, ip: Option<Cidr>) {
        let ids = self.moderation.revoke(user, ip);
        if ids.is_empty() {
            self.send_error(user_id, ClientError::NotBanned);
            return;
        }
        self.persist(vec![Write::Revoke {
            ids,
            by: Some(staff),
            at: now_ms(),
        }]);
        info!("`{}` pardoned {:?} {:?}.", staff, user, ip);
        self.send(user_id, ClientPacket::Success { reason: SuccessReason::Pardon });
    }

    /// Finds a user by id or name, online ones first; answers `UnknownUser` when there is none.
    pub(super) fn find_user<F>(&mut self, ctx: &mut Context<Self>, user_id: InternalId, query: String, then: F)
    where
        F: FnOnce(&mut ChatServer, Found) + 'static,
    {
        let unknown = query.clone();
        self.look_up_user(ctx, user_id, query, move |actor, found| match found {
            Some(found) => then(actor, found),
            None => actor.send_error(user_id, ClientError::UnknownUser(unknown)),
        });
    }

    /// Finds a user by id or name, online ones first; answers `Internal` when the database fails.
    pub(super) fn look_up_user<F>(&mut self, ctx: &mut Context<Self>, user_id: InternalId, query: String, then: F)
    where
        F: FnOnce(&mut ChatServer, Option<Found>) + 'static,
    {
        if let Some(found) = self.find_online(&query) {
            then(self, Some(found));
            return;
        }

        let store = self.store.clone();
        let find = FindUser(query.clone());
        ctx.spawn(async move { store.send(find).await }.into_actor(self).map(move |result, actor, _ctx| {
            let model = match result {
                Ok(Ok(Some(model))) => model,
                Ok(Ok(None)) => return then(actor, None),
                _ => {
                    error!("Could not look up user `{}`", query);
                    return actor.send_error(user_id, ClientError::Internal);
                }
            };
            let found = Found {
                user: UserRef::from(&model),
                last_ip: model.last_ip.and_then(|ip| ip.parse().ok()),
                staff: actor.users.get(&model.id).is_some_and(|user| actor.has_staff_role(&user.roles)),
            };
            then(actor, Some(found));
        }));
    }

    fn find_online(&self, query: &str) -> Option<Found> {
        let id = Uuid::parse_str(query)
            .ok()
            .filter(|id| self.users.contains_key(id))
            .or_else(|| {
                self.find_by_name(query)
                    .min_by_key(|(_, user)| user.kind != UserKind::Account)
                    .map(|(id, _)| id)
            })?;
        let online = self.users.get(&id)?;
        let session = online.connections.iter().find_map(|id| self.connections.get(id))?;
        Some(Found {
            user: UserRef {
                id,
                kind: online.kind,
                name: session.user.as_ref()?.name.clone(),
            },
            last_ip: Some(session.ip),
            staff: self.has_staff_role(&online.roles),
        })
    }
}
