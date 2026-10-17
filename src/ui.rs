// The genital of the app (get it long?? i could have just said skin wtf)

// Makes sure the shit actually gets used instead of slacking the fuck off
use crate::controls::spawn_mpris_server;
use crate::audio::{spawn_audio_engine, AudioCommand};
use crate::lyrics::{current_lyric_index, LyricLine};
use crate::state::{
    AppConfig, SharedState, TrackEntry, VisualizerColorMode, VisualizerStyle, NUM_BARS,
};
use eframe::egui::{
    self, Align, Color32, ColorImage, FontFamily, FontId, Layout, Pos2, Rect, RichText, Rounding,
    Sense, Stroke, TextureHandle, TextureOptions, Vec2,
};
use core::time::Duration;
use std::f32::consts::TAU;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};


// Yay Finally the UI elements that count
#[derive(PartialEq, Eq, Clone, Copy)]
enum ActiveView {
    Visualizer,
    Library,
    Lyrics,
    Settings,
}
// Some shit about the starting view
pub struct PlayerApp {
    state: Arc<Mutex<SharedState>>,
    audio_tx: Sender<AudioCommand>,
    active_view: ActiveView,

    library_path_input: String,
    track_filter: String,
    scrub_dragging_val: Option<f64>,

    cover_texture: Option<TextureHandle>,
    loaded_cover_id: u64,
    now_playing_expanded: bool,
}
// Audio actually gets spawned here (cuz if no audio then how music player?)
impl PlayerApp {
    pub fn new(state: Arc<Mutex<SharedState>>) -> Self {
        let library_path_input = state.lock().unwrap().config.library_path.clone();
        let audio_tx = spawn_audio_engine(Arc::clone(&state));
        // spawn_mpris_server(audio_tx.clone(),
        // Arc::clone(&state));
        // apparently no?

// More UI shit (actually sets first view as visualizer. Also the shit that i call **Expanded Now Playing**)
        Self {
            state,
            audio_tx,
            active_view: ActiveView::Visualizer,
            library_path_input,
            track_filter: String::new(),
            scrub_dragging_val: None,
            cover_texture: None,
            loaded_cover_id: 0,
            now_playing_expanded: false,
        }
    }
// Cover shit 
    fn sync_cover_texture(&mut self, ctx: &egui::Context) {
        let (art_id, maybe_art) = {
            let st = self.state.lock().unwrap();
            if st.cover_art_id != self.loaded_cover_id {
                (st.cover_art_id, st.cover_art.clone())
            } else {
                return;
            }
        };

        self.loaded_cover_id = art_id;
        if let Some(art) = maybe_art {
            let color_img = ColorImage::from_rgba_unmultiplied([art.width, art.height], &art.rgba);
            self.cover_texture =
                Some(ctx.load_texture("track_cover_art", color_img, TextureOptions::LINEAR));
        } else {
            self.cover_texture = None;
        }
    }
}

impl eframe::App for PlayerApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }
// egui scares me
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.sync_cover_texture(ctx);

        let (
            current_track,
            is_playing,
            elapsed,
            duration,
            spectrum,
            peaks,
            bass_impulse,
            lyrics,
            lyrics_source,
            is_fetching_lyrics,
            devices,
            config,
            is_scanning,
            status_msg,
            track_count,
        ) = {
            let st = self.state.lock().unwrap();
            (
                st.current_track.clone(),
                st.is_playing,
                st.elapsed,
                st.duration,
                st.spectrum,
                st.peaks,
                st.bass_impulse,
                st.lyrics.clone(),
                st.lyrics_source.clone(),
                st.is_fetching_lyrics,
                st.available_devices.clone(),
                st.config.clone(),
                st.is_scanning,
                st.status_msg.clone(),
                st.track_list.len(),
            )
        };

        if is_playing || spectrum.iter().any(|&v| v > 0.005) {
            ctx.request_repaint_after(Duration::from_millis(14));
        } else {
            ctx.request_repaint_after(Duration::from_millis(14));
        }

        if !ctx.wants_keyboard_input() {
            if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
                let _ = self.audio_tx.send(AudioCommand::TogglePause);
            }
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                let target = (elapsed + Duration::from_secs(5)).min(duration);
                let _ = self.audio_tx.send(AudioCommand::Seek(target));
            }
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                let target = elapsed.saturating_sub(Duration::from_secs(5));
                let _ = self.audio_tx.send(AudioCommand::Seek(target));
            }
        }

        let outer_frame = egui::Frame::none()
            .fill(Color32::from_rgb(10, 10, 12))
            .rounding(Rounding::same(12.0))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(32, 32, 40)))
            .inner_margin(egui::Margin::symmetric(24.0, 16.0));

        egui::CentralPanel::default()
            .frame(outer_frame)
            .show(ctx, |ui| {
                self.render_top_nav(ui, ctx, track_count);

                ui.add_space(6.0);
                ui.separator();

                // Persistent Footer with Now Playing Cover Art Card + Scrub Bar
                egui::TopBottomPanel::bottom("global_scrub_footer")
                    .frame(
                        egui::Frame::none()
                            .fill(Color32::from_rgb(10, 10, 12))
                            .inner_margin(egui::Margin::symmetric(0.0, 8.0)),
                    )
                    .show_inside(ui, |ui| {
                        self.render_bottom_now_playing_bar(
                            ui,
                            current_track.as_ref(),
                            is_playing,
                            elapsed,
                            duration,
                            bass_impulse,
                        );
                    });

                egui::CentralPanel::default()
                    .frame(egui::Frame::none().inner_margin(egui::Margin::symmetric(8.0, 10.0)))
                    .show_inside(ui, |ui| match self.active_view {
                        ActiveView::Visualizer => {
                            self.render_visualizer_view(
                                ui,
                                current_track.as_ref(),
                                &spectrum,
                                &peaks,
                                bass_impulse,
                                &config,
                                track_count,
                            );
                        }
                        ActiveView::Library => {
                            self.render_library_view(ui, is_scanning, &status_msg);
                        }
                        ActiveView::Lyrics => {
                            self.render_lyrics_view(
                                ui,
                                current_track.as_ref(),
                                &lyrics,
                                &lyrics_source,
                                is_fetching_lyrics,
                                elapsed,
                            );
                        }
                        ActiveView::Settings => {
                            self.render_settings_view(
                                ui,
                                &devices,
                                &config,
                                is_scanning,
                                &status_msg,
                            );
                        }
                    });
            });
    }
}
// lets imply that this is a player app (god i hate impl so much this is the crux of my errors)
impl PlayerApp {
    fn render_top_nav(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, track_count: usize) {
        let nav_height = 32.0;
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), nav_height),
            Sense::click_and_drag(),
        );
// Top Bar code ends
// Drag shit (that didnt work)
        if response.drag_started_by(egui::PointerButton::Primary) {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }

        let mut child_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::left_to_right(Align::Center)),
        );
// The beeg boy name, that can be changed.
        child_ui.label(
            RichText::new("A E T H E R")
                .font(FontId::new(11.5, FontFamily::Monospace))
                .color(Color32::from_rgb(110, 110, 125)),
        );
// Tab items
        child_ui.with_layout(Layout::top_down(Align::Center), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                self.nav_pill(ui, "VISUALIZER", ActiveView::Visualizer);
                self.nav_pill(
                    ui,
                    &format!("LIBRARY ({})", track_count),
                    ActiveView::Library,
                );
                self.nav_pill(ui, "LYRICS", ActiveView::Lyrics);
                self.nav_pill(ui, "SETTINGS", ActiveView::Settings);
            });
        });

        let mut right_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::right_to_left(Align::Center)),
        );

        if right_ui
            .add(
                egui::Button::new(
                    RichText::new("✕")
                        .size(13.0)
                        .color(Color32::from_rgb(160, 160, 175)),
                )
                .frame(false),
            )
            .on_hover_text("Quit")
            .clicked()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        right_ui.add_space(6.0);

        if right_ui
            .add(
                egui::Button::new(
                    RichText::new("—")
                        .size(13.0)
                        .color(Color32::from_rgb(160, 160, 175)),
                )
                .frame(false),
            )
            .on_hover_text("Minimize")
            .clicked()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
    }
// NavPill??
    fn nav_pill(&mut self, ui: &mut egui::Ui, label: &str, target: ActiveView) {
        let selected = self.active_view == target;
        let text_color = if selected {
            Color32::WHITE
        } else {
            Color32::from_rgb(115, 115, 130)
        };

        let btn = egui::Button::new(
            RichText::new(label)
                .font(FontId::new(11.5, FontFamily::Proportional))
                .color(text_color),
        )
        .fill(if selected {
            Color32::from_rgb(28, 28, 36)
        } else {
            Color32::TRANSPARENT
        })
        .stroke(if selected {
            Stroke::new(1.0_f32, Color32::from_rgb(60, 60, 75))
        } else {
            Stroke::NONE
        })
        .rounding(Rounding::same(6.0));

        if ui.add(btn).clicked() {
            self.active_view = target;
        }
    }
// the fft boom boom haha skrttt
    fn render_visualizer_view(
        &mut self,
        ui: &mut egui::Ui,
        current_track: Option<&TrackEntry>,
        spectrum: &[f32; NUM_BARS],
        peaks: &[f32; NUM_BARS],
        bass_impulse: f32,
        config: &AppConfig,
        track_count: usize,
    ) {
        ui.vertical_centered(|ui| {
            let avail_h = ui.available_height();
            ui.add_space((avail_h * 0.02).max(2.0));

            // the boom boom fucker
            let art_dim = if config.viz_style == VisualizerStyle::RadialPulse {
                (avail_h * 0.48).clamp(160.0, 240.0)
            } else {
                (avail_h * 0.42).clamp(145.0, 215.0)
            };

            let (art_rect, art_resp) =
                ui.allocate_exact_size(Vec2::splat(art_dim), Sense::click());

            let painter = ui.painter();

            // If RadialView is alive and not rotting, draw 360-degree frequency rays around the center block
            if config.viz_style == VisualizerStyle::RadialPulse {
                self.draw_radial_halo(painter, art_rect.center(), art_dim * 0.52, spectrum, config);
            }

            // Sub-bass border luminance thing idk boom boom
            let border_lum = (45.0 + bass_impulse * 175.0).clamp(45.0, 255.0) as u8;
            let border_color = self.resolve_bar_color(0, NUM_BARS, bass_impulse.min(1.0), config);

            painter.rect(
                art_rect,
                Rounding::same(14.0),
                Color32::from_rgb(16, 16, 20),
                Stroke::new(
                    1.2_f32 + bass_impulse * 1.8,
                    Color32::from_rgb(
                        border_lum.max(border_color.r() / 2),
                        border_lum.max(border_color.g() / 2),
                        border_lum.max(border_color.b() / 2),
                    ),
                ),
            );

            // Render the boom boom if no cover art/overlay is enabled (dont really matter now cuz visualizer doesnt render the cover art sooo)
            if self.cover_texture.is_none() || config.show_speaker_overlay {
                self.draw_reactive_speaker_driver(
                    painter,
                    art_rect.center(),
                    art_dim,
                    bass_impulse,
                    spectrum,
                    &config
                );
            }
// Make sure retards know what this shit does haha
            if art_resp.on_hover_text("Click to Play / Pause").clicked() {
                let _ = self.audio_tx.send(AudioCommand::TogglePause);
            }

            ui.add_space(14.0);

            // Track info from well. info Metadata. are you fucking stupid.
            if let Some(track) = current_track {
                ui.label(
                    RichText::new(&track.title)
                        .font(FontId::new(20.5, FontFamily::Proportional))
                        .color(Color32::WHITE)
                        .strong(),
                );
                ui.add_space(2.0);
                ui.label(
                    RichText::new(format!(
                        "{}  •  {}",
                        track.artist.to_uppercase(),
                        track.album
                    ))
                    .font(FontId::new(11.5, FontFamily::Monospace))
                    .color(Color32::from_rgb(135, 135, 152)),
                );
            } else {
              // If nothing playing, does this.
                ui.label(
                    RichText::new("No Track Active")
                        .font(FontId::new(20.0, FontFamily::Proportional))
                        .color(Color32::from_rgb(190, 190, 205)),
                );
                ui.add_space(4.0);
                if track_count > 0 {
                    if ui
                        .button(
                            RichText::new(format!("▶ Play Library ({} tracks)", track_count))
                                .size(12.5),
                        )
                        .clicked()
                    {
                        let _ = self.audio_tx.send(AudioCommand::PlayTrack(0));
            //Actually gives the button for config.
                    }
                } else if ui
                    .button(RichText::new("Configure Music Library Path →").size(12.0))
                    .clicked()
                {
                    self.active_view = ActiveView::Settings;
                }
            }

            ui.add_space(16.0);

            // FFT shit that actually loads.
            let spec_width = ui.available_width().min(680.0);
            let spec_height = ui.available_height().clamp(65.0, 140.0);
            let (spec_rect, _) =
                ui.allocate_exact_size(Vec2::new(spec_width, spec_height), Sense::hover());

            let painter = ui.painter_at(spec_rect);
            let active_bars = config.viz_bar_count.clamp(16, NUM_BARS);

         // Yay finally visualizer styles and not one like a basic bitch
          match config.viz_style {
                VisualizerStyle::ClassicBars | VisualizerStyle::RadialPulse => {
                    let gap = 4.0;
                    let bar_w = ((spec_rect.width() - gap * (active_bars as f32 - 1.0))
                        / active_bars as f32)
                        .max(2.0);

                    for i in 0..active_bars {
                        let src_idx = (i * NUM_BARS) / active_bars;
                        let amp = spectrum[src_idx].clamp(0.03, 1.0);
                        let peak = peaks[src_idx].clamp(0.03, 1.0);

                        let x = spec_rect.left() + (i as f32) * (bar_w + gap);
                        let bar_h = amp * spec_rect.height();

                        let bar_rect = Rect::from_min_max(
                            Pos2::new(x, spec_rect.bottom() - bar_h),
                            Pos2::new(x + bar_w, spec_rect.bottom()),
                        );

                        let color = self.resolve_bar_color(i, active_bars, amp, config);
                        painter.rect_filled(bar_rect, Rounding::same(2.0), color);

                        // Floating peak-hold cap
                        let peak_y = spec_rect.bottom() - peak * spec_rect.height() - 3.0;
                        if peak_y >= spec_rect.top() {
                            let cap_rect = Rect::from_min_max(
                                Pos2::new(x, peak_y),
                                Pos2::new(x + bar_w, peak_y + 2.0),
                            );
                            painter.rect_filled(
                                cap_rect,
                                Rounding::same(1.0),
                                Color32::from_rgba_unmultiplied(255, 255, 255, 165),
                            );
                        }
                    }
                }

                VisualizerStyle::MirroredWave => {
                    let mid_y = spec_rect.center().y;
                    let gap = 3.0;
                    let bar_w = ((spec_rect.width() - gap * (active_bars as f32 - 1.0))
                        / active_bars as f32)
                        .max(2.0);

                    for i in 0..active_bars {
                        let src_idx = (i * NUM_BARS) / active_bars;
                        let amp = spectrum[src_idx].clamp(0.02, 1.0);
                        let half_h = amp * (spec_rect.height() * 0.48);
                        let x = spec_rect.left() + (i as f32) * (bar_w + gap);

                        let upper_rect = Rect::from_min_max(
                            Pos2::new(x, mid_y - half_h),
                            Pos2::new(x + bar_w, mid_y),
                        );
                        let lower_rect = Rect::from_min_max(
                            Pos2::new(x, mid_y + 1.5),
                            Pos2::new(x + bar_w, mid_y + 1.5 + half_h * 0.65),
                        );

                        let color = self.resolve_bar_color(i, active_bars, amp, config);
                        let reflection = Color32::from_rgba_unmultiplied(
                            color.r() / 2,
                            color.g() / 2,
                            color.b() / 2,
                            110,
                        );

                        painter.rect_filled(upper_rect, Rounding::same(2.0), color);
                        painter.rect_filled(lower_rect, Rounding::same(2.0), reflection);
                    }
                }
// SHAIBARPOONK BEBIIIIIIII
                VisualizerStyle::CyberNeon => {
                    let horizon_y = spec_rect.bottom() - spec_rect.height() * 0.28;
                    let gap = 4.0;
                    let bar_w = ((spec_rect.width() - gap * (active_bars as f32 - 1.0))
                        / active_bars as f32)
                        .max(2.0);

                    // laser sht
                    painter.line_segment(
                        [
                            Pos2::new(spec_rect.left(), horizon_y),
                            Pos2::new(spec_rect.right(), horizon_y),
                        ],
                        Stroke::new(1.0_f32, Color32::from_rgb(90, 220, 255)),
                    );

                    for i in 0..active_bars {
                        let src_idx = (i * NUM_BARS) / active_bars;
                        let amp = spectrum[src_idx].clamp(0.03, 1.0);
                        let main_h = amp * (spec_rect.height() * 0.70);
                        let refl_h = amp * (spec_rect.height() * 0.25);
                        let x = spec_rect.left() + (i as f32) * (bar_w + gap);

                        let color = self.resolve_bar_color(i, active_bars, amp, config);

                        // Outer neon halo
                        let glow_rect = Rect::from_min_max(
                            Pos2::new(x - 1.5, horizon_y - main_h - 1.5),
                            Pos2::new(x + bar_w + 1.5, horizon_y),
                        );
                        painter.rect_filled(
                            glow_rect,
                            Rounding::same(3.0),
                            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 42),
                        );

                        // Main bar
                        let main_rect = Rect::from_min_max(
                            Pos2::new(x, horizon_y - main_h),
                            Pos2::new(x + bar_w, horizon_y),
                        );
                        painter.rect_filled(main_rect, Rounding::same(2.0), color);

                        // Reflect shit (somehow equi has this.)
                        let refl_rect = Rect::from_min_max(
                            Pos2::new(x, horizon_y + 2.0),
                            Pos2::new(x + bar_w, horizon_y + 2.0 + refl_h),
                        );
                        painter.rect_filled(
                            refl_rect,
                            Rounding::same(2.0),
                            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 65),
                        );
                    }
                }
            }
        });
    }

    /// Yay the overreacting boom boom
    fn draw_reactive_speaker_driver(
        &self,
        painter: &egui::Painter,
        center: Pos2,
        art_dim: f32,
        bass_impulse: f32,
        spectrum: &[f32; NUM_BARS],
        config: &AppConfig,
    ) {
        let bounce = bass_impulse.clamp(0.0, 1.45);
        let punch = bounce.powf(1.25); // we hate linear around here
        let mid_energy = (spectrum[6] + spectrum[10] + spectrum[14]) / 3.0;
        let high_energy = (spectrum[26] + spectrum[34] + spectrum[42]) / 3.0;

        // 3d effects on high boom boom shit
        let cone_center = center + Vec2::new(0.0, -punch * 4.5);
        let accent = self.resolve_bar_color(2, NUM_BARS, bounce.min(1.0), config);

        // Pressure shockwaves that arent even accurate (fuck this shit took too long)
        if bounce > 0.12 {
            for ring_idx in 0..2 {
                let phase = (bounce + ring_idx as f32 * 0.22).fract();
                let wave_r = art_dim * (0.33 + phase * 0.17);
                let wave_alpha = ((1.0 - phase) * bounce.min(1.0) * 190.0) as u8;
                painter.circle_stroke(
                    center,
                    wave_r,
                    Stroke::new(
                        1.5_f32 + (1.0 - phase) * 2.2,
                        Color32::from_rgba_unmultiplied(
                            accent.r(),
                            accent.g(),
                            accent.b(),
                            wave_alpha,
                        ),
                    ),
                );
            }
        }

        // code that misbehaves, mostly cosmetic
        let chassis_r = art_dim * 0.42;
        painter.circle_filled(center, chassis_r, Color32::from_rgb(13, 13, 17));
        painter.circle_stroke(
            center,
            chassis_r,
            Stroke::new(2.2_f32, Color32::from_rgb(48, 48, 60)),
        );

        for i in 0..4 {
            let angle = (i as f32 / 4.0) * TAU + std::f32::consts::FRAC_PI_4;
            let screw_pos = center + Vec2::new(angle.cos(), angle.sin()) * (chassis_r * 0.88);
            painter.circle_filled(screw_pos, 2.5, Color32::from_rgb(70, 70, 86));
        }

        // 3. High-roll rubber surround ring (stretches & thickens as cone jumps forward)
        let surround_r = art_dim * 0.32 + punch * 14.0;
        painter.circle_filled(cone_center, surround_r, Color32::from_rgb(20, 20, 26));
        painter.circle_stroke(
            cone_center,
            surround_r,
            Stroke::new(
                4.5_f32 + punch * 4.0,
                Color32::from_rgb(65, 65, 82),
            ),
        );

        // Kevlar cone??? (scales rapidly with beat excursion)
        let cone_r = art_dim * 0.24 + punch * 18.0 + mid_energy * 5.0;
        painter.circle_filled(cone_center, cone_r, Color32::from_rgb(26, 26, 34));
        painter.circle_stroke(
            cone_center,
            cone_r,
            Stroke::new(1.8_f32 + punch * 1.2, accent),
        );

        // Reebs (16 radial tension lines)
        let dust_cap_r = art_dim * 0.095 + punch * 11.5;
        for i in 0..16 {
            let angle = (i as f32 / 16.0) * TAU;
            let dir = Vec2::new(angle.cos(), angle.sin());
            let p1 = cone_center + dir * (dust_cap_r + 2.0);
            let p2 = cone_center + dir * (cone_r - 2.0);
            let rib_lum = (65.0 + mid_energy * 110.0).clamp(65.0, 200.0) as u8;
            painter.line_segment(
                [p1, p2],
                Stroke::new(1.1_f32, Color32::from_rgb(rib_lum, rib_lum, rib_lum + 15)),
            );
        }

        // Dust cap shit
        let cap_center = cone_center + Vec2::new(0.0, -punch * 2.5);
        let cap_lum = (180.0 + high_energy * 75.0).clamp(180.0, 255.0) as u8;
        painter.circle_filled(cap_center, dust_cap_r, Color32::from_rgb(36, 36, 48));
        painter.circle_stroke(
            cap_center,
            dust_cap_r,
            Stroke::new(
                2.0_f32 + punch * 2.2,
                Color32::from_rgb(cap_lum, cap_lum, 255),
            ),
        );

        // Just a fucking dot
        painter.circle_filled(
            cap_center,
            3.5 + punch * 4.5,
            Color32::from_rgb(250, 250, 255),
        );
    ;
    }    


// "Halo 3 reference??" -- you maybe. also no. but i also dont know anymore
    fn draw_radial_halo(
        &self,
        painter: &egui::Painter,
        center: Pos2,
        base_radius: f32,
        spectrum: &[f32; NUM_BARS],
        config: &AppConfig,
    ) {
        let count = NUM_BARS;
        for i in 0..count {
            let amp = spectrum[i].clamp(0.02, 1.0);
            let angle = (i as f32 / count as f32) * TAU - std::f32::consts::FRAC_PI_2;
            let dir = Vec2::new(angle.cos(), angle.sin());

            let start = center + dir * (base_radius + 6.0);
            let end = center + dir * (base_radius + 8.0 + amp * 36.0);
            let color = self.resolve_bar_color(i, count, amp, config);

            painter.line_segment([start, end], Stroke::new(2.6_f32, color));
        }
    }
// Non-gameboy color stuff (unless your pc is a Moto 86k Macintosh)
    fn resolve_bar_color(
        &self,
        idx: usize,
        total: usize,
        amp: f32,
        config: &AppConfig,
    ) -> Color32 {
        let t = idx as f32 / (total.max(1) as f32);
        let boost = amp.clamp(0.0, 1.0);

        match config.viz_color {
            // The bit that works on a gameboy, audio will not though.
            VisualizerColorMode::Monochrome => {
                let lum = (95.0 + boost * 160.0).clamp(95.0, 255.0) as u8;
                Color32::from_rgb(lum, lum, lum.saturating_add(8))
            }
            // Color stuff. Also Cyberpunk 2077 reference??
            VisualizerColorMode::Cyberpunk => {
                let r = (30.0 + t * 215.0 + boost * 40.0).clamp(0.0, 255.0) as u8;
                let g = (220.0 * (1.0 - t * 0.65) + boost * 35.0).clamp(0.0, 255.0) as u8;
                let b = (245.0 - t * 35.0).clamp(0.0, 255.0) as u8;
                Color32::from_rgb(r, g, b)
            }
            VisualizerColorMode::EmeraldMatrix => {
                let r = (20.0 + boost * 75.0).clamp(0.0, 255.0) as u8;
                let g = (130.0 + boost * 125.0).clamp(0.0, 255.0) as u8;
                let b = (85.0 + t * 90.0 + boost * 40.0).clamp(0.0, 255.0) as u8;
                Color32::from_rgb(r, g, b)
            }
            // Fire?? idk i havent tested.
            VisualizerColorMode::SunsetFire => {
                let r = (200.0 + boost * 55.0).clamp(0.0, 255.0) as u8;
                let g = (65.0 + t * 130.0 + boost * 50.0).clamp(0.0, 255.0) as u8;
                let b = (40.0 + (1.0 - t) * 70.0).clamp(0.0, 255.0) as u8;
                Color32::from_rgb(r, g, b)
            }
        }
    }
// The function below is the shit that actually shows library for the non-blind people out there.
    fn render_library_view(&mut self, ui: &mut egui::Ui, is_scanning: bool, status_msg: &str) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("LOCAL MUSIC LIBRARY")
                    .font(FontId::new(12.0, FontFamily::Monospace))
                    .color(Color32::from_rgb(140, 140, 155)),
            );
// If it's finding for who asked or going "IM NOT CRAZY THE MUSIC WAS RIGHT HERE" do this.
            if is_scanning {
                ui.label(
                    RichText::new("• Scanning tags...")
                        .font(FontId::new(11.5, FontFamily::Monospace))
                        .color(Color32::from_rgb(220, 190, 110)),
                );
            } else if !status_msg.is_empty() {
                ui.label(
                    RichText::new(format!("• {}", status_msg))
                        .font(FontId::new(11.5, FontFamily::Monospace))
                        .color(Color32::from_rgb(120, 185, 140)),
                );
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.track_filter)
                        .desired_width(240.0)
                        .hint_text("Search title, artist, album..."),
                );
            });
        });
/// Future note, the scan function doesnt work if there is nothing in the music folder. so note to self.
        ui.add_space(8.0);

        let (tracks, active_idx) = {
            let st = self.state.lock().unwrap();
            (st.track_list.clone(), st.current_track_idx)
        };

        let filter_lower = self.track_filter.to_lowercase();
// When the app is blind and cant see shit.
        egui::Frame::none()
            .fill(Color32::from_rgb(14, 14, 18))
            .rounding(Rounding::same(8.0))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(26, 26, 34)))
            .inner_margin(egui::Margin::symmetric(14.0, 10.0))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if tracks.is_empty() {
                            ui.add_space(40.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    RichText::new("No audio files indexed yet.")
                                        .color(Color32::from_rgb(130, 130, 145)),
                                );
                                ui.add_space(6.0);
                                if ui.button("Set Library Folder in Settings →").clicked() {
                                    self.active_view = ActiveView::Settings;
                                }
                            });
                            return;
                        }

                        for (idx, track) in tracks.iter().enumerate() {
                            if !filter_lower.is_empty()
                                && !track.title.to_lowercase().contains(&filter_lower)
                                && !track.artist.to_lowercase().contains(&filter_lower)
                                && !track.album.to_lowercase().contains(&filter_lower)
                            {
                                continue;
                            }

                            let is_active = active_idx == Some(idx);
                            let dur_str = if track.duration_secs > 0 {
                                format_duration(Duration::from_secs(track.duration_secs))
                            } else {
                                "--:--".to_string()
                            };

                            let row_text = format!(
                                "{:02}   {}  —  {}   [{}]   ({})",
                                idx + 1,
                                track.artist,
                                track.title,
                                track.album,
                                dur_str
                            );

                            let resp = ui.selectable_label(
                                is_active,
                                RichText::new(row_text)
                                    .font(FontId::new(13.0, FontFamily::Proportional))
                                    .color(if is_active {
                                        Color32::BLACK
                                    } else {
                                        Color32::from_rgb(215, 215, 225)
                                    }),
                            );

                            if resp.clicked() {
                                let _ = self.audio_tx.send(AudioCommand::PlayTrack(idx));
                                self.active_view = ActiveView::Visualizer;
                            }
                        }
                    });
            });
    }
    

    fn render_lyrics_view(
        &mut self,
        ui: &mut egui::Ui,
        current_track: Option<&TrackEntry>,
        lyrics: &[LyricLine],
        lyrics_source: &str,
        is_fetching_lyrics: bool,
        elapsed: Duration,
    ) {
        // shit that shows lyric source (.lrc / FLAC tag / Online) + Manual Online Search button for the ones that are dumb (in the lyrics bit)
        ui.horizontal(|ui| {
            let src_label = if lyrics.is_empty() {
                "SOURCE: NONE".to_string()
                // Haha NONE hehehhe the app is dumb
            } else {
                format!("SOURCE: {}", lyrics_source.to_uppercase())
            };
            ui.label(
                RichText::new(src_label)
                    .font(FontId::new(11.0, FontFamily::Monospace))
                    .color(Color32::from_rgb(120, 120, 138)),
            );

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let btn_text = if is_fetching_lyrics {
                    "Searching LRCLIB..."
                } else {
                    "⟳ Search Online Lyrics (LRCLIB)"
                };
                if ui
                    .add_enabled(
                        current_track.is_some() && !is_fetching_lyrics,
                        egui::Button::new(RichText::new(btn_text).size(11.5)),
                    )
                    .clicked()
                {
                    let _ = self.audio_tx.send(AudioCommand::FetchOnlineLyrics);
                }
            });
        });

        ui.add_space(4.0);

        if lyrics.is_empty() {
            ui.centered_and_justified(|ui| {
                let msg = if is_fetching_lyrics {
                    "Querying LRCLIB online database for synchronized lyrics...".to_string()
                } else if let Some(track) = current_track {
                    format!(
                        "No local .lrc or embedded FLAC lyrics found for \"{}\".\nClick \"Search Online Lyrics\" above to fetch & cache them automatically.",
                        track.title
                    )
                } else {
                    "Start playing a track to view local, embedded FLAC, or online synchronized lyrics."
                        .to_string()
                };

                ui.label(
                    RichText::new(msg)
                        .font(FontId::new(14.0, FontFamily::Proportional))
                        .color(Color32::from_rgb(115, 115, 130)),
                );
            });
            return;
        }

        let active_idx = current_lyric_index(lyrics, elapsed);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(false)
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(60.0);

                    for (idx, line) in lyrics.iter().enumerate() {
                        let is_current = active_idx == Some(idx);
                        let distance = active_idx
                            .map(|curr| (idx as isize - curr as isize).unsigned_abs())
                            .unwrap_or(4);

                        let (font_size, text_color) = if is_current {
                            (23.0, Color32::WHITE)
                        } else if distance == 1 {
                            (16.5, Color32::from_rgb(120, 120, 135))
                        } else if distance == 2 {
                            (15.5, Color32::from_rgb(80, 80, 95))
                        } else {
                            (14.5, Color32::from_rgb(52, 52, 64))
                        };

                        let resp = ui
                            .add(
                                egui::Label::new(
                                    RichText::new(&line.text)
                                        .font(FontId::new(font_size, FontFamily::Proportional))
                                        .color(text_color)
                                        .strong(),
                                )
                                .sense(Sense::click()),
                            )
                            .on_hover_text("Click to jump to timestamp");

                        if resp.clicked() {
                            let _ = self.audio_tx.send(AudioCommand::Seek(line.timestamp));
                        }

                        if is_current {
                            resp.scroll_to_me(Some(Align::Center));
                        }

                        ui.add_space(14.0);
                    }

                    ui.add_space(120.0);
                });
            });
    }
// its over.
// Settings that maybe work?? idk.
    fn render_settings_view(
        &mut self,
        ui: &mut egui::Ui,
        devices: &[String],
        config: &AppConfig,
        is_scanning: bool,
        status_msg: &str,
    ) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // SECTION 1: AUDIO & LIBRARY SETTINGS
                ui.label(
                    RichText::new("AUDIO & LIBRARY")
                        .font(FontId::new(12.0, FontFamily::Monospace))
                        .color(Color32::from_rgb(140, 140, 155)),
                );
                ui.add_space(8.0);

                egui::Frame::none()
                    .fill(Color32::from_rgb(15, 15, 19))
                    .rounding(Rounding::same(8.0))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(28, 28, 36)))
                    .inner_margin(egui::Margin::symmetric(18.0, 14.0))
                    .show(ui, |ui| {
                        egui::Grid::new("settings_audio_grid")
                            .num_columns(2)
                            .spacing([28.0, 14.0])
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new("Music Library Path")
                                        .color(Color32::from_rgb(210, 210, 220)),
                                );
                                ui.horizontal(|ui| {
                                    let edit =
                                        egui::TextEdit::singleline(&mut self.library_path_input)
                                            .desired_width(340.0)
                                            .hint_text("/home/user/Music");
                                    ui.add(edit);

                                    let btn_label = if is_scanning {
                                        "Scanning..."
                                    } else {
                                        "Save & Scan Tags"
                                    };
                                    if ui
                                        .add_enabled(!is_scanning, egui::Button::new(btn_label))
                                        .clicked()
                                    {
                                        let path = PathBuf::from(self.library_path_input.trim());
                                        if let Ok(mut st) = self.state.lock() {
                                            st.config.library_path =
                                                self.library_path_input.trim().to_string();
                                            let _ = st.config.save();
                                        }
                                        let _ = self.audio_tx.send(AudioCommand::ScanLibrary(path));
                                    }
                                });
                                /// Note: doesnt work when no files in there. It will do a search loop.
                                ui.end_row();
// Something that can be changed in pwvucontrol but ok
                                ui.label(
                                    RichText::new("Audio Output Device")
                                        .color(Color32::from_rgb(210, 210, 220)),
                                );
                                ui.horizontal(|ui| {
                                    let mut current_dev = config.selected_device.clone();
                                    egui::ComboBox::from_id_salt("audio_output_device_combo")
                                        .selected_text(&current_dev)
                                        .width(280.0)
                                        .show_ui(ui, |ui| {
                                            for dev in devices {
                                                if ui
                                                    .selectable_value(
                                                        &mut current_dev,
                                                        dev.clone(),
                                                        dev,
                                                    )
                                                    .clicked()
                                                {
                                                    let _ = self.audio_tx.send(
                                                        AudioCommand::SetDevice(
                                                            current_dev.clone(),
                                                        ),
                                                    );
                                                    if let Ok(mut st) = self.state.lock() {
                                                        st.config.selected_device =
                                                            current_dev.clone();
                                                        let _ = st.config.save();
                                                    }
                                                }
                                            }
                                        });
                                });
                                ui.end_row();

                                ui.label(
                                    RichText::new("Master Volume")
                                        .color(Color32::from_rgb(210, 210, 220)),
                                );
                                ui.horizontal(|ui| {
                                    let mut vol_pct = config.master_volume * 100.0;
                                    let slider = egui::Slider::new(&mut vol_pct, 0.0..=100.0)
                                        .suffix("%")
                                        .fixed_decimals(0);
                                    let resp = ui.add(slider);
                                    if resp.changed() {
                                        let new_vol = (vol_pct / 100.0).clamp(0.0, 1.0);
                                        let _ = self.audio_tx.send(AudioCommand::SetVolume(new_vol));
                                    }
                                    if resp.drag_stopped() {
                                        if let Ok(st) = self.state.lock() {
                                            let _ = st.config.save();
                                        }
                                    }
                                });
                                ui.end_row();
// This code misbehaves. Be warned.
                                ui.label(
                                    RichText::new("Online Lyrics Auto-Search")
                                        .color(Color32::from_rgb(210, 210, 220)),
                                );
                                let mut auto_lyrics = config.auto_fetch_online_lyrics;
                                if ui
                                    .checkbox(
                                        &mut auto_lyrics,
                                        "Automatically fetch from LRCLIB when local/FLAC lyrics are missing",
                                    )
                                    .changed()
                                {
                                    if let Ok(mut st) = self.state.lock() {
                                        st.config.auto_fetch_online_lyrics = auto_lyrics;
                                        let _ = st.config.save();
                                    }
                                }
                                ui.end_row();
                            });
                    });

                ui.add_space(18.0);
// Some visualizer shit.

                // FFT settings maybe
                ui.label(
                    RichText::new("VISUALIZER ENGINE & AESTHETICS")
                        .font(FontId::new(12.0, FontFamily::Monospace))
                        .color(Color32::from_rgb(140, 140, 155)),
                );
                ui.add_space(8.0);

                egui::Frame::none()
                    .fill(Color32::from_rgb(15, 15, 19))
                    .rounding(Rounding::same(8.0))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(28, 28, 36)))
                    .inner_margin(egui::Margin::symmetric(18.0, 14.0))
                    .show(ui, |ui| {
                        egui::Grid::new("settings_visualizer_grid")
                            .num_columns(2)
                            .spacing([28.0, 14.0])
                            .show(ui, |ui| {
                                // Visualizer Mode
                                ui.label(
                                    RichText::new("Visualizer Style")
                                        .color(Color32::from_rgb(210, 210, 220)),
                                );
                                let mut style = config.viz_style;
                                egui::ComboBox::from_id_salt("viz_style_combo")
                                    .selected_text(style.label())
                                    .width(240.0)
                                    .show_ui(ui, |ui| {
                                        for candidate in [
                                            VisualizerStyle::ClassicBars,
                                            VisualizerStyle::MirroredWave,
                                            VisualizerStyle::RadialPulse,
                                            VisualizerStyle::CyberNeon,
                                            // Cyberpunk 2077 reference
                                        ] {
                                            if ui
                                                .selectable_value(
                                                    &mut style,
                                                    candidate,
                                                    candidate.label(),
                                                )
                                                .clicked()
                                            {
                                                if let Ok(mut st) = self.state.lock() {
                                                    st.config.viz_style = style;
                                                    let _ = st.config.save();
                                                }
                                            }
                                        }
                                    });
                                ui.end_row();

                                // Color Palette
                                ui.label(
                                    RichText::new("Spectrum Palette")
                                        .color(Color32::from_rgb(210, 210, 220)),
                                );
                                let mut color_mode = config.viz_color;
                                egui::ComboBox::from_id_salt("viz_color_combo")
                                    .selected_text(color_mode.label())
                                    .width(240.0)
                                    .show_ui(ui, |ui| {
                                        for candidate in [
                                            VisualizerColorMode::Monochrome,
                                            VisualizerColorMode::Cyberpunk,
                                            VisualizerColorMode::EmeraldMatrix,
                                            VisualizerColorMode::SunsetFire,
                                        ] {
                                            if ui
                                                .selectable_value(
                                                    &mut color_mode,
                                                    candidate,
                                                    candidate.label(),
                                                )
                                                .clicked()
                                            {
                                                if let Ok(mut st) = self.state.lock() {
                                                    st.config.viz_color = color_mode;
                                                    let _ = st.config.save();
                                                }
                                            }
                                        }
                                    });
                                ui.end_row();

                                // if the guy is **L O U D** or *not*
                                ui.label(
                                    RichText::new("Reactivity Gain")
                                        .color(Color32::from_rgb(210, 210, 220)),
                                );
                                let mut gain = config.viz_gain;
                                if ui
                                    .add(
                                        egui::Slider::new(&mut gain, 0.4..=2.5)
                                            .suffix("x")
                                            .fixed_decimals(2),
                                    )
                                    .changed()
                                {
                                    if let Ok(mut st) = self.state.lock() {
                                        st.config.viz_gain = gain;
                                        let _ = st.config.save();
                                    }
                                }
                                ui.end_row();

                                // Decay Smoothing
                                ui.label(
                                    RichText::new("Gravity / Smoothing")
                                        .color(Color32::from_rgb(210, 210, 220)),
                                );
                                let mut smoothing = config.viz_smoothing;
                                if ui
                                    .add(
                                        egui::Slider::new(&mut smoothing, 0.40..=0.94)
                                            .fixed_decimals(2),
                                    )
                                    .changed()
                                {
                                    if let Ok(mut st) = self.state.lock() {
                                        st.config.viz_smoothing = smoothing;
                                        let _ = st.config.save();
                                    }
                                }
                                ui.end_row();

                                // Frequency Density
                                ui.label(
                                    RichText::new("Spectrum Bar Density")
                                        .color(Color32::from_rgb(210, 210, 220)),
                                );
                                let mut bars = config.viz_bar_count;
                                if ui
                                    .add(egui::Slider::new(&mut bars, 16..=NUM_BARS).suffix(" bars"))
                                    .changed()
                                {
                                    if let Ok(mut st) = self.state.lock() {
                                        st.config.viz_bar_count = bars;
                                        let _ = st.config.save();
                                    }
                                }
                                ui.end_row();

                                // This shit doesnt matter cuz the cone shows regardless cuz i wiped the cover art bit but it still exists.
                                ui.label(
                                    RichText::new("Sub-Bass Speaker Cone")
                                        .color(Color32::from_rgb(210, 210, 220)),
                                );
                                let mut overlay = config.show_speaker_overlay;
                                if ui
                                    .checkbox(
                                        &mut overlay,
                                        "Overlay reactive speaker cone even when cover art is present",
                                    )
                                    .changed()
                                {
                                    if let Ok(mut st) = self.state.lock() {
                                        st.config.show_speaker_overlay = overlay;
                                        let _ = st.config.save();
                                    }
                                }
                                ui.end_row();
                            });
                    });

                if !status_msg.is_empty() {
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(status_msg)
                            .font(FontId::new(11.5, FontFamily::Monospace))
                            .color(Color32::from_rgb(130, 195, 150)),
                    );
                }
            });
    }

/// Now Playing that stays no matter what and still fucking stays. WHY DOES IT NOT REMOVE (ok it cant that is a feature.) Also **E X P A N D**  **D O N G**
    fn render_bottom_now_playing_bar(
        &mut self,
        ui: &mut egui::Ui,
        current_track: Option<&TrackEntry>,
        is_playing: bool,
        elapsed: Duration,
        duration: Duration,
        bass_impulse: f32,
    ) {
        // **E X P A N D**  **D O N G**
        if self.now_playing_expanded {
            egui::Frame::none()
                .fill(Color32::from_rgb(15, 15, 20))
                .rounding(Rounding::same(10.0))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(38, 38, 50)))
                .inner_margin(egui::Margin::symmetric(16.0, 12.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Large high-res cover art inside the expanded drawer
                        let art_size = 116.0;
                        let (art_rect, _) =
                            ui.allocate_exact_size(Vec2::splat(art_size), Sense::hover());
                        let painter = ui.painter();

                        painter.rect(
                            art_rect,
                            Rounding::same(8.0),
                            Color32::from_rgb(22, 22, 28),
                            Stroke::new(1.0_f32, Color32::from_rgb(58, 58, 74)),
                        );

                        if let Some(ref tex) = self.cover_texture {
                            painter.image(
                                tex.id(),
                                art_rect.shrink(1.5),
                                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        } else {
                            let c = art_rect.center();
                            painter.circle_stroke(
                                c,
                                28.0 + bass_impulse * 6.0,
                                Stroke::new(1.5_f32, Color32::from_rgb(130, 130, 150)),
                            );
                            painter.circle_filled(c, 7.0 + bass_impulse * 3.0, Color32::WHITE);
                        }

                        ui.add_space(14.0);

                        // Full un-truncated track metadata
                        ui.vertical(|ui| {
                            ui.add_space(4.0);
                            if let Some(track) = current_track {
                                ui.label(
                                    RichText::new("NOW PLAYING")
                                        .font(FontId::new(10.0, FontFamily::Monospace))
                                        .color(Color32::from_rgb(120, 185, 145)),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(&track.title)
                                        .font(FontId::new(18.0, FontFamily::Proportional))
                                        .color(Color32::WHITE)
                                        .strong(),
                                );
                                ui.label(
                                    RichText::new(&track.artist)
                                        .font(FontId::new(13.5, FontFamily::Proportional))
                                        .color(Color32::from_rgb(190, 190, 205)),
                                );
                                ui.add_space(2.0);
                                ui.label(
                                    RichText::new(format!("Album: {}", track.album))
                                        .font(FontId::new(11.5, FontFamily::Monospace))
                                        .color(Color32::from_rgb(125, 125, 142)),
                                );
                            } else {
                                ui.label(
                                    RichText::new("No Track Playing")
                                        .font(FontId::new(16.0, FontFamily::Proportional))
                                        .color(Color32::from_rgb(150, 150, 165)),
                                );
                            }
                        });

                        ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("▾ Collapse")
                                            .size(11.5)
                                            .color(Color32::from_rgb(165, 165, 180)),
                                    )
                                    .frame(false),
                                )
                                .clicked()
                            {
                                self.now_playing_expanded = false;
                            }
                        });
                    });
                });

            ui.add_space(8.0);
        }

        // my booty dont jiggle jiggle its flat (and also has controls ahh touch me daddy~)
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;

            // shit with cover art (if you touch it just right it's dong expands)
            let thumb_size = 38.0;
            let (thumb_rect, thumb_resp) =
                ui.allocate_exact_size(Vec2::splat(thumb_size), Sense::click());

            let painter = ui.painter();
            painter.rect(
                thumb_rect,
                Rounding::same(6.0),
                Color32::from_rgb(20, 20, 26),
                Stroke::new(1.0_f32, Color32::from_rgb(50, 50, 64)),
            );

            if let Some(ref tex) = self.cover_texture {
                painter.image(
                    tex.id(),
                    thumb_rect.shrink(1.0),
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            } else {
                let c = thumb_rect.center();
                painter.circle_stroke(
                    c,
                    10.0 + bass_impulse * 3.0,
                    Stroke::new(1.2_f32, Color32::from_rgb(130, 130, 150)),
                );
                painter.circle_filled(c, 3.0 + bass_impulse * 1.5, Color32::WHITE);
            }

            if thumb_resp
                .on_hover_text("Click to Expand / Collapse Now Playing Cover Art")
                .clicked()
            {
                self.now_playing_expanded = !self.now_playing_expanded;
            }

            // Info + expandable dong (god why am i calling the bar a dong)
            let info_resp = ui
                .vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.set_width(160.0);
                    let chevron = if self.now_playing_expanded { "▾" } else { "▴" };
                    if let Some(track) = current_track {
                        let title_short = truncate_str(&track.title, 18);
                        let artist_short = truncate_str(&track.artist, 22);
                        ui.label(
                            RichText::new(format!("{} {}", title_short, chevron))
                                .font(FontId::new(12.5, FontFamily::Proportional))
                                .color(Color32::WHITE)
                                .strong(),
                        );
                        ui.label(
                            RichText::new(artist_short)
                                .font(FontId::new(10.5, FontFamily::Monospace))
                                .color(Color32::from_rgb(130, 130, 148)),
                        );
                    } else {
                        ui.label(
                            RichText::new(format!("Idle {}", chevron))
                                .font(FontId::new(12.5, FontFamily::Proportional))
                                .color(Color32::from_rgb(130, 130, 145)),
                        );
                        ui.label(
                            RichText::new("MPRIS2 Ready")
                                .font(FontId::new(10.0, FontFamily::Monospace))
                                .color(Color32::from_rgb(90, 90, 105)),
                        );
                    }
                })
                .response
                .interact(Sense::click());

            if info_resp
                .on_hover_text("Click to Expand / Collapse Now Playing")
                .clicked()
            {
                self.now_playing_expanded = !self.now_playing_expanded;
            }

            // K O N T R O L S (that dont do shit while seeking)
            if ui
                .add(egui::Button::new(RichText::new("⏮").size(13.0)).frame(false))
                .on_hover_text("Previous Track")
                .clicked()
            {
                let _ = self.audio_tx.send(AudioCommand::PrevTrack);
            }

            let play_icon = if is_playing { "⏸" } else { "▶" };
            if ui
                .add(
                    egui::Button::new(
                        RichText::new(play_icon)
                            .size(15.0)
                            .color(Color32::WHITE),
                    )
                    .min_size(Vec2::new(28.0, 24.0)),
                )
                .on_hover_text("Play / Pause (Space)")
                .clicked()
            {
                let _ = self.audio_tx.send(AudioCommand::TogglePause);
            }

            if ui
                .add(egui::Button::new(RichText::new("⏭").size(13.0)).frame(false))
                .on_hover_text("Next Track")
                .clicked()
            {
                let _ = self.audio_tx.send(AudioCommand::NextTrack);
            }

            ui.add_space(4.0);

            // Elapsed + Scrub Slider + Total Duration
            let total_secs = duration.as_secs_f64().max(0.01);
            let current_secs = self
                .scrub_dragging_val
                .unwrap_or_else(|| elapsed.as_secs_f64().min(total_secs));

            ui.label(
                RichText::new(format_duration(Duration::from_secs_f64(current_secs)))
                    .font(FontId::new(11.5, FontFamily::Monospace))
                    .color(Color32::from_rgb(165, 165, 180)),
            );

            let avail_slider_w = (ui.available_width() - 56.0).max(80.0);
            ui.spacing_mut().slider_width = avail_slider_w;

            let mut slider_val = current_secs;
            let slider_resp = ui.add(
                egui::Slider::new(&mut slider_val, 0.0..=total_secs)
                    .show_value(false)
                    .trailing_fill(true),
            );
// THIS CODE IS OF THE PUSSYHOLE. IT IS SATANIC. THIS SHIT TOOK 10 DAYS TO FIX AND OPENCODE FUCKING WIPED IT ONCE. IT USES A FUCK TAPE FIX AND IS NOT PERSISTENT AND BREAKS WITH UPDATES.
/// Consider updating code.
            if slider_resp.dragged() {
                self.scrub_dragging_val = Some(slider_val);
            }

            if slider_resp.drag_stopped() || (slider_resp.changed() && !slider_resp.dragged()) {
                let seek_target = Duration::from_secs_f64(slider_val);
                self.scrub_dragging_val = None;
                let _ = self.audio_tx.send(AudioCommand::Seek(seek_target));
            }

            ui.label(
                RichText::new(format_duration(duration))
                    .font(FontId::new(11.5, FontFamily::Monospace))
                    .color(Color32::from_rgb(115, 115, 130)),
            );
        });
    }
    
}
fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn format_duration(dur: Duration) -> String {
    let total_secs = dur.as_secs();
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{:02}:{:02}", mins, secs)
}
// Time shit, if works works if not fuck my life

// aah fuck my life what am i doing. also code end. fuck off or build the code (if it fails pls fix it yourself youre not that incompetent. if youre bored use gemini idk just DIY dont bug me with shit i will laugh at you if you do.)