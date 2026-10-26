use crate::error::{Error, Result};
use crate::model::Article;
use crate::image::{first_img_src, normalize_image_url};
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



fn image_url_from_entry(entry: &feed_rs::model::Entry, summary_html: Option<&str>) -> Option<String> {
    for media in &entry.media {
        for thumb in &media.thumbnails {
            if let Some(u) = normalize_image_url(&thumb.image.uri) {
                return Some(u);
            }
        }
        for content in &media.content {
            let mime = content
                .content_type
                .as_ref()
                .map(|m| m.to_string())
                .unwrap_or_default()
                .to_ascii_lowercase();
            if let Some(ref url) = content.url {
                let s = url.as_str();
                let looks_img = mime.starts_with("image/")
                    || s.to_ascii_lowercase().contains(".jpg")
                    || s.to_ascii_lowercase().contains(".jpeg")
                    || s.to_ascii_lowercase().contains(".png")
                    || s.to_ascii_lowercase().contains(".webp")
                    || s.to_ascii_lowercase().contains(".gif");
                if looks_img {
                    if let Some(u) = normalize_image_url(s) {
                        return Some(u);
                    }
                }
            }
        }
    }
    // RSS enclosure / Atom link rel=enclosure with image type, or image-looking href.
    for link in &entry.links {
        let rel = link.rel.as_deref().unwrap_or("").to_ascii_lowercase();
        let mime = link.media_type.as_deref().unwrap_or("").to_ascii_lowercase();
        let href = &link.href;
        let lower = href.to_ascii_lowercase();
        let enclosure = rel == "enclosure" || rel == "related";
        let image_mime = mime.starts_with("image/");
        let image_ext = [".jpg", ".jpeg", ".png", ".webp", ".gif"].iter().any(|e| lower.contains(e));
        if (enclosure && (image_mime || image_ext)) || image_mime || (image_ext && rel.is_empty()) {
            if let Some(u) = normalize_image_url(href) {
                return Some(u);
            }
        }
    }
    if let Some(content) = entry.content.as_ref() {
        if let Some(ref src) = content.src {
            let mime = src.media_type.as_deref().unwrap_or("").to_ascii_lowercase();
            if mime.starts_with("image/") {
                if let Some(u) = normalize_image_url(&src.href) {
                    return Some(u);
                }
            }
        }
        if let Some(ref body) = content.body {
            if let Some(u) = first_img_src(body) {
                return Some(u);
            }
        }
    }
    summary_html.and_then(first_img_src)
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
        let summary = entry
            .summary
            .as_ref()
            .map(|s| s.content.clone())
            .or_else(|| entry.content.as_ref().and_then(|c| c.body.clone()));
        // Prefer MediaRSS thumbnail / image content, then enclosure-like links, then <img> in HTML.
        // Must run before we move `entry.title` / other Option fields.
        let image_url = image_url_from_entry(&entry, summary.as_deref());

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
        let published = entry.published.or(entry.updated);

        let mut article = Article {
            title,
            url,
            summary,
            image_url,
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
