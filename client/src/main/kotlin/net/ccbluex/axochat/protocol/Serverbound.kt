@file:UseSerializers(UuidSerializer::class)

package net.ccbluex.axochat.protocol

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.UseSerializers
import net.ccbluex.axochat.user.Player
import java.util.UUID

public sealed interface Serverbound {

    @Serializable
    @SerialName("Hello")
    public data class Hello(val protocol: Int) : Serverbound

    @Serializable
    @SerialName("RequestMojangInfo")
    public data object RequestMojangInfo : Serverbound

    /**
     * Sent after joining the session server with the hash from [Clientbound.MojangInfo].
     * After an account login it proves the Minecraft account instead of logging in.
     */
    @Serializable
    @SerialName("LoginMojang")
    public data class LoginMojang(val name: String, val uuid: UUID, val allowMessages: Boolean) : Serverbound

    @Serializable
    @SerialName("LoginAccount")
    public data class LoginAccount(val token: String, val allowMessages: Boolean) : Serverbound {
        override fun toString(): String = "LoginAccount(allowMessages=$allowMessages)"
    }

    @Serializable
    @SerialName("LoginJWT")
    public data class LoginJWT(val token: String, val allowMessages: Boolean) : Serverbound {
        override fun toString(): String = "LoginJWT(allowMessages=$allowMessages)"
    }

    @Serializable
    @SerialName("RequestJWT")
    public data object RequestJWT : Serverbound

    @Serializable
    @SerialName("Message")
    public data class Message(val content: String) : Serverbound

    @Serializable
    @SerialName("PrivateMessage")
    public data class PrivateMessage(val receiver: String, val content: String) : Serverbound

    /**
     * After [Hello]: [Channel.Direct] needs a LiquidBounce Account on both sides.
     */
    @Serializable
    @SerialName("ChatMessage")
    public data class ChatMessage(
        val channel: Channel,
        val content: String,
        /**
         * Service sessions only: the LiquidBounce Account id the message is sent for.
         */
        @SerialName("as")
        val account: String? = null,
    ) : Serverbound

    /**
     * Staff only.
     */
    @Serializable
    @SerialName("BanUser")
    public data class BanUser(val user: UUID) : Serverbound

    /**
     * Staff only.
     */
    @Serializable
    @SerialName("UnbanUser")
    public data class UnbanUser(val user: UUID) : Serverbound

    /**
     * Staff only. Punishes [user] (an id or a name, also offline) or else the [ip] range.
     *
     * @param duration in seconds, permanent without
     * @param includeIp also covers the last address of [user]
     */
    @Serializable
    @SerialName("Punish")
    public data class Punish(
        val user: String? = null,
        val ip: String? = null,
        val kind: PunishmentKind,
        val duration: Long? = null,
        val reason: String,
        val includeIp: Boolean = false,
    ) : Serverbound

    /**
     * Staff only. Lifts every punishment of [user], or the ones on exactly the [ip] range.
     */
    @Serializable
    @SerialName("Pardon")
    public data class Pardon(val user: String? = null, val ip: String? = null) : Serverbound

    /**
     * Staff only. Answered with [Clientbound.Punishments].
     */
    @Serializable
    @SerialName("RequestPunishments")
    public data class RequestPunishments(val user: String) : Serverbound

    /**
     * Reports [user] (an id or a name) to staff, answered with [SuccessReason.Report].
     *
     * @param message the id of a message of [user] the reporter received
     */
    @Serializable
    @SerialName("Report")
    public data class Report(val user: String, val message: Long? = null, val reason: String) : Serverbound

    /**
     * Staff only. Answered with [Clientbound.Reports].
     */
    @Serializable
    @SerialName("RequestReports")
    public data object RequestReports : Serverbound

    /**
     * Staff only.
     */
    @Serializable
    @SerialName("ResolveReport")
    public data class ResolveReport(val id: String) : Serverbound

    /**
     * Where the client is. The server keeps the latest one per connection, at most one per second.
     */
    @Serializable
    @SerialName("Presence")
    public data class Presence(
        val state: PresenceState,
        /**
         * Multiplayer only, as typed.
         */
        val server: String? = null,
        val dimension: String? = null,
        val position: BlockPosition? = null,
        val player: Player? = null,
        /**
         * An IANA time zone id.
         */
        val timezone: String,
        /**
         * The client's clock when sending, in milliseconds since the Unix epoch.
         */
        val time: Long,
    ) : Serverbound

    /**
     * Staff only. Answered with [Clientbound.Presence].
     */
    @Serializable
    @SerialName("RequestPresence")
    public data class RequestPresence(val user: String) : Serverbound

    /**
     * Staff only.
     */
    @Serializable
    @SerialName("RequestUserCount")
    public data object RequestUserCount : Serverbound
}

@Serializable
@JvmInline
public value class PresenceState(public val name: String) {
    override fun toString(): String = name

    public companion object {
        public val Menu: PresenceState = PresenceState("menu")
        public val Singleplayer: PresenceState = PresenceState("singleplayer")
        public val Multiplayer: PresenceState = PresenceState("multiplayer")
    }
}

@Serializable
public data class BlockPosition(val x: Int, val y: Int, val z: Int)
