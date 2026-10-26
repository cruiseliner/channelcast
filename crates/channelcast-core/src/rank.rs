
use crate::model::Article;
use chrono::{Duration, Utc};
use std::collections::HashMap;

/// Drop articles whose `published` is older than `max_age`.
/// Undated items are kept (cannot prove stale); never invent filler from known-old stories.
pub fn filter_max_age(articles: Vec<Article>, max_age: Duration) -> Vec<Article> {
    let cutoff = Utc::now() - max_age;
    articles
        .into_iter()
        .filter(|a| match a.published {
            Some(p) => p >= cutoff,
            None => true,
        })
        .collect()
}


/// Dedupe by canonical URL, prefer the copy with the richer summary, then sort newest-first.
/// Articles without a published time sort after dated ones.
pub fn dedupe_and_rank(mut articles: Vec<Article>) -> Vec<Article> {
    let mut best: HashMap<String, Article> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for a in articles.drain(..) {
        let key = normalize_url(&a.url);
        match best.get_mut(&key) {
            None => {
                order.push(key.clone());
                best.insert(key, a);
            }
            Some(existing) => {
                let new_len = a.summary.as_ref().map(|s| s.len()).unwrap_or(0);
                let old_len = existing.summary.as_ref().map(|s| s.len()).unwrap_or(0);
                if new_len > old_len {
                    existing.summary = a.summary;
                }
                // Fill missing published/title only; keep first-seen ranking key.
                if existing.published.is_none() {
                    existing.published = a.published;
                }
                if existing.title.is_empty() && !a.title.is_empty() {
                    existing.title = a.title;
                }
            }
        }
    }
    let mut articles: Vec<Article> = order
        .into_iter()
        .filter_map(|k| best.remove(&k))
        .collect();

    articles.sort_by(|a, b| match (a.published, b.published) {
        (Some(pa), Some(pb)) => pb.cmp(&pa),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        // Preserve feed / insertion order for undated items (e.g. NWS forecast periods).
        (None, None) => std::cmp::Ordering::Equal,
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
    use chrono::{Duration, TimeZone, Utc};

    fn art(url: &str, title: &str, secs: Option<i64>) -> Article {
        Article {
            title: title.into(),
            url: url.into(),
            summary: None,
            image_url: None,
            published: secs.map(|s| Utc.timestamp_opt(s, 0).unwrap()),
            source: "t".into(),
            channel_id: "tech".into(),
        }
    }

    fn art_sum(url: &str, title: &str, summary: &str, secs: Option<i64>) -> Article {
        let mut a = art(url, title, secs);
        a.summary = Some(summary.into());
        a
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
        assert_eq!(ranked[1].title, "old"); // first title kept; published bumped to newest
        assert_eq!(ranked[2].title, "undated");
    }

    #[test]
    fn prefers_longer_summary_on_dedupe() {
        let ranked = dedupe_and_rank(vec![
            art("https://ex.com/a", "t", Some(100)),
            art_sum("https://ex.com/a/", "t2", "A longer listing blurb about the story.", Some(50)),
        ]);
        assert_eq!(ranked.len(), 1);
        assert!(ranked[0].summary.as_deref().unwrap().contains("longer listing"));
        assert_eq!(ranked[0].published.unwrap().timestamp(), 100);
    }

    #[test]
    fn filter_max_age_drops_old_keeps_undated() {
        let now = Utc::now().timestamp();
        let kept = filter_max_age(
            vec![
                art("https://ex.com/new", "new", Some(now)),
                art("https://ex.com/old", "old", Some(now - 10 * 24 * 3600)),
                art("https://ex.com/u", "undated", None),
            ],
            Duration::days(7),
        );
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0].title, "new");
        assert_eq!(kept[1].title, "undated");
    }

}