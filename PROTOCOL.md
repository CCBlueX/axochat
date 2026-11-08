# The AxoChat protocol
The AxoChat protocol is based on websockets.
All packets are sent to the `/ws` endpoint.

<!-- markdown-toc start - Don't edit this section. Run M-x markdown-toc-refresh-toc -->
**Table of Contents**

- [The AxoChat protocol](#the-axochat-protocol)
- [Handshake](#handshake)
- [Minecraft proof](#minecraft-proof)
- [Channels](#channels)
- [Packets](#packets)
    - [Client](#client)
        - [ChatMessage](#chatmessage)
        - [Error](#error)
        - [Hello](#hello)
        - [Message](#message)
        - [MojangInfo](#mojanginfo)
        - [NewJWT](#newjwt)
        - [PrivateMessage](#privatemessage)
        - [Punished](#punished)
        - [Punishments](#punishments)
        - [ReportCreated](#reportcreated)
        - [Reports](#reports)
        - [Success](#success)
        - [UserCount](#usercount)
        - [Welcome](#welcome)
    - [Server](#server)
        - [BanUser](#banuser)
        - [ChatMessage](#chatmessage-1)
        - [Hello](#hello-1)
        - [LoginAccount](#loginaccount)
        - [LoginJWT](#loginjwt)
        - [LoginMojang](#loginmojang)
        - [Message](#message-1)
        - [Pardon](#pardon)
        - [PrivateMessage](#privatemessage-1)
        - [Punish](#punish)
        - [Report](#report)
        - [RequestJWT](#requestjwt)
        - [RequestMojangInfo](#requestmojanginfo)
        - [RequestPunishments](#requestpunishments)
        - [RequestReports](#requestreports)
        - [RequestUserCount](#requestusercount)
        - [ResolveReport](#resolvereport)
        - [UnbanUser](#unbanuser)

<!-- markdown-toc end -->

# Handshake
A client may send [Hello](#hello-1) before logging in to announce the newest protocol it speaks.
The server answers with [Hello](#hello) and the protocol both sides use.
Clients that announce protocol 2 or newer get the packets added since, such as [Welcome](#welcome),
and [Error](#error) packets with a `detail`.
They chat with [ChatMessage](#chatmessage-1) and get every message as [ChatMessage](#chatmessage).
Clients that send no `Hello` keep the original protocol, and servers that do not know `Hello` ignore it,
so a client that gets no answer within a few seconds uses the original protocol.

# Minecraft proof
After [LoginAccount](#loginaccount), a session can prove the Minecraft account it plays on,
with the same packets as a Mojang login:
[RequestMojangInfo](#requestmojanginfo), joining the session server with the hash from [MojangInfo](#mojanginfo),
then [LoginMojang](#loginmojang) (`allow_messages` is ignored).
The server answers with [Welcome](#welcome) (after a [handshake](#handshake)), now showing the account as `minecraft`,
and [Success](#success) with `Minecraft`; or with an [Error](#error), such as `LoginFailed` or `InvalidId`,
and the session stays logged in as before.
The proof lasts until the session ends.

# Channels
[ChatMessage](#chatmessage) packets name their channel as an object with a `type`:

- `{"type": "global"}` reaches everyone.
- `{"type": "direct", "user": "..."}` is a direct message between two LiquidBounce Accounts.
  When sending, `user` is the id or the name of an account that is online.
  The receiver gets the sender's id as `user`, and the sender's other connections after a handshake
  get the message with the receiver's id.

Clients without a handshake keep getting [Message](#message) and [PrivateMessage](#privatemessage),
also for messages sent as [ChatMessage](#chatmessage-1).
Clients should ignore messages in channels they do not know.

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

## Client
Client Packets are received by the client.

### ChatMessage
A message in a [channel](#channels), sent instead of [Message](#message) and [PrivateMessage](#privatemessage)
to clients after a [handshake](#handshake).

- `channel` is where the message went, as the receiver sees it.
- `id` identifies the message; ids grow with every message, also across server restarts.
- `time` is when the server got the message, in milliseconds since the Unix epoch.
- `author` is the sender, as `user` in [Welcome](#welcome).
- `content` is the message, see [Message](#message).

**Example**
```json
{
    "m": "ChatMessage",
    "c": {
        "channel": { "type": "direct", "user": "0190c3a2-5f1e-7c4b-9d2a-1b3c4d5e6f70" },
        "id": 4182,
        "time": 1791446400000,
        "author": {
            "id": "0190c3a2-5f1e-7c4b-9d2a-1b3c4d5e6f70",
            "kind": "account",
            "name": "Notch",
            "uuid": "069a79f4-44e9-4726-a5be-fca90e38aaf5",
            "roles": [],
            "highlight": false
        },
        "content": "Hello, User!"
    }
}
```

### Error
This packet may be sent at any time,
but is usually a response to a failed action of the client.

- `message` is the error code.
- `detail` is only sent after a [handshake](#handshake), and only for errors that have one,
  like the character of `InvalidCharacter`, the query of `UnknownUser`, the type of `UnknownChannel`
  or the reason of `InvalidPacket`.

Without a handshake, `InvalidCharacter` is sent as an object holding the character:
`"message": {"InvalidCharacter": "§"}`.

After a handshake, packets the server cannot decode are answered with `InvalidPacket`;
the connection stays open.

**Example**
```json
{
    "m": "Error",
    "c": {
        "message": "LoginFailed"
    }
}
```

### Hello
The answer to [Hello](#hello-1).

- `protocol` is the protocol the server uses for this connection, at most the one the client announced.

**Example**
```json
{
    "m": "Hello",
    "c": {
        "protocol": 2
    }
}
```

### Message
This packet will be sent to every authenticated client,
if another client successfully [sent a message](#message-1) to the server.
Clients after a [handshake](#handshake) get [ChatMessage](#chatmessage) instead.

- `author_info` is just the name and uuid of the user that sent the message.
  For a LiquidBounce Account, `uuid` is its linked Minecraft account, or the nil uuid if there is none.
- `content` is any message fitting the validation scheme of the server.
  Staff and users with a perk role may also use `§` color and formatting codes (except `§k`) and emoji.

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

### NewJWT
After the client sent the server a [RequestJWT](#requestjwt)
packet, the server will provide the client with json web token.
This token can be used in the [LoginJWT](#loginjwt) packet.

**Example**
```json
{
    "m": "NewJWT",
    "c": {
        "token": "VGhpcyBjb3VsZCBiZSBhIGpzb24gd2ViIHRva2VuLCBidXQgaXQgaXNuJ3QK"
    }
}
```

### PrivateMessage
The content of this packet will be sent to a authenticated client with `allow_messages` turned on,
if another client successfully [sent a private message](#privatemessage-1).
Clients after a [handshake](#handshake) get [ChatMessage](#chatmessage) instead.

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

### Punished
Sent to every logged in connection a [Punish](#punish) covers, after a [handshake](#handshake).
After a ban, the connection is also logged out with an `Error` of `Banned`.
While muted, messages are answered with `Muted` (`Banned` without a handshake).

- `kind` is `mute` or `ban`.
- `reason` is the reason staff gave.
- `expires` is when the punishment ends, in milliseconds since the Unix epoch, `null` if it is permanent.

**Example**
```json
{
    "m": "Punished",
    "c": {
        "kind": "mute",
        "reason": "Spam",
        "expires": 1762520460000
    }
}
```

### Punishments
The answer to [RequestPunishments](#requestpunishments).

- `user` is the `id`, `kind` and `name` of the user.
- `punishments` are the active punishments of the user:
  - `id` of the punishment.
  - `kind` is `mute` or `ban`.
  - `ip` is the address range it also covers, `null` if none.
  - `reason` is the reason staff gave.
  - `issued_by` is the id of the staff member, `null` if unknown.
  - `created` and `expires` are milliseconds since the Unix epoch; `expires` is `null` if it is permanent.

**Example**
```json
{
    "m": "Punishments",
    "c": {
        "user": {
            "id": "0190c3a2-5f1e-7c4b-9d2a-1b3c4d5e6f70",
            "kind": "account",
            "name": "Notch"
        },
        "punishments": [
            {
                "id": "019a5c1e-2b3d-7e4f-8a9b-0c1d2e3f4a5b",
                "kind": "ban",
                "ip": "203.0.113.7/32",
                "reason": "Spam",
                "issued_by": "0190c3a2-6a7b-7c8d-9e0f-a1b2c3d4e5f6",
                "created": 1762520400000,
                "expires": null
            }
        ]
    }
}
```

### ReportCreated
Sent to staff after a [handshake](#handshake) for every new [Report](#report).

- `report` is the report:
  - `id` of the report.
  - `reporter` and `target` are the `id`, `kind` and `name` of the users.
  - `channel` is where the reported message went, as the target saw it; for a direct message its `user` is the reporter.
    `null` without a message.
  - `message` and `content` are the id and the text of the reported message, `null` without one.
  - `reason` is the reason the reporter gave.
  - `time` is when it was reported, in milliseconds since the Unix epoch.

**Example**
```json
{
    "m": "ReportCreated",
    "c": {
        "report": {
            "id": "019a5c1e-2b3d-7e4f-8a9b-0c1d2e3f4a5b",
            "reporter": {
                "id": "0190c3a2-6a7b-7c8d-9e0f-a1b2c3d4e5f6",
                "kind": "account",
                "name": "jeb_"
            },
            "target": {
                "id": "0190c3a2-5f1e-7c4b-9d2a-1b3c4d5e6f70",
                "kind": "account",
                "name": "Notch"
            },
            "channel": { "type": "global" },
            "message": 4182,
            "content": "Hello, User!",
            "reason": "Spam",
            "time": 1791446400000
        }
    }
}
```

### Reports
The answer to [RequestReports](#requestreports).

- `reports` are the latest 50 unresolved reports, newest first, each as `report` in [ReportCreated](#reportcreated).

**Example**
```json
{
    "m": "Reports",
    "c": {
        "reports": []
    }
}
```

### Success
This packet is sent after either
[LoginMojang](#loginmojang), [LoginJWT](#loginjwt), [LoginAccount](#loginaccount),
[BanUser](#banuser), [UnbanUser](#unbanuser), [Punish](#punish), [Pardon](#pardon),
[Report](#report) or [ResolveReport](#resolvereport)
were processed successfully.

- `reason` is the reason for the success; it is one of the following possible
  values:
  - `Login`
  - `Ban`
  - `Unban`
  - `Minecraft` after a [Minecraft proof](#minecraft-proof)
  - `Punish`
  - `Pardon`
  - `Report`
  - `Resolve`

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
- `logged_in` is the amount of authenticated connections this server has open

**Example**
```json
{
    "m": "UserCount",
    "c": {
        "connections": 623,
        "logged_in": 531,
    }
}
```

### Welcome
Sent after a successful login, right before [Success](#success), to clients after a [handshake](#handshake).

- `user` is the logged in user:
  - `id` is the id of the user on this server.
  - `kind` is `account` for a LiquidBounce Account and `mojang` for a Minecraft account without one.
  - `name` is the name others see.
  - `uuid` is the Minecraft account shown for the user, the nil uuid if unknown.
  - `minecraft` is the `name` and `uuid` of the Minecraft account the session proved it plays on,
    left out without a [Minecraft proof](#minecraft-proof).
  - `roles` are the roles of the account, staff roles first: `id`, display `name` and whether it is a `staff` role.
  - `highlight` tells whether the user's messages stand out, for staff and perk roles.
- `staff` tells whether the user may moderate.

Clients should ignore fields and values they do not know.

**Example**
```json
{
    "m": "Welcome",
    "c": {
        "user": {
            "id": "0190c3a2-5f1e-7c4b-9d2a-1b3c4d5e6f70",
            "kind": "account",
            "name": "Notch",
            "uuid": "069a79f4-44e9-4726-a5be-fca90e38aaf5",
            "roles": [
                { "id": "premium", "name": "Premium", "staff": false }
            ]
        },
        "staff": false
    }
}
```

## Server
Server Packets are received by the server.

### BanUser
Staff can send this packet to ban other users from this chat.
Banned users are logged out and cannot log in again; they get an `Error` with `Banned`.

- `user` is the uuid of the user to ban.

**Example**
```json
{
    "m": "BanUser",
    "c": {
        "user": "069a79f4-44e9-4726-a5be-fca90e38aaf5"
    }
}
```

### ChatMessage
Sends `content` to a [channel](#channels), after a [handshake](#handshake).
It must fit the validation scheme described at [Message](#message).

- `{"type": "direct"}` needs a LiquidBounce Account, or the answer is `AccountRequired`.
  `PrivateMessageNotAccepted` means the receiver is not an online account, or none of its connections allows messages.
- An unknown `type` is answered with `UnknownChannel`.

**Example**
```json
{
    "m": "ChatMessage",
    "c": {
        "channel": { "type": "direct", "user": "Notch" },
        "content": "Hello, Notch!"
    }
}
```

A service session can send a message for a LiquidBounce Account, which then counts as that
account's message. `as` is the account id; only `{"type": "global"}` is allowed, other channels
are answered with `UnknownChannel`. The account's roles, mutes, bans and rate limit apply.
Errors about such a message carry the account id in `detail`: `NotPermitted` for other sessions,
`UnknownUser` for an unknown account.

```json
{
    "m": "ChatMessage",
    "c": {
        "channel": { "type": "global" },
        "content": "Hi!",
        "as": "<account id>"
    }
}
```

### Hello
Starts the [handshake](#handshake). Only answered before logging in.

- `protocol` is the newest protocol the client speaks.

**Example**
```json
{
    "m": "Hello",
    "c": {
        "protocol": 2
    }
}
```

### LoginAccount
Logs in with a LiquidBounce Account.
The server answers with [Welcome](#welcome) (after a [handshake](#handshake)) and [Success](#success),
or an [Error](#error): `LoginFailed` for an invalid token, `Banned` for a banned user.

- `token` is an OAuth access token of the LiquidBounce Service API.
- If `allow_messages` is true, other clients may send private messages
  to this client.

When the server has a service token configured (`SERVICE_TOKEN`), that token logs in as a staff
account named after `SERVICE_NAME` instead, a staff login for tooling.
IP bans do not apply to it.

**Example**
```json
{
    "m": "LoginAccount",
    "c": {
        "token": "VGhpcyBjb3VsZCBiZSBhbiBhY2Nlc3MgdG9rZW4K",
        "allow_messages": true
    }
}
```

### LoginJWT
To login using a json web token, the client has to send a `LoginJWT` packet.
it will send [Success](#success) if the login was successful.

- `token` can be retrieved by sending [RequestJWT](#requestjwt) on an already
- authenticated connection.
- If `allow_messages` is true, other clients may send private messages
  to this client.

**Example**
```json
{
    "m": "LoginJWT",
    "c": {
        "token": "VGhpcyBjb3VsZCBiZSBhIGpzb24gd2ViIHRva2VuLCBidXQgaXQgaXNuJ3QK",
        "allow_messages": true,
    }
}
```

### LoginMojang
After the client received a [MojangInfo](#mojanginfo) packet
and authenticating itself with mojang,
it has to send a `LoginMojang` packet to the server.
After the server receives a `LoginMojang` packet,
it will send [Success](#success) if the login was successful.
A Minecraft account linked to a LiquidBounce Account logs in as that account; this applies to [LoginJWT](#loginjwt) too.
Sessions logged in with [LoginAccount](#loginaccount) use it for a [Minecraft proof](#minecraft-proof) instead.

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

### Pardon
Staff can send this packet to lift punishments.
The answer is [Success](#success) with `Pardon`, or `NotBanned` if there was nothing to lift.

- `user` is the id or name of a user; every punishment of the user is lifted.
- `ip` is used without `user`: the punishments on exactly this address range are lifted.

**Example**
```json
{
    "m": "Pardon",
    "c": {
        "user": "Notch"
    }
}
```

### PrivateMessage
The `content` of this packet will be sent to the specified client
as [PrivateMessage](#privatemessage) if it fits the validation scheme.

- `receiver` is the name of the receiver, which may also be a Minecraft account.
  If nobody by that name accepts messages, the answer is `PrivateMessageNotAccepted`.

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

### Punish
Staff can send this packet to mute or ban a user or an address range.
Staff members cannot be punished. The answer is [Success](#success) with `Punish`,
`UnknownUser` if nobody matches `user`, or `NotPermitted`.
Users the punishment covers get [Punished](#punished).

- `user` is the id or name of a user, who may be offline.
  Names prefer LiquidBounce Accounts.
- `ip` is used without `user`: an address or range such as `203.0.113.7` or `2001:db8::/64`.
- `kind` is `mute` or `ban`. A ban also mutes and keeps the user from logging in.
- `duration` is the length in seconds; without it the punishment is permanent.
- `reason` is shown to the user, at most 256 characters.
- `include_ip` also punishes the last address of `user` (its /64 for IPv6). Defaults to `false`.

**Example**
```json
{
    "m": "Punish",
    "c": {
        "user": "Notch",
        "kind": "mute",
        "duration": 60,
        "reason": "Spam"
    }
}
```

### Report
Reports a user to staff, who get [ReportCreated](#reportcreated). Reports never punish by themselves.
The answer is [Success](#success) with `Report`, also when nobody matches `user` and when the same report
was already made; `NotPermitted` for reporting oneself; `InvalidId` if `message` is not a message of `user`
the reporter received; `RateLimited` after 10 reports within an hour.

- `user` is the id or name of the reported user.
- `message` is optional: the `id` of a [ChatMessage](#chatmessage) of the user that the reporter received,
  from this server run and among the last 1000 messages.
- `reason` is at most 256 characters.

**Example**
```json
{
    "m": "Report",
    "c": {
        "user": "Notch",
        "message": 4182,
        "reason": "Spam"
    }
}
```

### RequestJWT
To login using [LoginJWT](#loginjwt), a client needs to own a json web token.
This token can be retrieved by sending `RequestJWT` as an already authenticated
client to the server.
The server will send a [NewJWT](#newjwt) packet to the client.
The token stands for a Minecraft account, so after [LoginAccount](#loginaccount) the server sends `NotSupported` instead.

This packet has no body.

**Example**
```json
{
    "m": "RequestJWT"
}
```

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

### RequestPunishments
Staff can send this packet to see the active punishments of a user.
The answer is [Punishments](#punishments), or `UnknownUser`.

- `user` is the id or name of a user.

**Example**
```json
{
    "m": "RequestPunishments",
    "c": {
        "user": "Notch"
    }
}
```

### RequestReports
Staff can send this packet to get [Reports](#reports).
This packet has no body.

**Example**
```json
{
    "m": "RequestReports"
}
```

### RequestUserCount
After receiving this packet, the server will then send a [UserCount](#usercount)
packet to the client.

This packet has no body.

**Example**
```json
{
    "m": "RequestUserCount"
}
```

### ResolveReport
Staff can send this packet to take a report off [Reports](#reports).
The answer is [Success](#success) with `Resolve`.

- `id` of the report.

**Example**
```json
{
    "m": "ResolveReport",
    "c": {
        "id": "019a5c1e-2b3d-7e4f-8a9b-0c1d2e3f4a5b"
    }
}
```

### UnbanUser
Staff can send this packet to lift every ban and mute of a user.

- `user` is the uuid of the user to unban.

**Example**
```json
{
    "m": "UnbanUser",
    "c": {
        "user": "069a79f4-44e9-4726-a5be-fca90e38aaf5"
    }
}
```
