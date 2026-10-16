// Custom engine idk
// make sure the fucker uses the crates instead of nutting on them and doing shit all with them except being a crusty sock.
use crate::fft::{spawn_fft_worker, FFT_SIZE};
use crate::lyrics::{load_local_or_embedded_lyrics, spawn_online_lyrics_search};
use crate::mpris::spawn_mpris_server;
use crate::state::{CoverArtData, SharedState, TrackEntry};
use lofty::prelude::*;
use lofty::probe::Probe;
use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source, decoder};
use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use walkdir::WalkDir;

// half of these arent shit that require an audio engine
pub enum AudioCommand {
    PlayTrack(usize),
    TogglePause,
    NextTrack,
    PrevTrack,
    Seek(Duration),
    SetVolume(f32),
    SetDevice(String),
    ScanLibrary(PathBuf),
    FetchOnlineLyrics,
}
// shit is bad but works
pub struct SampleTap<S>
where
    S: Source<Item = f32>,
{
    inner: S,
    ring_buffer: Arc<Mutex<VecDeque<f32>>>,
    local_chunk: Vec<f32>,
    channels: u16,
    current_channel: u16,
    frame_accumulator: f32,
    samples_played: Arc<AtomicU64>,
}

impl<S> SampleTap<S>
where
    S: Source<Item = f32>,
{
    // Yea right just call it new AND DONT GIVE A SHIT
    pub fn new(
        inner: S,
        ring_buffer: Arc<Mutex<VecDeque<f32>>>,
        samples_played: Arc<AtomicU64>,
    ) -> Self {
        let channels = inner.channels().max(1);
        Self {
            inner,
            ring_buffer,
            local_chunk: Vec::with_capacity(256),
            channels,
            current_channel: 0,
            frame_accumulator: 0.0,
            samples_played,
        }
    }
}

impl<S> Iterator for SampleTap<S>
where
    S: Source<Item = f32>,
{
    type Item = f32;
// haha current channel 0 hehe
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.inner.next()?;
        self.samples_played.fetch_add(1, Ordering::Relaxed);

        self.frame_accumulator += sample;
        self.current_channel += 1;

        if self.current_channel >= self.channels {
            let mono = self.frame_accumulator / (self.channels as f32);
            self.frame_accumulator = 0.0;
            self.current_channel = 0;
            self.local_chunk.push(mono);

            if self.local_chunk.len() >= 128 {
                if let Ok(mut ring) = self.ring_buffer.try_lock() {
                    ring.extend(self.local_chunk.drain(..));
                    while ring.len() > FFT_SIZE * 2 {
                        ring.pop_front();
                    }
                } else {
                    self.local_chunk.clear();
                }
            }
        }

        Some(sample)
    }
}
// what for what?
impl<S> Source for SampleTap<S>
where
    S: Source<Item = f32>,
{
    #[inline]
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }

    #[inline]
    fn channels(&self) -> u16 {
        self.inner.channels()
    }

    #[inline]
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }

    #[inline]
    fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
        self.inner.try_seek(pos)
    }
}
// makes the engine start
pub fn spawn_audio_engine(shared_state: Arc<Mutex<SharedState>>) -> Sender<AudioCommand> {
    let (tx, rx) = mpsc::channel::<AudioCommand>();
    let tx_clone = tx.clone();
       spawn_mpris_server(tx.clone(), Arc::clone(&shared_state));

    let sample_ring = Arc::new(Mutex::new(VecDeque::with_capacity(FFT_SIZE * 2)));
    let sample_rate_atomic = Arc::new(AtomicU32::new(44100));

    spawn_fft_worker(
        Arc::clone(&sample_ring),
        Arc::clone(&shared_state),
        Arc::clone(&sample_rate_atomic),
    );

 
// The fuck you mean Aether Audio Engine its just rodio and prayers. no. 
    thread::Builder::new()
        .name("aether-audio-engine".into())
        .spawn(move || {
            let devices = enumerate_output_devices();
            let (initial_device, initial_lib_path, initial_volume) = {
                let mut state = shared_state.lock().unwrap();
                state.available_devices = devices;
                (
                    state.config.selected_device.clone(),
                    PathBuf::from(&state.config.library_path),
                    state.config.master_volume,
                )
            };

            let mut stream_bundle: Option<(OutputStream, OutputStreamHandle)> =
                open_device_stream(&initial_device);
            let mut active_sink: Option<Sink> = None;

            let samples_played = Arc::new(AtomicU64::new(0));
            let mut current_channels: u16 = 2;
            let mut current_sample_rate: u32 = 44100;
            let mut master_volume: f32 = initial_volume;

            let _ = tx_clone.send(AudioCommand::ScanLibrary(initial_lib_path));

            loop {
                match rx.recv_timeout(Duration::from_millis(25)) {
                    Ok(cmd) => match cmd {
                        AudioCommand::PlayTrack(idx) => {
                            let track_opt = {
                                let state = shared_state.lock().unwrap();
                                state.track_list.get(idx).cloned()
                            };

                            if let Some(track) = track_opt {
                                if stream_bundle.is_none() {
                                    let dev = shared_state
                                        .lock()
                                        .unwrap()
                                        .config
                                        .selected_device
                                        .clone();
                                    stream_bundle = open_device_stream(&dev);
                                }

                                if let Some((_, ref handle)) = stream_bundle {
                                    match File::open(&track.path) {
                                        Ok(file) => match Decoder::new(BufReader::new(file)) {
                                            Ok(decoder) => {
                                                let f32_source = decoder.convert_samples::<f32>();
                                                current_channels = f32_source.channels().max(1);
                                                current_sample_rate =
                                                    f32_source.sample_rate().max(1);
                                                sample_rate_atomic
                                                    .store(current_sample_rate, Ordering::Relaxed);

                                                let total_dur = f32_source
                                                    .total_duration()
                                                    .unwrap_or_else(|| {
                                                        Duration::from_secs(track.duration_secs)
                                                    });

                                                samples_played.store(0, Ordering::Relaxed);
                                                if let Ok(mut ring) = sample_ring.lock() {
                                                    ring.clear();
                                                }

                                                let tapped_source = SampleTap::new(
                                                    f32_source,
                                                    Arc::clone(&sample_ring),
                                                    Arc::clone(&samples_played),
                                                );
// yay some stuff.
                                                if let Ok(new_sink) = Sink::try_new(handle) {
                                                    new_sink.set_volume(master_volume);
                                                    new_sink.append(tapped_source);
                                                    new_sink.play();
                                                    active_sink = Some(new_sink);

                                                    // The stuff that makes music music: covers (painful haha)
                                                    let cover_art =
                                                        extract_cover_art(&track.path);
                                                    let (lyrics, lyrics_source) =
                                                        load_local_or_embedded_lyrics(&track.path);
// why is state code in the audio engine past me, it works tho, i wont yell at it.
                                                    let should_auto_fetch = {
                                                        let mut state =
                                                            shared_state.lock().unwrap();
                                                        let auto_online =
                                                            state.config.auto_fetch_online_lyrics;
                                                        state.current_track = Some(track.clone());
                                                        state.current_track_idx = Some(idx);
                                                        state.duration = total_dur;
                                                        state.elapsed = Duration::ZERO;
                                                        state.lyrics = lyrics.clone();
                                                        state.lyrics_source = lyrics_source;
                                                        state.cover_art = cover_art;
                                                        state.cover_art_id =
                                                            state.cover_art_id.wrapping_add(1);
                                                        state.is_playing = true;
                                                        state.status_msg.clear();
                                                        auto_online && lyrics.is_empty()
                                                    };

                                                    if should_auto_fetch {
                                                        spawn_online_lyrics_search(
                                                            track,
                                                            Arc::clone(&shared_state),
                                                        );
                                                    }
                                                }
                                            }
                                            // Why rodio
                                            Err(e) => {
                                                shared_state.lock().unwrap().status_msg =
                                                    format!("Decode error: {}", e);
                                            }
                                        },
                                        // haha your file isnt playable go jack off to the cd burner again
                                        Err(e) => {
                                            shared_state.lock().unwrap().status_msg =
                                                format!("File error: {}", e);
                                        }
                                    }
                                } else {
                                    shared_state.lock().unwrap().status_msg =
                                        "Audio output device unavailable.".to_string();
                                        // WHY IS IT UNAVAILABLE AAHHH YOURE USING A FUCKING MUSIC PLAYER WITHOUT AUDIO HAHHAHAHHAHAHHAHHA YOURE DUMB
                                }
                            }
                        }
// The garbage control code that connects to the sink (why.)
                        AudioCommand::TogglePause => {
                            if let Some(ref sink) = active_sink {
                                if sink.is_paused() {
                                    sink.play();
                                    shared_state.lock().unwrap().is_playing = true;
                                } else {
                                    sink.pause();
                                    shared_state.lock().unwrap().is_playing = false;
                                }
                            } else {
                                let has_tracks =
                                    !shared_state.lock().unwrap().track_list.is_empty();
                                if has_tracks {
                                    let _ = tx_clone.send(AudioCommand::PlayTrack(0));
                                }
                            }
                        }

                        AudioCommand::NextTrack => {
                            let next_idx = {
                                let state = shared_state.lock().unwrap();
                                if state.track_list.is_empty() {
                                    None
                                } else {
                                    Some(
                                        state
                                            .current_track_idx
                                            .map(|i| (i + 1) % state.track_list.len())
                                            .unwrap_or(0),
                                    )
                                }
                            };
                            if let Some(idx) = next_idx {
                                let _ = tx_clone.send(AudioCommand::PlayTrack(idx));
                            }
                        }

                        AudioCommand::PrevTrack => {
                            let prev_idx = {
                                let state = shared_state.lock().unwrap();
                                if state.track_list.is_empty() {
                                    None
                                } else {
                                    let len = state.track_list.len();
                                    Some(
                                        state
                                            .current_track_idx
                                            .map(|i| (i + len - 1) % len)
                                            .unwrap_or(0),
                                    )
                                }
                            };
                            if let Some(idx) = prev_idx {
                                let _ = tx_clone.send(AudioCommand::PlayTrack(idx));
                            }
                        }
// Code that does fuck all and says that the seek is (unseekable).
                        AudioCommand::Seek(target_pos) => {
                            let mut seek_failed = false;
                            if let Some(ref sink) = active_sink {
                                    let seek_res = sink.try_seek(target_pos);
                                    if let Err(ref e) = seek_res { eprintln!("seek failed: {:?}", e);}
                                    if seek_res.is_ok(){

                                      let target_samples = (target_pos.as_secs_f64()
                                        * (current_sample_rate as f64)
                                        * (current_channels as f64))
                                        as u64;
                                    samples_played.store(target_samples, Ordering::Relaxed);
                                    shared_state.lock().unwrap().elapsed = target_pos;
// The weird fix held with fuck tape, somehow works?? of the pussyhole.                                    
                                } else { seek_failed = true; }
                            } if seek_failed {
                                let track_opt = shared_state.lock().unwrap().current_track.clone();
                                if let (Some(track), Some((_, handle))) = (track_opt, stream_bundle.as_ref()) {
                                    if let Ok(file) = File::open(&track.path) {
                                        if let Ok(decoder) = Decoder::new(BufReader::new(file)) {
                                            let src = decoder.convert_samples::<f32>().skip_duration(target_pos);
                                            let tapped = SampleTap::new(
                                                src,
                                                Arc::clone(&sample_ring),
                                                Arc::clone(&samples_played),
                                            );
                                        
                                       if let Ok(new_sink) = Sink::try_new(handle) {
                                        let was_paused =
                                            active_sink.as_ref().map(|s| s.is_paused()).unwrap_or(false);
                                        if let Some(old) = active_sink.take() {
                                            old.stop();
                                        }
                                        
                                        new_sink.set_volume(master_volume);
                                        new_sink.append(tapped);
                                        if was_paused {
                                            new_sink.pause();
                                        }
                                        let target_samples = (target_pos.as_secs_f64()
                                        * (current_sample_rate as f64)
                                        * (current_channels as f64))
                                        as u64;
                                    samples_played.store(target_samples, Ordering::Relaxed);
                                    shared_state.lock().unwrap().elapsed = target_pos;
                                    active_sink = Some(new_sink);        
                                       }
                                    }
                                }
                            }
                        }
                    }
// make the yeller quiet or louder                    
                        AudioCommand::SetVolume(vol) => {
                            master_volume = vol.clamp(0.0, 1.0);
                            if let Some(ref sink) = active_sink {
                                sink.set_volume(master_volume);
                            }
                            shared_state.lock().unwrap().config.master_volume = master_volume;
                        }
// wake the yeller.
                        AudioCommand::SetDevice(device_name) => {
                            active_sink = None;
                            stream_bundle = open_device_stream(&device_name);

                            let (resume_idx, resume_pos) = {
                                let mut state = shared_state.lock().unwrap();
                                state.config.selected_device = device_name.clone();
                                state.status_msg = format!("Switched output to {}", device_name);
                                (state.current_track_idx, state.elapsed)
                            };

                            if let Some(idx) = resume_idx {
                                let _ = tx_clone.send(AudioCommand::PlayTrack(idx));
                                if resume_pos > Duration::from_secs(1) {
                                    let _ = tx_clone.send(AudioCommand::Seek(resume_pos));
                                }
                            }
                        }
// More shit that isnt meant to be in the engine
                        AudioCommand::ScanLibrary(path) => {
                            spawn_library_scanner(path, Arc::clone(&shared_state));
                        }
// Make sure online lyrics is clickable.
                        AudioCommand::FetchOnlineLyrics => {
                            let track_opt =
                                shared_state.lock().unwrap().current_track.clone();
                            if let Some(track) = track_opt {
                                spawn_online_lyrics_search(track, Arc::clone(&shared_state));
                            }
                        }
                    },
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
// some order stuff
                if let Some(ref sink) = active_sink {
                    let played = samples_played.load(Ordering::Relaxed);
                    let denom = (current_sample_rate as f64) * (current_channels as f64);
                    if denom > 0.0 {
                        let secs = (played as f64) / denom;
                        if let Ok(mut state) = shared_state.lock() {
                            state.elapsed = Duration::from_secs_f64(secs);
                            if !state.duration.is_zero() && state.elapsed > state.duration {
                                state.duration = state.elapsed;
                            }
                        }
                    }
// not a kitchen sink but if its empty make the sink.
                    if sink.empty() {
                        active_sink = None;
                        let next_idx = {
                            let mut state = shared_state.lock().unwrap();
                            state.is_playing = false;
                            if state.track_list.len() > 1 {
                                state
                                    .current_track_idx
                                    .map(|i| (i + 1) % state.track_list.len())
                            } else {
                                None
                            }
                        };

                        if let Some(idx) = next_idx {
                            let _ = tx_clone.send(AudioCommand::PlayTrack(idx));
                        }
                    }
                }
            }
        })
        .expect("Failed to spawn audio engine thread");
// Fuck if this happens i am going to delete everything and go fuck a first class attendant on a plane flying 30 thousnad meters in the air (no im ace but ill get anger)
    tx
}
// Figure out what fucking devices i have and what the fuck they are doing (if they are fucking any other app) (the last line was funny but not relevant god i hate my brain)
fn enumerate_output_devices() -> Vec<String> {
    let mut names = vec!["Default".to_string()];
    let host = rodio::cpal::default_host();
    if let Ok(devices) = host.output_devices() {
        for dev in devices {
            if let Ok(name) = dev.name() {
                if !names.contains(&name) {
                    names.push(name);
                }
            }
        }
    }
    names
}
// Actually open the device. (of the pussyhole. old as shit.)
fn open_device_stream(device_name: &str) -> Option<(OutputStream, OutputStreamHandle)> {
    if device_name != "Default" {
        let host = rodio::cpal::default_host();
        if let Ok(mut devices) = host.output_devices() {
            if let Some(dev) = devices.find(|d| d.name().map(|n| n == device_name).unwrap_or(false))
            {
                if let Ok(bundle) = OutputStream::try_from_device(&dev) {
                    return Some(bundle);
                }
            }
        }
    }
    OutputStream::try_default().ok()
}
// Haha why is this in a audio engine
fn spawn_library_scanner(root_path: PathBuf, shared_state: Arc<Mutex<SharedState>>) {
    {
        let mut state = shared_state.lock().unwrap();
        if state.is_scanning {
            return;
        }
        state.is_scanning = true;
        state.status_msg = format!("Scanning tags in {}...", root_path.display());
    }

    thread::Builder::new()
        .name("aether-library-scanner".into())
        .spawn(move || {
            let mut discovered = Vec::new();
            let valid_exts = ["mp3", "flac", "wav", "ogg", "m4a", "aac", "opus"];

            if root_path.exists() {
                for entry in WalkDir::new(&root_path)
                    .follow_links(true)
                    .max_depth(6)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    let path = entry.path();
                    if path.is_file() {
                        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                            if valid_exts.contains(&ext.to_lowercase().as_str()) {
                                discovered.push(read_track_metadata(path));
                            }
                        }
                    }
                }
            } else{
                println!("Path not found. Try mounting the drive, moving to another folder or checking your music.");
            }
        // Haha i presented rustc with garbage and said its good code even though its of the pussyhole. also this println shit does fuck all how do i fix it ahhhhhhhhh.
// Why is the sort in audio.rs why me
// Edit: this shit so shitty im presenting people with garbage with good ui haha
            discovered.sort_by(|a, b| {
                a.artist
                    .to_lowercase()
                    .cmp(&b.artist.to_lowercase())
                    .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
            });

            if let Ok(mut state) = shared_state.lock() {
                let count = discovered.len();
                state.track_list = discovered;
                state.is_scanning = false;
                state.status_msg = format!("Indexed {} tracks via audio tags.", count);
            }
        })
        .ok();
}

/// Gemini made this code for metadata (sorry)
fn read_track_metadata(path: &Path) -> TrackEntry {
    let fallback_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown Track")
        .to_string();

    let fallback_folder = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown Artist")
        .to_string();

    if let Ok(tagged_file) = Probe::open(path).and_then(|p| p.read()) {
        let duration_secs = tagged_file.properties().duration().as_secs();
        if let Some(tag) = tagged_file
            .primary_tag()
            .or_else(|| tagged_file.first_tag())
        {
            let title = tag
                .title()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| fallback_stem.clone());

            let artist = tag
                .artist()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| fallback_folder.clone());

            let album = tag
                .album()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "Single".to_string());

            return TrackEntry {
                path: path.to_path_buf(),
                title,
                artist,
                album,
                duration_secs,
            };
        }
    }
// why unknown. WHY DONT YOU KNOW IT
    TrackEntry {
        path: path.to_path_buf(),
        title: fallback_stem,
        artist: fallback_folder,
        album: "Unknown Album".to_string(),
        duration_secs: 0,
    }
}

/// Final option: GPU cover art extracting (oh no rip 3dfx :p)
fn extract_cover_art(audio_path: &Path) -> Option<CoverArtData> {
    let mut raw_bytes: Option<Vec<u8>> = None;

    if let Ok(tagged_file) = Probe::open(audio_path).and_then(|p| p.read()) {
        for tag in tagged_file.tags() {
            if let Some(pic) = tag.pictures().first() {
                raw_bytes = Some(pic.data().to_vec());
                break;
            }
        }
    }

    // Nuclear solution if everything gets fucked to shit.
    if raw_bytes.is_none() {
        if let Some(parent) = audio_path.parent() {
            for candidate in [
                "cover.jpg",
                "cover.png",
                "folder.jpg",
                "folder.png",
                "front.jpg",
            ] {
                let p = parent.join(candidate);
                if p.exists() {
                    if let Ok(bytes) = fs::read(p) {
                        raw_bytes = Some(bytes);
                        break;
                    }
                }
            }
        }
    }
// memory?? i dont know this tihs
    let bytes = raw_bytes?;
    let decoded = image::load_from_memory(&bytes).ok()?;
    let resized = decoded.resize_to_fill(512, 512, image::imageops::FilterType::Triangle);
    let rgba_img = resized.to_rgba8();
    let (width, height) = rgba_img.dimensions();

    // This shit that i dont understand that somehow fixes MPRIS. Courtesy of several hours on stackoverflow.
    let cache_path = std::env::temp_dir().join("aether_current_cover.png");
    let _ = rgba_img.save(&cache_path);
// yay covers
    Some(CoverArtData {
        width: width as usize,
        height: height as usize,
        rgba: rgba_img.into_raw(),
        cache_path: Some(cache_path),
    })
}
