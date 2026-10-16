// Main script (cuz what is a rust crate without main.rs)
// Make sure the motherfucker doesnt spam dumb idiotic warnings
#![allow(warnings)]
// Actually define the modules instead of being blind, wE aRe nOT sAvAgES hErE
mod audio;
mod fft;
mod lyrics;
mod mpris;
mod state;
mod ui;
mod doc;
use font_kit::family::Family;
// Use the shit that actually got pulled instead of sitting in the builder's cache (like a bitch who freeloads and does fuck all)
use font_kit::family_name::FamilyName::{self, Fantasy};
use font_kit::handle::Handle;
use font_kit::properties::Properties;
use font_kit::source::SystemSource;
use eframe::egui;
use rustfft::num_traits::ops::bytes;
use state::SharedState;
use core::convert::From;
use std::path;
use std::sync::{Arc, Mutex};
use ui::PlayerApp;
// Something to do with egui i guess?? actually loads options instead of fucking around.
fn main() -> eframe::Result<()> {
    let shared_state = Arc::new(Mutex::new(SharedState::default()));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_decorations(false)
            .with_transparent(true)
            .with_inner_size([920.0, 640.0])
            .with_min_inner_size([720.0, 500.0])
            .with_title("Aether"),
        vsync: true,
        hardware_acceleration: eframe::HardwareAcceleration::Required,
        ..Default::default()
    };
// The app name (also aether sounds fucking badass goodjob me)
    eframe::run_native(
        "Aether",
        options,
        Box::new(move |cc| {
            load_font(&cc.egui_ctx);
            setup_hyper_minimal_theme(&cc.egui_ctx);
            Ok(Box::new(PlayerApp::new(shared_state)))
        }),
    )
}
// yea its fucking minimal as fuck lets gooo
fn setup_hyper_minimal_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
// A bunch of shit (mostly styling and visuals. but also widgets lets not pretend our app is a music app when all it does is show egui examples )
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.window_margin = egui::Margin::same(0.0);
    style.spacing.slider_width = 160.0;

    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = egui::Color32::from_rgb(10, 10, 12);
    visuals.window_fill = egui::Color32::from_rgb(10, 10, 12);
    visuals.extreme_bg_color = egui::Color32::from_rgb(18, 18, 22);
    visuals.faint_bg_color = egui::Color32::from_rgb(15, 15, 19);

    visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(18, 18, 22);
    visuals.widgets.noninteractive.fg_stroke =
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(150, 150, 160));
    visuals.widgets.noninteractive.rounding = egui::Rounding::same(6.0);

    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(26, 26, 32);
    visuals.widgets.inactive.fg_stroke =
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(200, 200, 210));
    visuals.widgets.inactive.rounding = egui::Rounding::same(6.0);

    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(42, 42, 52);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, egui::Color32::WHITE);
    visuals.widgets.hovered.rounding = egui::Rounding::same(6.0);

    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(245, 245, 250);
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, egui::Color32::BLACK);
    visuals.widgets.active.rounding = egui::Rounding::same(6.0);

    visuals.selection.bg_fill = egui::Color32::from_rgb(235, 235, 245);
    visuals.selection.stroke = egui::Stroke::new(1.0_f32, egui::Color32::BLACK);

    style.visuals = visuals;
    ctx.set_style(style);
}

fn load_font(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let source = SystemSource::new();
    let cjk_fonts = [
        FamilyName::Title("Noto Sans CJK SC".into()),
        FamilyName::Title("Noto Sans CJK JP".into()),
        FamilyName::Title("Noto Sans CJK KR".into()),

    ];

    let mut register = |family: FamilyName, font_key: &'static str, target_family: egui::FontFamily| {
        if let Ok(handle) = source.select_best_match(&[family], &Properties::new()) {
            let data = match handle {
                Handle::Path {path, .. } => std::fs::read(path).ok(),
                Handle::Memory { bytes, .. } => Some((*bytes).clone()),
            };
            if let Some(bytes) = data{
                fonts.font_data.insert(
                    font_key.to_string(),
                    egui::FontData::from_owned(bytes),
                );
                fonts
                    .families
                    .entry(target_family)
                    .or_default()
                    .insert(0, font_key.to_string());
            }
        }
    };
    register(FamilyName::SansSerif, "sys_sans", egui::FontFamily::Proportional);
    register(FamilyName::Monospace, "sys_mono", egui::FontFamily::Monospace);

    if let Ok(handle) = source.select_best_match(&cjk_fonts, &Properties::new()) {
        let (data, font_index) = match handle {
            Handle::Path { path, font_index} => {
                eprintln!("Loaded CJK font: {} (index {})", path.display(), font_index);
                (std::fs::read(path).ok(), font_index)
            }
            

            Handle::Memory { bytes, font_index} => {
                (Some(bytes.to_vec().clone()), font_index)
            }
        };
        if let Some(bytes) = data {
            let mut font_data = egui::FontData::from_owned(bytes);
            font_data.index = font_index;
            fonts.font_data.insert(
                "sys_cjk".to_string(),
                font_data,
            );
            // do the fonts 
            // .families and whatever else again because sys_latin and sys_cjk different
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(1,"sys_cjk".to_string());

        }
        ctx.set_fonts(fonts);
    }
    
}


// End of file, go fuck yourself. (or look at the other files??)