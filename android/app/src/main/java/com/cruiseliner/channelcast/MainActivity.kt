package com.cruiseliner.channelcast

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

/**
 * Stub Activity for phone + TV flavors.
 * Deep links: channelcast://open | channelcast://channel/{id}
 * Explicit component for TV launcher: com.cruiseliner.channelcast[.tv]/.MainActivity
 */
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val initial = channelIdFromIntent(intent)
        setContent {
            MaterialTheme {
                Surface(modifier = Modifier.fillMaxSize()) {
                    ChannelCastStubScreen(
                        initialChannelId = initial,
                        onChannelSelected = { id ->
                            Toast.makeText(
                                this,
                                "Stub: would load headlines for \"$id\" via UniFFI",
                                Toast.LENGTH_SHORT,
                            ).show()
                        },
                    )
                }
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        channelIdFromIntent(intent)?.let { id ->
            Toast.makeText(this, "Opened channel: $id", Toast.LENGTH_SHORT).show()
        }
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
}

@Composable
fun ChannelCastStubScreen(
    initialChannelId: String?,
    onChannelSelected: (String) -> Unit,
) {
    var selected by remember { mutableStateOf(initialChannelId) }
    val channels = PlaceholderChannels.all.filter { it.enabledByDefault || it.id == "watchman" }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text("ChannelCast", style = MaterialTheme.typography.headlineLarge)
        Text(
            text = "Android scaffold — core feeds via UniFFI later",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        if (selected != null) {
            Text(
                text = "Deep link channel: $selected",
                style = MaterialTheme.typography.labelLarge,
            )
        }
        Text(
            text = "channelcast://open  ·  channelcast://channel/{id}",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        HorizontalDivider()
        Text("Channels", style = MaterialTheme.typography.titleMedium)
        LazyColumn(
            contentPadding = PaddingValues(vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
            modifier = Modifier.fillMaxWidth(),
        ) {
            items(channels, key = { it.id }) { ch ->
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable {
                            selected = ch.id
                            onChannelSelected(ch.id)
                        }
                        .padding(vertical = 10.dp, horizontal = 4.dp),
                ) {
                    Text(ch.name, style = MaterialTheme.typography.titleMedium)
                    Text(
                        text = "${ch.id} — ${ch.description}",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    }
}
