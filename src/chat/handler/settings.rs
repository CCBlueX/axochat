use crate::chat::{ChatServer, ClientPacket, InternalId, SettingsView, UserId};
use crate::error::ClientError;

impl ChatServer {
    /// Settings and moderation packets share a budget.
    pub(super) fn acting_user(&mut self, user_id: InternalId) -> Option<UserId> {
        let user = self.logged_in(user_id)?;
        let online = self.users.get_mut(&user)?;
        if !online.actions.allow() {
            self.send_error(user_id, ClientError::RateLimited);
            return None;
        }
        Some(user)
    }

    pub(super) fn handle_settings(
        &mut self,
        user_id: InternalId,
        allow_messages: Option<bool>,
        watch_servers: Option<bool>,
        server_chat: Option<bool>,
    ) {
        let Some(user) = self.acting_user(user_id) else { return };
        let staff = self.is_staff(user);
        if watch_servers == Some(true) && !staff {
            self.send_error(user_id, ClientError::NotPermitted);
            return;
        }
        if let Some(connection) = self.connections.get_mut(&user_id) {
            connection.allow_messages = allow_messages.unwrap_or(connection.allow_messages);
            connection.server_chat = server_chat.unwrap_or(connection.server_chat);
            connection.watch_servers = staff && watch_servers.unwrap_or(connection.watch_servers);
        }

        if let Some(settings) = self.settings(user_id) {
            self.send_v2(user_id, ClientPacket::Settings(settings));
        }
    }

    pub(super) fn settings(&self, user_id: InternalId) -> Option<SettingsView> {
        let connection = self.connections.get(&user_id)?;
        let user = connection.user()?;
        Some(SettingsView {
            allow_messages: connection.allow_messages,
            watch_servers: self.is_staff(user).then_some(connection.watch_servers),
            server_chat: connection.server_chat,
        })
    }
}
