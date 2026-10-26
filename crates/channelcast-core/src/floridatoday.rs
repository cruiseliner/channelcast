//! Personal Florida Today (Gannett) HTML + news-sitemap source.
//!
//! Titles/summaries come from the public `/space/` listing or `news-sitemap.xml`
//! (space-path filter). Optional subscriber Cookie unlocks fuller article body
//! text for local reading. Never commit cookies or passwords — see README.

use crate::error::{Error, Result};
use crate::model::Article;
use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime, Utc};
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

const USER_AGENT: &str =
    "ChannelCast/0.1 (+https://github.com/cruiseliner/channelcast; personal subscriber use)";

const SOURCE_LABEL: &str = "Florida Today";

/// Max article pages to enrich with body text when a cookie is present.
const MAX_BODY_FETCHES: usize = 8;

/// Polite pause between authenticated article fetches.
const BODY_FETCH_PAUSE: Duration = Duration::from_millis(400);

/// True for Florida Today listing, section, or news-sitemap URLs we handle as HTML/XML
/// (not RSS).
pub fn is_floridatoday_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.contains("floridatoday.com")
        && (lower.contains("news-sitemap.xml")
            || lower.contains("/space/")
            || lower.contains("/space?")
            || lower.ends_with("/space"))
}

fn is_sitemap_url(url: &str) -> bool {
    url.to_ascii_lowercase().contains("news-sitemap.xml")
}

/// Resolve optional subscriber Cookie header.
///
/// Precedence:
/// 1. `CHANNELCAST_FLORIDATODAY_COOKIE` — raw `Cookie` header value
/// 2. `CHANNELCAST_FLORIDATODAY_COOKIE_FILE` — path to Netscape cookie file or raw header
/// 3. `.channelcast/floridatoday.cookies` (cwd) or `~/.channelcast/floridatoday.cookies`
pub fn resolve_cookie() -> Option<String> {
    if let Ok(v) = std::env::var("CHANNELCAST_FLORIDATODAY_COOKIE") {
        let t = v.trim().to_string();
        if !t.is_empty() {
            return Some(strip_cookie_prefix(&t));
        }
    }
    if let Ok(path) = std::env::var("CHANNELCAST_FLORIDATODAY_COOKIE_FILE") {
        if let Some(c) = load_cookie_file(Path::new(&path)) {
            return Some(c);
        }
        eprintln!(
            "channelcast: Florida Today cookie file not readable: {path} (continuing without auth)"
        );
    }
    for candidate in default_cookie_paths() {
        if let Some(c) = load_cookie_file(&candidate) {
            return Some(c);
        }
    }
    None
}

fn default_cookie_paths() -> Vec<PathBuf> {
    let mut out = vec![
        PathBuf::from(".channelcast/floridatoday.cookies"),
        PathBuf::from("floridatoday.cookies"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        out.push(PathBuf::from(home).join(".channelcast/floridatoday.cookies"));
    }
    out
}

fn strip_cookie_prefix(s: &str) -> String {
    let t = s.trim();
    if let Some(rest) = t
        .strip_prefix("Cookie:")
        .or_else(|| t.strip_prefix("cookie:"))
    {
        rest.trim().to_string()
    } else {
        t.to_string()
    }
}

fn load_cookie_file(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Netscape cookie jar (# Netscape / tab-separated) or raw Cookie header.
    if trimmed.contains('\t') || trimmed.starts_with('#') {
        return netscape_to_cookie_header(trimmed);
    }
    Some(strip_cookie_prefix(trimmed))
}

fn netscape_to_cookie_header(text: &str) -> Option<String> {
    let mut pairs = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 7 {
            continue;
        }
        let domain = cols[0].to_ascii_lowercase();
        if !domain.contains("floridatoday.com") && !domain.contains("gannett") {
            continue;
        }
        let name = cols[5].trim();
        let value = cols[6].trim();
        if name.is_empty() {
            continue;
        }
        pairs.push(format!("{name}={value}"));
    }
    if pairs.is_empty() {
        None
    } else {
        Some(pairs.join("; "))
    }
}

/// Fetch a Florida Today listing or news-sitemap URL into articles.
pub fn fetch_floridatoday(url: &str, channel_id: &str) -> Result<Vec<Article>> {
    let cookie = resolve_cookie();
    if cookie.is_none() {
        eprintln!(
            "channelcast: Florida Today — no subscriber cookie \
             (set CHANNELCAST_FLORIDATODAY_COOKIE or .channelcast/floridatoday.cookies); \
             listing titles only, no full article body"
        );
    }

    let client = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(30))
        .build()?;

    // Fragments are not sent over HTTP; strip for the GET.
    let fetch_url = url.split('#').next().unwrap_or(url);

    let mut req = client.get(fetch_url);
    if let Some(ref c) = cookie {
        req = req.header(reqwest::header::COOKIE, c.as_str());
    }
    let response = req.send()?.error_for_status()?;
    let bytes = response.bytes()?;

    let mut articles = if is_sitemap_url(url) {
        parse_sitemap_bytes(&bytes, channel_id)?
    } else {
        parse_listing_bytes(&bytes, channel_id)?
    };

    if let Some(ref c) = cookie {
        enrich_with_bodies(&client, c, &mut articles)?;
    }

    Ok(articles)
}

fn enrich_with_bodies(
    client: &reqwest::blocking::Client,
    cookie: &str,
    articles: &mut [Article],
) -> Result<()> {
    let n = articles.len().min(MAX_BODY_FETCHES);
    for article in articles.iter_mut().take(n) {
        match fetch_article_body(client, cookie, &article.url) {
            Ok(Some(body)) => {
                // Prefer fuller body; keep existing blurb if body empty.
                if !body.is_empty() {
                    article.summary = Some(body);
                }
            }
            Ok(None) => {}
            Err(e) => {
                eprintln!(
                    "channelcast: Florida Today article body skip {}: {e}",
                    article.url
                );
            }
        }
        thread::sleep(BODY_FETCH_PAUSE);
    }
    Ok(())
}

fn fetch_article_body(
    client: &reqwest::blocking::Client,
    cookie: &str,
    url: &str,
) -> Result<Option<String>> {
    let response = client
        .get(url)
        .header(reqwest::header::COOKIE, cookie)
        .send()?
        .error_for_status()?;
    let bytes = response.bytes()?;
    Ok(parse_article_body_bytes(&bytes))
}

/// Parse `/space/` (or similar) listing HTML into articles.
pub fn parse_listing_bytes(bytes: &[u8], channel_id: &str) -> Result<Vec<Article>> {
    let html = std::str::from_utf8(bytes).map_err(|e| Error::Feed(format!("utf8: {e}")))?;
    parse_listing_html(html, channel_id)
}

fn parse_listing_html(html: &str, channel_id: &str) -> Result<Vec<Article>> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Hero: <a href=/story/... class=gnt_m_he>TITLE<div ... data-c-dt="..."></div></a>
    for caps in regex_anchor_iter(html, "gnt_m_he") {
        push_listing_item(&mut out, &mut seen, caps, channel_id, true);
    }
    // List tiles: class=gnt_m_flm_a with optional data-c-br
    for caps in regex_anchor_iter(html, "gnt_m_flm_a") {
        push_listing_item(&mut out, &mut seen, caps, channel_id, false);
    }

    if out.is_empty() {
        return Err(Error::Feed(
            "floridatoday listing: no story links found (HTML shape may have changed)".into(),
        ));
    }
    Ok(out)
}

struct AnchorCaps {
    href: String,
    attrs: String,
    inner: String,
}

/// Find `<a href=... class=CLASS ...>...</a>` with CLASS as a class token.
fn regex_anchor_iter<'a>(html: &'a str, class_token: &'a str) -> Vec<AnchorCaps> {
    let mut out = Vec::new();
    let mut rest = html;
    let class_needle = class_token;
    while let Some(idx) = rest.find("<a ") {
        let slice = &rest[idx..];
        let Some(end_rel) = find_closing_a(slice) else {
            break;
        };
        let tag = &slice[..end_rel];
        rest = &slice[end_rel..];

        // Must include the class token as a whole class name.
        if !class_attr_contains(tag, class_needle) {
            continue;
        }
        let Some(href) = extract_href(tag) else {
            continue;
        };
        if !is_story_path(&href) {
            continue;
        }
        let abs = absolutize(&href);
        let (open_end, inner) = split_open_inner(tag);
        let attrs = open_end.to_string();
        out.push(AnchorCaps {
            href: abs,
            attrs,
            inner: inner.to_string(),
        });
    }
    out
}

fn class_attr_contains(tag: &str, token: &str) -> bool {
    // Match class=token, class="... token ...", class='...'
    if let Some(i) = tag.find("class=") {
        let after = &tag[i + 6..];
        let value = if after.starts_with('"') {
            after[1..].split('"').next().unwrap_or("")
        } else if after.starts_with('\'') {
            after[1..].split('\'').next().unwrap_or("")
        } else {
            after.split(|c: char| c.is_whitespace() || c == '>').next().unwrap_or("")
        };
        value.split_whitespace().any(|c| c == token)
    } else {
        false
    }
}

fn find_closing_a(slice: &str) -> Option<usize> {
    // Naive but adequate for FT listing anchors (no nested <a>).
    slice.find("</a>").map(|i| i + 4)
}

fn extract_href(tag: &str) -> Option<String> {
    let i = tag.find("href=")?;
    let after = &tag[i + 5..];
    let raw = if after.starts_with('"') {
        after[1..].split('"').next()?.to_string()
    } else if after.starts_with('\'') {
        after[1..].split('\'').next()?.to_string()
    } else {
        after
            .split(|c: char| c.is_whitespace() || c == '>')
            .next()?
            .to_string()
    };
    if raw.is_empty() || raw.starts_with("javascript:") {
        None
    } else {
        Some(raw)
    }
}

fn split_open_inner(tag: &str) -> (&str, &str) {
    if let Some(i) = tag.find('>') {
        (&tag[..=i], &tag[i + 1..tag.len().saturating_sub(4)]) // drop trailing </a> already excluded
    } else {
        (tag, "")
    }
}

fn push_listing_item(
    out: &mut Vec<Article>,
    seen: &mut std::collections::HashSet<String>,
    caps: AnchorCaps,
    channel_id: &str,
    _hero: bool,
) {
    if !seen.insert(caps.href.clone()) {
        return;
    }
    let title = strip_tags(&caps.inner);
    let title = collapse_ws(&title);
    if title.is_empty() {
        return;
    }
    let summary = attr_value(&caps.attrs, "data-c-br").map(|s| collapse_ws(&s));
    let published = attr_value(&caps.inner, "data-c-dt")
        .or_else(|| attr_value(&caps.attrs, "data-c-dt"))
        .and_then(|s| parse_ft_display_date(&s));

    out.push(Article {
        title,
        url: caps.href,
            image_url: None,
        summary,
        published,
        source: SOURCE_LABEL.into(),
        channel_id: channel_id.to_string(),
    });
}

fn attr_value(hay: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=");
    let i = hay.find(&needle)?;
    let after = &hay[i + needle.len()..];
    if after.starts_with('"') {
        Some(after[1..].split('"').next()?.to_string())
    } else if after.starts_with('\'') {
        Some(after[1..].split('\'').next()?.to_string())
    } else {
        Some(
            after
                .split(|c: char| c.is_whitespace() || c == '>')
                .next()?
                .to_string(),
        )
    }
}

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    // decode a few common entities
    out.replace("&amp;", "&")
        .replace("&#34;", "\"")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn absolutize(href: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        href.to_string()
    } else if href.starts_with('/') {
        format!("https://www.floridatoday.com{href}")
    } else {
        format!("https://www.floridatoday.com/{href}")
    }
}

fn is_story_path(href: &str) -> bool {
    let h = href.to_ascii_lowercase();
    h.contains("/story/")
        || h.contains("/videos/")
        || h.contains("/picture-gallery/")
}

/// Keep sitemap / listing URLs that are Space Coast / space desk stories.
pub fn is_space_story_url(url: &str) -> bool {
    let u = url.to_ascii_lowercase();
    if u.contains("/tech/science/space/") {
        return true;
    }
    if u.contains("/space/nasa/") || u.contains("/space/spacex/") {
        return true;
    }
    // Local Brevard pieces that clearly name space orgs / KSC impact.
    if u.contains("/story/news/local/") {
        const KEYS: &[&str] = &[
            "spacex",
            "nasa",
            "kennedy",
            "blue-origin",
            "blueorigin",
            "starship",
            "falcon",
            "artemis",
            "cape-canaveral",
            "merritt-island",
            "launch",
            "rocket",
        ];
        return KEYS.iter().any(|k| u.contains(k));
    }
    false
}

/// Parse Google news-sitemap XML; keep space-related locs.
pub fn parse_sitemap_bytes(bytes: &[u8], channel_id: &str) -> Result<Vec<Article>> {
    let xml = std::str::from_utf8(bytes).map_err(|e| Error::Feed(format!("utf8: {e}")))?;
    parse_sitemap_xml(xml, channel_id)
}

fn parse_sitemap_xml(xml: &str, channel_id: &str) -> Result<Vec<Article>> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<url>") {
        let after = &rest[start..];
        let Some(end) = after.find("</url>") else {
            break;
        };
        let block = &after[..end + 6];
        rest = &after[end + 6..];

        let loc = tag_text(block, "loc").unwrap_or_default();
        let loc = collapse_ws(&loc);
        if loc.is_empty() || !is_space_story_url(&loc) {
            continue;
        }
        let title = tag_text(block, "news:title")
            .or_else(|| tag_text(block, "title"))
            .map(|t| collapse_ws(&t))
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| "(untitled)".into());
        let published = tag_text(block, "news:publication_date")
            .or_else(|| tag_text(block, "lastmod"))
            .and_then(|s| parse_rfc3339_ish(&collapse_ws(&s)));

        out.push(Article {
            title,
            url: loc,
            summary: None,
            image_url: None,
            published,
            source: SOURCE_LABEL.into(),
            channel_id: channel_id.to_string(),
        });
    }
    if out.is_empty() {
        return Err(Error::Feed(
            "floridatoday sitemap: no space-path stories in this snapshot".into(),
        ));
    }
    Ok(out)
}

fn tag_text(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let i = block.find(&open)?;
    let after = &block[i + open.len()..];
    let j = after.find(&close)?;
    Some(after[..j].to_string())
}

/// Extract summary from article HTML: JSON-LD description, else body paragraphs.
pub fn parse_article_body_bytes(bytes: &[u8]) -> Option<String> {
    let html = std::str::from_utf8(bytes).ok()?;
    parse_article_summary(html)
}

fn parse_article_summary(html: &str) -> Option<String> {
    // Prefer concatenated body paragraphs when present (subscriber view).
    let mut paras = Vec::new();
    let mut rest = html;
    while let Some(idx) = rest.find("class=gnt_ar_b_p") {
        // walk back to <p
        let before = &rest[..idx];
        let p_start = before.rfind("<p").unwrap_or(idx);
        let from_p = &rest[p_start..];
        let Some(end) = from_p.find("</p>") else {
            break;
        };
        let p = &from_p[..end + 4];
        rest = &from_p[end + 4..];
        let text = collapse_ws(&strip_tags(p));
        if text.len() > 40 {
            paras.push(text);
        }
        if paras.len() >= 12 {
            break;
        }
    }
    if !paras.is_empty() {
        let joined = paras.join("\n\n");
        // Truncate extremely long bodies for headline cache.
        if joined.len() > 4000 {
            return Some(format!("{}…", &joined[..3999]));
        }
        return Some(joined);
    }

    // Fallback: JSON-LD description / headline context.
    if let Some(desc) = json_string_field(html, "description") {
        return Some(desc);
    }
    None
}

fn json_string_field(html: &str, field: &str) -> Option<String> {
    let needle = format!("\"{field}\":\"");
    let i = html.find(&needle)?;
    let after = &html[i + needle.len()..];
    let mut out = String::new();
    let mut chars = after.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => break,
            '\\' => {
                if let Some(n) = chars.next() {
                    match n {
                        'n' => out.push('\n'),
                        'r' => {}
                        't' => out.push('\t'),
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        'u' => {
                            // skip \uXXXX
                            let hex: String = chars.by_ref().take(4).collect();
                            if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                                if let Some(ch) = char::from_u32(cp) {
                                    out.push(ch);
                                }
                            }
                        }
                        other => out.push(other),
                    }
                }
            }
            _ => out.push(c),
        }
    }
    let t = collapse_ws(&out);
    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}

fn parse_rfc3339_ish(s: &str) -> Option<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    // Truncate oversized fractional seconds if needed.
    if let Some(dot) = s.find('.') {
        let head = &s[..dot];
        if let Some(tz_at) = s[dot..].find('Z').or_else(|| s[dot..].find('+')) {
            let rebuilt = format!("{head}Z");
            if let Ok(dt) = DateTime::parse_from_rfc3339(&rebuilt) {
                return Some(dt.with_timezone(&Utc));
            }
            let _ = tz_at;
        }
    }
    None
}

/// Parse FT display stamps like `5:39 p.m. ET Oct. 23` or `Oct. 22, 2026`.
fn parse_ft_display_date(s: &str) -> Option<DateTime<Utc>> {
    let s = collapse_ws(s);
    let noon = || chrono::NaiveTime::from_hms_opt(12, 0, 0);
    // "Oct. 22, 2026"
    if let Ok(d) = NaiveDate::parse_from_str(&s, "%b. %d, %Y") {
        return Some(NaiveDateTime::new(d, noon()?).and_utc());
    }
    if let Ok(d) = NaiveDate::parse_from_str(&s, "%B %d, %Y") {
        return Some(NaiveDateTime::new(d, noon()?).and_utc());
    }
    // "5:39 p.m. ET Oct. 23" — assume current year (UTC date of run).
    // Loose parse: extract month day.
    let lower = s.to_ascii_lowercase();
    let months = [
        ("jan", 1),
        ("feb", 2),
        ("mar", 3),
        ("apr", 4),
        ("may", 5),
        ("jun", 6),
        ("jul", 7),
        ("aug", 8),
        ("sep", 9),
        ("oct", 10),
        ("nov", 11),
        ("dec", 12),
    ];
    let mut month = None;
    let mut day = None;
    for (name, num) in months {
        if let Some(pos) = lower.find(name) {
            month = Some(num);
            let after = &lower[pos + name.len()..];
            let digits: String = after
                .chars()
                .skip_while(|c| !c.is_ascii_digit())
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if let Ok(d) = digits.parse::<u32>() {
                day = Some(d);
            }
            break;
        }
    }
    let year = Utc::now().year();
    if let (Some(m), Some(d)) = (month, day) {
        if let Some(date) = NaiveDate::from_ymd_opt(year, m, d) {
            return Some(NaiveDateTime::new(date, noon()?).and_utc());
        }
    }
    None
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_ft_urls() {
        assert!(is_floridatoday_url("https://www.floridatoday.com/space/"));
        assert!(is_floridatoday_url(
            "https://www.floridatoday.com/news-sitemap.xml#space"
        ));
        assert!(!is_floridatoday_url("https://spacecoastdaily.com/feed/"));
    }

    #[test]
    fn space_url_filter() {
        assert!(is_space_story_url(
            "https://www.floridatoday.com/story/tech/science/space/2026/10/23/nasa-x/1/"
        ));
        assert!(is_space_story_url(
            "https://www.floridatoday.com/story/news/local/2026/10/24/spacex-launch-lights/1/"
        ));
        assert!(!is_space_story_url(
            "https://www.floridatoday.com/story/news/politics/elections/2026/10/24/x/1/"
        ));
    }

    #[test]
    fn netscape_cookie_parse() {
        let jar = "\
# Netscape HTTP Cookie File
.floridatoday.com\tTRUE\t/\tFALSE\t0\tgup_anon\tabc123
www.floridatoday.com\tFALSE\t/\tTRUE\t0\tsession-id\txyz
.example.com\tTRUE\t/\tFALSE\t0\tother\tnope
";
        let h = netscape_to_cookie_header(jar).unwrap();
        assert!(h.contains("gup_anon=abc123"));
        assert!(h.contains("session-id=xyz"));
        assert!(!h.contains("other=nope"));
    }
}
