use crate::error::{Error, Result};
use serde::Deserialize;
use std::path::Path;

/// One subscribed channel (topic) with zero or more feed URLs.
#[derive(Debug, Clone, Deserialize)]
pub struct Channel {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub feeds: Vec<String>,
    /// When false, channel is omitted from default `channels` listing.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Marks a personal / optional channel (shown in listing notes).
    #[serde(default)]
    pub personal: bool,
    /// Optional case-insensitive keyword allowlist (title + summary).
    /// Empty = no filter (keep all fetched items).
    #[serde(default)]
    pub keywords: Vec<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelConfig {
    pub channels: Vec<Channel>,
}

impl ChannelConfig {
    pub fn load_from_str(s: &str) -> Result<Self> {
        let cfg: ChannelConfig = toml::from_str(s)?;
        if cfg.channels.is_empty() {
            return Err(Error::Config("no channels defined".into()));
        }
        Ok(cfg)
    }

    pub fn load_path(path: impl AsRef<Path>) -> Result<Self> {
        let text = std::fs::read_to_string(path.as_ref())?;
        Self::load_from_str(&text)
    }

    /// Built-in defaults (same as repo `channels.toml`).
    pub fn defaults() -> Result<Self> {
        Self::load_from_str(include_str!("../../../channels.toml"))
    }

    pub fn get(&self, id: &str) -> Result<&Channel> {
        self.channels
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| Error::ChannelNotFound(id.to_string()))
    }

    pub fn listed(&self, include_disabled: bool) -> Vec<&Channel> {
        self.channels
            .iter()
            .filter(|c| include_disabled || c.enabled)
            .collect()
    }
}

/// Keep articles whose title or summary contains any keyword (case-insensitive).
/// If `keywords` is empty, returns `articles` unchanged.
pub fn filter_by_keywords(
    articles: Vec<crate::model::Article>,
    keywords: &[String],
) -> Vec<crate::model::Article> {
    if keywords.is_empty() {
        return articles;
    }
    let keys: Vec<String> = keywords.iter().map(|k| k.to_lowercase()).collect();
    articles
        .into_iter()
        .filter(|a| {
            let hay = format!(
                "{} {}",
                a.title,
                a.summary.as_deref().unwrap_or("")
            )
            .to_lowercase();
            keys.iter().any(|k| hay.contains(k))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Article;

    fn art(title: &str, summary: Option<&str>) -> Article {
        Article {
            title: title.into(),
            url: format!("https://ex.com/{}", title.len()),
            summary: summary.map(str::to_string),
            image_url: None,
            published: None,
            source: "t".into(),
            channel_id: "watchman".into(),
        }
    }

    #[test]
    fn keyword_filter_keeps_matches_only() {
        let items = vec![
            art("Market opens higher", None),
            art("Turkey and Syria talks", Some("Ankara")),
            art("Sports scores", Some("red heifer ceremony noted")),
        ];
        let keys = vec!["turkey".into(), "red heifer".into()];
        let out = filter_by_keywords(items, &keys);
        assert_eq!(out.len(), 2);
    }
}
