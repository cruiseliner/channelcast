use crate::error::Result;
use crate::model::Article;
use rusqlite::{params, Connection};
use std::path::Path;

/// Minimal SQLite cache stub: upsert articles by URL, list by channel.
pub struct Cache {
    conn: Connection,
}

impl Cache {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        let cache = Self { conn };
        cache.migrate()?;
        Ok(cache)
    }

    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let cache = Self { conn };
        cache.migrate()?;
        Ok(cache)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS articles (
                url TEXT PRIMARY KEY NOT NULL,
                title TEXT NOT NULL,
                summary TEXT,
                published_rfc3339 TEXT,
                source TEXT NOT NULL,
                channel_id TEXT NOT NULL,
                fetched_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_articles_channel
                ON articles(channel_id, published_rfc3339 DESC);
            "#,
        )?;
        Ok(())
    }

    pub fn upsert_articles(&self, articles: &[Article]) -> Result<usize> {
        let mut count = 0;
        let tx = self.conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                r#"
                INSERT INTO articles (url, title, summary, published_rfc3339, source, channel_id)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ON CONFLICT(url) DO UPDATE SET
                    title = excluded.title,
                    summary = excluded.summary,
                    published_rfc3339 = excluded.published_rfc3339,
                    source = excluded.source,
                    channel_id = excluded.channel_id,
                    fetched_at = datetime('now')
                "#,
            )?;
            for a in articles {
                let published = a.published.map(|t| t.to_rfc3339());
                stmt.execute(params![
                    a.url,
                    a.title,
                    a.summary,
                    published,
                    a.source,
                    a.channel_id
                ])?;
                count += 1;
            }
        }
        tx.commit()?;
        Ok(count)
    }

    pub fn list_channel(&self, channel_id: &str, limit: usize) -> Result<Vec<Article>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT url, title, summary, published_rfc3339, source, channel_id
            FROM articles
            WHERE channel_id = ?1
            ORDER BY published_rfc3339 IS NULL, published_rfc3339 DESC
            LIMIT ?2
            "#,
        )?;
        let rows = stmt.query_map(params![channel_id, limit as i64], |row| {
            let published_s: Option<String> = row.get(3)?;
            let published = published_s
                .as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc));
            Ok(Article {
                url: row.get(0)?,
                title: row.get(1)?,
                summary: row.get(2)?,
                published,
                source: row.get(4)?,
                channel_id: row.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }
}
