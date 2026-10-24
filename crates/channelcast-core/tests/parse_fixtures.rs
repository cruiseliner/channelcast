use channelcast_core::{dedupe_and_rank, parse_feed_bytes, Cache, ChannelConfig};

#[test]
fn parses_sample_rss() {
    let bytes = include_bytes!("fixtures/sample.rss");
    let articles = parse_feed_bytes(bytes, "https://example.com/rss", "tech").unwrap();
    assert_eq!(articles.len(), 2);
    assert_eq!(articles[0].title, "First Story");
    assert!(articles[0].url.contains("stories/1"));
    assert!(articles[0].published.is_some());
}

#[test]
fn parses_sample_atom() {
    let bytes = include_bytes!("fixtures/sample.atom");
    let articles = parse_feed_bytes(bytes, "https://example.org/atom", "tech").unwrap();
    assert_eq!(articles.len(), 2);
    assert_eq!(articles[0].title, "Atom Entry One");
    assert_eq!(articles[0].url, "https://example.org/one");
}

#[test]
fn defaults_config_loads() {
    let cfg = ChannelConfig::defaults().unwrap();
    assert!(cfg.get("tech").unwrap().feeds.len() >= 1);
    assert!(cfg.get("weather").unwrap().feeds.is_empty());
    let pw = cfg.get("prowrestling").unwrap();
    assert!(pw.personal);
    assert!(!pw.feeds.is_empty());
    let w = cfg.get("watchman").unwrap();
    assert!(!w.enabled);
    assert!(w.personal);
    assert!(!w.keywords.is_empty());
    assert!(!w.feeds.is_empty());
    assert_eq!(cfg.listed(false).len(), cfg.channels.iter().filter(|c| c.enabled).count());
}

#[test]
fn cache_roundtrip() {
    let bytes = include_bytes!("fixtures/sample.rss");
    let articles = parse_feed_bytes(bytes, "https://example.com/rss", "tech").unwrap();
    let ranked = dedupe_and_rank(articles);
    let cache = Cache::open_memory().unwrap();
    cache.upsert_articles(&ranked).unwrap();
    let listed = cache.list_channel("tech", 10).unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].title, "Second Story");
}
