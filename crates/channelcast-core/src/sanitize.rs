//! Plain-text summary + absolute http(s) URL hygiene for every source.
//!
//! RSS (Fightful, Google News, JPost, …) often ships HTML in `<description>`.
//! Never surface markup in the list/story pane; never treat HTML as a URL.

use crate::model::Article;

/// Clean summary + URL on an article in place. Drop the summary when nothing
/// readable remains after stripping tags / Google News link soup.
pub fn sanitize_article(article: &mut Article) {
    article.title = collapse_ws(&strip_tags(&article.title));
    if article.title.is_empty() {
        article.title = "(untitled)".into();
    }
    article.summary = article
        .summary
        .take()
        .and_then(|s| clean_summary(&s));
    article.url = normalize_article_url(&article.url).unwrap_or_default();
}

/// Sanitize a batch; drop items whose URL is not a usable absolute http(s) link.
pub fn sanitize_articles(mut articles: Vec<Article>) -> Vec<Article> {
    for a in &mut articles {
        sanitize_article(a);
    }
    articles.retain(|a| is_valid_article_url(&a.url));
    articles
}

/// Strip tags, decode entities, collapse whitespace. Empty → None.
/// Also rejects Google News “Article URL:” / bare-anchor soup with no prose.
pub fn clean_summary(raw: &str) -> Option<String> {
    let mut text = collapse_ws(&strip_tags(raw));
    if text.is_empty() {
        return None;
    }
    // Drop leading "Article URL:…" soup (Google News / some CMS embeds), keep any prose after.
    text = strip_article_url_prefix(&text);
    text = collapse_ws(&text);
    if text.is_empty() {
        return None;
    }
    if looks_like_url_only(&text) {
        return None;
    }
    Some(text)
}

fn strip_article_url_prefix(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let Some(idx) = lower.find("article url:") else {
        return text.to_string();
    };
    let after = &text[idx + "article url:".len()..];
    // Skip the URL token(s) that follow, then keep remaining prose.
    let mut rest = after.trim_start();
    // May be "https://…" or bare path; consume until we hit sentence-like prose
    // (space + capital letter word longer than a domain fragment) — simpler: if the
    // remainder still starts with http, strip that one token.
    if rest.to_ascii_lowercase().starts_with("http://") || rest.to_ascii_lowercase().starts_with("https://")
        || rest.to_ascii_lowercase().starts_with("https:/")
        || rest.to_ascii_lowercase().starts_with("http:/")
    {
        if let Some(sp) = rest.find(char::is_whitespace) {
            rest = rest[sp..].trim_start();
        } else {
            rest = "";
        }
    } else {
        // "Article URL:x Rest…" or "Article URL: Rest" — drop first short token if tiny
        if let Some(sp) = rest.find(char::is_whitespace) {
            let first = &rest[..sp];
            if first.len() < 8 || first.contains('/') {
                rest = rest[sp..].trim_start();
            }
        } else if rest.len() < 80 && (rest.contains('/') || rest.contains('.')) {
            rest = "";
        }
    }
    // If "Article URL" was mid-string, also drop the prefix before it when it's empty-ish.
    let before = text[..idx].trim();
    if before.is_empty() {
        rest.to_string()
    } else if rest.is_empty() {
        before.to_string()
    } else {
        format!("{before} {rest}")
    }
}

fn looks_like_url_only(text: &str) -> bool {
    let t = text.trim();
    if t.starts_with("http://") || t.starts_with("https://") {
        // Entire blurb is one URL, or URL + short source name.
        let rest = t
            .split_whitespace()
            .skip(1)
            .collect::<Vec<_>>()
            .join(" ");
        return rest.len() < 40;
    }
    false
}

/// Accept only absolute http(s) URLs with a host; reject HTML fragments / truncation.
pub fn is_valid_article_url(url: &str) -> bool {
    normalize_article_url(url).is_some()
}

/// Trim, reject markup, require `http://` or `https://` with a non-empty host.
/// Returns a cleaned owned String, or None if unusable.
pub fn normalize_article_url(url: &str) -> Option<String> {
    let u = url.trim();
    if u.is_empty() || u.contains('<') || u.contains('>') || u.contains('"') {
        return None;
    }
    // Truncation smell: "https:/" or "http:/" without the second slash+host.
    let lower = u.to_ascii_lowercase();
    if lower == "https:/" || lower == "http:/" || lower == "https://" || lower == "http://" {
        return None;
    }
    let (scheme, rest) = if let Some(r) = u.strip_prefix("https://") {
        ("https://", r)
    } else if let Some(r) = u.strip_prefix("http://") {
        ("http://", r)
    } else {
        return None;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    if host.is_empty() || !host.contains('.') {
        // Allow localhost-style only if dotted; reject empty / garbage hosts.
        return None;
    }
    // Reject whitespace inside the URL.
    if u.chars().any(|c| c.is_whitespace()) {
        return None;
    }
    Some(format!("{scheme}{rest}"))
}

/// Remove HTML tags; decode a small set of entities. Unclosed tags drop the rest
/// of the tag-looking span until `>` or end (so truncated markup does not leak).
pub fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => {
                in_tag = true;
                // Keep a word break when tags sit between text nodes.
                if out.chars().last().is_some_and(|p| !p.is_whitespace()) {
                    out.push(' ');
                }
            }
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    decode_entities(&out)
}

fn decode_entities(s: &str) -> String {
    let mut out = s
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#34;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&mdash;", "—")
        .replace("&ndash;", "–")
        .replace("&rsquo;", "’")
        .replace("&lsquo;", "‘")
        .replace("&rdquo;", "”")
        .replace("&ldquo;", "“");
    // Numeric &#NNN; and &#xHH;
    out = decode_numeric_entities(&out);
    out
}

fn decode_numeric_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("&#") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 2..];
        let (hex, digits) = if after.as_bytes().first() == Some(&b'x') || after.as_bytes().first() == Some(&b'X')
        {
            (true, &after[1..])
        } else {
            (false, after)
        };
        let end = digits.find(';');
        if let Some(e) = end {
            let num = &digits[..e];
            let cp = if hex {
                u32::from_str_radix(num, 16).ok()
            } else {
                num.parse::<u32>().ok()
            };
            if let Some(ch) = cp.and_then(char::from_u32) {
                out.push(ch);
                rest = &digits[e + 1..];
                continue;
            }
        }
        // Not a valid entity — keep the '&#' literally and advance one char.
        out.push_str("&#");
        rest = after;
    }
    out.push_str(rest);
    out
}

pub fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_fightful_style_paragraph() {
        let s = clean_summary("<p>Amber Nova bonded out and has a compliance date set.</p>").unwrap();
        assert_eq!(s, "Amber Nova bonded out and has a compliance date set.");
        assert!(!s.contains('<'));
    }

    #[test]
    fn drops_google_news_article_url_soup() {
        let raw = r#"<p>Article URL:<a href="https://news.google.com/rss/articles/CBMiabc">https://example.com/story</a></p>"#;
        assert!(clean_summary(raw).is_none());
        let with_blurb = r#"<p>Article URL:<a href="https:/broken">x</a></p><p>Real blurb about the story.</p>"#;
        assert_eq!(
            clean_summary(with_blurb).as_deref(),
            Some("Real blurb about the story.")
        );
    }

    #[test]
    fn drops_anchor_only_gnews_desc() {
        let raw = r#"<a href="https://news.google.com/rss/articles/CBMiabc">Title here</a>&nbsp;&nbsp;<font>Israel365</font>"#;
        // After strip: "Title here Israel365" — that IS useful prose, keep it.
        let s = clean_summary(raw).unwrap();
        assert!(s.contains("Title here"));
        assert!(!s.contains('<'));
    }

    #[test]
    fn rejects_truncated_https() {
        assert!(normalize_article_url("https:/").is_none());
        assert!(normalize_article_url("https://").is_none());
        assert!(normalize_article_url("<a href=\"https://x.com\">").is_none());
        assert_eq!(
            normalize_article_url("https://www.fightful.com/wrestling-news/foo/").as_deref(),
            Some("https://www.fightful.com/wrestling-news/foo/")
        );
    }

    #[test]
    fn sanitize_drops_bad_url_articles() {
        let arts = sanitize_articles(vec![
            Article {
                title: "Good".into(),
                url: "https://example.com/a".into(),
                summary: Some("<p>Hi</p>".into()),
                published: None,
                source: "t".into(),
                channel_id: "tech".into(),
            },
            Article {
                title: "Bad".into(),
                url: "https:/".into(),
                summary: Some("x".into()),
                published: None,
                source: "t".into(),
                channel_id: "tech".into(),
            },
        ]);
        assert_eq!(arts.len(), 1);
        assert_eq!(arts[0].summary.as_deref(), Some("Hi"));
    }
}
