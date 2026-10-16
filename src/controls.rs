// OS media controls via souvlaki (MPRIS on Linux, SMTC on Windows, MPRemoteCommandCenter on macOS).
// Replaces mpris.rs: same call shape as spawn_mpris_server.
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

// Everything we need from SharedState, copied out so the lock is held as briefly as possible.
struct Snap {
    idx: Option<usize>,
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

fn seek_by(tx: &Sender<AudioCommand>, state: &Arc<Mutex<SharedState>>, dir: SeekDirection, by: Duration) {
    let Ok(st) = state.lock() else { return };
    let target = match dir {
        SeekDirection::Forward => st.elapsed + by,
        SeekDirection::Backward => st.elapsed.saturating_sub(by),
    };
    let _ = tx.send(AudioCommand::Seek(target));
}

pub fn spawn_controls(tx: Sender<AudioCommand>, state: Arc<Mutex<SharedState>>, hwnd: Option<usize>) {
    thread::Builder::new()
        .name("aether-controls".into())
        .spawn(move || {
            let mut controls = match MediaControls::new(PlatformConfig {
                dbus_name: "aether", // -> org.mpris.MediaPlayer2.aether
                display_name: "Aether",
                hwnd: hwnd.map(|h| h as *mut std::ffi::c_void), // Windows needs a real HWND, None elsewhere
            }) {
                Ok(c) => c,
                Err(_) => return,
            };

            // ---- incoming: OS / media keys / widgets -> audio engine ----
            let ev_tx = tx.clone();
            let ev_state = Arc::clone(&state);
            let playing_now = {
                let s = Arc::clone(&state);
                move || s.lock().map(|s| s.is_playing).unwrap_or(false)
            };
            let _ = controls.attach(move |e| match e {
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
                MediaControlEvent::SetPosition(p) => {
                    let _ = ev_tx.send(AudioCommand::Seek(p.0));
                }
                MediaControlEvent::Seek(dir) => seek_by(&ev_tx, &ev_state, dir, DEFAULT_SEEK),
                MediaControlEvent::SeekBy(dir, by) => seek_by(&ev_tx, &ev_state, dir, by),
                MediaControlEvent::SetVolume(v) => {
                    let _ = ev_tx.send(AudioCommand::SetVolume(v as f32));
                }
                _ => {}
            });

            // ---- outgoing: SharedState -> OS ----
            let mut last_idx: Option<usize> = None;
            let mut last_cover: Option<String> = None;
            let mut last_playing = false;
            let mut last_elapsed = Duration::ZERO;
            let mut first = true;

            loop {
                thread::sleep(POLL);
                let Some(s) = snapshot(&state) else { continue };

                let track_changed = first || s.idx != last_idx || s.cover != last_cover;
                if track_changed && s.has_track {
                    let _ = controls.set_metadata(MediaMetadata {
                        title: Some(&s.title),
                        artist: Some(&s.artist),
                        album: Some(&s.album),
                        cover_url: s.cover.as_deref(),
                        duration: (!s.duration.is_zero()).then_some(s.duration),
                    });
                }

                // If playback position moved more than expected, the user seeked: resync the OS.
                let expected = if last_playing { last_elapsed + POLL } else { last_elapsed };
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
                last_cover = s.cover;
                last_playing = s.playing;
                last_elapsed = s.elapsed;
            }
        })
        .ok();
}
