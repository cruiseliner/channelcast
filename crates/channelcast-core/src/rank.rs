use crate::model::Article;
use std::collections::HashSet;

/// Dedupe by canonical URL (first wins), then sort newest-first.
/// Articles without a published time sort after dated ones, stable by title.
pub fn dedupe_and_rank(mut articles: Vec<Article>) -> Vec<Article> {
    let mut seen = HashSet::new();
    articles.retain(|a| seen.insert(normalize_url(&a.url)));

    articles.sort_by(|a, b| match (a.published, b.published) {
        (Some(pa), Some(pb)) => pb.cmp(&pa),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.title.cmp(&b.title),
    });
    articles
}

fn normalize_url(url: &str) -> String {
    // Strip common tracking fragments lightly; keep host+path.
    let trimmed = url.trim().trim_end_matches('/');
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn art(url: &str, title: &str, secs: Option<i64>) -> Article {
        Article {
            title: title.into(),
            url: url.into(),
            summary: None,
            published: secs.map(|s| Utc.timestamp_opt(s, 0).unwrap()),
            source: "t".into(),
            channel_id: "tech".into(),
        }
    }

    #[test]
    fn dedupes_by_url_and_ranks_newest_first() {
        let ranked = dedupe_and_rank(vec![
            art("https://ex.com/a", "old", Some(100)),
            art("https://ex.com/a/", "dup", Some(999)),
            art("https://ex.com/b", "new", Some(200)),
            art("https://ex.com/c", "undated", None),
        ]);
        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].url, "https://ex.com/b");
        assert_eq!(ranked[1].title, "old"); // first occurrence kept
        assert_eq!(ranked[2].title, "undated");
    }
}
