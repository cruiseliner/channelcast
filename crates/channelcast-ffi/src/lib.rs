//! UniFFI surface for ChannelCast Android (and any future mobile host).
//!
//! Exposes channel listing and headline fetch from `channelcast-core`.

uniffi::setup_scaffolding!();

use channelcast_core::{headlines_for_channel, ChannelConfig};

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
