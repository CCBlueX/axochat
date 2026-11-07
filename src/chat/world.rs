use crate::ip::canonical;

use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use uuid::Uuid;

const DEFAULT_PORT: u16 = 25565;
/// LiquidProxy routes: `<secret>.<location>.liquidproxy.net`, formerly `<secret>.socks.liquidbounce.net`.
const ROUTE_DOMAINS: [&str; 2] = [".liquidproxy.net", ".liquidbounce.net"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Player {
    pub uuid: Uuid,
    pub name: String,
}

/// The Minecraft server a connection plays on.
#[derive(Debug, Clone)]
pub struct Location {
    /// What servers are matched by.
    pub key: String,
    /// Normalized for display.
    pub address: String,
}

#[derive(Debug, PartialEq)]
pub struct ServerAddress {
    /// Normalized for display.
    pub display: String,
    /// What servers are matched by.
    pub key: String,
    pub private: bool,
}

pub fn server_key(address: &str, client: IpAddr) -> Option<ServerAddress> {
    let address = address.trim().to_lowercase();
    let (host, port) = match address.strip_prefix('[') {
        Some(rest) => {
            let (host, rest) = rest.split_once(']')?;
            (host.to_owned(), rest.strip_prefix(':').map(str::parse).transpose().ok()?)
        }
        None => match address.rsplit_once(':') {
            Some((host, port)) if !host.contains(':') => (host.to_owned(), Some(port.parse().ok()?)),
            _ => (address.clone(), None),
        },
    };
    let host = host.trim_end_matches('.').to_owned();
    // a LiquidProxy route's hostname works as its owner's subscription; it is never kept or shared
    if host.is_empty() || ROUTE_DOMAINS.iter().any(|domain| host.ends_with(domain)) {
        return None;
    }
    let port = port.unwrap_or(DEFAULT_PORT);
    let bracketed = if host.contains(':') { format!("[{}]", host) } else { host.clone() };
    let display = if port == DEFAULT_PORT { bracketed.clone() } else { format!("{}:{}", bracketed, port) };

    let private = match host.parse::<IpAddr>().map(canonical) {
        Ok(IpAddr::V4(ip)) => ip.is_private() || ip.is_loopback() || ip.is_link_local() || ip.is_unspecified(),
        Ok(IpAddr::V6(ip)) => ip.is_loopback() || ip.is_unspecified() || (ip.segments()[0] & 0xfe00) == 0xfc00 || (ip.segments()[0] & 0xffc0) == 0xfe80,
        Err(_) => host == "localhost" || host.ends_with(".local") || !host.contains('.'),
    };

    let stripped = ["mc.", "play.", "www."]
        .iter()
        .find_map(|prefix| host.strip_prefix(prefix).filter(|rest| rest.contains('.')))
        .unwrap_or(bracketed.as_str());
    let key = format!("{}:{}", stripped, port);
    // a LAN address only means the same server behind the same public address
    let key = if private { format!("{}@{}", key, canonical(client)) } else { key };
    Some(ServerAddress { display, key, private })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> IpAddr {
        "203.0.113.7".parse().unwrap()
    }

    #[test]
    fn keys() {
        let key = |address: &str| {
            let server = server_key(address, client()).unwrap();
            (server.display, server.key, server.private)
        };
        assert_eq!(key("Hypixel.NET."), ("hypixel.net".into(), "hypixel.net:25565".into(), false));
        assert_eq!(key("mc.hypixel.net:25565"), ("mc.hypixel.net".into(), "hypixel.net:25565".into(), false));
        assert_eq!(key("play.example.org:25566").1, "example.org:25566");
        assert_eq!(key("mc.com").1, "mc.com:25565", "a prefix is only stripped when a domain remains");
        assert_eq!(key("[2001:db8::1]:25570"), ("[2001:db8::1]:25570".into(), "[2001:db8::1]:25570".into(), false));
        assert_eq!(key("192.168.1.20"), ("192.168.1.20".into(), "192.168.1.20:25565@203.0.113.7".into(), true));
        assert!(key("localhost:25570").2);
        assert!(server_key("Ab3xYz.EU-FRA.LiquidProxy.net.", client()).is_none(), "routes are never kept");
        assert!(server_key("Ab3xYz.socks.liquidbounce.net:25565", client()).is_none());
        assert!(server_key("", client()).is_none());
        assert!(server_key("host:notaport", client()).is_none());
    }
}
