package com.cruiseliner.channelcast

/**
 * Stub channel list mirroring channels.toml ids.
 * Replaced by channelcast-core via UniFFI/JNI when the Rust bridge lands.
 */
data class ChannelStub(
    val id: String,
    val name: String,
    val description: String,
    val enabledByDefault: Boolean = true,
)

object PlaceholderChannels {
    val all: List<ChannelStub> = listOf(
        ChannelStub("top", "Top", "World / general headlines"),
        ChannelStub("tech", "Tech", "Technology and software"),
        ChannelStub("entertainment", "Entertainment", "USA Today entertainment"),
        ChannelStub("markets", "Markets", "Business and markets"),
        ChannelStub("sports", "Sports", "Sports headlines"),
        ChannelStub("prowrestling", "Pro Wrestling", "Fightful + F4W", enabledByDefault = true),
        ChannelStub("watchman", "Watchman", "Israel / End Times watch (personal)", enabledByDefault = true),
        ChannelStub("weather", "Weather", "NWS Cape Canaveral / ZIP 32920"),
        ChannelStub("local", "Local", "Space Coast Daily + Florida Today space"),
        ChannelStub("spacecoast", "Space Coast", "NASA / KSC / SpaceX / Blue Origin"),
    )
}
