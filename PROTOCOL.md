# The AxoChat protocol
The AxoChat protocol is based on websockets.
All packets are sent to the `/ws` endpoint.

There are two versions. [v1](#protocol-v1) is the original protocol, which
every client speaks. [v2](#protocol-v2) extends it with LiquidBounce Account
login, roles, channels and moderation.

# Packets
Packets are sent in websocket `text` messages encoded as JSON objects.
They all have a structure like that, with `c` being optional:
```json
{
    "m": "Name",
    "c": {
        "...": "...",
        "...": false
    }
}
```

Not every packet has a body:
```json
{
    "m": "Name"
}
```

# Protocol v1

## Client
Client Packets are received by the client.

### Error
This packet may be sent at any time,
but is usually a response to a failed action of the client.
`message` is always the name of the error.

**Example**
```json
{
    "m": "Error",
    "c": {
        "message": "LoginFailed"
    }
}
```

### Message
This packet will be sent to every logged in client,
if another client successfully [sent a message](#message-1) to the server.

- `author_info` is just the name and uuid of the user that sent the message.
  For a LiquidBounce Account, `uuid` is its linked Minecraft account, or the
  nil UUID.
- `content` is any message fitting the validation scheme of the server.

**Example**
```json
{
    "m": "Message",
    "c": {
        "author_info": {
            "name": "Notch",
            "uuid": "069a79f4-44e9-4726-a5be-fca90e38aaf5"
        },
        "content": "Hello, World!"
    }
}
```

### MojangInfo
After the client sent the server a [RequestMojangInfo](#requestmojanginfo)
packet, the server will provide the client with a `session_hash`.
A session hash is synonymous with a *server id* in the context of
[authentication with mojang](https://wiki.vg/Protocol_Encryption#Authentication).
The client has to send a [LoginMojang](#loginmojang) packet to the server
after authenticating itself with mojang.

**Example**
```json
{
    "m": "MojangInfo",
    "c": {
        "session_hash": "88e16a1019277b15d58faf0541e11910eb756f6"
    }
}
```

### PrivateMessage
The content of this packet will be sent to a logged in client with `allow_messages` turned on,
if another client successfully [sent a private message](#privatemessage-1).

- `author_info` is just the name and uuid of the user that sent the message.
- `content` is any message fitting the validation scheme of the server.

**Example**
```json
{
    "m": "PrivateMessage",
    "c": {
        "author_info": {
            "name": "Notch",
            "uuid": "069a79f4-44e9-4726-a5be-fca90e38aaf5"
        },
        "content": "Hello, User!"
    }
}
```

### Success
This packet is sent after either
[LoginMojang](#loginmojang), [BanUser](#banuser) or [UnbanUser](#unbanuser)
were processed successfully.

- `reason` is the reason for the success; it is one of the following possible
  values:
  - `Login`
  - `Ban`
  - `Unban`

**Example**
```json
{
    "m": "Success",
    "c": {
        "reason": "Login"
    }
}
```

### UserCount
This packet is sent after [RequestUserCount](#requestusercount) was received.

- `connections` is the amount of connections this server has open
- `logged_in` is the amount of users logged in

**Example**
```json
{
    "m": "UserCount",
    "c": {
        "connections": 623,
        "logged_in": 531
    }
}
```

## Server
Server Packets are received by the server.

### BanUser
Staff can send this packet to mute users.

- `user` is the Minecraft uuid of the user to mute. It mutes that Minecraft
  account and any LiquidBounce Account it is linked to.

**Example**
```json
{
    "m": "BanUser",
    "c": {
        "user": "069a79f4-44e9-4726-a5be-fca90e38aaf5"
    }
}
```

### LoginJWT
No longer supported and answered with the error `NotSupported`; log in with
[LoginAccount](#loginaccount) instead.

### LoginMojang
After the client received a [MojangInfo](#mojanginfo) packet
and authenticating itself with mojang,
it has to send a `LoginMojang` packet to the server.
After the server receives a `LoginMojang` packet,
it will send [Success](#success) if the login was successful,
or the error `Banned` if the user or its address is banned.

- `name` needs to be associated with the uuid.
- `uuid` is not guaranteed to be hyphenated.
- If `allow_messages` is true, other clients may send private messages
  to this client.

**Example**
```json
{
    "m": "LoginMojang",
    "c": {
        "name": "Notch",
        "uuid": "069a79f4-44e9-4726-a5be-fca90e38aaf5",
        "allow_messages": true
   }
}
```

### Message
The `content` of this packet will be sent to every client
as [Message](#message) if it fits the validation scheme.

**Example**
```json
{
    "m": "Message",
    "c": {
        "content": "Hello, World!"
    }
}
```

### PrivateMessage
The `content` of this packet will be sent to the specified client
as [PrivateMessage](#privatemessage) if it fits the validation scheme.

- `receiver` is the name of the receiver.

**Example**
```json
{
    "m": "PrivateMessage",
    "c": {
        "content": "Hello, Notch!",
        "receiver": "Notch"
    }
}
```

### RequestJWT
No longer supported and answered with the error `NotSupported`.

### RequestMojangInfo
To login via mojang, the client has to send a `RequestMojangInfo` packet.
The server will then send a [MojangInfo](#mojanginfo) to the client.

This packet has no body.

**Example**
```json
{
    "m": "RequestMojangInfo"
}
```

### RequestUserCount
Staff can send this packet to receive a [UserCount](#usercount).

This packet has no body.

**Example**
```json
{
    "m": "RequestUserCount"
}
```

### UnbanUser
Staff can send this packet to lift the mutes of a Minecraft uuid.

- `user` is the uuid of the user to unmute.

**Example**
```json
{
    "m": "UnbanUser",
    "c": {
        "user": "069a79f4-44e9-4726-a5be-fca90e38aaf5"
    }
}
```

# Protocol v2

A client speaks v2 by sending [Hello](#hello-1) before it logs in. Sessions
without it stay on v1 and never receive a v2 packet or field. Everything from
v1 keeps working on v2, but a v2 client receives [ChatMessage](#chatmessage)
instead of `Message` and `PrivateMessage`.

v2 clients ignore packets and fields they do not know. Optional fields are
sent as `null`, except `watch_servers` in [Settings](#settings) and the
`detail` of [Error](#error), which are left out when absent.

```mermaid
sequenceDiagram
    Client->>Server: Hello
    Server->>Client: Hello
    alt LiquidBounce Account
        Client->>Server: LoginAccount
    else Minecraft account
        Client->>Server: RequestMojangInfo
        Server->>Client: MojangInfo
        Client->>Server: LoginMojang
    end
    Server->>Client: Welcome, Settings
    Server->>Client: Success (Login)
    opt LiquidBounce Account playing on a Minecraft account
        Client->>Server: RequestMojangInfo
        Server->>Client: MojangInfo
        Client->>Server: LoginMojang
        Server->>Client: Success (Minecraft)
    end
```

The server pings every 30 seconds and closes a v2 connection that has not
answered for 90 seconds.

One address may keep 16 connections that are not logged in; more are closed.
Logins from one address get a burst of 10, then one every two seconds; later
ones wait their turn for up to a minute, beyond that the answer is
`RateLimited`. Proving a Minecraft session counts as a login.

## Identities
A user is either a LiquidBounce Account (`account`) or a Minecraft account
verified through the session server (`mojang`). A Minecraft login whose UUID is
linked to a LiquidBounce Account logs in as that account.

Direct messages are between LiquidBounce Accounts on both sides; a Minecraft
account gets `AccountRequired`. Everyone chats in `global` and `server` and
reports.

After logging in, an account session can prove the Minecraft account it plays
on with `RequestMojangInfo` and `LoginMojang`, answered by `Success`
(`Minecraft`). Readers of its messages then see that account as `minecraft`.

Every user has a public `id`. Wherever a packet takes a `user`, it accepts an
`id` or the name of a LiquidBounce Account. A Minecraft account is named after
itself, so only staff find it by name; everyone else uses the `id` its messages
carry. Nobody can tell whether a Minecraft account uses LiquidChat: direct
messages to it are not accepted and reports succeed.

Names are not unique: they find online users first, then everyone else, the
user seen first winning. Account
names drop formatting codes and invisible or direction-changing characters, turn
whitespace into `_` and are at most 32 characters long.

### UserRef
- `uuid` is the Minecraft UUID used for the head, the nil UUID if unknown.
- `minecraft` is the Minecraft account an online account proved it plays on.

```json
{
    "id": "0192f0e4-6b1e-7c4d-9a51-2f8f3c1d7a10",
    "kind": "account",
    "name": "1zun4",
    "uuid": "069a79f4-44e9-4726-a5be-fca90e38aaf5",
    "minecraft": { "uuid": "069a79f4-44e9-4726-a5be-fca90e38aaf5", "name": "Izuna" }
}
```

### Author
A [UserRef](#userref) with the user's roles, staff roles first.
`highlight` is set for donators and staff.

```json
{
    "id": "0192f0e4-6b1e-7c4d-9a51-2f8f3c1d7a10",
    "kind": "account",
    "name": "1zun4",
    "uuid": "069a79f4-44e9-4726-a5be-fca90e38aaf5",
    "minecraft": { "uuid": "069a79f4-44e9-4726-a5be-fca90e38aaf5", "name": "Izuna" },
    "roles": [{ "id": "premium", "name": "Premium", "staff": false }],
    "highlight": true
}
```

### Channels
| Channel | Recipients |
|---|---|
| `global` | everyone logged in |
| `server` | users on the same Minecraft server with `server_chat` on; the sender too |
| `server/<address>` | receive only: `server` messages from every Minecraft server, for staff with `watch_servers` on |
| `user/<user>` | a direct message; received as `user/<sender id>`, echoed to the sender as `user/<receiver id>` |

Donators and staff may use `§` color and format codes (except `§k`) and emoji.

### Errors
v2 adds a `detail` to [Error](#error) where it helps, and these codes:
`UnknownUser`, `UnknownChannel`, `Muted`, `AccountRequired`, and `InvalidPacket` for packets the server cannot decode.

```json
{
    "m": "Error",
    "c": {
        "message": "InvalidCharacter",
        "detail": "€"
    }
}
```

## Client

### Hello
Answers [Hello](#hello-1).

```json
{ "m": "Hello", "c": { "protocol": 2 } }
```

### Welcome
Sent before `Success` on login.

```json
{
    "m": "Welcome",
    "c": {
        "user": { "id": "...", "kind": "mojang", "name": "Notch", "uuid": "...", "roles": [], "highlight": false },
        "staff": false
    }
}
```

### Settings
Sent on login and after [Settings](#settings-1). `watch_servers` is only sent
to staff.

```json
{
    "m": "Settings",
    "c": {
        "allow_messages": true,
        "server_chat": false,
        "watch_servers": false
    }
}
```

### ChatMessage
- `id` identifies the message for [Report](#report).
- `time` is in milliseconds since the Unix epoch.

```json
{
    "m": "ChatMessage",
    "c": {
        "channel": "server",
        "id": 4182,
        "time": 1791446400000,
        "author": { "id": "...", "kind": "account", "name": "Izuna", "uuid": "...", "roles": [], "highlight": false },
        "content": "Hello, server!"
    }
}
```

### Punished
The receiver was muted or banned. `expires` is `null` for permanent ones.

```json
{ "m": "Punished", "c": { "kind": "mute", "reason": "Spam", "expires": 1791450000000 } }
```

### Punishments
Staff only, answers [RequestPunishments](#requestpunishments).

```json
{
    "m": "Punishments",
    "c": {
        "user": { "id": "...", "kind": "mojang", "name": "Spammer", "uuid": "..." },
        "punishments": [
            { "id": "0192f0e7-...", "kind": "mute", "ip": null, "reason": "Spam", "issued_by": null, "created": 1791446400000, "expires": 1791450000000 }
        ]
    }
}
```

### Reports
Staff only, answers [RequestReports](#requestreports) with the latest 50
unresolved reports. `ReportCreated` carries a single new `report`.

```json
{
    "m": "Reports",
    "c": {
        "reports": [
            {
                "id": "0192f0e8-...",
                "reporter": { "id": "...", "kind": "account", "name": "Izuna", "uuid": "..." },
                "target": { "id": "...", "kind": "mojang", "name": "Spammer", "uuid": "..." },
                "channel": "global",
                "message": 4182,
                "content": "buy cheap coins",
                "reason": "Spam",
                "time": 1791446400000
            }
        ]
    }
}
```

`Success` gains the reasons `Report`, `Punish`, `Pardon`, `Resolve` and `Minecraft`.

## Server

### Hello
Sent before logging in. Ignored afterwards.

```json
{ "m": "Hello", "c": { "protocol": 2 } }
```

### LoginAccount
Logs in with a LiquidBounce Account access token.

When the server has a service token configured (`SERVICE_TOKEN`), that token
logs in as a staff account named after `SERVICE_NAME` instead, for staff
tooling. Login rate limits and IP bans do not apply to it.

```json
{ "m": "LoginAccount", "c": { "token": "eyJhbGciOi...", "allow_messages": true } }
```

### Settings
Every field is optional.

- `server_chat` joins the `server` channel. It shows the user to everyone
  else on that Minecraft server who uses it.
- `watch_servers` is staff only, otherwise the answer is `NotPermitted`. It
  receives the `server` channel of every Minecraft server as
  `server/<address>`, in place of the session's own `server` channel.

```json
{ "m": "Settings", "c": { "server_chat": true } }
```

### ChatMessage
```json
{ "m": "ChatMessage", "c": { "channel": "user/Notch", "content": "Hi!" } }
```

### Location
The Minecraft server the client plays on, sent when joining or leaving a
server. `"server": null` means singleplayer or no world. At most 5 in a burst,
then 2 per second; excess is dropped.

`server` is the address as typed. LiquidProxy routes (`*.liquidproxy.net`,
`*.liquidbounce.net`) work as their owner's subscription: clients must not send
them, and the server drops them.

```json
{ "m": "Location", "c": { "server": "mc.hypixel.net" } }
```

### Report
`message` is the [ChatMessage](#chatmessage) id, if any; it must be the
target's and readable by the reporter, or the answer is `InvalidId`.

A user files at most 10 reports per hour, beyond that the answer is
`RateLimited`; repeating a report changes nothing. Reports never punish on
their own; staff review them.

```json
{ "m": "Report", "c": { "user": "Spammer", "message": 4182, "reason": "Spam" } }
```

### Punish
Staff only. `user` or `ip` (address or CIDR). `duration` is in seconds; absent
means permanent. `include_ip` also covers the user's last address (/32, or /64
for IPv6). A `mute` stops sending, a `ban` refuses logins.

```json
{ "m": "Punish", "c": { "user": "Spammer", "kind": "ban", "duration": 86400, "reason": "Spam", "include_ip": true } }
```

### Pardon
Staff only. Revokes the active punishments of a `user` or an `ip`.

```json
{ "m": "Pardon", "c": { "user": "Spammer" } }
```

### RequestPunishments
Staff only.

```json
{ "m": "RequestPunishments", "c": { "user": "Spammer" } }
```

### RequestReports
Staff only.

```json
{ "m": "RequestReports" }
```

### ResolveReport
Staff only.

```json
{ "m": "ResolveReport", "c": { "id": "0192f0e8-..." } }
```
