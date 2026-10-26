//! UniFFI surface for ChannelCast Android (and any future mobile host).
//!
//! Exposes channel listing, headline fetch, and the live Weather dashboard.

uniffi::setup_scaffolding!();

use channelcast_core::{fetch_weather_dashboard, headlines_for_channel, ChannelConfig};

/// Channel metadata for the Android channel strip.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiChannel {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub personal: bool,
}

/// One headline row for Compose lists.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiHeadline {
    pub title: String,
    pub url: String,
    pub source: String,
    pub summary: Option<String>,
    /// Story photo / infographic when MediaRSS, HTML img, or og:image provided one.
    pub image_url: Option<String>,
    /// RFC 3339 UTC when known.
    pub published: Option<String>,
    pub channel_id: String,
}

/// Current conditions (Weather tab "Now").
#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiWeatherNow {
    pub location: String,
    pub temperature_f: i32,
    pub condition: String,
    pub wind: String,
    pub humidity_pct: Option<i32>,
    pub icon_url: Option<String>,
    pub observed_at: Option<String>,
}

/// Hourly strip cell.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiWeatherHour {
    pub label: String,
    pub temperature_f: i32,
    pub condition: String,
    pub icon_url: Option<String>,
    pub precip_pct: Option<i32>,
}

/// One day in the 3-day outlook.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiWeatherDay {
    pub name: String,
    pub high_f: i32,
    pub low_f: i32,
    pub condition: String,
    pub icon_url: Option<String>,
    pub precip_pct: Option<i32>,
}

/// Official MLB graphicast hazard image (Cover-crop in UI).
#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiWeatherGraphic {
    pub title: String,
    pub description: String,
    pub image_url: String,
    pub link_url: String,
}

/// Tropical card — only present when Atlantic/Caribbean/Florida interest exists.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiWeatherTropical {
    pub title: String,
    pub summary: String,
    pub image_url: String,
    pub article_url: String,
    pub article_title: String,
}

/// Full Weather tab payload (B2: Now → hourly → 3-day → graphics → optional tropical).
#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiWeatherDashboard {
    pub now: FfiWeatherNow,
    pub hourly: Vec<FfiWeatherHour>,
    pub daily: Vec<FfiWeatherDay>,
    pub graphics: Vec<FfiWeatherGraphic>,
    /// Zero or one entry — empty when no tropical interest.
    pub tropical: Vec<FfiWeatherTropical>,
    pub alert_labels: Vec<String>,
}

/// Bridge errors surfaced to Kotlin as exceptions.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum ChannelCastFfiError {
    #[error("{msg}")]
    Message { msg: String },
}

impl From<channelcast_core::Error> for ChannelCastFfiError {
    fn from(e: channelcast_core::Error) -> Self {
        ChannelCastFfiError::Message {
            msg: e.to_string(),
        }
    }
}

fn load_config() -> Result<ChannelConfig, ChannelCastFfiError> {
    ChannelConfig::defaults().map_err(Into::into)
}

/// List channels from the built-in `channels.toml`.
///
/// When `include_disabled` is false, only `enabled = true` channels are returned
/// (Watchman / weather / local / spacecoast are enabled in the shipped defaults).
#[uniffi::export]
pub fn list_channels(include_disabled: bool) -> Result<Vec<FfiChannel>, ChannelCastFfiError> {
    let cfg = load_config()?;
    Ok(cfg
        .listed(include_disabled)
        .into_iter()
        .map(|c| FfiChannel {
            id: c.id.clone(),
            name: c.name.clone(),
            description: c.description.clone(),
            enabled: c.enabled,
            personal: c.personal,
        })
        .collect())
}

/// Fetch, filter, and rank headlines for `channel_id`.
///
/// Network I/O is blocking — call from a background thread (Kotlin `Dispatchers.IO`).
/// No on-disk SQLite cache on Android yet (`cache_path` is always `None`).
#[uniffi::export]
pub fn fetch_headlines(
    channel_id: String,
    limit: u32,
) -> Result<Vec<FfiHeadline>, ChannelCastFfiError> {
    let cfg = load_config()?;
    let limit = limit.max(1) as usize;
    let articles = headlines_for_channel(&cfg, &channel_id, limit, None)?;
    Ok(articles
        .into_iter()
        .map(|a| FfiHeadline {
            title: a.title,
            url: a.url,
            source: a.source,
            summary: a.summary,
            image_url: a.image_url,
            published: a.published.map(|t| t.to_rfc3339()),
            channel_id: a.channel_id,
        })
        .collect())
}

/// Live Weather dashboard for home ZIP 32920 / NWS MLB (B2 tab content).
#[uniffi::export]
pub fn fetch_weather() -> Result<FfiWeatherDashboard, ChannelCastFfiError> {
    let d = fetch_weather_dashboard()?;
    Ok(FfiWeatherDashboard {
        now: FfiWeatherNow {
            location: d.now.location,
            temperature_f: d.now.temperature_f,
            condition: d.now.condition,
            wind: d.now.wind,
            humidity_pct: d.now.humidity_pct,
            icon_url: d.now.icon_url,
            observed_at: d.now.observed_at,
        },
        hourly: d
            .hourly
            .into_iter()
            .map(|h| FfiWeatherHour {
                label: h.label,
                temperature_f: h.temperature_f,
                condition: h.condition,
                icon_url: h.icon_url,
                precip_pct: h.precip_pct,
            })
            .collect(),
        daily: d
            .daily
            .into_iter()
            .map(|day| FfiWeatherDay {
                name: day.name,
                high_f: day.high_f,
                low_f: day.low_f,
                condition: day.condition,
                icon_url: day.icon_url,
                precip_pct: day.precip_pct,
            })
            .collect(),
        graphics: d
            .graphics
            .into_iter()
            .map(|g| FfiWeatherGraphic {
                title: g.title,
                description: g.description,
                image_url: g.image_url,
                link_url: g.link_url,
            })
            .collect(),
        tropical: d
            .tropical
            .into_iter()
            .map(|t| FfiWeatherTropical {
                title: t.title,
                summary: t.summary,
                image_url: t.image_url,
                article_url: t.article_url,
                article_title: t.article_title,
            })
            .collect(),
        alert_labels: d.alert_labels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_channel_order_matches_adam_strip() {
        let chans = list_channels(false).expect("config");
        let ids: Vec<&str> = chans.iter().map(|c| c.id.as_str()).collect();
        let expected = [
            "top",
            "local",
            "spacecoast",
            "markets",
            "tech",
            "entertainment",
            "prowrestling",
            "watchman",
            "weather",
        ];
        assert_eq!(ids, expected, "channels.toml order drives phone+TV strip");
    }

    #[test]
    fn lists_enabled_channels_including_watchman() {
        let chans = list_channels(false).expect("config");
        assert!(chans.iter().any(|c| c.id == "watchman"));
        assert!(chans.iter().any(|c| c.id == "spacecoast"));
        assert!(chans.iter().any(|c| c.id == "top"));
    }

    #[test]
    fn fetch_watchman_smoke() {
        let items = fetch_headlines("watchman".into(), 5).expect("fetch");
        assert!(!items.is_empty(), "expected watchman headlines");
        for h in items.iter().take(5) {
            println!("WATCHMAN: {} | {}", h.source, h.title);
            assert!(!h.title.is_empty());
            assert!(!h.url.is_empty());
        }
    }

    #[test]
    fn fetch_weather_smoke() {
        let w = fetch_weather().expect("weather");
        assert!(w.now.temperature_f > -20 && w.now.temperature_f < 130);
        assert!(!w.hourly.is_empty());
        assert!(!w.daily.is_empty());
        assert!(w.tropical.is_empty(), "no Atlantic tropical today");
        println!(
            "WEATHER {}°F {} hourly={} daily={} graphics={}",
            w.now.temperature_f,
            w.now.condition,
            w.hourly.len(),
            w.daily.len(),
            w.graphics.len()
        );
    }
}
