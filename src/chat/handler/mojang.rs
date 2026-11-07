use super::login::login_minecraft;
use crate::auth::{authenticate, UserInfo};
use crate::chat::{ChatServer, ClientPacket, InternalId, Kind, SuccessReason, User};
use crate::error::ClientError;
use log::*;

use actix::*;
use rand::Rng;
use std::str::FromStr;
use uuid::Uuid;

impl ChatServer {
    pub(super) fn handle_request_mojang_info(&mut self, user_id: InternalId) {
        let Some(session) = self.connections.get_mut(&user_id) else { return };

        let mut bytes = [0; 20];
        self.rng.fill_bytes(&mut bytes);
        // we'll just ignore one bit so we that don't have to deal with a '-' sign
        bytes[0] &= 0b0111_1111;

        let session_hash = crate::auth::encode_sha1_bytes(&bytes);
        session.session_hash = Some(session_hash.clone());
        self.send(user_id, ClientPacket::MojangInfo { session_hash });
    }

    pub(super) fn login_mojang(&mut self, user_id: InternalId, info: User, ctx: &mut Context<Self>) {
        let Some(session) = self.connections.get(&user_id) else { return };
        if let Some(stored_id) = session.stored_id {
            if self.users.get(&stored_id).is_some_and(|online| online.kind == Kind::Account) {
                self.prove_minecraft(user_id, stored_id, info, ctx);
                return;
            }
        }
        let session_hash = session.session_hash.clone();
        let Some(visit) = self.visit(user_id) else { return };
        let Some(session_hash) = session_hash else {
            info!("User `{}` did not request mojang info, but tried to log in.", user_id);
            self.send_error(user_id, ClientError::MojangRequestMissing);
            return;
        };
        let session_url = self.config.mojang.session_url.clone();
        let (api, store) = (self.api.clone(), self.store.clone());
        let allow_messages = info.allow_messages;

        ctx.spawn(
            async move {
                let player = verify(&session_url, user_id, &info, &session_hash).await?;
                login_minecraft(api, store, visit, player).await
            }
            .into_actor(self)
            .map(move |result, actor, _ctx| actor.finish_login(user_id, result, allow_messages)),
        );
    }

    /// An account session shows the Minecraft account it plays on.
    fn prove_minecraft(&mut self, user_id: InternalId, stored_id: Uuid, info: User, ctx: &mut Context<Self>) {
        let Some(session_hash) = self.connections.get_mut(&user_id).and_then(|session| session.session_hash.take()) else {
            self.send_error(user_id, ClientError::MojangRequestMissing);
            return;
        };
        let session_url = self.config.mojang.session_url.clone();

        ctx.spawn(
            async move { verify(&session_url, user_id, &info, &session_hash).await }
                .into_actor(self)
                .map(move |result, actor, _ctx| {
                    let Some(session) = actor.connections.get_mut(&user_id) else { return };
                    if session.stored_id != Some(stored_id) {
                        return;
                    }
                    match result {
                        Ok(player) => {
                            if let Some(user) = &mut session.user {
                                user.uuid = player.uuid;
                            }
                            session.minecraft = Some(player);
                            if session.hello {
                                if let Some(user) = actor.author(user_id) {
                                    let staff = actor.is_staff(user_id);
                                    actor.send(user_id, ClientPacket::Welcome { user, staff });
                                }
                            }
                            actor.send(user_id, ClientPacket::Success { reason: SuccessReason::Minecraft });
                        }
                        Err(error) => actor.send_error(user_id, error),
                    }
                }),
        );
    }
}

async fn verify(session_url: &str, user_id: InternalId, info: &User, session_hash: &str) -> Result<UserInfo, ClientError> {
    let profile = authenticate(session_url, &info.name, session_hash).await.map_err(|err| {
        warn!("Could not authenticate user `{}`: {}", user_id, err);
        ClientError::LoginFailed
    })?;
    if Uuid::from_str(&profile.id).ok() != Some(info.uuid) {
        return Err(ClientError::InvalidId);
    }
    info!("User `{}` has uuid `{}` and username `{}`", user_id, profile.id, profile.name);
    Ok(UserInfo { name: profile.name, uuid: info.uuid })
}
