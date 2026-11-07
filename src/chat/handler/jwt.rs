use super::mojang::login_minecraft;
use crate::auth::UserInfo;
use crate::chat::world::Player;
use crate::chat::{ChatServer, ClientPacket, InternalId, Kind, Login};
use crate::error::ClientError;
use log::*;

use actix::*;

impl ChatServer {
    pub(super) fn handle_request_jwt(&mut self, user_id: InternalId) {
        let Some(connection) = self.connections.get(&user_id) else { return };
        let Some(auth) = &self.authenticator else {
            info!("User `{}` tried to request not supported JWT", user_id);
            self.send_error(user_id, ClientError::NotSupported);
            return;
        };
        let Login::User(user) = connection.login else {
            info!("User `{}` tried to get JWT but is not logged in.", user_id);
            self.send_error(user_id, ClientError::NotLoggedIn);
            return;
        };
        // the token stands for a Minecraft account, so accounts need to have proven one
        let identity = self.identity(user);
        let info = match (&connection.minecraft, identity.kind) {
            (Some(player), _) => UserInfo {
                name: player.name.clone(),
                uuid: player.uuid,
            },
            (None, Kind::Mojang) => UserInfo {
                name: identity.name.clone(),
                uuid: identity.uuid,
            },
            (None, Kind::Account) => {
                self.send_error(user_id, ClientError::NotSupported);
                return;
            }
        };
        match auth.new_token(info) {
            Ok(token) => {
                self.send(user_id, ClientPacket::NewJWT { token });
            }
            Err(err) => {
                warn!("Could not create new token for user `{}`: {}", user_id, err);
                self.send_error(user_id, ClientError::Internal);
            }
        }
    }

    pub(super) fn login_jwt(
        &mut self,
        user_id: InternalId,
        token: &str,
        allow_messages: bool,
        ctx: &mut Context<Self>,
    ) {
        let Some(auth) = &self.authenticator else {
            info!("User `{}` tried to request not supported JWT", user_id);
            self.send_error(user_id, ClientError::NotSupported);
            return;
        };
        let info = auth.auth(token);
        let Some(visit) = self.begin_login(user_id, true) else { return };
        let info = match info {
            Ok(info) => info,
            Err(err) => {
                info!("Login of user `{}` using JWT failed: {}", user_id, err);
                self.finish_login(user_id, Err(ClientError::LoginFailed), allow_messages);
                return;
            }
        };
        let delay = visit.delay;
        let (api, store, logins) = (self.api.clone(), self.store.clone(), self.logins.clone());

        ctx.spawn(
            async move {
                actix::clock::sleep(delay).await;
                let _permit = logins.acquire().await;
                login_minecraft(api, store, visit, Player { uuid: info.uuid, name: info.name }).await
            }
            .into_actor(self)
            .map(move |result, actor, _ctx| actor.finish_login(user_id, result, allow_messages)),
        );
    }
}
