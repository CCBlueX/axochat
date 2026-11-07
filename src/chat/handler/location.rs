use super::message::send_frame;
use crate::chat::world::{server_key, Location};
use crate::chat::{Channel, ChatServer, ClientPacket, InternalId, Protocol, UserId};
use crate::error::ClientError;

impl ChatServer {
    pub(super) fn handle_location(&mut self, user_id: InternalId, server: Option<String>) {
        if self.logged_in(user_id).is_none() {
            return;
        }
        let Some(connection) = self.connections.get_mut(&user_id) else { return };
        if !connection.limits.location.allow() {
            return;
        }
        connection.location = server
            .as_deref()
            .and_then(|server| server_key(server, connection.ip))
            .map(|server| Location {
                key: server.key,
                address: server.display,
            });
    }

    pub(super) fn send_server_message(&mut self, user_id: InternalId, user: UserId, content: String) {
        let location = self.connections.get(&user_id).and_then(|connection| {
            connection.location.clone().filter(|_| connection.server_chat)
        });
        let Some(location) = location else {
            self.send(user_id, ClientPacket::error_with(ClientError::UnknownChannel, "server"));
            return;
        };

        let message = self.record(Channel::Server, user, &content, None);
        let frame = self.chat_frame(message, Channel::Server);
        let watched = self.chat_frame(message, Channel::Watched(location.address));
        for connection in self.connections.values() {
            let Some(recipient) = connection.user() else { continue };
            if connection.protocol < Protocol::V2 {
                continue;
            }
            if connection.watch_servers && self.is_staff(recipient) {
                send_frame(connection, &watched);
            } else if connection.server_chat
                && connection.location.as_ref().is_some_and(|other| other.key == location.key)
            {
                send_frame(connection, &frame);
            }
        }
    }
}
