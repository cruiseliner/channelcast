package com.cruiseliner.channelcast

import uniffi.channelcast_ffi.FfiHeadline
import uniffi.channelcast_ffi.FfiWeatherDashboard

/**
 * In-memory keep-alive for channel/weather tab content.
 * Mimics pushed pages: once opened, reuse until TTL expires or user pull-to-refresh.
 *
 * Process-scoped singleton so orientation / Activity recreate does not wipe the cache
 * (Compose `remember` alone dies with the Composition).
 */
internal const val TAB_CACHE_TTL_MS: Long = 10L * 60L * 1000L // 10 minutes

internal data class TimedCache<T>(
    val value: T,
    val fetchedAtMs: Long = System.currentTimeMillis(),
) {
    fun isFresh(
        nowMs: Long = System.currentTimeMillis(),
        ttlMs: Long = TAB_CACHE_TTL_MS,
    ): Boolean = nowMs - fetchedAtMs < ttlMs
}

/** Survives config changes; shared by phone + TV Compose trees. */
internal object TabCacheStore {
    private val headlines = LinkedHashMap<String, TimedCache<List<FfiHeadline>>>()
    @Volatile private var weather: TimedCache<FfiWeatherDashboard>? = null

    @Synchronized
    fun headlines(id: String): TimedCache<List<FfiHeadline>>? = headlines[id]

    @Synchronized
    fun putHeadlines(id: String, items: List<FfiHeadline>) {
        headlines[id] = TimedCache(items)
    }

    @Synchronized
    fun invalidateHeadlines(id: String) {
        headlines.remove(id)
    }

    fun weather(): TimedCache<FfiWeatherDashboard>? = weather

    fun putWeather(dash: FfiWeatherDashboard) {
        weather = TimedCache(dash)
    }

    fun invalidateWeather() {
        weather = null
    }
}
