// Linux specific bullshittery
use crate::audio::AudioCommand;
use crate::state::SharedState;
use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use zbus::blocking::Connection;
use zbus::interface;
use zbus::zvariant::{ObjectPath, Value};

struct MprisRoot;
// Actually sets up MPRIS
#[interface(name = "org.mpris.MediaPlayer2")]
impl MprisRoot {
    #[zbus(property)]
      fn desktop_entry(&self) -> &str {
        "aether" // WHY END-4
    }
    #[zbus(property)]
    fn can_quit(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn can_raise(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn has_track_list(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn identity(&self) -> &str {
        "Aether"
    }

    #[zbus(property)]
    fn supported_uri_schemes(&self) -> Vec<&str> {
        vec!["file"]
    }

    #[zbus(property)]
    fn supported_mime_types(&self) -> Vec<&str> {
        vec![
            "audio/mpeg",
            "audio/flac",
            "audio/x-flac",
            "audio/ogg",
            "audio/wav",
            "audio/mp4",
        ]
    }

    fn raise(&self) {}
    fn quit(&self) {}
}

struct MprisPlayer {
    tx: Sender<AudioCommand>,
    state: Arc<Mutex<SharedState>>,
}
// Defines to MPRIS what the fuck this shit actually is (or something idk)
#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl MprisPlayer {
    fn next(&self) {
        let _ = self.tx.send(AudioCommand::NextTrack);
    }

    fn previous(&self) {
        let _ = self.tx.send(AudioCommand::PrevTrack);
    }

    fn pause(&self) {
        let is_playing = self.state.lock().map(|s| s.is_playing).unwrap_or(false);
        if is_playing {
            let _ = self.tx.send(AudioCommand::TogglePause);
        }
    }

    fn play_pause(&self) {
        let _ = self.tx.send(AudioCommand::TogglePause);
    }

    fn stop(&self) {
        let is_playing = self.state.lock().map(|s| s.is_playing).unwrap_or(false);
        if is_playing {
            let _ = self.tx.send(AudioCommand::TogglePause);
        }
    }

    fn play(&self) {
        let is_playing = self.state.lock().map(|s| s.is_playing).unwrap_or(false);
        if !is_playing {
            let _ = self.tx.send(AudioCommand::TogglePause);
        }
    }

    fn seek(&self, offset_micros: i64) {
        if let Ok(st) = self.state.lock() {
            let current_micros = st.elapsed.as_micros() as i64;
            let target_micros = (current_micros + offset_micros).max(0) as u64;
            let _ = self
                .tx
                .send(AudioCommand::Seek(Duration::from_micros(target_micros)));
        }
    }

    fn set_position(&self, _track_id: ObjectPath<'_>, position_micros: i64) {
        if position_micros >= 0 {
            let _ = self.tx.send(AudioCommand::Seek(Duration::from_micros(
                position_micros as u64,
            )));
        }
    }
// wE aRe nOt sAvAgES hErE!! iF nO pLaY sTr tHeN lEeNuX eRrOr aNd pIiIiIiIiIiIZdE_ sorry mama
    #[zbus(property)]
    fn playback_status(&self) -> String {
        if let Ok(st) = self.state.lock() {
            if st.is_playing {
                "Playing".to_string()
            } else if st.current_track.is_some() {
                "Paused".to_string()
            } else {
                "Stopped".to_string()
            }
        } else {
            "Stopped".to_string()
        }
    }
// How it's gonna display (unless youre legally blind in that case why are you even here)
    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, Value<'static>> {
        let mut map = HashMap::new();
        if let Ok(st) = self.state.lock() {
            if let Some(ref track) = st.current_track {
                if let Ok(track_id) = ObjectPath::try_from("/org/mpris/MediaPlayer2/Track/1") {
                    map.insert("mpris:trackid".to_string(), Value::new(track_id));
                }
                map.insert(
                    "mpris:length".to_string(),
                    Value::I64(st.duration.as_micros() as i64),
                );
                map.insert(
                    "xesam:title".to_string(),
                    Value::from(vec![track.title.clone()]),
                );
                map.insert(
                    "xesam:artist".to_string(),
                    Value::new(vec![track.artist.clone()]),
                );
                map.insert(
                    "xesam:album".to_string(),
                    Value::new(track.album.clone()),
                );
                if let Some(ref art) = st.cover_art {
                    if let Some(ref cache_path) = art.cache_path {
                        let art_url = format!("file://{}", cache_path.display());
                        map.insert("mpris:artUrl".to_string(), Value::new(art_url));
                    }
                }
            }
        }
        map
    }
// eY aYu! iF nO cOnTrOlS tHeN hOw mEdIa pLaYeR -- You probably. In that case. HERE THEY ARE!!
    #[zbus(property)]
    fn volume(&self) -> f64 {
        self.state
            .lock()
            .map(|s| s.config.master_volume as f64)
            .unwrap_or(0.75)
    }

    #[zbus(property)]
    fn set_volume(&self, val: f64) {
        let _ = self.tx.send(AudioCommand::SetVolume(val as f32));
    }

    #[zbus(property)]
    fn position(&self) -> i64 {
        self.state
            .lock()
            .map(|s| s.elapsed.as_micros() as i64)
            .unwrap_or(0)
    }

    #[zbus(property)]
    fn minimum_rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    fn maximum_rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    fn rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_play(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_pause(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_seek(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_control(&self) -> bool {
        true
    }
}
//fuck that shit was long. anyway actually launches the mpris server so the linux kernel doesnt go "iM a lEgAlLy bLiNd pEnGuIn cHeEp cHeEp mOtHafUkA"
pub fn spawn_mpris_server(tx: Sender<AudioCommand>, state: Arc<Mutex<SharedState>>) {
    thread::Builder::new()
        .name("aether-mpris-dbus".into())
        .spawn(move || {
            let conn = match Connection::session() {
                Ok(c) => c,
                Err(_) => return,
            };

            let _ = conn.object_server().at("/org/mpris/MediaPlayer2", MprisRoot);
            let _ = conn.object_server().at(
                "/org/mpris/MediaPlayer2",
                MprisPlayer {
                    tx,
                    state: Arc::clone(&state),
                },
            );
            let _ = conn.request_name("org.mpris.MediaPlayer2.aether");

            // Keep D-Bus server alive and emit property change notifications when track or state changes
            let mut last_track_idx = None;
            let mut last_playing = false;

            loop {
                thread::sleep(Duration::from_millis(250));

                let (curr_idx, curr_playing) = {
                    if let Ok(st) = state.lock() {
                        (st.current_track_idx, st.is_playing)
                    } else {
                        continue;
                    }
                };

                if curr_idx != last_track_idx || curr_playing != last_playing {
                    last_track_idx = curr_idx;
                    last_playing = curr_playing;

                    if let Ok(iface_ref) = conn
                        .object_server()
                        .interface::<_, MprisPlayer>("/org/mpris/MediaPlayer2")
                    {
                        let ctxt = iface_ref.signal_context();
                        let iface_2 = iface_ref.get();
                        let iface: &MprisPlayer = &*iface_2;
                        let _ = MprisPlayer::playback_status_invalidate(iface, ctxt);
                        let _ = MprisPlayer::playback_status_changed(iface, ctxt);
                        let _ = MprisPlayer::metadata_invalidate(iface, ctxt);
                        let _ = MprisPlayer::metadata_changed(iface,ctxt);
                    }
                }
            }
        })
        .ok();
}
// And that's it. hope ts works ;)