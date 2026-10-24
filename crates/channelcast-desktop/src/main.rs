//! Minimal ChannelCast desktop shell (egui/eframe).
//! Channel list + headlines; opens article URLs in the system browser.

use anyhow::{Context, Result};
use channelcast_core::{headlines_for_channel, Article, Channel, ChannelConfig};
use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

const HEADLINE_LIMIT: usize = 40;

enum FetchMsg {
    Ok {
        channel_id: String,
        articles: Vec<Article>,
    },
    Err {
        channel_id: String,
        message: String,
    },
}

struct App {
    config: ChannelConfig,
    channels: Vec<Channel>,
    selected: Option<String>,
    articles: Vec<Article>,
    status: String,
    fetching: bool,
    cache_path: PathBuf,
    rx: Option<Receiver<FetchMsg>>,
}

impl App {
    fn new(config: ChannelConfig, cache_path: PathBuf) -> Self {
        let channels: Vec<Channel> = config
            .listed(false)
            .into_iter()
            .cloned()
            .collect();
        let selected = channels.first().map(|c| c.id.clone());
        let mut app = Self {
            config,
            channels,
            selected,
            articles: Vec::new(),
            status: "Select a channel, then Refresh.".into(),
            fetching: false,
            cache_path,
            rx: None,
        };
        if let Some(id) = app.selected.clone() {
            app.start_fetch(id);
        }
        app
    }

    fn start_fetch(&mut self, channel_id: String) {
        if self.fetching {
            return;
        }
        let Some(ch) = self.config.get(&channel_id).ok().cloned() else {
            self.status = format!("Unknown channel '{channel_id}'");
            return;
        };
        if ch.feeds.is_empty() {
            self.status = format!("Channel '{channel_id}' has no feeds yet");
            self.articles.clear();
            return;
        }

        self.fetching = true;
        self.status = format!("Fetching {channel_id}…");
        self.articles.clear();

        let config = self.config.clone();
        let cache = self.cache_path.clone();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);

        thread::spawn(move || {
            let path = Some(cache.as_path());
            let msg = match headlines_for_channel(&config, &channel_id, HEADLINE_LIMIT, path) {
                Ok(articles) => FetchMsg::Ok {
                    channel_id,
                    articles,
                },
                Err(e) => FetchMsg::Err {
                    channel_id,
                    message: e.to_string(),
                },
            };
            let _ = tx.send(msg);
        });
    }

    fn poll_fetch(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.rx else {
            return;
        };
        match rx.try_recv() {
            Ok(FetchMsg::Ok {
                channel_id,
                articles,
            }) => {
                self.fetching = false;
                self.rx = None;
                if self.selected.as_deref() == Some(channel_id.as_str()) {
                    let n = articles.len();
                    self.articles = articles;
                    self.status = if n == 0 {
                        format!("No headlines for '{channel_id}'")
                    } else {
                        format!("{n} headlines — {channel_id}")
                    };
                }
                ctx.request_repaint();
            }
            Ok(FetchMsg::Err {
                channel_id,
                message,
            }) => {
                self.fetching = false;
                self.rx = None;
                if self.selected.as_deref() == Some(channel_id.as_str()) {
                    self.articles.clear();
                    self.status = format!("Error ({channel_id}): {message}");
                }
                ctx.request_repaint();
            }
            Err(TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
            Err(TryRecvError::Disconnected) => {
                self.fetching = false;
                self.rx = None;
                self.status = "Fetch thread disconnected".into();
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_fetch(ctx);

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("ChannelCast");
                ui.separator();
                ui.label(egui::RichText::new("on-demand news").weak());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let can_refresh = self.selected.is_some() && !self.fetching;
                    if ui
                        .add_enabled(can_refresh, egui::Button::new("Refresh"))
                        .clicked()
                    {
                        if let Some(id) = self.selected.clone() {
                            self.start_fetch(id);
                        }
                    }
                });
            });
            ui.label(egui::RichText::new(&self.status).small());
        });

        egui::SidePanel::left("channels")
            .resizable(true)
            .default_width(180.0)
            .show(ctx, |ui| {
                ui.heading("Channels");
                ui.separator();
                let mut switch_to: Option<String> = None;
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for ch in &self.channels {
                        let selected = self.selected.as_deref() == Some(ch.id.as_str());
                        let mut label = ch.name.clone();
                        if ch.personal {
                            label.push_str(" ·");
                        }
                        if ui.selectable_label(selected, &label).clicked() && !selected {
                            switch_to = Some(ch.id.clone());
                        }
                        if !ch.description.is_empty() && selected {
                            ui.label(egui::RichText::new(&ch.description).small().weak());
                        }
                    }
                });
                if let Some(id) = switch_to {
                    self.selected = Some(id.clone());
                    if !self.fetching {
                        self.start_fetch(id);
                    }
                }
                ui.separator();
                ui.label(
                    egui::RichText::new("Enabled channels only (like CLI). · = personal")
                        .small()
                        .weak(),
                );
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            let title = self
                .selected
                .as_ref()
                .and_then(|id| self.channels.iter().find(|c| &c.id == id))
                .map(|c| c.name.as_str())
                .unwrap_or("Headlines");
            ui.heading(title);
            ui.separator();

            if self.fetching {
                ui.spinner();
                ui.label("Loading…");
                return;
            }

            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, a) in self.articles.iter().enumerate() {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(format!("{}.", i + 1)).weak());
                            ui.strong(&a.title);
                        });
                        let when = a
                            .published
                            .map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string())
                            .unwrap_or_else(|| "unknown date".into());
                        ui.label(
                            egui::RichText::new(format!("{} · {}", a.source, when))
                                .small()
                                .weak(),
                        );
                        if let Some(sum) = a.summary.as_deref() {
                            let brief: String = sum.chars().take(160).collect();
                            if !brief.is_empty() {
                                ui.label(egui::RichText::new(brief).small());
                            }
                        }
                        ui.horizontal(|ui| {
                            if ui.link("Open in browser").clicked() {
                                if let Err(e) = open::that(&a.url) {
                                    self.status = format!("Could not open URL: {e}");
                                }
                            }
                            ui.label(egui::RichText::new(&a.url).small().weak());
                        });
                    });
                    ui.add_space(4.0);
                }
            });
        });
    }
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

fn main() -> eframe::Result<()> {
    let config = match load_config(None) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("channelcast-desktop: failed to load config: {e:#}");
            std::process::exit(1);
        }
    };

    let cache_path = PathBuf::from(".channelcast/cache.db");
    if let Some(parent) = cache_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([960.0, 640.0])
            .with_title("ChannelCast"),
        ..Default::default()
    };

    eframe::run_native(
        "ChannelCast",
        options,
        Box::new(move |_cc| Ok(Box::new(App::new(config, cache_path)))),
    )
}
