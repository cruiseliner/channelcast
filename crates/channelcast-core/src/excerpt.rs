//! Fetch short prose excerpts for articles whose feed summary is missing or thin
//! (e.g. Hacker News metadata-only descriptions).
//!
//! Uses public HTML only: og:description → meta description → first paragraph.
//! Short timeouts; never blocks the channel on a slow host.

use crate::model::Article;
use crate::sanitize::{is_hn_comments_url, is_valid_article_url, strip_tags};
use std::time::Duration;

const USER_AGENT: &str =
    "ChannelCast/0.1 (+https://github.com/cruiseliner/channelcast; excerpt enrich)";

/// Prefer excerpts at least this long (Fightful-style blurbs).
pub const MIN_GOOD_EXCERPT: usize = 80;

const FETCH_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_FETCHES: usize = 18;
const PAUSE_BETWEEN: Duration = Duration::from_millis(80);

/// True when the list/reader should try to replace this summary with page prose.
pub fn needs_excerpt(article: &Article) -> bool {
    if !is_valid_article_url(&article.url) || is_hn_comments_url(&article.url) {
        return false;
    }
    match article.summary.as_deref() {
        None => true,
        Some(s) => is_thin_summary(s),
    }
}

fn is_thin_summary(s: &str) -> bool {
    let t = s.trim();
    let lower = t.to_ascii_lowercase();
    // Always replace synthetic HN / placeholder lines.
    if lower.contains("points on hacker news")
        || lower.contains("discussed on hacker news")
        || lower.contains("no summary in feed")
    {
        return true;
    }
    // "From host · N points …" synthetic HN line
    if lower.starts_with("from ") && lower.contains(" · ") && lower.contains("point") {
        return true;
    }
    // Short stubs only — leave ~60+ char RSS blurbs (Ars) alone so HN can use the budget.
    t.len() < 60
}

/// Replace thin/missing summaries in-place for up to `MAX_FETCHES` articles.
pub fn enrich_thin_summaries(articles: &mut [Article]) {
    let client = match reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(FETCH_TIMEOUT)
        .redirect(reqwest::redirect::Policy::limited(6))
        .build()
    {
        Ok(c) => c,
        Err(_) => return,
    };

    let mut fetched = 0usize;
    for article in articles.iter_mut() {
        if fetched >= MAX_FETCHES {
            break;
        }
        if !needs_excerpt(article) {
            continue;
        }
        match fetch_excerpt(&client, &article.url) {
            Some(excerpt) => {
                article.summary = Some(excerpt);
                fetched += 1;
            }
            None => {
                fetched += 1; // count attempts so one bad host cannot burn the whole budget
            }
        }
        std::thread::sleep(PAUSE_BETWEEN);
    }
}

fn fetch_excerpt(client: &reqwest::blocking::Client, url: &str) -> Option<String> {
    let resp = client
        .get(url)
        .header(
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .send()
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let bytes = resp.bytes().ok()?;
    // Cap read — meta tags live in the head.
    let slice = if bytes.len() > 512_000 {
        &bytes[..512_000]
    } else {
        &bytes[..]
    };
    let html = std::str::from_utf8(slice).ok()?;
    extract_excerpt_from_html(html)
}

/// Public for unit tests.
pub fn extract_excerpt_from_html(html: &str) -> Option<String> {
    meta_content(html, "property", "og:description")
        .or_else(|| meta_content(html, "name", "description"))
        .or_else(|| meta_content(html, "name", "twitter:description"))
        .or_else(|| json_ld_description(html))
        .or_else(|| first_paragraph(html))
        .and_then(|s| finalize_excerpt(&s))
}

fn finalize_excerpt(raw: &str) -> Option<String> {
    let t = collapse_ws(&decode_basic_entities(raw));
    if t.len() < 40 {
        return None;
    }
    let lower = t.to_ascii_lowercase();
    // Never keep HN/link-soup meta as an "excerpt".
    if lower.contains("points on hacker news")
        || lower.contains("discussed on hacker news")
        || lower.contains("comments url")
        || lower.contains("article url:")
    {
        return None;
    }
    // Keep list blurbs readable; full body stays in the reader when prefetch works.
    if t.len() > 600 {
        Some(format!("{}…", t.chars().take(597).collect::<String>().trim_end()))
    } else {
        Some(t)
    }
}

fn meta_content(html: &str, attr: &str, value: &str) -> Option<String> {
    // <meta property="og:description" content="…">
    let lower = html.to_ascii_lowercase();
    let needle = format!("{attr}=\"{value}\"");
    let needle2 = format!("{attr}='{value}'");
    let idx = lower
        .find(&needle)
        .or_else(|| lower.find(&needle2))?;
    // Search backward for <meta and forward for content=
    let start = html[..idx].rfind("<meta").unwrap_or(idx);
    let window = &html[start..(start + 1200).min(html.len())];
    content_attr(window)
}

fn content_attr(tag_window: &str) -> Option<String> {
    let lower = tag_window.to_ascii_lowercase();
    let key = "content=";
    let i = lower.find(key)?;
    let after = tag_window[i + key.len()..].trim_start();
    let quote = after.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let body = &after[1..];
    let end = body.find(quote)?;
    Some(body[..end].to_string())
}

fn json_ld_description(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let mut search_from = 0;
    while let Some(rel) = lower[search_from..].find("\"description\"") {
        let i = search_from + rel;
        let after = &html[i + "\"description\"".len()..];
        let after_trim = after.trim_start();
        if !after_trim.starts_with(':') {
            search_from = i + 1;
            continue;
        }
        let rest = after_trim[1..].trim_start();
        if !rest.starts_with('"') {
            search_from = i + 1;
            continue;
        }
        let mut out = String::new();
        let mut chars = rest[1..].chars();
        while let Some(c) = chars.next() {
            match c {
                '"' => break,
                '\\' => {
                    if let Some(n) = chars.next() {
                        match n {
                            'n' => out.push(' '),
                            'r' | 't' => out.push(' '),
                            '"' | '\\' | '/' => out.push(n),
                            'u' => {
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
        if t.len() >= 40 {
            return Some(t);
        }
        search_from = i + 1;
    }
    None
}

fn first_paragraph(html: &str) -> Option<String> {
    let mut rest = html;
    for _ in 0..40 {
        let Some(idx) = rest.to_ascii_lowercase().find("<p") else {
            return None;
        };
        let from = &rest[idx..];
        let Some(gt) = from.find('>') else {
            return None;
        };
        let inner_start = &from[gt + 1..];
        let Some(end) = inner_start.to_ascii_lowercase().find("</p>") else {
            rest = &from[1..];
            continue;
        };
        let inner = &inner_start[..end];
        rest = &inner_start[end + 4..];
        let text = collapse_ws(&strip_tags(inner));
        if text.len() < 60 {
            continue;
        }
        let lower = text.to_ascii_lowercase();
        if lower.contains("skip to")
            || lower.contains("cookie")
            || lower.contains("subscribe")
            || lower.contains("sign in")
            || lower.contains("javascript")
        {
            continue;
        }
        return Some(text);
    }
    None
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn decode_basic_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .replace("&mdash;", "—")
        .replace("&ndash;", "–")
        .replace("&rsquo;", "’")
        .replace("&lsquo;", "‘")
        .replace("&rdquo;", "”")
        .replace("&ldquo;", "“")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn og_description_preferred() {
        let html = r#"<html><head>
<meta property="og:description" content="A solid blurb about the story that is long enough to keep in the channel list pane." />
<meta name="description" content="weaker" />
</head><body><p>Not used when og exists and is long enough for the threshold.</p></body></html>"#;
        let e = extract_excerpt_from_html(html).unwrap();
        assert!(e.contains("solid blurb"));
    }

    #[test]
    fn thin_hn_meta_needs_excerpt() {
        let a = Article {
            title: "t".into(),
            url: "https://example.com/post".into(),
            summary: Some("From example.com · 42 points on Hacker News · 10 comments".into()),
            published: None,
            source: "HN".into(),
            channel_id: "tech".into(),
        };
        assert!(needs_excerpt(&a));
    }

    #[test]
    fn good_prose_skips_excerpt() {
        let blurb = "Amber Nova bonded out and has a compliance date set for next month after the incident.";
        assert!(blurb.len() >= MIN_GOOD_EXCERPT);
        let a = Article {
            title: "t".into(),
            url: "https://www.fightful.com/a".into(),
            summary: Some(blurb.into()),
            published: None,
            source: "Fightful".into(),
            channel_id: "prowrestling".into(),
        };
        assert!(!needs_excerpt(&a));
    }
}
