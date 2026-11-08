package net.ccbluex.axochat.protocol

import net.ccbluex.axochat.user.UserKind
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertNull

class AxochatCodecTest {

    @Test
    fun `tokens never reach logs`() {
        assertFalse("secret" in Serverbound.LoginAccount("secret", true).toString())
        assertFalse("secret" in Serverbound.LoginJWT("secret", true).toString())
        assertFalse("secret" in Clientbound.NewJWT("secret").toString())
    }

    @Test
    fun `errors without hello come as objects`() {
        val error = assertIs<Clientbound.Error>(
            AxochatCodec.decode("""{"m":"Error","c":{"message":{"InvalidCharacter":"y"}}}"""),
        )
        assertEquals(ErrorCode.InvalidCharacter, error.code)
        assertEquals("y", error.details)

        val detailed = assertIs<Clientbound.Error>(
            AxochatCodec.decode("""{"m":"Error","c":{"message":"InvalidCharacter","detail":"y"}}"""),
        )
        assertEquals(ErrorCode.InvalidCharacter, detailed.code)
        assertEquals("y", detailed.details)
    }

    @Test
    fun `unknown packets, values and fields decode`() {
        val welcome = assertIs<Clientbound.Welcome>(AxochatCodec.decode(
            """{"m":"Welcome","c":{"staff":false,"future":1,""" +
                """"user":{"id":"a","kind":"robot","name":"R","uuid":"069a79f4-44e9-4726-a5be-fca90e38aaf5"}}}""",
        ))
        assertEquals(UserKind("robot"), welcome.user.kind)
        assertIs<Clientbound.Unknown>(AxochatCodec.decode("""{"m":"Later","c":{"x":1}}"""))
    }

    @Test
    fun `channels travel as objects`() {
        assertEquals(
            """{"m":"ChatMessage","c":{"channel":{"type":"direct","user":"Notch"},"content":"hi"}}""",
            AxochatCodec.encode(Serverbound.ChatMessage(Channel.Direct("Notch"), "hi")),
        )
        assertEquals(
            """{"m":"ChatMessage","c":{"channel":{"type":"global"},"content":"hi"}}""",
            AxochatCodec.encode(Serverbound.ChatMessage(Channel.Global, "hi")),
        )
        assertEquals(
            """{"m":"ChatMessage","c":{"channel":{"type":"global"},"content":"hi","as":"abc"}}""",
            AxochatCodec.encode(Serverbound.ChatMessage(Channel.Global, "hi", account = "abc")),
        )

        fun packet(channel: String) =
            """{"m":"ChatMessage","c":{"channel":$channel,"id":7,"time":1,"content":"hi",""" +
                """"author":{"id":"a","kind":"account","name":"A","uuid":"069a79f4-44e9-4726-a5be-fca90e38aaf5"}}}"""
        fun received(channel: String) = assertIs<Clientbound.ChatMessage>(AxochatCodec.decode(packet(channel))).channel
        assertEquals(Channel.Global, received("""{"type":"global"}"""))
        assertEquals(Channel.Direct("b"), received("""{"type":"direct","user":"b"}"""))
        assertEquals(Channel.Unknown("party"), received("""{"type":"party","id":3}"""))
        assertNull(AxochatCodec.decodeOrNull(packet(""""global"""")))
        assertNull(AxochatCodec.decodeOrNull(packet("""{"type":"direct"}""")))
    }

    @Test
    fun `moderation packets`() {
        assertEquals(
            """{"m":"Punish","c":{"user":"Notch","kind":"ban","reason":"spam","include_ip":true}}""",
            AxochatCodec.encode(Serverbound.Punish(user = "Notch", kind = PunishmentKind.Ban, reason = "spam", includeIp = true)),
        )
        val punished = assertIs<Clientbound.Punished>(
            AxochatCodec.decode("""{"m":"Punished","c":{"kind":"mute","reason":"spam","expires":60000}}"""),
        )
        assertEquals(Clientbound.Punished(PunishmentKind.Mute, "spam", 60000), punished)
        assertEquals(Clientbound.MessageRemoved(7), AxochatCodec.decode("""{"m":"MessageRemoved","c":{"id":7}}"""))
        assertEquals("""{"m":"RemoveMessage","c":{"id":7}}""", AxochatCodec.encode(Serverbound.RemoveMessage(7)))
    }

    @Test
    fun `report packets`() {
        assertEquals(
            """{"m":"Report","c":{"user":"Notch","message":7,"reason":"spam"}}""",
            AxochatCodec.encode(Serverbound.Report("Notch", 7, "spam")),
        )
        val created = assertIs<Clientbound.ReportCreated>(AxochatCodec.decode(
            """{"m":"ReportCreated","c":{"report":{"id":"r","reporter":{"id":"a","kind":"account","name":"A"},""" +
                """"target":{"id":"b","kind":"account","name":"B"},"channel":{"type":"direct","user":"a"},""" +
                """"message":7,"content":"hi","reason":"spam","time":1}}}""",
        ))
        assertEquals(Channel.Direct("a"), created.report.channel)
        assertEquals("hi", created.report.content)
    }
}
