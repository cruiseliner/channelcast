use crate::error::{Error, Result};
use crate::model::Article;
use crate::floridatoday::{fetch_floridatoday, is_floridatoday_url};
use crate::nws::{is_nws_forecast_url, parse_nws_forecast_bytes};
use feed_rs::parser;

const USER_AGENT: &str = "ChannelCast/0.1 (+https://github.com/cruiseliner/channelcast)";

/// Fetch a feed URL over HTTP and parse RSS/Atom — or NWS forecast JSON — into articles.
pub fn fetch_feed(url: &str, channel_id: &str) -> Result<Vec<Article>> {
    if is_floridatoday_url(url) {
        // Personal subscriber HTML/sitemap path (cookie optional; see floridatoday module).
        return fetch_floridatoday(url, channel_id);
    }
    let client = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let mut req = client.get(url);
    if is_nws_forecast_url(url) {
        // NWS prefers an identifying User-Agent (set above) and geo+json Accept.
        req = req.header("Accept", "application/geo+json, application/ld+json, application/json");
    }
    let response = req.send()?.error_for_status()?;
    let bytes = response.bytes()?;
    if is_nws_forecast_url(url) {
        return parse_nws_forecast_bytes(&bytes, url, channel_id);
    }
    parse_feed_bytes(&bytes, url, channel_id)
}

/// Parse RSS/Atom bytes (also used by tests with fixtures).
pub fn parse_feed_bytes(bytes: &[u8], source_url: &str, channel_id: &str) -> Result<Vec<Article>> {
    let feed = parser::parse(bytes).map_err(|e| Error::Feed(e.to_string()))?;
    let source_label = feed
        .title
        .as_ref()
        .map(|t| t.content.clone())
        .unwrap_or_else(|| source_url.to_string());

    let mut out = Vec::with_capacity(feed.entries.len());
    for entry in feed.entries {
        let title = entry
            .title
            .map(|t| t.content)
            .unwrap_or_else(|| "(untitled)".into());
        let url = entry
            .links
            .first()
            .map(|l| l.href.clone())
            .filter(|u| !u.is_empty())
            .or_else(|| {
                let id = entry.id.clone();
                if id.is_empty() || !id.starts_with("http") {
                    None
                } else {
                    Some(id)
                }
            })
            .unwrap_or_default();
        if url.is_empty() {
            continue;
        }
        let summary = entry
            .summary
            .map(|s| s.content)
            .or_else(|| entry.content.and_then(|c| c.body));
        let published = entry.published.or(entry.updated);

        out.push(Article {
            title,
            url,
            summary,
            published,
            source: source_label.clone(),
            channel_id: channel_id.to_string(),
        });
    }
    Ok(out)
}

/// Fetch every feed for a channel and concatenate (no dedupe yet).
pub fn fetch_channel_feeds(feeds: &[String], channel_id: &str) -> Result<Vec<Article>> {
    if feeds.is_empty() {
        return Err(Error::StubChannel(channel_id.to_string()));
    }
    let mut all = Vec::new();
    let mut errors = Vec::new();
    for feed_url in feeds {
        match fetch_feed(feed_url, channel_id) {
            Ok(mut items) => all.append(&mut items),
            Err(e) => errors.push(format!("{feed_url}: {e}")),
        }
    }
    if all.is_empty() && !errors.is_empty() {
        return Err(Error::Feed(errors.join("; ")));
    }
    Ok(all)
}
