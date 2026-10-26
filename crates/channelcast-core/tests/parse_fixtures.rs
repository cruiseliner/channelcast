use channelcast_core::{
    dedupe_and_rank, parse_article_body_bytes, parse_feed_bytes, parse_listing_bytes,
    parse_nws_forecast_bytes, parse_sitemap_bytes, parse_usatoday_article_body_bytes,
    parse_usatoday_listing_bytes, parse_usatoday_sitemap_bytes, Cache, ChannelConfig,
    UsaTodaySitemapFilter,
};

#[test]
fn parses_sample_rss() {
    let bytes = include_bytes!("fixtures/sample.rss");
    let articles = parse_feed_bytes(bytes, "https://example.com/rss", "tech").unwrap();
    assert_eq!(articles.len(), 3);
    assert_eq!(articles[0].title, "First Story");
    assert!(articles[0].url.contains("stories/1"));
    assert!(articles[0].published.is_some());
    // First item had Article URL HTML soup + blurb — soup gone, prose kept.
    let s0 = articles[0].summary.as_deref().unwrap_or("");
    assert!(!s0.contains('<'), "summary must be plain text: {s0}");
    assert!(!s0.to_ascii_lowercase().contains("article url"), "{s0}");
    assert!(s0.contains("Real blurb"), "{s0}");
    let s2 = articles[2].summary.as_deref().unwrap_or("");
    assert_eq!(s2, "Plain prose after tags for wrestling fans.");
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
    let weather = cfg.get("weather").unwrap();
    assert!(!weather.feeds.is_empty());
    assert!(weather.personal);
    // Live dashboard is fetch_weather_dashboard(); feeds keep alerts Atom for CLI/fallback.
    assert!(weather
        .feeds
        .iter()
        .any(|u| u.contains("api.weather.gov/alerts/active")));
    let local = cfg.get("local").unwrap();
    assert!(!local.feeds.is_empty());
    assert!(local.personal);
    let sc = cfg.get("spacecoast").unwrap();
    assert!(sc.enabled);
    assert!(sc.personal);
    assert!(!sc.feeds.is_empty());
    assert!(!sc.keywords.is_empty());
    let pw = cfg.get("prowrestling").unwrap();
    assert!(pw.personal);
    assert!(!pw.feeds.is_empty());
    let w = cfg.get("watchman").unwrap();
    assert!(w.enabled);
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
    assert_eq!(listed.len(), 3);
    assert_eq!(listed[0].title, "HTML Summary Story"); // newest dated
}

#[test]
fn parses_nws_forecast_fixture() {
    let bytes = include_bytes!("fixtures/nws_forecast.json");
    let articles = parse_nws_forecast_bytes(
        bytes,
        "https://api.weather.gov/gridpoints/MLB/56,65/forecast",
        "weather",
    )
    .unwrap();
    assert_eq!(articles.len(), 3);
    assert!(articles[0].title.contains("This Afternoon"));
    assert!(articles[1].title.contains("Tonight"));
    assert!(articles[0].url.contains("#period-"));
    assert!(articles[0].summary.as_deref().unwrap_or("").contains("Starts "));
    assert!(articles[0].source.contains("MLB"));
}


#[test]
fn parses_floridatoday_space_listing() {
    let bytes = include_bytes!("fixtures/floridatoday_space.html");
    let articles = parse_listing_bytes(bytes, "spacecoast").unwrap();
    assert!(articles.len() >= 5, "got {}", articles.len());
    assert!(articles.iter().any(|a| a.title.contains("Artemis")));
    assert!(articles[0].url.starts_with("https://www.floridatoday.com/"));
    assert_eq!(articles[0].source, "Florida Today");
    // list tiles carry data-c-br blurbs
    assert!(articles.iter().any(|a| a.summary.as_deref().unwrap_or("").len() > 20));
}

#[test]
fn parses_floridatoday_sitemap_space_filter() {
    let bytes = include_bytes!("fixtures/floridatoday_sitemap_space.xml");
    let articles = parse_sitemap_bytes(bytes, "spacecoast").unwrap();
    assert!(!articles.is_empty());
    assert!(articles.iter().all(|a| {
        let u = a.url.to_ascii_lowercase();
        u.contains("/tech/science/space/") || u.contains("spacex") || u.contains("nasa")
    }));
    assert!(articles.iter().any(|a| a.published.is_some()));
}

#[test]
fn parses_floridatoday_article_body_fixture() {
    let bytes = include_bytes!("fixtures/floridatoday_article.html");
    let summary = parse_article_body_bytes(bytes).expect("summary");
    assert!(summary.len() > 40, "{summary}");
}

#[test]
fn defaults_include_floridatoday_on_spacecoast_and_local() {
    let cfg = ChannelConfig::defaults().unwrap();
    let sc = cfg.get("spacecoast").unwrap();
    assert!(sc
        .feeds
        .iter()
        .any(|u| u.contains("floridatoday.com/space")));
    assert!(sc
        .feeds
        .iter()
        .any(|u| u.contains("news-sitemap.xml")));
    let local = cfg.get("local").unwrap();
    assert!(local
        .feeds
        .iter()
        .any(|u| u.contains("floridatoday.com/space")));
}


#[test]
fn parses_usatoday_nation_listing() {
    let bytes = include_bytes!("fixtures/usatoday_nation.html");
    let articles = parse_usatoday_listing_bytes(bytes, "top").unwrap();
    assert!(articles.len() >= 3, "got {}", articles.len());
    assert!(articles[0].url.starts_with("https://www.usatoday.com/"));
    assert_eq!(articles[0].source, "USA Today");
}

#[test]
fn parses_usatoday_tech_listing() {
    let bytes = include_bytes!("fixtures/usatoday_tech.html");
    let articles = parse_usatoday_listing_bytes(bytes, "tech").unwrap();
    assert!(articles.len() >= 2, "got {}", articles.len());
    assert_eq!(articles[0].source, "USA Today");
}

#[test]
fn parses_usatoday_sitemap_filters() {
    let bytes = include_bytes!("fixtures/usatoday_sitemap_mixed.xml");
    let nation = parse_usatoday_sitemap_bytes(bytes, "top", UsaTodaySitemapFilter::Nation).unwrap();
    assert!(!nation.is_empty());
    assert!(nation.iter().all(|a| {
        let u = a.url.to_ascii_lowercase();
        u.contains("/story/news/nation/") || u.contains("/story/news/politics/")
    }));
    let tech = parse_usatoday_sitemap_bytes(bytes, "tech", UsaTodaySitemapFilter::Tech).unwrap();
    assert!(!tech.is_empty());
    assert!(tech.iter().all(|a| a.url.to_ascii_lowercase().contains("/story/tech/")));
    let ent = parse_usatoday_sitemap_bytes(
        bytes,
        "entertainment",
        UsaTodaySitemapFilter::Entertainment,
    )
    .unwrap();
    assert!(!ent.is_empty());
    assert!(ent
        .iter()
        .all(|a| a.url.to_ascii_lowercase().contains("/story/entertainment/")));
    // sports blocks in fixture must not leak into nation/tech/ent
    assert!(nation.iter().all(|a| !a.url.contains("/story/sports/")));
}

#[test]
fn parses_usatoday_article_body_fixture() {
    let bytes = include_bytes!("fixtures/usatoday_article.html");
    let summary = parse_usatoday_article_body_bytes(bytes).expect("summary");
    assert!(summary.len() > 40, "{summary}");
}

#[test]
fn defaults_include_usatoday_on_top_tech_entertainment() {
    let cfg = ChannelConfig::defaults().unwrap();
    let top = cfg.get("top").unwrap();
    assert!(top.feeds.iter().any(|u| u.contains("usatoday.com/news/nation")));
    assert!(top
        .feeds
        .iter()
        .any(|u| u.contains("news-sitemap.xml#nation")));
    let tech = cfg.get("tech").unwrap();
    assert!(tech.feeds.iter().any(|u| u.contains("usatoday.com/tech")));
    assert!(tech
        .feeds
        .iter()
        .any(|u| u.contains("news-sitemap.xml#tech")));
    let ent = cfg.get("entertainment").unwrap();
    assert!(ent.enabled);
    assert!(ent.personal);
    assert!(ent
        .feeds
        .iter()
        .any(|u| u.contains("usatoday.com/entertainment")));
    assert!(ent
        .feeds
        .iter()
        .any(|u| u.contains("news-sitemap.xml#entertainment")));
}
