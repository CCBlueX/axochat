package net.ccbluex.axochat.protocol

public sealed interface Channel {

    public data object Global : Channel {
        override fun toString(): String = "global"
    }

    /**
     * Users on the same Minecraft server who turned server chat on.
     */
    public data object Server : Channel {
        override fun toString(): String = "server"
    }

    /**
     * Server chat of [address] as staff watching every server receive it; cannot be sent to.
     */
    public data class Watched(val address: String) : Channel {
        override fun toString(): String = "server/$address"
    }

    /**
     * [user] is an id, or a name when sending.
     */
    public data class User(val user: String) : Channel {
        override fun toString(): String = "user/$user"
    }

    public companion object {
        public fun parse(channel: String): Channel? = when {
            channel == "global" -> Global
            channel == "server" -> Server
            channel.startsWith("server/") -> Watched(channel.removePrefix("server/"))
            channel.startsWith("user/") -> User(channel.removePrefix("user/"))
            else -> null
        }
    }
}
