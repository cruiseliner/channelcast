//! ChannelCast desktop — PointCast-inspired on-demand news UI (egui/eframe).
//! Channel strip + headlines; opens article URLs in the system browser.

use anyhow::{Context, Result};
use channelcast_core::{headlines_for_channel, Article, Channel, ChannelConfig};
use chrono::{DateTime, Local, Utc};
use eframe::egui::{self, Color32, FontId, RichText, Sense, Stroke, Vec2};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

const HEADLINE_LIMIT: usize = 40;

/// Dark “news ticker” palette.
mod theme {
    use eframe::egui::Color32;
    pub const BG: Color32 = Color32::from_rgb(12, 14, 18);
    pub const PANEL: Color32 = Color32::from_rgb(18, 22, 28);
    pub const STRIP: Color32 = Color32::from_rgb(22, 28, 36);
    pub const SELECTED: Color32 = Color32::from_rgb(28, 70, 110);
    pub const SELECTED_BORDER: Color32 = Color32::from_rgb(64, 160, 220);
    pub const HOVER: Color32 = Color32::from_rgb(32, 40, 52);
    pub const ACCENT: Color32 = Color32::from_rgb(80, 180, 255);
    pub const PERSONAL: Color32 = Color32::from_rgb(180, 140, 60);
    pub const MUTED: Color32 = Color32::from_rgb(140, 150, 165);
    pub const TITLE: Color32 = Color32::from_rgb(230, 235, 240);
    pub const STATUS_BG: Color32 = Color32::from_rgb(8, 10, 14);
    pub const ERR: Color32 = Color32::from_rgb(220, 90, 90);
    pub const OK: Color32 = Color32::from_rgb(90, 190, 120);
}

enum FetchMsg {
    Ok {
        channel_id: String,
        articles: Vec<Article>,
        fetched_at: DateTime<Utc>,
    },
    Err {
        channel_id: String,
        message: String,
        fetched_at: DateTime<Utc>,
    },
}

struct App {
    config: ChannelConfig,
    channels: Vec<Channel>,
    selected: Option<String>,
    articles: Vec<Article>,
    selected_headline: usize,
    scroll_selected: bool,
    status: String,
    status_is_err: bool,
    fetching: bool,
    last_fetch_at: Option<DateTime<Utc>>,
    cache_path: PathBuf,
    rx: Option<Receiver<FetchMsg>>,
    /// When set, capture a PNG after headlines paint, then quit.
    screenshot_path: Option<PathBuf>,
    screenshot_requested: bool,
    settle_frames: u8,
    quit_after_screenshot: bool,
}

impl App {
    fn new(
        config: ChannelConfig,
        cache_path: PathBuf,
        initial_channel: Option<String>,
        screenshot_path: Option<PathBuf>,
    ) -> Self {
        let channels: Vec<Channel> = config.listed(false).into_iter().cloned().collect();
        let selected = initial_channel
            .filter(|id| channels.iter().any(|c| &c.id == id))
            .or_else(|| channels.first().map(|c| c.id.clone()));
        let mut app = Self {
            config,
            channels,
            selected,
            articles: Vec::new(),
            selected_headline: 0,
            scroll_selected: false,
            status: "Ready — pick a channel or press Refresh.".into(),
            status_is_err: false,
            fetching: false,
            last_fetch_at: None,
            cache_path,
            rx: None,
            screenshot_path,
            screenshot_requested: false,
            settle_frames: 0,
            quit_after_screenshot: false,
        };
        if let Some(id) = app.selected.clone() {
            app.start_fetch(id);
        }
        app
    }

    fn maybe_request_screenshot(&mut self, ctx: &egui::Context) {
        if self.screenshot_path.is_none() || self.screenshot_requested || self.fetching {
            return;
        }
        // Wait until we have headlines (or a settled empty/error state) and a few paint frames.
        if self.last_fetch_at.is_none() {
            return;
        }
        if self.settle_frames < 4 {
            self.settle_frames += 1;
            ctx.request_repaint();
            return;
        }
        self.screenshot_requested = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        ctx.request_repaint();
    }

    fn handle_screenshot_events(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot_path.clone() else {
            return;
        };
        let image = ctx.input(|i| {
            i.events.iter().find_map(|e| {
                if let egui::Event::Screenshot { image, .. } = e {
                    Some(image.clone())
                } else {
                    None
                }
            })
        });
        let Some(image) = image else {
            return;
        };
        let [w, h] = image.size;
        let raw = image.as_raw();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match image::save_buffer(&path, raw, w as u32, h as u32, image::ColorType::Rgba8) {
            Ok(()) => {
                eprintln!(
                    "channelcast-desktop: wrote screenshot {} ({}x{})",
                    path.display(),
                    w,
                    h
                );
                self.status = format!("Screenshot saved: {}", path.display());
                self.status_is_err = false;
            }
            Err(e) => {
                eprintln!("channelcast-desktop: screenshot save failed: {e:#}");
                self.status = format!("Screenshot failed: {e}");
                self.status_is_err = true;
            }
        }
        self.quit_after_screenshot = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn apply_visuals(ctx: &egui::Context) {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = theme::PANEL;
        visuals.window_fill = theme::BG;
        visuals.extreme_bg_color = theme::BG;
        visuals.faint_bg_color = theme::STRIP;
        visuals.override_text_color = Some(theme::TITLE);
        visuals.widgets.noninteractive.bg_fill = theme::STRIP;
        visuals.widgets.inactive.bg_fill = theme::STRIP;
        visuals.widgets.hovered.bg_fill = theme::HOVER;
        visuals.widgets.active.bg_fill = theme::SELECTED;
        visuals.selection.bg_fill = theme::SELECTED;
        visuals.selection.stroke = Stroke::new(1.0_f32, theme::SELECTED_BORDER);
        ctx.set_visuals(visuals);

        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = Vec2::new(8.0, 6.0);
        style.spacing.button_padding = Vec2::new(10.0, 5.0);
        ctx.set_style(style);
    }

    fn start_fetch(&mut self, channel_id: String) {
        if self.fetching {
            return;
        }
        let Some(ch) = self.config.get(&channel_id).ok().cloned() else {
            self.status = format!("Unknown channel '{channel_id}'");
            self.status_is_err = true;
            return;
        };
        if ch.feeds.is_empty() {
            self.status = format!("Channel '{channel_id}' has no feeds yet");
            self.status_is_err = true;
            self.articles.clear();
            self.selected_headline = 0;
            return;
        }

        self.fetching = true;
        self.status_is_err = false;
        self.status = format!("Fetching {channel_id}…");
        self.articles.clear();
        self.selected_headline = 0;

        let config = self.config.clone();
        let cache = self.cache_path.clone();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);

        thread::spawn(move || {
            let path = Some(cache.as_path());
            let fetched_at = Utc::now();
            let msg = match headlines_for_channel(&config, &channel_id, HEADLINE_LIMIT, path) {
                Ok(articles) => FetchMsg::Ok {
                    channel_id,
                    articles,
                    fetched_at,
                },
                Err(e) => FetchMsg::Err {
                    channel_id,
                    message: e.to_string(),
                    fetched_at,
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
                fetched_at,
            }) => {
                self.fetching = false;
                self.rx = None;
                self.last_fetch_at = Some(fetched_at);
                if self.selected.as_deref() == Some(channel_id.as_str()) {
                    let n = articles.len();
                    self.articles = articles;
                    self.selected_headline = 0;
                    self.status_is_err = false;
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
                fetched_at,
            }) => {
                self.fetching = false;
                self.rx = None;
                self.last_fetch_at = Some(fetched_at);
                if self.selected.as_deref() == Some(channel_id.as_str()) {
                    self.articles.clear();
                    self.selected_headline = 0;
                    self.status_is_err = true;
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
                self.status_is_err = true;
                self.status = "Fetch thread disconnected".into();
            }
        }
    }

    fn open_selected(&mut self) {
        let Some(a) = self.articles.get(self.selected_headline) else {
            return;
        };
        if let Err(e) = open::that(&a.url) {
            self.status_is_err = true;
            self.status = format!("Could not open URL: {e}");
        } else {
            self.status_is_err = false;
            self.status = format!("Opened: {}", a.title);
        }
    }

    fn move_headline(&mut self, delta: i32) {
        if self.articles.is_empty() {
            return;
        }
        let n = self.articles.len() as i32;
        let cur = self.selected_headline as i32;
        let next = (cur + delta).rem_euclid(n) as usize;
        self.selected_headline = next;
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        if self.fetching {
            return;
        }
        let (down, up, enter, next_ch, prev_ch, refresh) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::J) || i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::K) || i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::CloseBracket),
                i.key_pressed(egui::Key::OpenBracket),
                i.key_pressed(egui::Key::R) && !i.modifiers.ctrl && !i.modifiers.command,
            )
        });
        if down {
            self.move_headline(1);
            self.scroll_selected = true;
        }
        if up {
            self.move_headline(-1);
            self.scroll_selected = true;
        }
        if enter {
            self.open_selected();
        }
        if next_ch {
            self.cycle_channel(1);
        }
        if prev_ch {
            self.cycle_channel(-1);
        }
        if refresh {
            if let Some(id) = self.selected.clone() {
                self.start_fetch(id);
            }
        }
    }

    fn cycle_channel(&mut self, delta: i32) {
        if self.channels.is_empty() || self.fetching {
            return;
        }
        let cur = self
            .selected
            .as_ref()
            .and_then(|id| self.channels.iter().position(|c| &c.id == id))
            .unwrap_or(0) as i32;
        let n = self.channels.len() as i32;
        let next = (cur + delta).rem_euclid(n) as usize;
        let id = self.channels[next].id.clone();
        self.selected = Some(id.clone());
        self.start_fetch(id);
    }

    fn format_when(published: Option<DateTime<Utc>>) -> String {
        match published {
            Some(t) => t.with_timezone(&Local).format("%a %b %d · %I:%M %p").to_string(),
            None => "time unknown".into(),
        }
    }

    fn status_bar_text(&self) -> String {
        let fetch = match self.last_fetch_at {
            Some(t) => {
                let local = t.with_timezone(&Local);
                format!("last fetch {}", local.format("%I:%M:%S %p"))
            }
            None => "not fetched yet".into(),
        };
        if self.fetching {
            format!("fetching… · {fetch}")
        } else if self.status_is_err {
            format!("{} · {fetch}", self.status)
        } else {
            format!("{} · {fetch}", self.status)
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        Self::apply_visuals(ctx);
        self.poll_fetch(ctx);
        self.handle_keys(ctx);
        self.maybe_request_screenshot(ctx);
        self.handle_screenshot_events(ctx);
        if self.quit_after_screenshot {
            return;
        }

        // Top ticker bar
        egui::TopBottomPanel::top("top")
            .frame(
                egui::Frame::NONE
                    .fill(theme::STATUS_BG)
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(30, 38, 48))),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("CHANNELCAST")
                            .font(FontId::proportional(18.0))
                            .color(theme::ACCENT)
                            .strong(),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("on-demand news")
                            .small()
                            .color(theme::MUTED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let can_refresh = self.selected.is_some() && !self.fetching;
                        let btn = egui::Button::new(
                            RichText::new(if self.fetching { "…" } else { "Refresh" })
                                .color(theme::TITLE),
                        )
                        .fill(theme::STRIP)
                        .stroke(Stroke::new(1.0_f32, theme::SELECTED_BORDER));
                        if ui.add_enabled(can_refresh, btn).clicked() {
                            if let Some(id) = self.selected.clone() {
                                self.start_fetch(id);
                            }
                        }
                        ui.label(
                            RichText::new("j/k · Enter open · [ ] channel · r refresh")
                                .small()
                                .color(theme::MUTED),
                        );
                    });
                });
            });

        // Bottom status
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::NONE
                    .fill(theme::STATUS_BG)
                    .inner_margin(egui::Margin::symmetric(12, 6))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(30, 38, 48))),
            )
            .show(ctx, |ui| {
                let color = if self.fetching {
                    theme::ACCENT
                } else if self.status_is_err {
                    theme::ERR
                } else {
                    theme::OK
                };
                ui.horizontal(|ui| {
                    if self.fetching {
                        ui.spinner();
                    }
                    ui.label(
                        RichText::new(self.status_bar_text())
                            .small()
                            .color(color),
                    );
                });
            });

        // Channel strip (left)
        egui::SidePanel::left("channels")
            .resizable(true)
            .default_width(200.0)
            .frame(
                egui::Frame::NONE
                    .fill(theme::STRIP)
                    .inner_margin(egui::Margin::symmetric(8, 10))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(30, 38, 48))),
            )
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("CHANNELS")
                        .small()
                        .strong()
                        .color(theme::MUTED),
                );
                ui.add_space(6.0);

                let mut switch_to: Option<String> = None;
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for ch in &self.channels {
                            let selected = self.selected.as_deref() == Some(ch.id.as_str());
                            let (rect, response) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width(), 36.0),
                                Sense::click(),
                            );

                            let fill = if selected {
                                theme::SELECTED
                            } else if response.hovered() {
                                theme::HOVER
                            } else {
                                Color32::TRANSPARENT
                            };
                            ui.painter().rect_filled(rect, 4.0, fill);
                            if selected {
                                let accent = egui::Rect::from_min_size(
                                    rect.min,
                                    Vec2::new(3.0, rect.height()),
                                );
                                ui.painter().rect_filled(accent, 0.0, theme::ACCENT);
                            }

                            let text_pos = rect.min + Vec2::new(10.0, 8.0);
                            ui.painter().text(
                                text_pos,
                                egui::Align2::LEFT_TOP,
                                &ch.name,
                                FontId::proportional(14.0),
                                if selected {
                                    theme::TITLE
                                } else {
                                    Color32::from_rgb(200, 208, 218)
                                },
                            );
                            if ch.personal {
                                ui.painter().text(
                                    egui::pos2(rect.max.x - 8.0, rect.min.y + 10.0),
                                    egui::Align2::RIGHT_TOP,
                                    "●",
                                    FontId::proportional(10.0),
                                    theme::PERSONAL,
                                );
                            }

                            if response.clicked() && !selected {
                                switch_to = Some(ch.id.clone());
                            }
                            if selected && !ch.description.is_empty() {
                                ui.add_space(2.0);
                                ui.label(
                                    RichText::new(&ch.description)
                                        .small()
                                        .color(theme::MUTED),
                                );
                                ui.add_space(4.0);
                            }
                        }
                    });

                if let Some(id) = switch_to {
                    self.selected = Some(id.clone());
                    if !self.fetching {
                        self.start_fetch(id);
                    }
                }

                ui.add_space(8.0);
                ui.separator();
                ui.label(
                    RichText::new("● personal / local")
                        .small()
                        .color(theme::PERSONAL),
                );
            });

        // Headlines
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(theme::BG)
                    .inner_margin(egui::Margin::symmetric(14, 10)),
            )
            .show(ctx, |ui| {
                let ch = self
                    .selected
                    .as_ref()
                    .and_then(|id| self.channels.iter().find(|c| &c.id == id));
                let title = ch.map(|c| c.name.as_str()).unwrap_or("Headlines");
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(title)
                            .font(FontId::proportional(20.0))
                            .strong()
                            .color(theme::TITLE),
                    );
                    if let Some(c) = ch {
                        if c.personal {
                            ui.label(
                                RichText::new("personal")
                                    .small()
                                    .color(theme::PERSONAL),
                            );
                        }
                    }
                });
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(6.0);

                if self.fetching {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new("Loading headlines…").color(theme::ACCENT));
                    });
                    return;
                }

                if self.articles.is_empty() {
                    ui.label(RichText::new("No headlines.").color(theme::MUTED));
                    return;
                }

                let mut open_idx: Option<usize> = None;
                let selected_idx = self.selected_headline;

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (i, a) in self.articles.iter().enumerate() {
                            let is_sel = i == selected_idx;
                            let fill = if is_sel {
                                theme::SELECTED
                            } else {
                                theme::PANEL
                            };
                            let stroke = if is_sel {
                                Stroke::new(1.0_f32, theme::SELECTED_BORDER)
                            } else {
                                Stroke::new(1.0_f32, Color32::from_rgb(28, 34, 44))
                            };

                            egui::Frame::NONE
                                .fill(fill)
                                .stroke(stroke)
                                .corner_radius(4.0)
                                .inner_margin(egui::Margin::symmetric(10, 8))
                                .show(ui, |ui| {
                                    let response = ui.interact(
                                        ui.max_rect(),
                                        ui.id().with(("hl", i)),
                                        Sense::click(),
                                    );
                                    if response.clicked() {
                                        open_idx = Some(i);
                                    }

                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(format!("{:02}", i + 1))
                                                .monospace()
                                                .color(if is_sel {
                                                    theme::ACCENT
                                                } else {
                                                    theme::MUTED
                                                }),
                                        );
                                        ui.label(
                                            RichText::new(&a.title)
                                                .strong()
                                                .color(theme::TITLE),
                                        );
                                    });
                                    ui.label(
                                        RichText::new(format!(
                                            "{} · {}",
                                            a.source,
                                            Self::format_when(a.published)
                                        ))
                                        .small()
                                        .color(theme::MUTED),
                                    );
                                    if let Some(sum) = a.summary.as_deref() {
                                        let brief: String = sum.chars().take(160).collect();
                                        if !brief.trim().is_empty() {
                                            ui.label(
                                                RichText::new(brief)
                                                    .small()
                                                    .color(Color32::from_rgb(180, 188, 200)),
                                            );
                                        }
                                    }
                                });
                            ui.add_space(6.0);

                            if is_sel && self.scroll_selected {
                                ui.scroll_to_cursor(Some(egui::Align::Center));
                            }
                        }
                    });

                if let Some(i) = open_idx {
                    self.selected_headline = i;
                    self.open_selected();
                }
                self.scroll_selected = false;
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

#[derive(Debug, Default)]
struct CliArgs {
    channel: Option<String>,
    screenshot: Option<PathBuf>,
}

fn parse_args() -> CliArgs {
    let mut args = CliArgs::default();
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--channel" => {
                args.channel = it.next();
            }
            "--screenshot" => {
                args.screenshot = it.next().map(PathBuf::from);
            }
            "-h" | "--help" => {
                eprintln!(
                    "Usage: channelcast-desktop [--channel ID] [--screenshot PATH]\n\n                     --channel ID       Select and fetch this channel on startup\n                     --screenshot PATH  After headlines paint, write PNG and quit"
                );
                std::process::exit(0);
            }
            other => {
                eprintln!("channelcast-desktop: unknown argument '{other}' (try --help)");
                std::process::exit(2);
            }
        }
    }
    args
}

fn main() -> eframe::Result<()> {
    let cli = parse_args();
    let config = match load_config(None) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("channelcast-desktop: failed to load config: {e:#}");
            std::process::exit(1);
        }
    };

    if let Some(id) = &cli.channel {
        if config.get(id).is_err() {
            eprintln!("channelcast-desktop: unknown channel '{id}'");
            std::process::exit(1);
        }
    }

    let cache_path = PathBuf::from(".channelcast/cache.db");
    if let Some(parent) = cache_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 680.0])
            .with_min_inner_size([720.0, 480.0])
            .with_title("ChannelCast"),
        ..Default::default()
    };

    let initial = cli.channel.clone();
    let shot = cli.screenshot.clone();
    eframe::run_native(
        "ChannelCast",
        options,
        Box::new(move |cc| {
            App::apply_visuals(&cc.egui_ctx);
            Ok(Box::new(App::new(config, cache_path, initial, shot)))
        }),
    )
}
