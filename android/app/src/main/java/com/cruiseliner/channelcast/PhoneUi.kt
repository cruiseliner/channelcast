package com.cruiseliner.channelcast

import android.util.Log
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
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
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
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
import uniffi.channelcast_ffi.fetchHeadlines
import uniffi.channelcast_ffi.listChannels
import java.time.OffsetDateTime
import java.time.format.DateTimeFormatter
import java.util.Locale

/**
 * Phone / S25 Ultra UI: Material 3 dark feed inspired by modern news apps
 * (hero photo cards, photo+headline rows, category chips) — original ChannelCast branding.
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
    var status by remember { mutableStateOf("Loading channels…") }
    var refreshTick by remember { mutableStateOf(0) }

    LaunchedEffect(Unit) {
        try {
            val listed = withContext(Dispatchers.IO) { listChannels(false) }
            channels = listed
            status = "${listed.size} channels"
            if (selectedId == null) {
                selectedId = listed.firstOrNull { it.id == "top" }?.id
                    ?: listed.firstOrNull()?.id
            }
            Log.i("ChannelCast", "phone listChannels -> ${listed.size}")
        } catch (e: ChannelCastFfiException) {
            channelsError = e.message ?: e.toString()
            status = "Channel list failed"
            Log.e("ChannelCast", "phone listChannels failed", e)
        } catch (e: Throwable) {
            channelsError = e.message ?: e.toString()
            status = "Native bridge failed"
            Log.e("ChannelCast", "phone listChannels crashed", e)
        }
    }

    LaunchedEffect(selectedId, refreshTick) {
        val id = selectedId ?: return@LaunchedEffect
        loading = true
        headlinesError = null
        headlines = emptyList()
        status = "Fetching $id…"
        try {
            val items = withContext(Dispatchers.IO) { fetchHeadlines(id, 40u) }
            headlines = items
            val withImg = items.count { !it.imageUrl.isNullOrBlank() }
            status = "$id · ${items.size} stories · $withImg photos"
            Log.i("ChannelCast", "phone fetchHeadlines($id) -> ${items.size} (images=$withImg)")
        } catch (e: ChannelCastFfiException) {
            headlinesError = e.message ?: e.toString()
            status = "Fetch failed for $id"
            Log.e("ChannelCast", "phone fetchHeadlines($id) failed", e)
        } catch (e: Throwable) {
            headlinesError = e.message ?: e.toString()
            status = "Fetch crashed for $id"
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

    val selectedName = channels.firstOrNull { it.id == selectedId }?.name ?: "Today"

    Scaffold(
        containerColor = surface,
        topBar = {
            TopAppBar(
                title = {
                    Column {
                        Text(
                            "ChannelCast",
                            fontWeight = FontWeight.Bold,
                            letterSpacing = (-0.3).sp,
                        )
                        Text(
                            text = status,
                            style = MaterialTheme.typography.labelSmall,
                            color = muted,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = surface,
                    titleContentColor = onSurface,
                ),
            )
        },
    ) { inner ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(inner),
        ) {
            if (channelsError != null) {
                Text(
                    text = channelsError!!,
                    color = MaterialTheme.colorScheme.error,
                    style = MaterialTheme.typography.bodyMedium,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                )
            }

            LazyRow(
                contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
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
                isRefreshing = refreshing || (loading && headlines.isEmpty()),
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
                when {
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
                            contentPadding = PaddingValues(
                                start = 16.dp,
                                end = 16.dp,
                                top = 4.dp,
                                bottom = 32.dp,
                            ),
                            verticalArrangement = Arrangement.spacedBy(14.dp),
                            modifier = Modifier
                                .fillMaxSize()
                                .navigationBarsPadding(),
                        ) {
                            item(key = "section-header") {
                                SectionHeader(
                                    title = selectedName,
                                    subtitle = formatSectionDate(),
                                    onSurface = onSurface,
                                    muted = muted,
                                )
                            }
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
}

@Composable
private fun SectionHeader(
    title: String,
    subtitle: String,
    onSurface: Color,
    muted: Color,
) {
    Column(Modifier.padding(top = 4.dp, bottom = 2.dp)) {
        Text(
            text = subtitle.uppercase(Locale.US),
            style = MaterialTheme.typography.labelMedium,
            fontWeight = FontWeight.SemiBold,
            letterSpacing = 1.2.sp,
            color = muted,
        )
        Spacer(Modifier.height(4.dp))
        Text(
            text = title,
            style = MaterialTheme.typography.headlineMedium,
            fontWeight = FontWeight.Bold,
            color = onSurface,
            letterSpacing = (-0.5).sp,
        )
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

private fun formatSectionDate(): String = try {
    java.time.LocalDate.now().format(DateTimeFormatter.ofPattern("EEEE, MMMM d", Locale.US))
} catch (_: Exception) {
    "Today"
}
