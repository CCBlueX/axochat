use super::login::login_minecraft;
use crate::chat::{ChatServer, ClientPacket, InternalId};
use crate::error::ClientError;
use log::*;

use actix::*;

impl ChatServer {
    pub(super) fn handle_request_jwt(&mut self, user_id: InternalId) {
        let Some(session) = self.connections.get(&user_id) else { return };
        let Some(auth) = &self.authenticator else {
            info!("User `{}` tried to request not supported JWT", user_id);
            self.send_error(user_id, ClientError::NotSupported);
            return;
        };
        if !session.is_logged_in() {
            info!("User `{}` tried to get JWT but is not logged in.", user_id);
            self.send_error(user_id, ClientError::NotLoggedIn);
            return;
        }
        // the token stands for a Minecraft account, which account logins have not proven
        let Some(player) = session.minecraft.clone() else {
            self.send_error(user_id, ClientError::NotSupported);
            return;
        };
        match auth.new_token(player) {
            Ok(token) => self.send(user_id, ClientPacket::NewJWT { token }),
            Err(err) => {
                warn!("Could not create new token for user `{}`: {}", user_id, err);
                self.send_error(user_id, ClientError::Internal);
            }
        }
    }

    pub(super) fn handle_login_jwt(&mut self, user_id: InternalId, jwt: &str, allow_messages: bool, ctx: &mut Context<Self>) {
        let Some(auth) = &self.authenticator else {
            info!("User `{}` tried to request not supported JWT", user_id);
            self.send_error(user_id, ClientError::NotSupported);
            return;
        };
        let player = match auth.auth(jwt) {
            Ok(player) => player,
            Err(err) => {
                info!("Login of user `{}` using JWT failed: {}", user_id, err);
                self.send_error(user_id, ClientError::LoginFailed);
                return;
            }
        };
        let Some(visit) = self.visit(user_id) else { return };
        let (api, store) = (self.api.clone(), self.store.clone());
        ctx.spawn(
            login_minecraft(api, store, visit, player)
                .into_actor(self)
                .map(move |result, actor, _ctx| actor.finish_login(user_id, result, allow_messages)),
        );
    }
}
