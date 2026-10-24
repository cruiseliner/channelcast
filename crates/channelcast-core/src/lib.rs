//! ChannelCast core library: channel config, RSS/Atom fetch, SQLite cache, ranking.

pub mod cache;
pub mod config;
pub mod error;
pub mod fetch;
pub mod model;
pub mod rank;

pub use cache::Cache;
pub use config::{filter_by_keywords, Channel, ChannelConfig};
pub use error::{Error, Result};
pub use fetch::{fetch_channel_feeds, fetch_feed, parse_feed_bytes};
pub use model::Article;
pub use rank::dedupe_and_rank;

use std::path::Path;

/// Load config, fetch a channel's feeds, dedupe/rank, optional keyword filter, optional cache.
pub fn headlines_for_channel(
    config: &ChannelConfig,
    channel_id: &str,
    limit: usize,
    cache_path: Option<&Path>,
) -> Result<Vec<Article>> {
    let channel = config.get(channel_id)?;
    let raw = fetch_channel_feeds(&channel.feeds, &channel.id)?;
    let filtered = filter_by_keywords(raw, &channel.keywords);
    let ranked = dedupe_and_rank(filtered);

    if let Some(path) = cache_path {
        let cache = Cache::open(path)?;
        cache.upsert_articles(&ranked)?;
    }

    Ok(ranked.into_iter().take(limit).collect())
}
