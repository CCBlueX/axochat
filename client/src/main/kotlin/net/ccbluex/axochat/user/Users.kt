@file:UseSerializers(UuidSerializer::class)

package net.ccbluex.axochat.user

import kotlinx.serialization.Serializable
import kotlinx.serialization.UseSerializers
import net.ccbluex.axochat.protocol.Channel
import net.ccbluex.axochat.protocol.PunishmentKind
import net.ccbluex.axochat.protocol.UuidSerializer
import java.util.UUID

/**
 * [Mojang] is a Minecraft account without a LiquidBounce Account.
 */
@Serializable
@JvmInline
public value class UserKind(public val name: String) {
    override fun toString(): String = name

    public companion object {
        public val Account: UserKind = UserKind("account")
        public val Mojang: UserKind = UserKind("mojang")
    }
}

@Serializable
public data class Player(val name: String, val uuid: UUID)

@Serializable
public data class Role(val id: String, val name: String, val staff: Boolean)

/**
 * @param uuid the Minecraft account shown as their head, nil if unknown
 * @param minecraft the Minecraft account an account session proved it plays on
 * @param roles staff roles first
 * @param highlight staff and perk roles stand out
 */
@Serializable
public data class Author(
    val id: String,
    val kind: UserKind,
    val name: String,
    val uuid: UUID,
    val minecraft: Player? = null,
    val roles: List<Role> = emptyList(),
    val highlight: Boolean = false,
) {
    val isAccount: Boolean get() = kind == UserKind.Account
}

/**
 * The author of [net.ccbluex.axochat.protocol.Clientbound.Message] and
 * [net.ccbluex.axochat.protocol.Clientbound.PrivateMessage].
 */
@Serializable
public data class LegacyUser(val name: String, val uuid: UUID)

@Serializable
public data class UserRef(val id: String, val kind: UserKind, val name: String)

/**
 * @param ip the range it also covers
 * @param issuedBy the id of the staff member
 * @param created milliseconds since the Unix epoch
 * @param expires milliseconds since the Unix epoch, null if permanent
 */
@Serializable
public data class Punishment(
    val id: String,
    val kind: PunishmentKind,
    val ip: String? = null,
    val reason: String,
    val issuedBy: String? = null,
    val created: Long,
    val expires: Long? = null,
)

/**
 * @param channel where the reported message went, as the target saw it
 * @param message the id of the reported message
 * @param content the reported message
 * @param time milliseconds since the Unix epoch
 */
@Serializable
public data class ReportView(
    val id: String,
    val reporter: UserRef,
    val target: UserRef,
    val channel: Channel? = null,
    val message: Long? = null,
    val content: String? = null,
    val reason: String,
    val time: Long,
)
