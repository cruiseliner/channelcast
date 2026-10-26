package com.cruiseliner.channelcast

import android.util.Log
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.channelcast_ffi.ChannelCastFfiException
import uniffi.channelcast_ffi.FfiChannel
import uniffi.channelcast_ffi.FfiHeadline
import uniffi.channelcast_ffi.fetchHeadlines
import uniffi.channelcast_ffi.listChannels

/**
 * Phone / S25 Ultra UI: Material 3, channel chips, headline cards, pull-to-refresh.
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
            status = "$id · ${items.size} headlines"
            Log.i("ChannelCast", "phone fetchHeadlines($id) -> ${items.size}")
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

    val surface = Color(0xFF0E1014)
    val card = Color(0xFF171B24)
    val chipSelected = Color(0xFF2A3A55)
    val accent = Color(0xFF8FA3C1)

    Scaffold(
        containerColor = surface,
        topBar = {
            TopAppBar(
                title = {
                    Column {
                        Text(
                            "ChannelCast",
                            fontWeight = FontWeight.SemiBold,
                        )
                        Text(
                            text = status,
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = Color(0xFF12141A),
                    titleContentColor = Color(0xFFE8ECF4),
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
                contentPadding = PaddingValues(horizontal = 16.dp, vertical = 10.dp),
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
                            )
                        },
                        colors = FilterChipDefaults.filterChipColors(
                            selectedContainerColor = chipSelected,
                            selectedLabelColor = Color(0xFFF2F5FA),
                            containerColor = Color(0xFF1A1F2A),
                            labelColor = Color(0xFFC5CDD8),
                        ),
                        border = FilterChipDefaults.filterChipBorder(
                            enabled = true,
                            selected = selected,
                            borderColor = Color(0xFF2A3344),
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
                                "No headlines yet — pick a channel or pull to refresh.",
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
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
                                bottom = 28.dp,
                            ),
                            verticalArrangement = Arrangement.spacedBy(10.dp),
                            modifier = Modifier
                                .fillMaxSize()
                                .navigationBarsPadding(),
                        ) {
                            items(headlines, key = { it.url + it.title }) { h ->
                                HeadlineCard(
                                    headline = h,
                                    cardColor = card,
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
private fun HeadlineCard(
    headline: FfiHeadline,
    cardColor: Color,
    onClick: () -> Unit,
) {
    Card(
        onClick = onClick,
        shape = RoundedCornerShape(14.dp),
        colors = CardDefaults.cardColors(containerColor = cardColor),
        elevation = CardDefaults.cardElevation(defaultElevation = 0.dp),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Column(Modifier.padding(horizontal = 16.dp, vertical = 14.dp)) {
            Text(
                text = headline.title,
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.SemiBold,
                color = Color(0xFFF2F5FA),
            )
            Spacer(Modifier.height(6.dp))
            val meta = buildString {
                append(headline.source)
                headline.published?.let {
                    append(" · ")
                    append(it.take(16).replace('T', ' '))
                }
            }
            Text(
                text = meta,
                style = MaterialTheme.typography.labelMedium,
                color = Color(0xFF8FA3C1),
            )
            plainTextSummary(headline.summary)?.let { summary ->
                Spacer(Modifier.height(8.dp))
                Text(
                    text = summary.take(220),
                    style = MaterialTheme.typography.bodyMedium,
                    color = Color(0xFFB0B8C8),
                    maxLines = 3,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}
