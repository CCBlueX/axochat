package net.ccbluex.axochat.protocol

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.contentOrNull
import net.ccbluex.axochat.moderation.Punishment
import net.ccbluex.axochat.moderation.PunishmentKind
import net.ccbluex.axochat.moderation.Report
import net.ccbluex.axochat.user.Author
import net.ccbluex.axochat.user.LegacyUser
import net.ccbluex.axochat.user.UserRef

public sealed interface Clientbound {

    @Serializable
    @SerialName("Hello")
    public data class Hello(val protocol: Int) : Clientbound

    @Serializable
    @SerialName("MojangInfo")
    public data class MojangInfo(val sessionHash: String) : Clientbound

    /**
     * Protocol v1; v2 sends [ChatMessage].
     */
    @Serializable
    @SerialName("Message")
    public data class Message(val authorInfo: LegacyUser, val content: String) : Clientbound

    /**
     * Protocol v1; v2 sends [ChatMessage].
     */
    @Serializable
    @SerialName("PrivateMessage")
    public data class PrivateMessage(val authorInfo: LegacyUser, val content: String) : Clientbound

    @Serializable
    @SerialName("UserCount")
    public data class UserCount(val connections: Int, val loggedIn: Int) : Clientbound

    @Serializable
    @SerialName("Success")
    public data class Success(val reason: SuccessReason) : Clientbound

    /**
     * Older servers sent `InvalidCharacter` as `{"InvalidCharacter": "x"}`, hence the raw [message].
     */
    @Serializable
    @SerialName("Error")
    public data class Error(val message: JsonElement = JsonNull, val detail: String? = null) : Clientbound {
        val code: ErrorCode
            get() = when (message) {
                is JsonNull -> ErrorCode.Internal
                is JsonPrimitive -> ErrorCode(message.content)
                is JsonObject -> ErrorCode(message.keys.firstOrNull() ?: ErrorCode.Internal.code)
                else -> ErrorCode.Internal
            }

        val details: String?
            get() = detail ?: ((message as? JsonObject)?.values?.firstOrNull() as? JsonPrimitive)?.contentOrNull
    }

    @Serializable
    @SerialName("Punished")
    public data class Punished(val kind: PunishmentKind, val reason: String, val expires: Long? = null) : Clientbound

    @Serializable
    @SerialName("Punishments")
    public data class Punishments(val user: UserRef, val punishments: List<Punishment> = emptyList()) : Clientbound

    @Serializable
    @SerialName("Welcome")
    public data class Welcome(val user: Author, val staff: Boolean) : Clientbound

    @Serializable
    @SerialName("Settings")
    /**
     * @param watchServers only sent to staff
     */
    public data class Settings(
        val allowMessages: Boolean,
        val serverChat: Boolean,
        val watchServers: Boolean? = null,
    ) : Clientbound

    @Serializable
    @SerialName("ChatMessage")
    public data class ChatMessage(
        val channel: String,
        val id: Long,
        val time: Long,
        val author: Author,
        val content: String,
    ) : Clientbound {
        val target: Channel? get() = Channel.parse(channel)
    }

    @Serializable
    @SerialName("Reports")
    public data class Reports(val reports: List<Report> = emptyList()) : Clientbound

    @Serializable
    @SerialName("ReportCreated")
    public data class ReportCreated(val report: Report) : Clientbound

    public data class Unknown(val name: String, val content: JsonElement?) : Clientbound
}
