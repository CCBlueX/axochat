use crate::chat::{ChatServer, ClientPacket, InternalId};
use crate::error::*;
use log::*;

impl ChatServer {
    pub(super) fn send_user_count(&mut self, user_id: InternalId) {
        let Some(session) = self.connections.get(&user_id) else { return };
        if !session.is_logged_in() {
            info!("`{}` is not logged in.", user_id);
            self.send_error(user_id, ClientError::NotLoggedIn);
            return;
        }
        if !self.is_staff(user_id) {
            info!("`{}` tried to get the user count without permission", user_id);
            self.send_error(user_id, ClientError::NotPermitted);
            return;
        }
        self.send(
            user_id,
            ClientPacket::UserCount {
                connections: self.connections.len() as u32,
                logged_in: self.users.len() as u32,
            },
        );
    }
}
