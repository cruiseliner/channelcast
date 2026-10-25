//! ChannelCast core library: channel config, RSS/Atom fetch, SQLite cache, ranking.
//! Minimal NWS forecast JSON + personal Gannett (Florida Today / USA Today) HTML/sitemap support.

pub mod cache;
pub mod config;
pub mod error;
pub mod fetch;
pub mod floridatoday;
pub mod usatoday;
pub mod model;
pub mod nws;
pub mod rank;
pub mod sanitize;
pub mod excerpt;

pub use cache::Cache;
pub use config::{filter_by_keywords, Channel, ChannelConfig};
pub use error::{Error, Result};
pub use fetch::{fetch_channel_feeds, fetch_feed, parse_feed_bytes};
pub use floridatoday::{
    is_floridatoday_url, parse_article_body_bytes, parse_listing_bytes, parse_sitemap_bytes,
};
pub use usatoday::{
    filter_for_url as usatoday_filter_for_url, is_usatoday_url,
    parse_article_body_bytes as parse_usatoday_article_body_bytes,
    parse_listing_bytes as parse_usatoday_listing_bytes,
    parse_sitemap_bytes as parse_usatoday_sitemap_bytes, SitemapFilter as UsaTodaySitemapFilter,
};
pub use model::Article;
pub use nws::{is_nws_forecast_url, parse_nws_forecast_bytes};
pub use rank::dedupe_and_rank;
pub use sanitize::{clean_summary, is_valid_article_url, sanitize_article, sanitize_articles, strip_tags};
pub use excerpt::{enrich_thin_summaries, extract_excerpt_from_html, needs_excerpt, MIN_GOOD_EXCERPT};

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
    let cleaned = sanitize_articles(raw);
    let filtered = filter_by_keywords(cleaned, &channel.keywords);
    let mut ranked = dedupe_and_rank(filtered);

    // Pull og:description / meta for thin feed blurbs (HN meta, bare sitemaps).
    let enrich_n = ranked.len().min(limit.max(12));
    if enrich_n > 0 {
        excerpt::enrich_thin_summaries(&mut ranked[..enrich_n]);
    }

    if let Some(path) = cache_path {
        let cache = Cache::open(path)?;
        cache.upsert_articles(&ranked)?;
    }

    Ok(ranked.into_iter().take(limit).collect())
}
