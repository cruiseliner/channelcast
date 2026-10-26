package com.cruiseliner.channelcast

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
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
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import coil.compose.AsyncImage
import uniffi.channelcast_ffi.FfiWeatherDashboard
import uniffi.channelcast_ffi.FfiWeatherDay
import uniffi.channelcast_ffi.FfiWeatherGraphic
import uniffi.channelcast_ffi.FfiWeatherHour
import uniffi.channelcast_ffi.FfiWeatherNow
import uniffi.channelcast_ffi.FfiWeatherTropical

/**
 * Weather tab B2 stack for phone (and reusable pieces for TV):
 * Now → hourly → 3-day → cover-cropped MLB graphicast hazards → optional NHC tropical.
 * Condition icons are live NWS icon PNGs (sun / cloud / rain glyphs), never placeholder dots.
 */
@Composable
fun WeatherDashboardContent(
    dash: FfiWeatherDashboard?,
    loading: Boolean,
    error: String?,
    onOpenUrl: (String, String?, String?) -> Unit,
    contentPadding: PaddingValues,
    phoneDensity: Boolean = true,
) {
    val card = Color(0xFF1C1C1E)
    val onSurface = Color(0xFFF5F5F7)
    val muted = Color(0xFF8E8E93)
    val accent = Color(0xFF9AB0D0)
    val alertAmber = Color(0xFFFFB74D)

    when {
        loading && dash == null && error == null -> {
            Box(
                Modifier.fillMaxSize().padding(contentPadding),
                contentAlignment = Alignment.Center,
            ) {
                CircularProgressIndicator(color = accent)
            }
        }
        error != null && dash == null -> {
            Box(
                Modifier.fillMaxSize().padding(24.dp).padding(contentPadding),
                contentAlignment = Alignment.Center,
            ) {
                Text(error, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyLarge)
            }
        }
        dash != null -> {
            LazyColumn(
                contentPadding = contentPadding,
                verticalArrangement = Arrangement.spacedBy(if (phoneDensity) 12.dp else 16.dp),
                modifier = Modifier.fillMaxSize(),
            ) {
                if (dash.alertLabels.isNotEmpty()) {
                    item(key = "alerts") {
                        AlertChipRow(labels = dash.alertLabels, accent = alertAmber)
                    }
                }
                item(key = "now") {
                    NowCard(now = dash.now, card = card, onSurface = onSurface, muted = muted, accent = accent)
                }
                item(key = "hourly") {
                    HourlyStrip(hours = dash.hourly, card = card, onSurface = onSurface, muted = muted)
                }
                item(key = "daily") {
                    ThreeDayCard(days = dash.daily, card = card, onSurface = onSurface, muted = muted)
                }
                items(dash.graphics, key = { "g-" + it.imageUrl }) { g ->
                    HazardGraphicCard(
                        graphic = g,
                        card = card,
                        onSurface = onSurface,
                        muted = muted,
                        onClick = { onOpenUrl(g.linkUrl, g.title, g.description) },
                    )
                }
                items(dash.tropical, key = { "t-" + it.articleUrl }) { t ->
                    TropicalCard(
                        tropical = t,
                        card = card,
                        onSurface = onSurface,
                        muted = muted,
                        onOpenGraphic = { onOpenUrl(t.imageUrl, t.title, t.summary) },
                        onOpenArticle = { onOpenUrl(t.articleUrl, t.articleTitle, t.summary) },
                    )
                }
                item(key = "src") {
                    Text(
                        text = "NWS Melbourne · ZIP 32920",
                        style = MaterialTheme.typography.labelSmall,
                        color = muted,
                        modifier = Modifier.padding(horizontal = 4.dp, vertical = 8.dp),
                    )
                }
            }
        }
        else -> {
            Box(
                Modifier.fillMaxSize().padding(24.dp).padding(contentPadding),
                contentAlignment = Alignment.Center,
            ) {
                Text("No weather yet — pull to refresh.", color = muted)
            }
        }
    }
}

@Composable
private fun AlertChipRow(labels: List<String>, accent: Color) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState()),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        labels.forEach { label ->
            Text(
                text = label,
                color = Color(0xFF1A1208),
                style = MaterialTheme.typography.labelMedium,
                fontWeight = FontWeight.SemiBold,
                modifier = Modifier
                    .clip(RoundedCornerShape(20.dp))
                    .background(accent)
                    .padding(horizontal = 12.dp, vertical = 6.dp),
            )
        }
    }
}

@Composable
private fun NowCard(
    now: FfiWeatherNow,
    card: Color,
    onSurface: Color,
    muted: Color,
    accent: Color,
) {
    Card(
        shape = RoundedCornerShape(18.dp),
        colors = CardDefaults.cardColors(containerColor = card),
        elevation = CardDefaults.cardElevation(defaultElevation = 0.dp),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 18.dp, vertical = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f)) {
                Text(
                    text = now.location,
                    style = MaterialTheme.typography.labelLarge,
                    color = accent,
                )
                Spacer(Modifier.height(4.dp))
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        text = "${now.temperatureF}°",
                        style = MaterialTheme.typography.displaySmall,
                        fontWeight = FontWeight.Bold,
                        color = onSurface,
                        letterSpacing = (-1).sp,
                    )
                    Spacer(Modifier.width(12.dp))
                    Column {
                        Text(
                            text = now.condition,
                            style = MaterialTheme.typography.titleMedium,
                            fontWeight = FontWeight.SemiBold,
                            color = onSurface,
                            maxLines = 2,
                            overflow = TextOverflow.Ellipsis,
                        )
                        Text(
                            text = buildString {
                                append("Wind ${now.wind}")
                                now.humidityPct?.let { append(" · Humidity $it%") }
                            },
                            style = MaterialTheme.typography.bodySmall,
                            color = muted,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
            }
            WeatherIcon(url = now.iconUrl, size = 64.dp, condition = now.condition)
        }
    }
}

@Composable
private fun HourlyStrip(
    hours: List<FfiWeatherHour>,
    card: Color,
    onSurface: Color,
    muted: Color,
) {
    Card(
        shape = RoundedCornerShape(16.dp),
        colors = CardDefaults.cardColors(containerColor = card),
        elevation = CardDefaults.cardElevation(defaultElevation = 0.dp),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Column(Modifier.padding(vertical = 12.dp)) {
            Text(
                text = "Hourly",
                style = MaterialTheme.typography.labelLarge,
                color = muted,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
            )
            LazyRow(
                contentPadding = PaddingValues(horizontal = 12.dp),
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                items(hours, key = { it.label + it.temperatureF + it.condition }) { h ->
                    Column(
                        horizontalAlignment = Alignment.CenterHorizontally,
                        modifier = Modifier
                            .width(64.dp)
                            .padding(vertical = 4.dp),
                    ) {
                        Text(h.label, style = MaterialTheme.typography.labelSmall, color = muted, maxLines = 1)
                        Spacer(Modifier.height(6.dp))
                        WeatherIcon(url = h.iconUrl, size = 32.dp, condition = h.condition)
                        Spacer(Modifier.height(6.dp))
                        Text(
                            "${h.temperatureF}°",
                            style = MaterialTheme.typography.titleMedium,
                            fontWeight = FontWeight.SemiBold,
                            color = onSurface,
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun ThreeDayCard(
    days: List<FfiWeatherDay>,
    card: Color,
    onSurface: Color,
    muted: Color,
) {
    Card(
        shape = RoundedCornerShape(16.dp),
        colors = CardDefaults.cardColors(containerColor = card),
        elevation = CardDefaults.cardElevation(defaultElevation = 0.dp),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Column(Modifier.padding(horizontal = 14.dp, vertical = 12.dp)) {
            Text(
                text = "3-day",
                style = MaterialTheme.typography.labelLarge,
                color = muted,
                modifier = Modifier.padding(bottom = 8.dp),
            )
            days.forEachIndexed { idx, day ->
                DayRow(day = day, onSurface = onSurface, muted = muted)
                if (idx != days.lastIndex) {
                    Spacer(Modifier.height(10.dp))
                }
            }
        }
    }
}

@Composable
private fun DayRow(day: FfiWeatherDay, onSurface: Color, muted: Color) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Text(
            text = day.name,
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.SemiBold,
            color = onSurface,
            modifier = Modifier.width(56.dp),
        )
        WeatherIcon(url = day.iconUrl, size = 28.dp, condition = day.condition)
        Spacer(Modifier.width(10.dp))
        Text(
            text = day.condition,
            style = MaterialTheme.typography.bodyMedium,
            color = muted,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        Text(
            text = "${day.highF}°",
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.SemiBold,
            color = onSurface,
        )
        Spacer(Modifier.width(10.dp))
        Text(
            text = "${day.lowF}°",
            style = MaterialTheme.typography.titleMedium,
            color = muted,
        )
    }
}

@Composable
private fun HazardGraphicCard(
    graphic: FfiWeatherGraphic,
    card: Color,
    onSurface: Color,
    muted: Color,
    onClick: () -> Unit,
) {
    Card(
        onClick = onClick,
        shape = RoundedCornerShape(16.dp),
        colors = CardDefaults.cardColors(containerColor = card),
        elevation = CardDefaults.cardElevation(defaultElevation = 0.dp),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Column {
            // Fit B: cover-crop official MLB PNG to phone width.
            AsyncImage(
                model = graphic.imageUrl,
                contentDescription = graphic.title,
                contentScale = ContentScale.Crop,
                modifier = Modifier
                    .fillMaxWidth()
                    .aspectRatio(16f / 10f),
            )
            Column(Modifier.padding(horizontal = 14.dp, vertical = 12.dp)) {
                Text(
                    text = graphic.title,
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                    color = onSurface,
                )
                if (graphic.description.isNotBlank()) {
                    Spacer(Modifier.height(4.dp))
                    Text(
                        text = graphic.description,
                        style = MaterialTheme.typography.bodySmall,
                        color = muted,
                        maxLines = 3,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        }
    }
}

@Composable
private fun TropicalCard(
    tropical: FfiWeatherTropical,
    card: Color,
    onSurface: Color,
    muted: Color,
    onOpenGraphic: () -> Unit,
    onOpenArticle: () -> Unit,
) {
    Card(
        shape = RoundedCornerShape(16.dp),
        colors = CardDefaults.cardColors(containerColor = card),
        elevation = CardDefaults.cardElevation(defaultElevation = 0.dp),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Column {
            AsyncImage(
                model = tropical.imageUrl,
                contentDescription = tropical.title,
                contentScale = ContentScale.Crop,
                modifier = Modifier
                    .fillMaxWidth()
                    .aspectRatio(16f / 10f),
            )
            Column(Modifier.padding(horizontal = 14.dp, vertical = 12.dp)) {
                Text(
                    text = tropical.title,
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.Bold,
                    color = onSurface,
                )
                Spacer(Modifier.height(4.dp))
                Text(tropical.summary, style = MaterialTheme.typography.bodySmall, color = muted)
                Spacer(Modifier.height(10.dp))
                Text(
                    text = tropical.articleTitle,
                    style = MaterialTheme.typography.labelLarge,
                    color = Color(0xFF9AB0D0),
                    modifier = Modifier
                        .clip(RoundedCornerShape(8.dp))
                        .background(Color(0xFF2C2C2E))
                        .clickable(onClick = onOpenArticle)
                        .padding(horizontal = 12.dp, vertical = 8.dp),
                )
                Spacer(Modifier.height(8.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    Text(
                        "Map",
                        color = Color(0xFF9AB0D0),
                        style = MaterialTheme.typography.labelLarge,
                        modifier = Modifier
                            .clip(RoundedCornerShape(8.dp))
                            .background(Color(0xFF2C2C2E))
                            .clickable(onClick = onOpenGraphic)
                            .padding(horizontal = 12.dp, vertical = 8.dp),
                    )
                    Text(
                        "Advisory",
                        color = Color(0xFF9AB0D0),
                        style = MaterialTheme.typography.labelLarge,
                        modifier = Modifier
                            .clip(RoundedCornerShape(8.dp))
                            .background(Color(0xFF2C2C2E))
                            .clickable(onClick = onOpenArticle)
                            .padding(horizontal = 12.dp, vertical = 8.dp),
                    )
                }
            }
        }
    }
}


@Composable
private fun WeatherIcon(url: String?, size: androidx.compose.ui.unit.Dp, condition: String) {
    if (!url.isNullOrBlank()) {
        AsyncImage(
            model = url,
            contentDescription = condition,
            contentScale = ContentScale.Fit,
            modifier = Modifier.size(size),
        )
    } else {
        // Fallback glyph from condition text — never a meaningless blue dot.
        Text(
            text = conditionGlyph(condition),
            fontSize = (size.value * 0.7f).sp,
            modifier = Modifier.size(size),
            color = Color(0xFFFFC857),
        )
    }
}

/** Simple unicode weather glyph when NWS icon URL is missing. */
fun conditionGlyph(condition: String): String {
    val c = condition.lowercase()
    return when {
        "thunder" in c || "tstorm" in c -> "⛈"
        "snow" in c || "sleet" in c || "ice" in c -> "❄"
        "rain" in c || "shower" in c || "drizzle" in c -> "🌧"
        "fog" in c || "haze" in c || "mist" in c -> "🌫"
        "cloud" in c && ("partly" in c || "mostly sunny" in c || "mostly clear" in c) -> "⛅"
        "cloud" in c || "overcast" in c -> "☁"
        "clear" in c || "sunny" in c || "fair" in c -> "☀"
        "wind" in c -> "🌬"
        else -> "🌤"
    }
}
