use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Where a message went, as its receiver sees it.
#[derive(Serialize, Clone, Copy)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Channel {
    Global,
    /// The other side of a direct message.
    Direct { user: Uuid },
}

/// The channel of a serverbound `ChatMessage`.
#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(try_from = "RawChannel")]
pub enum Target {
    Global,
    /// An id or a name.
    Direct(String),
    Unknown(String),
}

#[derive(Deserialize)]
struct RawChannel {
    #[serde(rename = "type")]
    kind: String,
    user: Option<String>,
}

impl TryFrom<RawChannel> for Target {
    type Error = &'static str;

    fn try_from(raw: RawChannel) -> Result<Target, Self::Error> {
        match raw.kind.as_str() {
            "global" => Ok(Target::Global),
            "direct" => raw.user.map(Target::Direct).ok_or("a direct channel needs a user"),
            _ => Ok(Target::Unknown(raw.kind)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Channel, Target};
    use uuid::Uuid;

    fn target(json: &str) -> Result<Target, serde_json::Error> {
        serde_json::from_str(json)
    }

    #[test]
    fn wire_form() {
        assert_eq!(target(r#"{"type":"global"}"#).unwrap(), Target::Global);
        assert_eq!(target(r#"{"type":"direct","user":"Notch"}"#).unwrap(), Target::Direct("Notch".into()));
        assert_eq!(target(r#"{"type":"party","id":1}"#).unwrap(), Target::Unknown("party".into()));
        assert!(target(r#"{"type":"direct"}"#).is_err());
        assert!(target(r#""global""#).is_err());

        let user = Uuid::nil();
        assert_eq!(serde_json::to_string(&Channel::Global).unwrap(), r#"{"type":"global"}"#);
        assert_eq!(
            serde_json::to_string(&Channel::Direct { user }).unwrap(),
            r#"{"type":"direct","user":"00000000-0000-0000-0000-000000000000"}"#
        );
    }
}
