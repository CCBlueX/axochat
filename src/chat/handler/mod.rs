mod account;
mod punish;
mod report;
mod count;
mod hello;
mod jwt;
mod location;
mod login;
mod message;
mod mojang;
mod settings;

use super::{ChatServer, ServerPacket, ServerPacketId};

use actix::*;

impl Handler<ServerPacketId> for ChatServer {
    type Result = ();

    fn handle(
        &mut self,
        ServerPacketId { user_id, packet }: ServerPacketId,
        ctx: &mut Context<Self>,
    ) {
        match packet {
            ServerPacket::RequestMojangInfo => {
                self.handle_request_mojang_info(user_id);
            }
            ServerPacket::LoginMojang(info) => {
                self.login_mojang(user_id, info, ctx);
            }
            ServerPacket::LoginAccount { token, allow_messages } => {
                self.login_account(user_id, token, allow_messages, ctx);
            }
            ServerPacket::RequestJWT | ServerPacket::LoginJWT(_) => {
                self.handle_jwt(user_id);
            }
            ServerPacket::Message { content } => self.handle_message(user_id, content),
            ServerPacket::ChatMessage { channel, content } => self.handle_chat_message(user_id, channel, content),
            ServerPacket::PrivateMessage { receiver, content } => {
                self.handle_private_message(user_id, receiver, content);
            }
            ServerPacket::BanUser { user } => {
                self.ban_user(user_id, user, ctx);
            }
            ServerPacket::UnbanUser { user } => {
                self.unban_user(user_id, user, ctx);
            }
            ServerPacket::Punish {
                user,
                ip,
                kind,
                duration,
                reason,
                include_ip,
            } => {
                self.handle_punish(user_id, user, ip, kind, duration, reason, include_ip, ctx);
            }
            ServerPacket::Pardon { user, ip } => {
                self.handle_pardon(user_id, user, ip, ctx);
            }
            ServerPacket::Settings {
                allow_messages,
                watch_servers,
                server_chat,
            } => {
                self.handle_settings(user_id, allow_messages, watch_servers, server_chat);
            }
            ServerPacket::Report { user, message, reason } => {
                self.handle_report(user_id, user, message, reason, ctx);
            }
            ServerPacket::RequestReports => {
                self.handle_request_reports(user_id, ctx);
            }
            ServerPacket::ResolveReport { id } => {
                self.handle_resolve_report(user_id, id);
            }
            ServerPacket::Location { server } => {
                self.handle_location(user_id, server);
            }
            ServerPacket::RequestPunishments { user } => {
                self.handle_request_punishments(user_id, user, ctx);
            }
            ServerPacket::RequestUserCount => {
                self.send_user_count(user_id);
            }
            ServerPacket::Hello { protocol } => {
                self.handle_hello(user_id, protocol);
            }
        }
    }
}
