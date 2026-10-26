package com.cruiseliner.channelcast

import android.util.Log
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import coil.compose.AsyncImage
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.channelcast_ffi.ChannelCastFfiException
import uniffi.channelcast_ffi.FfiChannel
import uniffi.channelcast_ffi.FfiHeadline
import uniffi.channelcast_ffi.FfiWeatherDashboard
import uniffi.channelcast_ffi.fetchHeadlines
import uniffi.channelcast_ffi.fetchWeather
import uniffi.channelcast_ffi.listChannels
import java.time.OffsetDateTime
import java.time.format.DateTimeFormatter
import java.util.Locale

/**
 * Phone / S25 Ultra UI: Material 3 dark image-forward feed (hero cards, photo rows, chips).
 * Chrome is minimal — no app-title bar, no section/count header, content fills the screen.
 * Shares UniFFI feeds with the TV leanback layout; tap opens via Custom Tabs.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PhoneChannelCastScreen(
    initialChannelId: String?,
    onOpenUrl: (String, String?, String?) -> Unit,
) {
    var channels by remember { mutableStateOf<List<FfiChannel>>(emptyList()) }
    var channelsError by remember { mutableStateOf<String?>(null) }
    var selectedId by remember { mutableStateOf(initialChannelId) }
    var headlines by remember { mutableStateOf<List<FfiHeadline>>(emptyList()) }
    var loading by remember { mutableStateOf(false) }
    var refreshing by remember { mutableStateOf(false) }
    var headlinesError by remember { mutableStateOf<String?>(null) }
    var weather by remember { mutableStateOf<FfiWeatherDashboard?>(null) }
    var weatherError by remember { mutableStateOf<String?>(null) }
    var refreshTick by remember { mutableStateOf(0) }
    // Keep-alive caches: tab switches reuse fresh content; pull-to-refresh forces fetch.
    val headlinesCache = remember { mutableStateMapOf<String, TimedCache<List<FfiHeadline>>>() }
    var weatherCache by remember { mutableStateOf<TimedCache<FfiWeatherDashboard>?>(null) }
    var lastHandledRefreshTick by remember { mutableStateOf(0) }

    LaunchedEffect(Unit) {
        try {
            val listed = withContext(Dispatchers.IO) { listChannels(false) }
            channels = listed
            if (selectedId == null) {
                selectedId = listed.firstOrNull { it.id == "top" }?.id
                    ?: listed.firstOrNull()?.id
            }
            Log.i("ChannelCast", "phone listChannels -> ${listed.size}")
        } catch (e: ChannelCastFfiException) {
            channelsError = e.message ?: e.toString()
            Log.e("ChannelCast", "phone listChannels failed", e)
        } catch (e: Throwable) {
            channelsError = e.message ?: e.toString()
            Log.e("ChannelCast", "phone listChannels crashed", e)
        }
    }

    LaunchedEffect(selectedId, refreshTick) {
        val id = selectedId ?: return@LaunchedEffect
        val forceRefresh = refreshTick != lastHandledRefreshTick
        if (forceRefresh) lastHandledRefreshTick = refreshTick
        headlinesError = null
        weatherError = null

        if (id == "weather") {
            headlines = emptyList()
            val hit = weatherCache
            if (!forceRefresh && hit != null && hit.isFresh()) {
                weather = hit.value
                loading = false
                refreshing = false
                Log.i("ChannelCast", "phone weather cache hit (age=${System.currentTimeMillis() - hit.fetchedAtMs}ms)")
                return@LaunchedEffect
            }
            // Keep showing stale weather while refreshing when we have it.
            if (hit != null) weather = hit.value
            loading = weather == null
            try {
                val dash = withContext(Dispatchers.IO) { fetchWeather() }
                weather = dash
                weatherCache = TimedCache(dash)
                Log.i(
                    "ChannelCast",
                    "phone fetchWeather -> ${dash.now.temperatureF}F hourly=${dash.hourly.size} graphics=${dash.graphics.size} tropical=${dash.tropical.size}",
                )
            } catch (e: ChannelCastFfiException) {
                weatherError = e.message ?: e.toString()
                if (weather == null) weather = null
                Log.e("ChannelCast", "phone fetchWeather failed", e)
            } catch (e: Throwable) {
                weatherError = e.message ?: e.toString()
                if (weather == null) weather = null
                Log.e("ChannelCast", "phone fetchWeather crashed", e)
            } finally {
                loading = false
                refreshing = false
            }
            return@LaunchedEffect
        }

        val hit = headlinesCache[id]
        if (!forceRefresh && hit != null && hit.isFresh()) {
            headlines = hit.value
            loading = false
            refreshing = false
            Log.i("ChannelCast", "phone headlines cache hit for $id (age=${System.currentTimeMillis() - hit.fetchedAtMs}ms)")
            return@LaunchedEffect
        }
        if (hit != null) headlines = hit.value else headlines = emptyList()
        loading = headlines.isEmpty()
        try {
            val items = withContext(Dispatchers.IO) { fetchHeadlines(id, 40u) }
            headlines = items
            headlinesCache[id] = TimedCache(items)
            val withImg = items.count { !it.imageUrl.isNullOrBlank() }
            Log.i("ChannelCast", "phone fetchHeadlines($id) -> ${items.size} (images=$withImg)")
        } catch (e: ChannelCastFfiException) {
            headlinesError = e.message ?: e.toString()
            Log.e("ChannelCast", "phone fetchHeadlines($id) failed", e)
        } catch (e: Throwable) {
            headlinesError = e.message ?: e.toString()
            Log.e("ChannelCast", "phone fetchHeadlines($id) crashed", e)
        } finally {
            loading = false
            refreshing = false
        }
    }

    // Deep black / gray Material dark suited to OLED (S25 Ultra).
    val surface = Color(0xFF121212)
    val card = Color(0xFF1C1C1E)
    val chipSelected = Color(0xFF2C2C2E)
    val accent = Color(0xFF9AB0D0)
    val onSurface = Color(0xFFF5F5F7)
    val muted = Color(0xFF8E8E93)
    val navBottom = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()

    // No TopAppBar / section header chrome — chips + stories own the screen.
    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(surface)
            .statusBarsPadding(),
    ) {
        if (channelsError != null) {
            Text(
                text = channelsError!!,
                color = MaterialTheme.colorScheme.error,
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
            )
        }

        LazyRow(
            contentPadding = PaddingValues(horizontal = 12.dp, vertical = 6.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            modifier = Modifier.fillMaxWidth(),
        ) {
            items(channels, key = { it.id }) { ch ->
                val selected = ch.id == selectedId
                FilterChip(
                    selected = selected,
                    onClick = { selectedId = ch.id },
                    label = {
                        Text(
                            ch.name,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Medium,
                        )
                    },
                    colors = FilterChipDefaults.filterChipColors(
                        selectedContainerColor = chipSelected,
                        selectedLabelColor = onSurface,
                        containerColor = Color(0xFF1C1C1E),
                        labelColor = muted,
                    ),
                    border = FilterChipDefaults.filterChipBorder(
                        enabled = true,
                        selected = selected,
                        borderColor = Color(0xFF2C2C2E),
                        selectedBorderColor = accent,
                    ),
                )
            }
        }

        PullToRefreshBox(
            isRefreshing = refreshing || (loading && (
                if (selectedId == "weather") weather == null else headlines.isEmpty()
            )),
            onRefresh = {
                if (selectedId != null) {
                    refreshing = true
                    refreshTick += 1
                }
            },
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth(),
        ) {
            val weatherPad = PaddingValues(
                start = 12.dp,
                end = 12.dp,
                top = 4.dp,
                bottom = 12.dp + navBottom,
            )
            if (selectedId == "weather") {
                WeatherDashboardContent(
                    dash = weather,
                    loading = loading,
                    error = weatherError,
                    onOpenUrl = onOpenUrl,
                    contentPadding = weatherPad,
                    phoneDensity = true,
                )
            } else when {
                loading && headlines.isEmpty() && headlinesError == null -> {
                    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                        CircularProgressIndicator(color = accent)
                    }
                }
                headlinesError != null -> {
                    Box(
                        Modifier
                            .fillMaxSize()
                            .padding(24.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            headlinesError!!,
                            color = MaterialTheme.colorScheme.error,
                            style = MaterialTheme.typography.bodyLarge,
                        )
                    }
                }
                headlines.isEmpty() -> {
                    Box(
                        Modifier
                            .fillMaxSize()
                            .padding(24.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            "No stories yet — pick a channel or pull to refresh.",
                            color = muted,
                            style = MaterialTheme.typography.bodyLarge,
                        )
                    }
                }
                else -> {
                    LazyColumn(
                        contentPadding = weatherPad,
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                        modifier = Modifier.fillMaxSize(),
                    ) {
                        itemsIndexed(headlines, key = { _, h -> h.url + h.title }) { index, h ->
                            val hasImage = !h.imageUrl.isNullOrBlank()
                            if (index == 0 && hasImage) {
                                HeroStoryCard(
                                    headline = h,
                                    cardColor = card,
                                    onSurface = onSurface,
                                    muted = muted,
                                    onClick = { onOpenUrl(h.url, h.title, h.summary) },
                                )
                            } else if (hasImage) {
                                PhotoRowCard(
                                    headline = h,
                                    cardColor = card,
                                    onSurface = onSurface,
                                    muted = muted,
                                    onClick = { onOpenUrl(h.url, h.title, h.summary) },
                                )
                            } else {
                                TextStoryCard(
                                    headline = h,
                                    cardColor = card,
                                    onSurface = onSurface,
                                    muted = muted,
                                    accent = accent,
                                    onClick = { onOpenUrl(h.url, h.title, h.summary) },
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun HeroStoryCard(
    headline: FfiHeadline,
    cardColor: Color,
    onSurface: Color,
    muted: Color,
    onClick: () -> Unit,
) {
    Card(
        onClick = onClick,
        shape = RoundedCornerShape(18.dp),
        colors = CardDefaults.cardColors(containerColor = cardColor),
        elevation = CardDefaults.cardElevation(defaultElevation = 0.dp),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Box {
            AsyncImage(
                model = headline.imageUrl,
                contentDescription = null,
                contentScale = ContentScale.Crop,
                modifier = Modifier
                    .fillMaxWidth()
                    .aspectRatio(16f / 10f),
            )
            Box(
                modifier = Modifier
                    .matchParentSize()
                    .background(
                        Brush.verticalGradient(
                            colors = listOf(
                                Color.Transparent,
                                Color(0xCC121212),
                                Color(0xF2121212),
                            ),
                        ),
                    ),
            )
            Column(
                modifier = Modifier
                    .align(Alignment.BottomStart)
                    .padding(horizontal = 16.dp, vertical = 16.dp),
            ) {
                Text(
                    text = metaLine(headline),
                    style = MaterialTheme.typography.labelMedium,
                    color = Color(0xFFB8C0CC),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Spacer(Modifier.height(6.dp))
                Text(
                    text = headline.title,
                    style = MaterialTheme.typography.headlineSmall,
                    fontWeight = FontWeight.Bold,
                    color = onSurface,
                    maxLines = 3,
                    overflow = TextOverflow.Ellipsis,
                    letterSpacing = (-0.2).sp,
                )
                plainTextSummary(headline.summary)?.let { summary ->
                    Spacer(Modifier.height(8.dp))
                    Text(
                        text = summary.take(160),
                        style = MaterialTheme.typography.bodyMedium,
                        color = Color(0xFFC8CED8),
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        }
    }
}

@Composable
private fun PhotoRowCard(
    headline: FfiHeadline,
    cardColor: Color,
    onSurface: Color,
    muted: Color,
    onClick: () -> Unit,
) {
    Card(
        onClick = onClick,
        shape = RoundedCornerShape(16.dp),
        colors = CardDefaults.cardColors(containerColor = cardColor),
        elevation = CardDefaults.cardElevation(defaultElevation = 0.dp),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Row(
            modifier = Modifier.padding(12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f)) {
                Text(
                    text = metaLine(headline),
                    style = MaterialTheme.typography.labelMedium,
                    color = muted,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Spacer(Modifier.height(6.dp))
                Text(
                    text = headline.title,
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                    color = onSurface,
                    maxLines = 3,
                    overflow = TextOverflow.Ellipsis,
                )
                plainTextSummary(headline.summary)?.let { summary ->
                    Spacer(Modifier.height(6.dp))
                    Text(
                        text = summary.take(120),
                        style = MaterialTheme.typography.bodySmall,
                        color = Color(0xFFA1A1A6),
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
            Spacer(Modifier.width(12.dp))
            AsyncImage(
                model = headline.imageUrl,
                contentDescription = null,
                contentScale = ContentScale.Crop,
                modifier = Modifier
                    .size(width = 108.dp, height = 108.dp)
                    .clip(RoundedCornerShape(12.dp)),
            )
        }
    }
}

@Composable
private fun TextStoryCard(
    headline: FfiHeadline,
    cardColor: Color,
    onSurface: Color,
    muted: Color,
    accent: Color,
    onClick: () -> Unit,
) {
    Card(
        onClick = onClick,
        shape = RoundedCornerShape(16.dp),
        colors = CardDefaults.cardColors(containerColor = cardColor),
        elevation = CardDefaults.cardElevation(defaultElevation = 0.dp),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Column(Modifier.padding(horizontal = 16.dp, vertical = 14.dp)) {
            Text(
                text = metaLine(headline),
                style = MaterialTheme.typography.labelMedium,
                color = accent,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Spacer(Modifier.height(6.dp))
            Text(
                text = headline.title,
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.SemiBold,
                color = onSurface,
            )
            plainTextSummary(headline.summary)?.let { summary ->
                Spacer(Modifier.height(8.dp))
                Text(
                    text = summary.take(220),
                    style = MaterialTheme.typography.bodyMedium,
                    color = Color(0xFFA1A1A6),
                    maxLines = 3,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

private fun metaLine(headline: FfiHeadline): String = buildString {
    append(headline.source)
    headline.published?.let { rfc ->
        formatShortTime(rfc)?.let {
            append(" · ")
            append(it)
        }
    }
}

private fun formatShortTime(rfc3339: String): String? = try {
    val odt = OffsetDateTime.parse(rfc3339)
    odt.format(DateTimeFormatter.ofPattern("MMM d · h:mm a", Locale.US))
} catch (_: Exception) {
    rfc3339.take(16).replace('T', ' ')
}

