# ChannelCast

On-demand multi-channel news app (PointCast-inspired). **Rust core** first; Windows desktop UI next, then Linux/Mac, then an Android APK via a thin shell around the same core.

**Shipping name:** ChannelCast (not PointCast — that 1990s name was later co-opted).

## What it is

- An app **you open when you want news**
- Topic **channels** (Top, Tech, Markets, Sports, …) backed by free public RSS/Atom feeds
- A shared **`channelcast-core`** library: config → fetch → optional keyword filter → dedupe by URL → rank by recency → optional SQLite cache
- A CLI (`channelcast`) so the core is usable before any GUI ships

## What it is not

- **Not** the 1990s PointCast Network screensaver / SmartScreen push client
- **Not** [The Morning Newspaper](https://x.ai/bot/1xAJYJPes3X7dUM2mk9Di) (that bot turns *your* email + calendar into a personal paper)
- **Not** ad-sponsored push media
- **Not** LLM “prophecy” generation — the optional `watchman` channel only aggregates allowlisted news RSS and filters by editable keywords
- **No Grok Bot required** — the app is standalone
- Listing an outlet in `channels.toml` is **not an endorsement** of its views

## Architecture

```mermaid
flowchart LR
  subgraph sources [Sources]
    RSS[RSS / Atom]
    NWS[NWS forecast JSON + alerts Atom]
  end
  subgraph core [channelcast-core]
    Cfg[channels.toml]
    Fetch[Fetch + parse]
    Kw[Keyword filter optional]
    Rank[Dedupe + rank]
    Store[SQLite cache]
  end
  subgraph ui [Thin UI later]
    CLI[channelcast CLI]
    Desk[Desktop app]
    And[Android APK]
  end
  RSS --> Fetch
  NWS --> Fetch
  Cfg --> Fetch --> Kw --> Rank --> Store
  Store --> CLI
  Store -.-> Desk
  Store -.-> And
```

## Quick start

```bash
# from repo root
cargo run -p channelcast-cli -- channels
cargo run -p channelcast-cli -- channels --all   # include disabled personal channels
cargo run -p channelcast-cli -- headlines tech
cargo run -p channelcast-cli -- headlines top -n 10
cargo run -p channelcast-cli -- headlines prowrestling
cargo run -p channelcast-cli -- headlines watchman   # works even when disabled in listing
cargo run -p channelcast-cli -- headlines weather    # NWS Cape Canaveral / ZIP 32920
cargo run -p channelcast-cli -- headlines local
cargo run -p channelcast-cli -- headlines spacecoast
cargo run -p channelcast-cli -- fetch markets
```

Binary name: **`channelcast`**.

Optional flags:

- `--config path/to/channels.toml` — override feed list
- `--cache path.db` — SQLite cache (default `.channelcast/cache.db`)
- `--no-cache` — skip disk cache

## Default channels (v0.1)

| id | status | feeds (HTTP 200 verified 2026-10-24) |
|----|--------|--------------------------------------|
| `top` | on | BBC World, NPR News |
| `tech` | on | Hacker News (hnrss), Ars Technica |
| `markets` | on | BBC Business, CNBC Top News |
| `sports` | on | BBC Sport, ESPN Top |
| `prowrestling` | on, personal | Fightful, F4W Online |
| `watchman` | **disabled by default**, personal | Jerusalem Post frontpage, Times of Israel, BBC Middle East + keyword filter |
| `weather` | on, personal | ZIP **32920** Cape Canaveral FL — NWS MLB grid 56,65 forecast JSON + alerts Atom zone FLZ747 |
| `local` | on, personal | Space Coast Daily (Brevard community, ZIP 32920) |
| `spacecoast` | on, personal | NASA Kennedy + Artemis blogs, Spaceflight Now, NASASpaceflight + KSC/SpaceX/Blue Origin keywords |

### Feed verification notes

**Committed (200 + RSS/Atom):** Fightful, F4W Online, JPost frontpage RSS, Times of Israel, BBC Middle East, plus the general channels above.

**Weather / local (ZIP 32920, Cape Canaveral / Cocoa Beach FL):**

| Endpoint | Result |
|----------|--------|
| `api.weather.gov/points/28.3903,-80.6043` | 200 → office **MLB**, grid **56,65**, zone **FLZ747** |
| `.../gridpoints/MLB/56,65/forecast` | 200 GeoJSON — parsed by `channelcast_core::nws` (not RSS) |
| `.../alerts/active.atom?zone=FLZ747` | 200 Atom — normal feed-rs path |
| `spacecoastdaily.com/feed/` | 200 RSS (local) |
| `blogs.nasa.gov/kennedy/feed/` | 200 RSS (spacecoast) |
| `blogs.nasa.gov/artemis/feed/` | 200 RSS (spacecoast) |
| `spaceflightnow.com/feed/` | 200 RSS (spacecoast) |
| `nasaspaceflight.com/feed/` | 200 RSS (spacecoast) |

**Tried and dropped:**

| URL / site | Result |
|------------|--------|
| PWInsider RSS paths | 404 |
| israel365news direct `/feed/` | Cloudflare 403 from CLI — **workaround:** Google News `site:israel365news.com` RSS in watchman feeds |
| ProphecyNewsWatch feed URLs | 404 |
| watch.org/feed | 404 |
| Haaretz cmlink | 200 but HTML, not RSS |
| CNN Middle East RSS | failed to connect from this environment |
| Blue Origin / SpaceX official RSS | no usable public feed (429/HTML) |
| NASA commercialcrew blog `/feed/` | 200 but HTML, not RSS |
| Florida Today space RSS paths | 404 |

**Needs RSS or licensed API (not scraped in v0.1):** israel365news, other “watchman-style” prophecy portals without a public feed. Add URLs to `channels.toml` when they expose RSS.

Edit `channels.toml` to add feeds, flip `enabled`, or change `watchman.keywords` (Turkey, Gog/Magog, Damascus, temple, red heifer, Ezekiel, … — editable config, not hardcoded theology).

## Development

```bash
cargo test
cargo run -p channelcast-cli -- headlines tech
```

Fixture-based tests (RSS + Atom parse, cache round-trip, keyword filter) do not need the network. Live `headlines` / `fetch` do.

## Roadmap

1. **Now** — Rust core + CLI (this repo)
2. **Desktop UI** — Windows first (egui/iced or similar), then Linux/Mac
3. **Weather / local / spacecoast** — done for ZIP 32920 (NWS MLB + Brevard RSS + KSC/NASA/SpaceX feeds); other ZIPs need their own grid/feeds
4. **Android APK** — thin shell + `channelcast-core` via UniFFI/JNI
5. Optional companion bot later for digests — not required for the app

## License

MIT — see [LICENSE](LICENSE).
