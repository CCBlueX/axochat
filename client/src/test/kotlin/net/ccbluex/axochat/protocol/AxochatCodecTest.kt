package net.ccbluex.axochat.protocol

import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import net.ccbluex.axochat.user.LegacyUser
import net.ccbluex.axochat.user.UserKind
import java.util.UUID
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertNull

class AxochatCodecTest {

    private val notch = UUID.fromString("069a79f4-44e9-4726-a5be-fca90e38aaf5")

    @Test
    fun `packets without content leave it out`() {
        assertEquals("""{"m":"RequestMojangInfo"}""", AxochatCodec.encode(Serverbound.RequestMojangInfo))
        assertEquals("""{"m":"Hello","c":{"protocol":2}}""", AxochatCodec.encode(Serverbound.Hello(2)))
    }

    @Test
    fun `fields are snake case and absent optionals are left out`() {
        assertEquals(
            """{"m":"LoginAccount","c":{"token":"t","allow_messages":true}}""",
            AxochatCodec.encode(Serverbound.LoginAccount("t", true)),
        )
        assertEquals("""{"m":"Settings","c":{"server_chat":true}}""", AxochatCodec.encode(Serverbound.Settings(serverChat = true)))
        assertEquals(
            """{"m":"Settings","c":{"watch_servers":true}}""",
            AxochatCodec.encode(Serverbound.Settings(watchServers = true)),
        )
        assertEquals("""{"m":"Location","c":{"server":"mc.hypixel.net"}}""", AxochatCodec.encode(Serverbound.Location("mc.hypixel.net")))
        assertEquals("""{"m":"Location","c":{}}""", AxochatCodec.encode(Serverbound.Location(null)))
    }

    @Test
    fun `tokens never reach logs`() {
        assertEquals("LoginAccount(allowMessages=true)", Serverbound.LoginAccount("secret", true).toString())
    }

    @Test
    fun `legacy packets decode`() {
        val message = AxochatCodec.decode(
            """{"m":"Message","c":{"author_info":{"name":"Notch","uuid":"$notch"},"content":"Hello!"}}""",
        )
        assertEquals(Clientbound.Message(LegacyUser("Notch", notch), "Hello!"), message)
        assertEquals(Clientbound.Success(SuccessReason.Login), AxochatCodec.decode("""{"m":"Success","c":{"reason":"Login"}}"""))
    }

    @Test
    fun `error codes read in both shapes`() {
        val modern = assertIs<Clientbound.Error>(
            AxochatCodec.decode("""{"m":"Error","c":{"message":"InvalidCharacter","detail":"x"}}"""),
        )
        assertEquals(ErrorCode.InvalidCharacter, modern.code)
        assertEquals("x", modern.details)

        val legacy = assertIs<Clientbound.Error>(
            AxochatCodec.decode("""{"m":"Error","c":{"message":{"InvalidCharacter":"y"}}}"""),
        )
        assertEquals(ErrorCode.InvalidCharacter, legacy.code)
        assertEquals("y", legacy.details)
    }

    @Test
    fun `v2 messages decode with their author`() {
        val packet = assertIs<Clientbound.ChatMessage>(AxochatCodec.decode(
            """{"m":"ChatMessage","c":{"channel":"server","id":4182,"time":1791446400000,"author":{"id":"a",""" +
                """"kind":"account","name":"1zun4","uuid":"$notch","minecraft":{"uuid":"$notch","name":"Izuna"},""" +
                """"roles":[{"id":"premium","name":"Premium","staff":false}],"highlight":true},"content":"hi"}}""",
        ))
        assertEquals(Channel.Server, packet.target)
        assertEquals(UserKind.Account, packet.author.kind)
        assertEquals("Izuna", packet.author.minecraft?.name)
        assertEquals("Premium", packet.author.roles.single().name)
    }

    @Test
    fun `unknown values and fields from newer servers decode`() {
        val packet = assertIs<Clientbound.ChatMessage>(AxochatCodec.decode(
            """{"m":"ChatMessage","c":{"channel":"global","id":1,"time":0,"future":1,""" +
                """"author":{"id":"a","kind":"robot","name":"R","uuid":"$notch"},"content":"hi"}}""",
        ))
        assertEquals(UserKind("robot"), packet.author.kind)
        assertNull(packet.author.minecraft)
    }

    @Test
    fun `settings show watching only to staff`() {
        val player = assertIs<Clientbound.Settings>(
            AxochatCodec.decode("""{"m":"Settings","c":{"allow_messages":true,"server_chat":false}}"""),
        )
        assertNull(player.watchServers)
        val staff = assertIs<Clientbound.Settings>(
            AxochatCodec.decode("""{"m":"Settings","c":{"allow_messages":true,"server_chat":true,"watch_servers":true}}"""),
        )
        assertEquals(true, staff.watchServers)
    }

    @Test
    fun `channels parse`() {
        assertEquals(Channel.Watched("hypixel.net"), Channel.parse("server/hypixel.net"))
        assertEquals("server/hypixel.net", Channel.Watched("hypixel.net").toString())
        assertEquals(Channel.User("Notch"), Channel.parse("user/Notch"))
        assertNull(Channel.parse("lobby"))
    }

    @Test
    fun `unknown packets are kept, broken ones are not`() {
        assertEquals(
            Clientbound.Unknown("SomethingNew", buildJsonObject { put("x", JsonPrimitive(1)) }),
            AxochatCodec.decode("""{"m":"SomethingNew","c":{"x":1}}"""),
        )
        assertNull(AxochatCodec.decodeOrNull("""{"c":{}}"""))
        assertNull(AxochatCodec.decodeOrNull("""[]"""))
        assertNull(AxochatCodec.decodeOrNull("""{"m":"Hello","c":{"protocol":"two"}}"""))
    }
}
