package com.cruiseliner.channelcast

import android.content.Intent
import android.text.Html
import android.os.Build
import android.content.res.Configuration
import android.net.Uri
import android.os.Bundle
import android.util.Log
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.browser.customtabs.CustomTabsIntent
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
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
 * Phone + TV Activity backed by channelcast-core via UniFFI.
 * Deep links: channelcast://open | channelcast://channel/{id}
 */
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val initial = channelIdFromIntent(intent)
        setContent {
            MaterialTheme(colorScheme = darkColorScheme()) {
                Surface(modifier = Modifier.fillMaxSize()) {
                    ChannelCastScreen(
                        initialChannelId = initial,
                        onOpenUrl = { url, title -> openArticle(url, title) },
                    )
                }
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
    }

    private fun channelIdFromIntent(intent: Intent?): String? {
        val data: Uri = intent?.data ?: return null
        if (data.scheme != "channelcast") return null
        return when (data.host) {
            "open" -> null
            "channel" -> data.pathSegments.firstOrNull()
            else -> null
        }
    }

    private fun isTelevision(): Boolean {
        val uiMode = resources.configuration.uiMode and Configuration.UI_MODE_TYPE_MASK
        return uiMode == Configuration.UI_MODE_TYPE_TELEVISION
    }

    private fun openInWebView(url: String, title: String?) {
        startActivity(
            Intent(this, ArticleActivity::class.java).apply {
                putExtra(ArticleActivity.EXTRA_URL, url)
                if (!title.isNullOrBlank()) {
                    putExtra(ArticleActivity.EXTRA_TITLE, title)
                }
            },
        )
    }

    /**
     * TV: always use in-app WebView (no browser / Custom Tabs on most TCL sets).
     * Phone: Custom Tabs → ACTION_VIEW → WebView fallback.
     */
    private fun openArticle(url: String, title: String? = null) {
        if (!isValidHttpUrl(url)) {
            Toast.makeText(this, "No usable link for this story", Toast.LENGTH_SHORT).show()
            Log.w(TAG, "Rejecting bad article URL: $url")
            return
        }
        val uri = Uri.parse(url.trim())
        if (isTelevision()) {
            // Prefer in-app WebView on TV (no reliable browser on many TCL sets).
            openInWebView(url.trim(), title)
            return
        }
        try {
            CustomTabsIntent.Builder().build().launchUrl(this, uri)
            return
        } catch (_: Exception) {
            // Fall through — no Custom Tabs provider.
        }
        try {
            startActivity(Intent(Intent.ACTION_VIEW, uri))
        } catch (e: Exception) {
            Log.w(TAG, "No browser for $url — WebView", e)
            openInWebView(url.trim(), title)
        }
    }

    companion object {
        private const val TAG = "ChannelCast"
    }
}


/** Safety net: core should already strip HTML; never show markup in the list/story pane. */
fun plainTextSummary(raw: String?): String? {
    if (raw.isNullOrBlank()) return null
    val spanned = if (Build.VERSION.SDK_INT >= 24) {
        Html.fromHtml(raw, Html.FROM_HTML_MODE_LEGACY)
    } else {
        @Suppress("DEPRECATION")
        Html.fromHtml(raw)
    }
    val plain = spanned.toString().replace('\u00A0', ' ').trim().replace(Regex("\\s+"), " ")
    if (plain.isEmpty()) return null
    val lower = plain.lowercase()
    if (lower.startsWith("article url:")) {
        val rest = plain.substringAfter(':').trim()
        val afterUrl = rest.split(Regex("\\s+"), limit = 2).getOrNull(1)?.trim().orEmpty()
        return afterUrl.ifBlank { null }
    }
    if ('<' in plain || '>' in plain) {
        return plain.replace(Regex("<[^>]*>"), "").trim().ifBlank { null }
    }
    return plain
}

fun isValidHttpUrl(url: String): Boolean {
    val u = url.trim()
    if (!(u.startsWith("https://") || u.startsWith("http://"))) return false
    if ('<' in u || '>' in u || u.contains(' ')) return false
    if (u == "https://" || u == "http://" || u.startsWith("https:/") && !u.startsWith("https://")) return false
    if (u.startsWith("http:/") && !u.startsWith("http://")) return false
    return try {
        val uri = Uri.parse(u)
        !uri.host.isNullOrBlank()
    } catch (_: Exception) {
        false
    }
}

@Composable
fun ChannelCastScreen(
    initialChannelId: String?,
    onOpenUrl: (String, String?) -> Unit,
) {
    var channels by remember { mutableStateOf<List<FfiChannel>>(emptyList()) }
    var channelsError by remember { mutableStateOf<String?>(null) }
    var selectedId by remember { mutableStateOf(initialChannelId) }
    var headlines by remember { mutableStateOf<List<FfiHeadline>>(emptyList()) }
    var selectedHeadline by remember { mutableStateOf<FfiHeadline?>(null) }
    var loading by remember { mutableStateOf(false) }
    var headlinesError by remember { mutableStateOf<String?>(null) }
    var status by remember { mutableStateOf("Loading channels…") }
    var refreshTick by remember { mutableStateOf(0) }

    LaunchedEffect(Unit) {
        try {
            val listed = withContext(Dispatchers.IO) {
                listChannels(false)
            }
            channels = listed
            status = "${listed.size} channels"
            if (selectedId == null) {
                selectedId = listed.firstOrNull { it.id == "top" }?.id
                    ?: listed.firstOrNull()?.id
            }
            Log.i("ChannelCast", "listChannels -> ${listed.size}")
        } catch (e: ChannelCastFfiException) {
            channelsError = e.message ?: e.toString()
            status = "Channel list failed"
            Log.e("ChannelCast", "listChannels failed", e)
        } catch (e: Throwable) {
            channelsError = e.message ?: e.toString()
            status = "Native bridge failed"
            Log.e("ChannelCast", "listChannels crashed", e)
        }
    }

    LaunchedEffect(selectedId, refreshTick) {
        val id = selectedId ?: return@LaunchedEffect
        loading = true
        headlinesError = null
        headlines = emptyList()
        selectedHeadline = null
        status = "Fetching $id…"
        try {
            val items = withContext(Dispatchers.IO) {
                fetchHeadlines(id, 40u)
            }
            headlines = items
            status = "$id · ${items.size} headlines"
            Log.i("ChannelCast", "fetchHeadlines($id) -> ${items.size}")
            items.take(3).forEach {
                Log.i("ChannelCast", "  · ${it.title}")
            }
        } catch (e: ChannelCastFfiException) {
            headlinesError = e.message ?: e.toString()
            status = "Fetch failed for $id"
            Log.e("ChannelCast", "fetchHeadlines($id) failed", e)
        } catch (e: Throwable) {
            headlinesError = e.message ?: e.toString()
            status = "Fetch crashed for $id"
            Log.e("ChannelCast", "fetchHeadlines($id) crashed", e)
        } finally {
            loading = false
        }
    }

    Row(modifier = Modifier.fillMaxSize()) {
        Column(
            modifier = Modifier
                .widthIn(min = 200.dp, max = 320.dp)
                .fillMaxHeight()
                .background(Color(0xFF12141A))
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text("ChannelCast", style = MaterialTheme.typography.headlineSmall)
            Text(
                text = status,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            HorizontalDivider()
            if (channelsError != null) {
                Text(
                    text = channelsError!!,
                    color = MaterialTheme.colorScheme.error,
                    style = MaterialTheme.typography.bodySmall,
                )
            }
            LazyColumn(
                contentPadding = PaddingValues(vertical = 4.dp),
                verticalArrangement = Arrangement.spacedBy(2.dp),
                modifier = Modifier.weight(1f),
            ) {
                items(channels, key = { it.id }) { ch ->
                    val selected = ch.id == selectedId
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .background(if (selected) Color(0xFF2A3344) else Color.Transparent)
                            .clickable { selectedId = ch.id }
                            .padding(vertical = 10.dp, horizontal = 8.dp),
                    ) {
                        Text(
                            text = ch.name + if (ch.personal) " ●" else "",
                            style = MaterialTheme.typography.titleMedium,
                        )
                        Text(
                            text = ch.id,
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }
            Text(
                text = "Refresh",
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier
                    .clickable {
                        if (selectedId != null) refreshTick += 1
                    }
                    .padding(8.dp),
            )
        }

        Column(
            modifier = Modifier
                .weight(1f)
                .fillMaxHeight()
                .padding(20.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(
                text = selectedId?.let { id ->
                    channels.firstOrNull { it.id == id }?.name ?: id
                } ?: "Headlines",
                style = MaterialTheme.typography.headlineSmall,
            )
            if (loading) {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    CircularProgressIndicator()
                }
            } else if (headlinesError != null) {
                Text(
                    text = headlinesError!!,
                    color = MaterialTheme.colorScheme.error,
                )
            } else if (headlines.isEmpty()) {
                Text(
                    text = "No headlines yet — pick a channel or wait for fetch.",
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            } else {
                LazyColumn(
                    verticalArrangement = Arrangement.spacedBy(6.dp),
                    contentPadding = PaddingValues(bottom = 24.dp),
                    modifier = Modifier.weight(1f, fill = false),
                ) {
                    items(headlines, key = { it.url + it.title }) { h ->
                        val selected = selectedHeadline?.url == h.url && selectedHeadline?.title == h.title
                        Column(
                            modifier = Modifier
                                .fillMaxWidth()
                                .background(if (selected) Color(0xFF1E2633) else Color.Transparent)
                                .clickable {
                                    if (selected) {
                                        onOpenUrl(h.url, h.title)
                                    } else {
                                        selectedHeadline = h
                                    }
                                }
                                .padding(vertical = 8.dp, horizontal = 4.dp),
                        ) {
                            Text(h.title, style = MaterialTheme.typography.titleMedium)
                            val meta = buildString {
                                append(h.source)
                                h.published?.let { append(" · "); append(it.take(16).replace('T', ' ')) }
                            }
                            Text(
                                text = meta,
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                            plainTextSummary(h.summary)?.let { summary ->
                                Text(
                                    text = summary.take(280),
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    maxLines = 3,
                                    overflow = TextOverflow.Ellipsis,
                                    modifier = Modifier.padding(top = 4.dp),
                                )
                            }
                        }
                        HorizontalDivider()
                    }
                }
            }
        }

        val detail = selectedHeadline
        if (detail != null) {
            Column(
                modifier = Modifier
                    .widthIn(min = 280.dp, max = 420.dp)
                    .fillMaxHeight()
                    .background(Color(0xFF161A22))
                    .padding(20.dp)
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Text("STORY", style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
                Text(detail.title, style = MaterialTheme.typography.titleLarge)
                Text(
                    text = buildString {
                        append(detail.source)
                        detail.published?.let { append(" · "); append(it.take(16).replace('T', ' ')) }
                    },
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                HorizontalDivider()
                Text(
                    text = plainTextSummary(detail.summary)
                        ?: "No summary in feed — open the article for full text.",
                    style = MaterialTheme.typography.bodyLarge,
                )
                Text(
                    text = "Open article",
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier
                        .clickable { onOpenUrl(detail.url, detail.title) }
                        .padding(vertical = 12.dp),
                )
            }
        }
    }
}
