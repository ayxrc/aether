// Some stuff that makes sure that the app actually works
use crate::lyrics::LyricLine;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

pub const NUM_BARS: usize = 48;
// How it's gonna make sure it knows the track
#[derive(Clone, Debug, PartialEq)]
pub struct TrackEntry {
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_secs: u64,
}
// Cover arts (unless if you are blind)
#[derive(Clone)]
pub struct CoverArtData {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
    pub cache_path: Option<PathBuf>,
}
// Visualizer (otherwise fft.rs would be fucking useless)
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisualizerStyle {
    ClassicBars,
    MirroredWave,
    RadialPulse,
    CyberNeon,
}

impl VisualizerStyle {
    pub fn label(&self) -> &'static str {
        match self {
            Self::ClassicBars => "Classic Spectrum Bars",
            Self::MirroredWave => "Mirrored Soundwave",
            Self::RadialPulse => "Radial Bass Halo",
            Self::CyberNeon => "Cyber Horizon Glow",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisualizerColorMode {
    Monochrome,
    Cyberpunk,
    EmeraldMatrix,
    SunsetFire,
}

impl VisualizerColorMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Monochrome => "Pure Monochrome",
            Self::Cyberpunk => "Cyberpunk (Cyan / Magenta)",
            Self::EmeraldMatrix => "Phosphor Emerald",
            Self::SunsetFire => "Amber / Crimson",
        }
    }
}
// long af
// App Config (cuz. idk go fuck yourself)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppConfig {
    pub library_path: String,
    pub selected_device: String,
    pub master_volume: f32,
    pub viz_style: VisualizerStyle,
    pub viz_color: VisualizerColorMode,
    pub viz_smoothing: f32,
    pub viz_gain: f32,
    pub viz_bar_count: usize,
    pub show_speaker_overlay: bool,
    pub auto_fetch_online_lyrics: bool,
}
// Directory shit
impl Default for AppConfig {
    fn default() -> Self {
        let default_music = directories::UserDirs::new()
            .and_then(|u| u.audio_dir().map(|p| p.to_string_lossy().to_string()))
            .unwrap_or_else(|| {
                std::env::var("HOME")
                    .map(|h| format!("{}/Music", h))
                    .unwrap_or_else(|_| ".".to_string())
            });

        Self {
            library_path: default_music,
            selected_device: "Default".to_string(),
            master_volume: 0.75,
            viz_style: VisualizerStyle::ClassicBars,
            viz_color: VisualizerColorMode::Monochrome,
            viz_smoothing: 0.82,
            viz_gain: 1.15,
            viz_bar_count: 48,
            show_speaker_overlay: false,
            auto_fetch_online_lyrics: true,
        }
    }
}
// Config shit
impl AppConfig {
    pub fn load() -> Self {
        if let Some(proj_dirs) = ProjectDirs::from("com", "ayxrc", "aether") {
            let config_file = proj_dirs.config_dir().join("config.json");
            if let Ok(data) = fs::read_to_string(config_file) {
                if let Ok(cfg) = serde_json::from_str(&data) {
                    return cfg;
                }
            }
        }
        Self::default()
    }
// More config shit (fuck you if you dont have XDG config dir you gobo linux slut i bet you wank off to /Applications you cock-sucker non standard follower)
    pub fn save(&self) -> Result<(), String> {
        if let Some(proj_dirs) = ProjectDirs::from("com", "ayxrc", "aether") {
            let dir = proj_dirs.config_dir();
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            let config_file = dir.join("config.json");
            let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
            fs::write(config_file, json).map_err(|e| e.to_string())?;
            Ok(())
        } else {
            Err("Unable to resolve XDG config directory.".to_string())
        }
    }
}
// God i hope no Navy SEAL who uses 4chan comes at me (reference haha)

// This shit tells the player what the fuck it's doing (somehow this was in state.rs and not main. wow more compile time great job me.)
pub struct SharedState {
    pub current_track: Option<TrackEntry>,
    pub track_list: Vec<TrackEntry>,
    pub current_track_idx: Option<usize>,

    pub is_playing: bool,
    pub elapsed: Duration,
    pub duration: Duration,

    pub spectrum: [f32; NUM_BARS],
    pub peaks: [f32; NUM_BARS],
    pub bass_impulse: f32,

    pub lyrics: Vec<LyricLine>,
    pub lyrics_source: String,
    pub is_fetching_lyrics: bool,

    pub cover_art: Option<CoverArtData>,
    pub cover_art_id: u64,

    pub available_devices: Vec<String>,
    pub config: AppConfig,
    pub is_scanning: bool,
    pub status_msg: String,
}
// Make sure the music player music players
impl Default for SharedState {
    fn default() -> Self {
        let config = AppConfig::load();
        Self {
            current_track: None,
            track_list: Vec::new(),
            current_track_idx: None,
            is_playing: false,
            elapsed: Duration::ZERO,
            duration: Duration::ZERO,
            spectrum: [0.0; NUM_BARS],
            peaks: [0.0; NUM_BARS],
            bass_impulse: 0.0,
            lyrics: Vec::new(),
            lyrics_source: String::new(),
            is_fetching_lyrics: false,
            cover_art: None,
            cover_art_id: 0,
            available_devices: vec!["Default".to_string()],
            config,
            is_scanning: false,
            status_msg: String::new(),
        }
    }
}
