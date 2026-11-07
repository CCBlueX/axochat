use serde::{Deserialize, Serialize};
use uuid::Uuid;

const MAX_TEXT: usize = 128;
const MAX_TIMEZONE: usize = 64;
const MAX_HORIZONTAL: u64 = 30_000_000;
const MAX_VERTICAL: u64 = 4096;
/// LiquidProxy routes: `<secret>.<location>.liquidproxy.net`, formerly `<secret>.socks.liquidbounce.net`.
const ROUTE_DOMAINS: [&str; 2] = [".liquidproxy.net", ".liquidbounce.net"];

/// Where a client is, as it last told.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Presence {
    pub state: State,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimension: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub player: Option<Player>,
    /// An IANA time zone id.
    pub timezone: String,
    /// The client's clock when sending, in milliseconds since the Unix epoch.
    pub time: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Menu,
    Singleplayer,
    Multiplayer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub x: i64,
    pub y: i64,
    pub z: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Player {
    pub uuid: Uuid,
    pub name: String,
}

impl Presence {
    /// Checks the limits and leaves out a LiquidProxy route, whose hostname works as its owner's subscription.
    pub fn validated(mut self) -> Result<Presence, String> {
        let texts = [self.server.as_deref(), self.dimension.as_deref(), self.player.as_ref().map(|player| player.name.as_str())];
        if texts.into_iter().flatten().any(|text| text.chars().count() > MAX_TEXT) {
            return Err(format!("presence texts are at most {} characters", MAX_TEXT));
        }
        if self.timezone.chars().count() > MAX_TIMEZONE {
            return Err(format!("the timezone is at most {} characters", MAX_TIMEZONE));
        }
        if let Some(Position { x, y, z }) = self.position {
            if x.unsigned_abs() > MAX_HORIZONTAL || z.unsigned_abs() > MAX_HORIZONTAL || y.unsigned_abs() > MAX_VERTICAL {
                return Err("the position is out of the world".into());
            }
        }
        if self.server.as_deref().is_some_and(is_route) {
            self.server = None;
        }
        Ok(self)
    }
}

fn is_route(address: &str) -> bool {
    let address = address.trim().to_lowercase();
    let host = match address.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') && port.parse::<u16>().is_ok() => host,
        _ => &address,
    };
    let host = host.trim_end_matches('.');
    ROUTE_DOMAINS.iter().any(|domain| host.ends_with(domain))
}

#[cfg(test)]
mod tests {
    use super::{Position, Presence, State};

    fn multiplayer(server: &str) -> Presence {
        let json = format!(
            r#"{{"state":"multiplayer","server":"{server}","dimension":"minecraft:overworld","position":{{"x":12,"y":64,"z":-300}},"timezone":"Europe/Berlin","time":1,"extra":true}}"#
        );
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn routes_are_left_out() {
        assert_eq!(multiplayer("mc.hypixel.net").validated().unwrap().server.as_deref(), Some("mc.hypixel.net"));
        for route in ["abc.de.liquidproxy.net", "ABC.socks.liquidbounce.net:25565", "abc.de.liquidproxy.net."] {
            let presence = multiplayer(route).validated().unwrap();
            assert_eq!((presence.state, presence.server), (State::Multiplayer, None));
        }
    }

    #[test]
    fn limits() {
        let mut presence = multiplayer("mc.hypixel.net");
        presence.position = Some(Position { x: 30_000_001, y: 0, z: 0 });
        assert!(presence.clone().validated().is_err());
        presence.position = Some(Position { x: 0, y: -4097, z: 0 });
        assert!(presence.clone().validated().is_err());
        presence.position = None;
        presence.timezone = "x".repeat(65);
        assert!(presence.validated().is_err());
    }
}
