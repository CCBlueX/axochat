use super::{ChatServer, ClientPacket};
use crate::auth::UserInfo;
use crate::chat::{now_ms, send_message, InternalId, User};
use crate::entity::punishment::Kind;

use crate::error::*;
use log::*;

impl ChatServer {
    pub(super) fn handle_message(&mut self, user_id: InternalId, content: String) {
        if self.check_ratelimit(user_id, content.clone()) {
            return;
        }
        let Some(info) = self.basic_check(user_id, &content) else { return };

        info!("User `{}` has written `{}`.", user_id, content);
        let client_packet = ClientPacket::Message {
            author_info: UserInfo {
                name: info.name,
                uuid: info.uuid,
            },
            content,
        };
        for session in self.connections.values() {
            send_message(&session.addr, client_packet.clone(), "broadcast message");
        }
    }

    pub(super) fn handle_private_message(&mut self, user_id: InternalId, receiver: String, content: String) {
        if self.check_ratelimit(user_id, content.clone()) {
            return;
        }

        if let Some(sender_info) = self.basic_check(user_id, &content) {
            let Some(receiver_user) = self.find_by_name(&receiver) else {
                debug!("User `{}` tried to write to non-existing user `{}`.", user_id, receiver);
                return;
            };

            for receiver_session in receiver_user
                .connections
                .iter()
                .filter_map(|id| self.connections.get(id))
            {
                match &receiver_session.user {
                    Some(info) if info.allow_messages => {
                        let client_packet = ClientPacket::PrivateMessage {
                            author_info: UserInfo {
                                name: sender_info.name.clone(),
                                uuid: sender_info.uuid,
                            },
                            content: content.clone(),
                        };
                        info!("User `{}` has written to `{}` privately.", user_id, receiver);
                        if send_message(&receiver_session.addr, client_packet, "private message") {
                            return;
                        }
                    }
                    _ => {}
                }
            }
        }

        self.send_error(user_id, ClientError::PrivateMessageNotAccepted);
    }

    /// The sender, if they may send this message.
    fn basic_check(&self, user_id: InternalId, content: &str) -> Option<User> {
        let session = self.connections.get(&user_id)?;
        let Some(info) = &session.user else {
            info!("`{}` is not logged in.", user_id);
            self.send_error(user_id, ClientError::NotLoggedIn);
            return None;
        };

        if let Err(err) = self.validator.validate(content, self.has_perks(user_id)) {
            info!("User `{}` tried to send invalid message: {}", user_id, err);
            if let Error::AxoChat { source } = err {
                self.send_error(user_id, source);
            }
            return None;
        }
        let muted = !self.is_staff(user_id)
            && self
                .moderation
                .find(Kind::Mute, session.stored_id, Some(session.ip), now_ms())
                .is_some();
        if muted {
            info!("User `{}` tried to send message while muted", user_id);
            self.send_error(user_id, ClientError::Banned);
            return None;
        }

        Some(info.clone())
    }

    fn check_ratelimit(&mut self, user_id: InternalId, message: String) -> bool {
        let Some(stored_id) = self.connections.get(&user_id).and_then(|session| session.stored_id) else {
            return false;
        };
        let Some(user) = self.users.get_mut(&stored_id) else { return false };
        if user.rate_limiter.check_new_message(message) {
            info!("User `{}` tried to send message, but was rate limited.", user_id);
            self.send_error(user_id, ClientError::RateLimited);
            true
        } else {
            false
        }
    }
}
