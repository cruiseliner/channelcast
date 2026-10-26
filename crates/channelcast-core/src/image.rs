//! Extract story images from MediaRSS, enclosures, and HTML (feed body or og:image).

use crate::sanitize::is_valid_article_url;

/// True when a URL looks like a usable http(s) image (not a tracking pixel heuristic alone).
pub fn is_plausible_image_url(url: &str) -> bool {
    let u = url.trim();
    if !is_valid_article_url(u) {
        return false;
    }
    let lower = u.to_ascii_lowercase();
    if lower.contains("1x1") || lower.contains("pixel") || lower.contains("spacer") {
        return false;
    }
    if lower.contains(".svg") && lower.contains("logo") {
        return false;
    }
    true
}

pub fn normalize_image_url(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }
    // protocol-relative
    let abs = if t.starts_with("//") {
        format!("https:{t}")
    } else {
        t.to_string()
    };
    if is_plausible_image_url(&abs) {
        Some(abs)
    } else {
        None
    }
}

/// First `<img src="…">` / `src='…'` in HTML (feed description / content).
pub fn first_img_src(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let mut search = 0usize;
    while let Some(rel) = lower[search..].find("<img") {
        let start = search + rel;
        let end = lower[start..].find('>').map(|i| start + i).unwrap_or(html.len());
        let tag = &html[start..end];
        if let Some(src) = attr_from_tag(tag, "src").or_else(|| attr_from_tag(tag, "data-src")) {
            if let Some(n) = normalize_image_url(&src) {
                // skip tiny / icon-ish paths
                let l = n.to_ascii_lowercase();
                if !(l.contains("favicon") || l.contains("/icon") || l.contains("sprite")) {
                    return Some(n);
                }
            }
        }
        search = end + 1;
        if search >= html.len() {
            break;
        }
    }
    None
}

fn attr_from_tag(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let key = format!("{name}=");
    let i = lower.find(&key)?;
    let after = &tag[i + key.len()..];
    let after = after.trim_start();
    let quote = after.chars().next()?;
    if quote == '"' || quote == '\'' {
        let rest = &after[1..];
        let end = rest.find(quote)?;
        Some(rest[..end].to_string())
    } else {
        let end = after
            .find(|c: char| c.is_whitespace() || c == '>')
            .unwrap_or(after.len());
        Some(after[..end].to_string())
    }
}

/// og:image / twitter:image / link rel=image_src from article HTML.
pub fn extract_image_from_html(html: &str) -> Option<String> {
    meta_content(html, "property", "og:image")
        .or_else(|| meta_content(html, "property", "og:image:secure_url"))
        .or_else(|| meta_content(html, "name", "twitter:image"))
        .or_else(|| meta_content(html, "name", "twitter:image:src"))
        .or_else(|| first_img_src(html))
        .and_then(|s| normalize_image_url(&s))
}

fn meta_content(html: &str, attr: &str, value: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let needle = format!("{attr}=\"{value}\"");
    let needle2 = format!("{attr}='{value}'");
    let idx = lower
        .find(&needle)
        .or_else(|| lower.find(&needle2))?;
    // walk back to <meta
    let window_start = lower[..idx].rfind("<meta").unwrap_or(idx);
    let window_end = lower[idx..]
        .find('>')
        .map(|i| idx + i)
        .unwrap_or_else(|| (idx + 400).min(html.len()));
    let window = &html[window_start..window_end];
    attr_from_tag(window, "content")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn img_src_from_description() {
        let html = r#"<p>Hi</p><img src="https://cdn.example.com/story.jpg" alt="x" />"#;
        assert_eq!(
            first_img_src(html).as_deref(),
            Some("https://cdn.example.com/story.jpg")
        );
    }

    #[test]
    fn og_image() {
        let html = r#"
<meta property="og:image" content="https://cdn.example.com/og.webp" />
<meta name="twitter:image" content="https://cdn.example.com/tw.jpg" />
"#;
        assert_eq!(
            extract_image_from_html(html).as_deref(),
            Some("https://cdn.example.com/og.webp")
        );
    }
}
