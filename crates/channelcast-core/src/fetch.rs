use crate::error::{Error, Result};
use crate::model::Article;
use crate::sanitize::{normalize_article_url, sanitize_article};
use crate::floridatoday::{fetch_floridatoday, is_floridatoday_url};
use crate::usatoday::{fetch_usatoday, is_usatoday_url};
use crate::nws::{is_nws_forecast_url, parse_nws_forecast_bytes};
use feed_rs::parser;

const USER_AGENT: &str = "ChannelCast/0.1 (+https://github.com/cruiseliner/channelcast)";

/// Fetch a feed URL over HTTP and parse RSS/Atom — or NWS forecast JSON — into articles.
pub fn fetch_feed(url: &str, channel_id: &str) -> Result<Vec<Article>> {
    if is_floridatoday_url(url) {
        // Personal subscriber HTML/sitemap path (cookie optional; see floridatoday module).
        return fetch_floridatoday(url, channel_id);
    }
    if is_usatoday_url(url) {
        // Personal Gannett HTML/sitemap path (cookie optional; see usatoday module).
        return fetch_usatoday(url, channel_id);
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
    let mut articles = parse_feed_bytes(&bytes, url, channel_id)?;
    // Resolve Google News redirects to publisher URLs (WebView-friendly).
    if url.to_ascii_lowercase().contains("news.google.com") {
        for a in &mut articles {
            a.url = unwrap_google_news_url(&client, &a.url);
        }
    }
    Ok(articles)
}


/// Follow redirects for Google News article URLs so WebView/open get a publisher page.
fn unwrap_google_news_url(client: &reqwest::blocking::Client, url: &str) -> String {
    let lower = url.to_ascii_lowercase();
    if !(lower.contains("news.google.com/rss/articles/")
        || lower.contains("news.google.com/articles/"))
    {
        return url.to_string();
    }
    match client
        .get(url)
        .header("Accept", "text/html,application/xhtml+xml")
        .send()
    {
        Ok(resp) => {
            let final_url = resp.url().to_string();
            if crate::sanitize::is_valid_article_url(&final_url)
                && !final_url.to_ascii_lowercase().contains("news.google.com")
            {
                final_url
            } else {
                url.to_string()
            }
        }
        Err(_) => url.to_string(),
    }
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
            .and_then(|u| normalize_article_url(&u))
            .unwrap_or_default();
        if url.is_empty() {
            continue;
        }
        let summary = entry
            .summary
            .map(|s| s.content)
            .or_else(|| entry.content.and_then(|c| c.body));
        let published = entry.published.or(entry.updated);

        let mut article = Article {
            title,
            url,
            summary,
            published,
            source: source_label.clone(),
            channel_id: channel_id.to_string(),
        };
        sanitize_article(&mut article);
        if article.url.is_empty() {
            continue;
        }
        out.push(article);
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
