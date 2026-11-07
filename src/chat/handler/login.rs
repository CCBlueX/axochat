use crate::chat::{new_id, now_ms, send_message, ChatServer, ClientPacket, InternalId, SuccessReason, User, UserSession};
use crate::entity::punishment::Kind;
use crate::error::ClientError;
use crate::message::RateLimiter;
use crate::store::Identify;
use log::*;

use actix::*;
use std::collections::HashSet;

impl ChatServer {
    /// Records a verified login in the database and lets the user in unless they are banned.
    pub(super) fn identify(&mut self, user_id: InternalId, info: User, ctx: &mut Context<Self>) {
        let Some(session) = self.connections.get(&user_id) else { return };
        let request = Identify {
            minecraft_uuid: info.uuid,
            name: info.name.chars().take(64).collect(),
            ip: session.ip,
            id: new_id(&mut self.rng),
            at: now_ms(),
        };
        let store = self.store.clone();
        ctx.spawn(
            async move { store.send(request).await }
                .into_actor(self)
                .map(move |result, actor, _ctx| match result {
                    Ok(Ok(model)) => actor.finish_login(user_id, info, model.id),
                    Ok(Err(err)) => {
                        error!("Could not store user `{}`: {}", info.uuid, err);
                        actor.send_error(user_id, ClientError::Internal);
                    }
                    Err(err) => {
                        error!("Store unavailable: {}", err);
                        actor.send_error(user_id, ClientError::Internal);
                    }
                }),
        );
    }

    fn finish_login(&mut self, user_id: InternalId, info: User, stored_id: uuid::Uuid) {
        let Some(session) = self.connections.get(&user_id) else { return };
        if session.is_logged_in() {
            self.send_error(user_id, ClientError::AlreadyLoggedIn);
            return;
        }

        let banned = !self.moderation.is_moderator(&info.uuid)
            && self
                .moderation
                .find(Kind::Ban, Some(stored_id), Some(session.ip), now_ms())
                .is_some();
        if banned {
            info!("Banned user `{}` ({}) tried to log in.", info.name, info.uuid);
            self.send_error(user_id, ClientError::Banned);
            return;
        }

        info!("User `{}` logged in as `{}` ({}).", user_id, info.name, info.uuid);
        self.users
            .entry(info.name.clone())
            .or_insert(UserSession {
                rate_limiter: RateLimiter::new(self.config.message.clone()),
                connections: HashSet::new(),
            })
            .connections
            .insert(user_id);

        let session = self.connections.get_mut(&user_id).expect("checked above");
        session.user = Some(info);
        session.stored_id = Some(stored_id);
        send_message(
            &session.addr,
            ClientPacket::Success {
                reason: SuccessReason::Login,
            },
            "login success",
        );
    }

    pub(super) fn send_error(&self, user_id: InternalId, message: ClientError) {
        if let Some(session) = self.connections.get(&user_id) {
            send_message(&session.addr, ClientPacket::Error { message }, "error");
        }
    }
}
