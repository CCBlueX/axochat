use super::{ChatServer, ClientPacket};
use crate::chat::{new_id, now_ms, InternalId, SuccessReason};
use crate::entity::punishment::Kind;
use crate::error::*;
use crate::moderation::Punishment;
use crate::store::{MinecraftUser, Write};
use log::*;

use actix::*;
use uuid::Uuid;

impl ChatServer {
    /// The database id of the moderator behind a connection.
    fn moderator(&self, user_id: InternalId) -> Option<Uuid> {
        let session = self.connections.get(&user_id)?;
        let (Some(info), Some(stored_id)) = (&session.user, session.stored_id) else {
            info!("`{}` is not logged in.", user_id);
            self.send_error(user_id, ClientError::NotLoggedIn);
            return None;
        };
        if !self.moderation.is_moderator(&info.uuid) {
            info!("`{}` tried to (un-)ban user without permission", user_id);
            self.send_error(user_id, ClientError::NotPermitted);
            return None;
        }
        Some(stored_id)
    }

    /// Bans the user of a Minecraft UUID permanently; they cannot log in until pardoned.
    pub(super) fn ban_user(&mut self, user_id: InternalId, uuid: Uuid, ctx: &mut Context<Self>) {
        let Some(staff) = self.moderator(user_id) else { return };
        if self.moderation.is_moderator(&uuid) {
            self.send_error(user_id, ClientError::NotPermitted);
            return;
        }

        let store = self.store.clone();
        let create = Some((new_id(&mut self.rng), now_ms()));
        ctx.spawn(
            async move { store.send(MinecraftUser { uuid, create }).await }
                .into_actor(self)
                .map(move |result, actor, _ctx| {
                    let Ok(Ok(Some(target))) = result else {
                        error!("Could not look up the user of `{}`", uuid);
                        actor.send_error(user_id, ClientError::Internal);
                        return;
                    };
                    let already = actor.moderation.find(Kind::Ban, Some(target.id), None, now_ms());
                    if already.is_some_and(|ban| ban.expires_at.is_none()) {
                        actor.send(user_id, ClientPacket::Success { reason: SuccessReason::Ban });
                        return;
                    }
                    let punishment = Punishment {
                        id: new_id(&mut actor.rng),
                        kind: Kind::Ban,
                        user: Some(target.id),
                        ip: None,
                        reason: String::new(),
                        issued_by: Some(staff),
                        created_at: now_ms(),
                        expires_at: None,
                    };
                    actor.punish(punishment);
                    info!("User `{}` banned.", uuid);
                    actor.send(user_id, ClientPacket::Success { reason: SuccessReason::Ban });
                }),
        );
    }

    /// Lifts every punishment of the user of a Minecraft UUID.
    pub(super) fn unban_user(&mut self, user_id: InternalId, uuid: Uuid, ctx: &mut Context<Self>) {
        let Some(staff) = self.moderator(user_id) else { return };

        let store = self.store.clone();
        ctx.spawn(
            async move { store.send(MinecraftUser { uuid, create: None }).await }
                .into_actor(self)
                .map(move |result, actor, _ctx| {
                    let target = match result {
                        Ok(Ok(target)) => target,
                        _ => {
                            error!("Could not look up the user of `{}`", uuid);
                            actor.send_error(user_id, ClientError::Internal);
                            return;
                        }
                    };
                    let ids = target.map(|target| actor.moderation.revoke(target.id)).unwrap_or_default();
                    if ids.is_empty() {
                        actor.send_error(user_id, ClientError::NotBanned);
                        return;
                    }
                    actor.persist(vec![Write::Revoke {
                        ids,
                        by: Some(staff),
                        at: now_ms(),
                    }]);
                    info!("User `{}` unbanned.", uuid);
                    actor.send(user_id, ClientPacket::Success { reason: SuccessReason::Unban });
                }),
        );
    }

    /// Stores the punishment and applies it to the connections it covers.
    fn punish(&mut self, punishment: Punishment) {
        self.persist(vec![Write::Punish(punishment.to_model())]);

        if punishment.kind == Kind::Ban {
            let covered: Vec<InternalId> = self
                .connections
                .iter()
                .filter(|(_, session)| {
                    session.user.as_ref().is_some_and(|info| !self.moderation.is_moderator(&info.uuid))
                        && punishment.covers(session.stored_id, Some(session.ip))
                })
                .map(|(id, _)| *id)
                .collect();
            for id in covered {
                self.send_error(id, ClientError::Banned);
                self.logout(id);
            }
        }

        self.moderation.add(punishment);
    }

    fn send(&self, user_id: InternalId, packet: ClientPacket) {
        if let Some(session) = self.connections.get(&user_id) {
            crate::chat::send_message(&session.addr, packet, "moderation");
        }
    }
}
