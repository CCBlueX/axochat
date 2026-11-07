mod account;
mod ban;
mod count;
mod hello;
mod jwt;
mod login;
mod message;
mod mojang;
mod presence;
mod punish;
mod report;

use super::{ChatServer, ClientPacket, ServerPacket, ServerPacketId};

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
            ServerPacket::RequestJWT => {
                self.handle_request_jwt(user_id);
            }
            ServerPacket::LoginJWT {
                token,
                allow_messages,
            } => {
                self.handle_login_jwt(user_id, &token, allow_messages, ctx);
            }
            ServerPacket::LoginAccount { token, allow_messages } => {
                self.login_account(user_id, token, allow_messages, ctx);
            }
            ServerPacket::Hello { protocol } => {
                self.handle_hello(user_id, protocol);
            }
            ServerPacket::Message { content } => self.handle_message(user_id, content),
            ServerPacket::PrivateMessage { receiver, content } => {
                self.handle_private_message(user_id, receiver, content);
            }
            ServerPacket::ChatMessage { channel, content, account: None } => {
                self.handle_chat_message(user_id, channel, content);
            }
            ServerPacket::ChatMessage { channel, content, account: Some(account) } => {
                self.handle_message_for(user_id, channel, content, account, ctx);
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
                let target = match (user, ip) {
                    (Some(query), _) => punish::Target::User { query, include_ip },
                    (None, Some(ip)) => punish::Target::Ip(ip),
                    (None, None) => punish::Target::Nobody,
                };
                self.handle_punish(user_id, target, kind, duration, reason, ctx);
            }
            ServerPacket::Pardon { user, ip } => {
                self.handle_pardon(user_id, user, ip, ctx);
            }
            ServerPacket::RequestPunishments { user } => {
                self.handle_request_punishments(user_id, user, ctx);
            }
            ServerPacket::Report { user, message, reason } => {
                self.handle_report(user_id, user, message, reason, ctx);
            }
            ServerPacket::RequestReports => self.handle_request_reports(user_id, ctx),
            ServerPacket::ResolveReport { id } => self.handle_resolve_report(user_id, id),
            ServerPacket::Presence(presence) => self.handle_presence(user_id, presence),
            ServerPacket::RequestPresence { user } => {
                self.handle_request_presence(user_id, user, ctx);
            }
            ServerPacket::RequestUserCount => {
                self.send_user_count(user_id);
            }
        }
    }
}
