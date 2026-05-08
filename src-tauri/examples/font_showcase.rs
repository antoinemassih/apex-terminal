//! Standalone font comparison tool. Runs via:
//!   cargo run --example font_showcase
//!
//! Compares candidate monospace fonts for Apex Terminal:
//!   - JetBrains Mono (current default)
//!   - Commit Mono (free Berkeley alternative)
//!   - Geist Mono (Vercel)
//!   - IBM Plex Mono (Berkeley's ancestor)
//!   - Cascadia Code (Microsoft)
//!
//! NOT embedded in the apex-native binary. This tool exists purely
//! for visual comparison / picking the right font.

use std::sync::Arc;

use eframe::egui;
use egui::{Color32, FontFamily, FontId, RichText};

const CANDIDATES: &[(&str, &str, &str)] = &[
    ("jetbrains", "JetBrains Mono", "JetBrainsMono-Regular.ttf"),
    ("commit", "Commit Mono", "CommitMono-400-Regular.otf"),
    ("geist", "Geist Mono", "GeistMono-Regular.ttf"),
    ("plex", "IBM Plex Mono", "IBMPlexMono-Regular.ttf"),
    ("cascadia", "Cascadia Code", "CascadiaCode-Regular.ttf"),
];

fn main() -> eframe::Result<()> {
    let mut native_options = eframe::NativeOptions::default();
    native_options.viewport = native_options
        .viewport
        .with_inner_size([1600.0, 1000.0])
        .with_title("Apex Font Showcase");

    eframe::run_native(
        "Apex Font Showcase",
        native_options,
        Box::new(|cc| {
            let loaded = install_fonts(&cc.egui_ctx);
            Ok(Box::new(App::new(loaded)))
        }),
    )
}

/// Returns list of (key, display_name) for fonts that successfully loaded.
fn install_fonts(ctx: &egui::Context) -> Vec<(&'static str, &'static str)> {
    let mut fonts = egui::FontDefinitions::default();
    let font_dir = std::path::PathBuf::from("examples/fonts");

    let mut loaded: Vec<(&'static str, &'static str)> = Vec::new();

    for (key, display, file) in CANDIDATES {
        let path = font_dir.join(file);
        match std::fs::read(&path) {
            Ok(data) => {
                fonts
                    .font_data
                    .insert((*key).to_string(), Arc::new(egui::FontData::from_owned(data)));
                fonts.families.insert(
                    FontFamily::Name((*key).into()),
                    vec![(*key).to_string()],
                );
                loaded.push((*key, *display));
            }
            Err(e) => {
                eprintln!(
                    "[font_showcase] could not load {} ({}): {}",
                    display,
                    path.display(),
                    e
                );
            }
        }
    }

    ctx.set_fonts(fonts);
    loaded
}

struct App {
    loaded: Vec<(&'static str, &'static str)>,
    size: f32,
    dark: bool,
    solo: Option<usize>,
}

impl App {
    fn new(loaded: Vec<(&'static str, &'static str)>) -> Self {
        Self {
            loaded,
            size: 13.0,
            dark: true,
            solo: None,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.dark {
            ctx.set_visuals(egui::Visuals::dark());
        } else {
            ctx.set_visuals(egui::Visuals::light());
        }

        egui::TopBottomPanel::top("controls").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Sample size:");
                ui.add(egui::Slider::new(&mut self.size, 9.0..=22.0).step_by(0.5));
                ui.separator();
                ui.checkbox(&mut self.dark, "Dark background");
                ui.separator();
                ui.label("Solo:");
                let mut solo_idx: i32 = self.solo.map(|v| v as i32).unwrap_or(-1);
                egui::ComboBox::from_id_salt("solo_combo")
                    .selected_text(if solo_idx < 0 {
                        "All".to_string()
                    } else {
                        self.loaded
                            .get(solo_idx as usize)
                            .map(|(_, d)| d.to_string())
                            .unwrap_or_else(|| "All".to_string())
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut solo_idx, -1, "All");
                        for (i, (_, display)) in self.loaded.iter().enumerate() {
                            ui.selectable_value(&mut solo_idx, i as i32, *display);
                        }
                    });
                self.solo = if solo_idx < 0 {
                    None
                } else {
                    Some(solo_idx as usize)
                };
                ui.separator();
                ui.label(format!("Loaded: {}/{}", self.loaded.len(), CANDIDATES.len()));
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::both().show(ui, |ui| {
                self.show_comparison(ui);
            });
        });
    }
}

impl App {
    fn show_comparison(&self, ui: &mut egui::Ui) {
        let visible: Vec<(&'static str, &'static str)> = match self.solo {
            Some(i) => self
                .loaded
                .get(i)
                .copied()
                .map(|x| vec![x])
                .unwrap_or_default(),
            None => self.loaded.clone(),
        };

        if visible.is_empty() {
            ui.label("No fonts loaded. Make sure examples/fonts/ contains the .ttf/.otf files.");
            return;
        }

        let total_w = ui.available_width();
        let col_w = (total_w / visible.len() as f32).max(180.0);

        ui.horizontal_top(|ui| {
            for (key, display) in &visible {
                ui.allocate_ui_with_layout(
                    egui::vec2(col_w, ui.available_height()),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        column(ui, key, display, self.size);
                    },
                );
            }
        });
    }
}

fn fid(key: &'static str, size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(key.into()))
}

fn column(ui: &mut egui::Ui, key: &'static str, display: &str, base_size: f32) {
    ui.vertical(|ui| {
        // Heading in the candidate font itself.
        ui.label(
            RichText::new(display)
                .font(fid(key, 20.0))
                .color(Color32::from_rgb(255, 200, 80)),
        );
        ui.add_space(2.0);
        ui.label(
            RichText::new(format!("[ {} ]", key))
                .font(FontId::new(11.0, FontFamily::Proportional))
                .color(Color32::GRAY),
        );
        ui.separator();

        section(ui, "Pangram (multiple sizes)");
        for sz in [11.0_f32, 13.0, 15.0, 18.0] {
            ui.label(
                RichText::new(format!(
                    "{:>4.0}: The quick brown fox jumps over the lazy dog 0123456789",
                    sz
                ))
                .font(fid(key, sz)),
            );
        }
        ui.add_space(6.0);

        section(ui, "Trading numerics (key test)");
        let rows = [
            "SPY        $574.32   +1.24%  RVOL 0.85  IV 12.4%",
            "AAPL       $218.45   -0.18%  RVOL 1.30  IV 18.2%",
            "NVDA       $138.07   +2.85%  RVOL 2.10  IV 32.5%",
            "TSLA       $245.91   -3.42%  RVOL 1.85  IV 45.1%",
        ];
        for row in rows {
            ui.label(RichText::new(row).font(fid(key, base_size)));
        }
        ui.add_space(6.0);

        section(ui, "Option chain row");
        ui.label(
            RichText::new("SPY 580C 11/15  $2.45 / $2.48  Δ 0.42  Γ 0.08  ν 0.32  Θ -0.18")
                .font(fid(key, base_size)),
        );
        ui.add_space(6.0);

        section(ui, "Ligatures / symbols");
        ui.label(
            RichText::new("=> != >= <= -> <- == === !== :: |> <| ++ -- /* */ // <=> ")
                .font(fid(key, base_size + 1.0)),
        );
        ui.label(
            RichText::new("if (x != null) { return x?.foo ?? bar; }")
                .font(fid(key, base_size)),
        );
        ui.add_space(6.0);

        section(ui, "OCC tickers (long alphanumeric)");
        let occ = [
            "O:SPY251115C00580000",
            "O:AAPL251212P00220000",
            "O:NVDA251220C00140000",
        ];
        for s in occ {
            ui.label(RichText::new(s).font(fid(key, base_size)));
        }
        ui.add_space(6.0);

        section(ui, "Digit alignment column");
        let nums = [
            "      1.00",
            "     12.34",
            "    123.45",
            "  1,234.56",
            " 12,345.67",
            "123,456.78",
        ];
        for n in nums {
            ui.label(RichText::new(n).font(fid(key, base_size)));
        }
        ui.add_space(6.0);

        section(ui, "Confusables: 0Oo 1lI |!i ;: `'\"");
        ui.label(
            RichText::new("0Oo 1lI |!i ;: `'\" {}[]() <>/\\")
                .font(fid(key, base_size + 2.0)),
        );

        ui.add_space(20.0);
    });
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.label(
        RichText::new(title)
            .font(FontId::new(10.0, FontFamily::Proportional))
            .color(Color32::from_rgb(120, 180, 255)),
    );
}

