//! Plain-text summary + absolute http(s) URL hygiene for every source.
//!
//! RSS (Fightful, Google News, JPost, …) often ships HTML in `<description>`.
//! Never surface markup in the list/story pane; never treat HTML as a URL.

use crate::model::Article;

/// Clean summary + URL on an article in place. Drop the summary when nothing
/// readable remains after stripping tags / Google News / HN link soup.
pub fn sanitize_article(article: &mut Article) {
    article.title = collapse_ws(&strip_tags(&article.title));
    if article.title.is_empty() {
        article.title = "(untitled)".into();
    }
    // Prefer the real story URL over an HN comments / item?id= link when the
    // feed description embeds `Article URL: …`.
    let raw_summary = article.summary.clone();
    if let Some(ref raw) = raw_summary {
        if let Some(story) = extract_story_url_from_description(raw) {
            if article.url.is_empty() || is_hn_comments_url(&article.url) {
                article.url = story;
            }
        }
    }
    let story_url = article.url.clone();
    article.summary = raw_summary.and_then(|s| {
        clean_summary(&s).or_else(|| hn_fallback_summary(&s, &story_url))
    });
    article.url = normalize_article_url(&article.url).unwrap_or_default();
}

/// When HN RSS is metadata-only (no prose blurb), still give the UI something useful.
fn hn_fallback_summary(raw: &str, story_url: &str) -> Option<String> {
    let lower = raw.to_ascii_lowercase();
    if !(lower.contains("points:") || lower.contains("comments url") || lower.contains("article url")) {
        return None;
    }
    // Only for HN-shaped descriptions — don't invent blurbs for other feeds.
    if !lower.contains("news.ycombinator.com") && !lower.contains("points:") {
        return None;
    }
    let host = url_host(story_url).unwrap_or_else(|| "linked site".into());
    let points = meta_number(raw, "Points:");
    let comments = meta_number(raw, "# Comments:").or_else(|| meta_number(raw, "#Comments:"));
    let mut parts = Vec::new();
    parts.push(format!("From {host}"));
    match points {
        Some(p) => parts.push(format!("{p} points on Hacker News")),
        None => parts.push("Discussed on Hacker News".into()),
    }
    if let Some(c) = comments {
        parts.push(format!("{c} comments"));
    }
    Some(parts.join(" · "))
}

fn meta_number(raw: &str, label: &str) -> Option<u32> {
    let lower = raw.to_ascii_lowercase();
    let lab = label.to_ascii_lowercase();
    let idx = lower.find(&lab)?;
    let after = raw[idx + label.len()..].trim_start();
    let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

fn url_host(url: &str) -> Option<String> {
    let u = url.trim();
    let rest = u.strip_prefix("https://").or_else(|| u.strip_prefix("http://"))?;
    let host = rest.split(['/', '?', '#']).next().unwrap_or("").trim();
    if host.is_empty() {
        None
    } else {
        Some(host.trim_start_matches("www.").to_string())
    }
}

/// Sanitize a batch; drop items whose URL is not a usable absolute http(s) link.
pub fn sanitize_articles(mut articles: Vec<Article>) -> Vec<Article> {
    for a in &mut articles {
        sanitize_article(a);
    }
    articles.retain(|a| is_valid_article_url(&a.url));
    articles
}

/// True when a URL is an HN discussion page (comments), not the linked story.
pub fn is_hn_comments_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    if !(lower.contains("news.ycombinator.com/item")
        || lower.contains("news.ycombinator.com/reply"))
    {
        return false;
    }
    lower.contains("id=") || lower.contains("/item?")
}

/// Pull `Article URL: <a href="…">` / `Article URL: https://…` from HN-style HTML.
pub fn extract_story_url_from_description(raw_html: &str) -> Option<String> {
    let lower = raw_html.to_ascii_lowercase();
    let key = "article url:";
    let idx = lower.find(key)?;
    let after = &raw_html[idx + key.len()..];
    let after_l = after.to_ascii_lowercase();
    if let Some(h) = after_l.find("href=") {
        let rest = &after[h + 5..];
        let rest_trim = rest.trim_start();
        let quote = rest_trim.chars().next()?;
        if quote == '"' || quote == '\'' {
            let body = &rest_trim[1..];
            if let Some(end) = body.find(quote) {
                let u = decode_entities(&body[..end]).trim().to_string();
                if is_valid_article_url(&u) && !is_hn_comments_url(&u) {
                    return normalize_article_url(&u);
                }
            }
        }
    }
    let trimmed = after.trim_start();
    let token = trimmed
        .split(|c: char| c.is_whitespace() || c == '<' || c == '"' || c == '\'')
        .next()
        .unwrap_or("");
    let u = decode_entities(token)
        .trim()
        .trim_end_matches(['.', ',', ';'])
        .to_string();
    if is_valid_article_url(&u) && !is_hn_comments_url(&u) {
        return normalize_article_url(&u);
    }
    None
}

/// Strip tags, decode entities, collapse whitespace. Empty → None.
/// Drops Google News / HN metadata soup (`Article URL`, `Comments URL`,
/// `Points:`, `# Comments:`) so the UI never shows comments-URL lines.
pub fn clean_summary(raw: &str) -> Option<String> {
    let stripped = strip_feed_metadata_blocks(raw);
    let mut text = collapse_ws(&strip_tags(&stripped));
    if text.is_empty() {
        return None;
    }
    text = strip_remaining_meta_labels(&text);
    text = collapse_ws(&text);
    if text.is_empty() {
        return None;
    }
    if looks_like_url_only(&text) || looks_like_hn_meta_only(&text) {
        return None;
    }
    const MAX: usize = 400;
    if text.chars().count() > MAX {
        let truncated: String = text.chars().take(MAX).collect();
        Some(format!("{truncated}…"))
    } else {
        Some(text)
    }
}

/// Remove HN / GNews metadata paragraphs before tag strip so href text cannot leak.
fn strip_feed_metadata_blocks(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    if !lower.contains("article url")
        && !lower.contains("comments url")
        && !lower.contains("points:")
        && !lower.contains("# comments")
    {
        return raw.to_string();
    }
    let mut out = String::with_capacity(raw.len());
    let mut start = 0usize;
    let mut i = 0usize;
    let bytes = raw.as_bytes();
    while i < bytes.len() {
        let at_break = if bytes[i] == b'<' {
            let slice = &lower[i..];
            slice.starts_with("</p>")
                || slice.starts_with("<br")
                || slice.starts_with("<p>")
                || slice.starts_with("<p ")
        } else {
            bytes[i] == b'\n'
        };
        let at_end = i + 1 == bytes.len();
        if at_break || at_end {
            let end = if at_break { i } else { i + 1 };
            if start < end {
                let chunk = &raw[start..end];
                if !is_metadata_chunk(&chunk.to_ascii_lowercase()) {
                    if !out.is_empty() {
                        out.push(' ');
                    }
                    out.push_str(chunk);
                }
            }
            if at_break {
                if bytes[i] == b'<' {
                    if let Some(rel) = raw[i..].find('>') {
                        i = i + rel + 1;
                        start = i;
                        continue;
                    }
                } else {
                    i += 1;
                    start = i;
                    continue;
                }
            } else {
                break;
            }
        }
        i += 1;
    }
    out
}

fn is_metadata_chunk(chunk_lower: &str) -> bool {
    let t = collapse_ws(chunk_lower);
    if t.is_empty() {
        return true;
    }
    if t.contains("article url") || t.contains("comments url") {
        return true;
    }
    let points = t.contains("points:");
    let ncomments = t.contains("# comments") || t.contains("#comments");
    if !(points || ncomments) {
        return false;
    }
    // Short score/comment count lines with no real prose.
    let alpha: String = t
        .chars()
        .filter(|c| c.is_ascii_alphabetic() || c.is_whitespace() || *c == '#')
        .collect();
    let compact = collapse_ws(&alpha);
    compact.len() < 48
}

/// After tag strip, remove leftover "Comments URL: https://…" / "Points: N" tails.
fn strip_remaining_meta_labels(text: &str) -> String {
    let mut s = text.to_string();
    let labels = [
        "comments url:",
        "comment url:",
        "article url:",
        "points:",
        "# comments:",
        "#comments:",
    ];
    for label in labels {
        loop {
            let lower = s.to_ascii_lowercase();
            let Some(idx) = lower.find(label) else { break };
            let prefix = s[..idx].trim_end().to_string();
            let mut rest = s[idx + label.len()..].trim_start().to_string();
            let rest_l = rest.to_ascii_lowercase();
            if rest_l.starts_with("http://") || rest_l.starts_with("https://") {
                if let Some(sp) = rest.find(char::is_whitespace) {
                    rest = rest[sp..].trim_start().to_string();
                } else {
                    rest.clear();
                }
            } else if let Some(sp) = rest.find(char::is_whitespace) {
                let first = &rest[..sp];
                if first.chars().all(|c| c.is_ascii_digit())
                    || first.len() < 12
                    || first.contains('/')
                {
                    rest = rest[sp..].trim_start().to_string();
                }
            } else if rest.chars().all(|c| c.is_ascii_digit()) || rest.len() < 12 {
                rest.clear();
            }
            s = if prefix.is_empty() {
                rest
            } else if rest.is_empty() {
                prefix
            } else {
                format!("{prefix} {rest}")
            };
        }
    }
    s
}

fn looks_like_hn_meta_only(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("comments url")
        || lower.contains("article url")
        || lower.contains("news.ycombinator.com/item")
}

fn looks_like_url_only(text: &str) -> bool {
    let t = text.trim();
    if t.starts_with("http://") || t.starts_with("https://") {
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
    fn hn_description_is_not_a_snippet() {
        let raw = r#"<p>Article URL: <a href="https://example.com/story">https://example.com/story</a></p>
<p>Comments URL: <a href="https://news.ycombinator.com/item?id=1">https://news.ycombinator.com/item?id=1</a></p>
<p>Points: 42</p><p># Comments: 7</p>"#;
        assert_eq!(clean_summary(raw), None);
        assert_eq!(
            extract_story_url_from_description(raw).as_deref(),
            Some("https://example.com/story")
        );
        let fb = hn_fallback_summary(raw, "https://example.com/story").unwrap();
        assert!(fb.contains("example.com"), "{fb}");
        assert!(fb.contains("42"), "{fb}");
        assert!(!fb.to_ascii_lowercase().contains("comments url"), "{fb}");
    }

    #[test]
    fn prefer_story_over_comments_link() {
        let mut a = Article {
            title: "T".into(),
            url: "https://news.ycombinator.com/item?id=9".into(),
            summary: Some(
                r#"<p>Article URL: <a href="https://ars.example/a">x</a></p>
<p>Comments URL: <a href="https://news.ycombinator.com/item?id=9">y</a></p>"#
                    .into(),
            ),
            published: None,
            source: "t".into(),
            channel_id: "tech".into(),
        };
        sanitize_article(&mut a);
        assert_eq!(a.url, "https://ars.example/a");
        // Metadata-only HN desc → synthetic host/points blurb (not Comments URL).
        let s = a.summary.as_deref().unwrap_or("");
        assert!(s.contains("ars.example") || s.contains("Hacker News"), "{s}");
        assert!(!s.to_ascii_lowercase().contains("comments url"), "{s}");
        assert!(is_hn_comments_url("https://news.ycombinator.com/item?id=1"));
        assert!(!is_hn_comments_url("https://example.com/post"));
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
