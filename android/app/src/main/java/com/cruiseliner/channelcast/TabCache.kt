package com.cruiseliner.channelcast

/**
 * In-memory keep-alive for channel/weather tab content.
 * Mimics pushed pages: once opened, reuse until TTL expires or user pull-to-refresh.
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
