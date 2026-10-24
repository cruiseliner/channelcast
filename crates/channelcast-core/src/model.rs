use chrono::{DateTime, Utc};

/// A single headline / article summary from a feed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Article {
    pub title: String,
    pub url: String,
    pub summary: Option<String>,
    pub published: Option<DateTime<Utc>>,
    pub source: String,
    pub channel_id: String,
}
