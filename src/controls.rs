// OS media controls via souvlaki 0.8.3
use crate::audio::AudioCommand;
use crate::state::SharedState;
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
    SeekDirection,
};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const POLL: Duration = Duration::from_millis(250);
const DEFAULT_SEEK: Duration = Duration::from_secs(5);

struct Snap {
    idx: Option<usize>,
    cover_id: u64,
    playing: bool,
    has_track: bool,
    elapsed: Duration,
    duration: Duration,
    title: String,
    artist: String,
    album: String,
    cover: Option<String>,
}

fn snapshot(state: &Arc<Mutex<SharedState>>) -> Option<Snap> {
    let st = state.lock().ok()?;
    let t = st.current_track.as_ref();
    Some(Snap {
        idx: st.current_track_idx,
        cover_id: st.cover_art_id,
        playing: st.is_playing,
        has_track: t.is_some(),
        elapsed: st.elapsed,
        duration: st.duration,
        title: t.map(|t| t.title.clone()).unwrap_or_default(),
        artist: t.map(|t| t.artist.clone()).unwrap_or_default(),
        album: t.map(|t| t.album.clone()).unwrap_or_default(),
        cover: st
            .cover_art
            .as_ref()
            .and_then(|a| a.cache_path.as_ref())
            .map(|p| format!("file://{}", p.display())),
    })
}

fn seek_by(
    tx: &Sender<AudioCommand>,
    state: &Arc<Mutex<SharedState>>,
    dir: SeekDirection,
    by: Duration,
) {
    let Ok(st) = state.lock() else { return };
    let target = match dir {
        SeekDirection::Forward => {
            if st.duration.is_zero() {
                st.elapsed + by
            } else {
                (st.elapsed + by).min(st.duration)
            }
        }
        SeekDirection::Backward => st.elapsed.saturating_sub(by),
    };
    let _ = tx.send(AudioCommand::Seek(target));
}

pub fn spawn_mpris_server(tx: Sender<AudioCommand>, state: Arc<Mutex<SharedState>>) {
    thread::Builder::new()
        .name("aether-controls".into())
        .spawn(move || {
            let hwnd = create_dummy_hwnd();
            let config = PlatformConfig {
                dbus_name: "aether",
                display_name: "Aether",
                hwnd,
            };

            let mut controls = match MediaControls::new(config) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to initialize souvlaki media controls: {:?}", e);
                    return;
                }
            };

            let ev_tx = tx.clone();
            let ev_state = Arc::clone(&state);
            let playing_now = {
                let s = Arc::clone(&state);
                move || s.lock().map(|s| s.is_playing).unwrap_or(false)
            };

            let _ = controls.attach(move |event: MediaControlEvent| match event {
                MediaControlEvent::Play => {
                    if !playing_now() {
                        let _ = ev_tx.send(AudioCommand::TogglePause);
                    }
                }
                MediaControlEvent::Pause | MediaControlEvent::Stop => {
                    if playing_now() {
                        let _ = ev_tx.send(AudioCommand::TogglePause);
                    }
                }
                MediaControlEvent::Toggle => {
                    let _ = ev_tx.send(AudioCommand::TogglePause);
                }
                MediaControlEvent::Next => {
                    let _ = ev_tx.send(AudioCommand::NextTrack);
                }
                MediaControlEvent::Previous => {
                    let _ = ev_tx.send(AudioCommand::PrevTrack);
                }
                MediaControlEvent::SetPosition(MediaPosition(pos)) => {
                    let _ = ev_tx.send(AudioCommand::Seek(pos));
                }
                MediaControlEvent::Seek(dir) => seek_by(&ev_tx, &ev_state, dir, DEFAULT_SEEK),
                MediaControlEvent::SeekBy(dir, by) => seek_by(&ev_tx, &ev_state, dir, by),
                MediaControlEvent::SetVolume(vol) => {
                    let _ = ev_tx.send(AudioCommand::SetVolume(vol as f32));
                }
                _ => {}
            });

            let mut last_idx: Option<usize> = None;
            let mut last_cover_id: u64 = 0;
            let mut last_playing = false;
            let mut last_elapsed = Duration::ZERO;
            let mut first = true;

            loop {
                pump_windows_messages();
                thread::sleep(POLL);

                let Some(s) = snapshot(&state) else { continue };

                let track_changed = first || s.idx != last_idx || s.cover_id != last_cover_id;
                if track_changed && s.has_track {
                    let _ = controls.set_metadata(MediaMetadata {
                        title: Some(s.title.as_str()),
                        artist: Some(s.artist.as_str()),
                        album: Some(s.album.as_str()),
                        cover_url: s.cover.as_deref(),
                        duration: (!s.duration.is_zero()).then_some(s.duration),
                    });
                }

                let expected = if last_playing {
                    last_elapsed + POLL
                } else {
                    last_elapsed
                };
                let jumped = s.elapsed.abs_diff(expected) > Duration::from_millis(1200);

                if track_changed || s.playing != last_playing || jumped {
                    let progress = Some(MediaPosition(s.elapsed));
                    let pb = if s.playing {
                        MediaPlayback::Playing { progress }
                    } else if s.has_track {
                        MediaPlayback::Paused { progress }
                    } else {
                        MediaPlayback::Stopped
                    };
                    let _ = controls.set_playback(pb);
                }

                first = false;
                last_idx = s.idx;
                last_cover_id = s.cover_id;
                last_playing = s.playing;
                last_elapsed = s.elapsed;
            }
        })
        .ok();
}

pub use spawn_mpris_server as spawn_controls;

#[cfg(target_os = "windows")]
fn create_dummy_hwnd() -> Option<*mut std::ffi::c_void> {
    use std::ptr::null_mut;
    use windows_sys::Win32::UI::WindowsAndMessaging::{CreateWindowExW, HWND_MESSAGE};

    unsafe {
        // Uses built-in Win32 "STATIC" class so we don't need RegisterClassW or Win32_Graphics_Gdi
        let class_name: [u16; 7] = [
            b'S' as u16,
            b'T' as u16,
            b'A' as u16,
            b'T' as u16,
            b'I' as u16,
            b'C' as u16,
            0,
        ];
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            class_name.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            null_mut(),
            null_mut(),
            null_mut(),
        );
        if hwnd.is_null() {
            None
        } else {
            Some(hwnd as *mut std::ffi::c_void)
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn create_dummy_hwnd() -> Option<*mut std::ffi::c_void> {
    None
}

#[cfg(target_os = "windows")]
fn pump_windows_messages() {
    use std::mem::zeroed;
    use std::ptr::null_mut;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
    };

    unsafe {
        let mut msg: MSG = zeroed();
        while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn pump_windows_messages() {}