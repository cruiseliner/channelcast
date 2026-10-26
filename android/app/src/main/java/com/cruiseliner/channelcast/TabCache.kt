package com.cruiseliner.channelcast

import android.util.Log
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import uniffi.channelcast_ffi.FfiHeadline
import uniffi.channelcast_ffi.FfiWeatherDashboard
import uniffi.channelcast_ffi.fetchHeadlines
import uniffi.channelcast_ffi.fetchWeather

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

/**
 * True for Compose/coroutine cancel — must never be shown as a channel error string
 * (e.g. "The coroutine scope left the composition").
 */
internal fun Throwable.isCoroutineCancellation(): Boolean {
    if (this is CancellationException) return true
    val msg = message.orEmpty()
    return msg.contains("coroutine scope left the composition", ignoreCase = true) ||
        msg.contains("Job was cancelled", ignoreCase = true)
}

/**
 * Process-scoped fetches so leaving a composition (tab switch / config) cancels only the
 * UI await — the network work keeps filling [TabCacheStore] for the next open.
 */
internal object TabLoadCoordinator {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val headlineLoads = LinkedHashMap<String, Deferred<Result<List<FfiHeadline>>>>()
    @Volatile private var weatherLoad: Deferred<Result<FfiWeatherDashboard>>? = null

    @Synchronized
    fun loadHeadlines(id: String, force: Boolean): Deferred<Result<List<FfiHeadline>>> {
        if (!force) {
            headlineLoads[id]?.takeIf { it.isActive }?.let { return it }
            TabCacheStore.headlines(id)?.takeIf { it.isFresh() }?.let { hit ->
                return scope.async { Result.success(hit.value) }
            }
        } else {
            headlineLoads.remove(id)?.cancel()
            TabCacheStore.invalidateHeadlines(id)
        }
        val deferred = scope.async {
            try {
                val items = fetchHeadlines(id, 40u)
                TabCacheStore.putHeadlines(id, items)
                Result.success(items)
            } catch (e: Throwable) {
                if (e is CancellationException) throw e
                Result.failure(e)
            }
        }
        headlineLoads[id] = deferred
        deferred.invokeOnCompletion { err ->
            if (err != null) {
                Log.i("ChannelCast", "headline load $id ended: ${err.javaClass.simpleName}")
            }
            synchronized(this) {
                if (headlineLoads[id] === deferred) headlineLoads.remove(id)
            }
        }
        return deferred
    }

    @Synchronized
    fun loadWeather(force: Boolean): Deferred<Result<FfiWeatherDashboard>> {
        if (!force) {
            weatherLoad?.takeIf { it.isActive }?.let { return it }
            TabCacheStore.weather()?.takeIf { it.isFresh() }?.let { hit ->
                return scope.async { Result.success(hit.value) }
            }
        } else {
            weatherLoad?.cancel()
            weatherLoad = null
            TabCacheStore.invalidateWeather()
        }
        val deferred = scope.async {
            try {
                val dash = fetchWeather()
                TabCacheStore.putWeather(dash)
                Result.success(dash)
            } catch (e: Throwable) {
                if (e is CancellationException) throw e
                Result.failure(e)
            }
        }
        weatherLoad = deferred
        deferred.invokeOnCompletion { err ->
            if (err != null) {
                Log.i("ChannelCast", "weather load ended: ${err.javaClass.simpleName}")
            }
            if (weatherLoad === deferred) weatherLoad = null
        }
        return deferred
    }
}
