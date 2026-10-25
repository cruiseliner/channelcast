use anyhow::{bail, Context, Result};
use channelcast_core::{headlines_for_channel, ChannelConfig};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(
    name = "channelcast",
    about = "ChannelCast — on-demand multi-channel news (PointCast-inspired)",
    version
)]
struct Cli {
    /// Path to channels.toml (defaults: ./channels.toml, then built-in defaults)
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    /// SQLite cache path (optional; created on fetch/headlines)
    #[arg(long, global = true, default_value = ".channelcast/cache.db")]
    cache: PathBuf,

    /// Skip writing the SQLite cache
    #[arg(long, global = true)]
    no_cache: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// List configured channels
    Channels {
        /// Include disabled channels (e.g. personal watchman)
        #[arg(long)]
        all: bool,
    },
    /// Fetch feeds for a channel into the local cache
    Fetch {
        /// Channel id (e.g. tech, top, prowrestling, watchman)
        channel: String,
    },
    /// Print top N headlines for a channel (fetches live)
    Headlines {
        /// Channel id (e.g. tech, top)
        channel: String,
        /// How many headlines to print
        #[arg(short = 'n', long, default_value_t = 15)]
        limit: usize,
    },
}

fn load_config(path: Option<&Path>) -> Result<ChannelConfig> {
    if let Some(p) = path {
        return ChannelConfig::load_path(p)
            .with_context(|| format!("loading config {}", p.display()));
    }
    let local = Path::new("channels.toml");
    if local.is_file() {
        return ChannelConfig::load_path(local).context("loading ./channels.toml");
    }
    ChannelConfig::defaults().context("loading built-in defaults")
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = load_config(cli.config.as_deref())?;
    let cache_path = if cli.no_cache {
        None
    } else {
        if let Some(parent) = cli.cache.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Some(cli.cache.as_path())
    };

    match cli.command {
        Commands::Channels { all } => {
            println!(
                "{:<14} {:<14} {:>5}  {}",
                "ID", "NAME", "FEEDS", "FLAGS / DESCRIPTION"
            );
            for ch in config.listed(all) {
                let mut flags = Vec::new();
                if !ch.enabled {
                    flags.push("disabled");
                }
                if ch.personal {
                    flags.push("personal");
                }
                if ch.feeds.is_empty() {
                    flags.push("stub");
                }
                if !ch.keywords.is_empty() {
                    flags.push("keywords");
                }
                let flag_s = if flags.is_empty() {
                    String::new()
                } else {
                    format!("[{}] ", flags.join(","))
                };
                println!(
                    "{:<14} {:<14} {:>5}  {}{}",
                    ch.id,
                    ch.name,
                    ch.feeds.len(),
                    flag_s,
                    ch.description
                );
            }
            if !all {
                let hidden = config.channels.iter().filter(|c| !c.enabled).count();
                if hidden > 0 {
                    println!("({hidden} disabled channel(s) hidden — pass --all to show)");
                }
            }
        }
        Commands::Fetch { channel } => {
            let ch = config.get(&channel)?;
            if ch.feeds.is_empty() {
                bail!("channel '{channel}' is a stub with no feeds yet");
            }
            let articles = headlines_for_channel(&config, &channel, usize::MAX, cache_path)?;
            println!(
                "Fetched {} unique headlines for channel '{channel}'",
                articles.len()
            );
        }
        Commands::Headlines { channel, limit } => {
            let articles = headlines_for_channel(&config, &channel, limit, cache_path)?;
            if articles.is_empty() {
                bail!(
                    "no headlines for '{channel}' (feeds empty, all failed, or keywords matched nothing)"
                );
            }
            for (i, a) in articles.iter().enumerate() {
                let when = a
                    .published
                    .map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string())
                    .unwrap_or_else(|| "unknown date".into());
                println!("{}. {}", i + 1, a.title);
                println!("   {} · {}", a.source, when);
                println!("   {}", a.url);
                if let Some(ref s) = a.summary {
                    let one = s.replace('\n', " ");
                    let show = if one.chars().count() > 220 {
                        format!("{}…", one.chars().take(217).collect::<String>())
                    } else {
                        one
                    };
                    println!("   summary: {show}");
                } else {
                    println!("   summary: (none)");
                }
            }
        }
    }
    Ok(())
}
