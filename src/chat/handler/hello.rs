use crate::chat::{ChatServer, ClientPacket, InternalId, PROTOCOL};

impl ChatServer {
    /// Answers with the protocol both sides speak; only before logging in, so a login keeps its form.
    pub(super) fn handle_hello(&mut self, user_id: InternalId, protocol: u32) {
        let Some(session) = self.connections.get_mut(&user_id) else { return };
        if session.is_logged_in() {
            return;
        }
        session.hello = protocol >= PROTOCOL;
        let protocol = if session.hello { PROTOCOL } else { 1 };
        self.send(user_id, ClientPacket::Hello { protocol });
    }
}
