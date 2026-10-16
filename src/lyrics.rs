// Fix for people who are smooth brain and forget words in songs.
use crate::state::{SharedState, TrackEntry};
use lofty::prelude::*;
use lofty::probe::Probe;
use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct LyricLine {
    pub timestamp: Duration,
    pub text: String,
}

#[derive(Deserialize)]
struct LrcLibResponse {
    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
    #[serde(rename = "plainLyrics")]
    plain_lyrics: Option<String>,
}

/// 1. Check for an LRC and the second bit is FLAC shit idk i cant explain shit
pub fn load_local_or_embedded_lyrics(audio_path: &Path) -> (Vec<LyricLine>, String) {
    // Stage 1: Sidecar .lrc file
    let lrc_path = audio_path.with_extension("lrc");
    if let Ok(content) = fs::read_to_string(&lrc_path) {
        let parsed = parse_lrc(&content);
        if !parsed.is_empty() {
            return (parsed, "Local .lrc file".to_string());
        }
    }

    // The motherfucker that actually pulls tags (ID3/ogg)
    if let Ok(tagged_file) = Probe::open(audio_path).and_then(|p| p.read()) {
        for tag in tagged_file.tags() {
            if let Some(lyrics_str) = tag.get_string(&ItemKey::Lyrics) {
                let parsed = parse_lrc_or_plain(lyrics_str);
                if !parsed.is_empty() {
                    return (parsed, "Embedded Audio Tag".to_string());
                }
            }
            // Check for whatever the fuck that these taggers poisoned the lyrical shit with and.. still use them? (idk something about ethics and standards im too teen to get it haha)
            for item in tag.items() {
                if let ItemKey::Unknown(key) = item.key() {
                    let k = key.to_uppercase();
                    if k.contains("LYRIC") || k == "UNSYNCEDLYRICS" || k == "SYLT" || k == "USLT"
                    {
                        if let Some(txt) = item.value().text() {
                            let parsed = parse_lrc_or_plain(txt);
                            if !parsed.is_empty() {
                                return (parsed, format!("Embedded Tag ({})", key));
                            }
                        }
                    }
                }
            }
        }
    }

    (Vec::new(), "None".to_string())
}

/// If all else fails, utilize the net and PULL THE lyric shit.
pub fn spawn_online_lyrics_search(track: TrackEntry, shared_state: Arc<Mutex<SharedState>>) {
    {
        let mut st = shared_state.lock().unwrap();
        if st.is_fetching_lyrics {
            return;
        }
        st.is_fetching_lyrics = true;
        st.status_msg = format!("Searching online lyrics for \"{}\"...", track.title);
    }

    thread::Builder::new()
        .name("aether-lyrics-fetcher".into())
        .spawn(move || {
            let result = query_lrclib(&track);

            let mut st = shared_state.lock().unwrap();
            st.is_fetching_lyrics = false;

            // Make sure the listener hasnt changed the song while lyrics were being searched (like a bitch. yes i see you sam)
            let still_same_track = st
                .current_track
                .as_ref()
                .map(|curr| curr.path == track.path)
                .unwrap_or(false);

            if !still_same_track {
                return;
            }

            match result {
                Ok((lines, raw_lrc, source_label)) => {
                    // If permissive, cache lrc with file cuz better that way (idk its probs boilerplate slop that looks like ai its 3am right now and i need to sleep)
                    if let Some(raw) = raw_lrc {
                        let lrc_path = track.path.with_extension("lrc");
                        let _ = fs::write(&lrc_path, raw);
                    }
                    st.lyrics = lines;
                    st.lyrics_source = source_label.clone();
                    st.status_msg = format!("Lyrics loaded ({})", source_label);
                }
                Err(err_msg) => {
                    st.status_msg = err_msg;
                }
            }
        })
        .ok();
}

fn query_lrclib(track: &TrackEntry) -> Result<(Vec<LyricLine>, Option<String>, String), String> {
    let artist_enc = urlencoding::encode(&track.artist);
    let title_enc = urlencoding::encode(&track.title);
    let album_enc = urlencoding::encode(&track.album);

    // Fetch the shit (i do feel better after that nap tho, forgot to commit so nothing got saved (yay))
    let get_url = format!(
        "https://lrclib.net/api/get?artist_name={}&track_name={}&album_name={}",
        artist_enc, title_enc, album_enc
    );
    // idfk why im doing this, its not even on github
    if let Ok(resp) = ureq::get(&get_url)
        .set("User-Agent", "AetherPlayer/0.2.0 (https://github.com/ayxrc/aether)")
        .call()
    {
        if let Ok(item) = resp.into_json::<LrcLibResponse>() {
            if let Some(synced) = item.synced_lyrics {
                let parsed = parse_lrc(&synced);
                if !parsed.is_empty() {
                    return Ok((parsed, Some(synced), "Online (LRCLIB Synced)".to_string()));
                }
            }
            if let Some(plain) = item.plain_lyrics {
                let parsed = parse_lrc_or_plain(&plain);
                if !parsed.is_empty() {
                    return Ok((parsed, None, "Online (LRCLIB Plain)".to_string()));
                }
            }
        }
    }

    // If all else fails (or if the internet dies like a bitch), try this URI
    let search_q = format!("{} {}", track.artist, track.title);
    let search_url = format!(
        "https://lrclib.net/api/search?q={}",
        urlencoding::encode(&search_q)
    );
    // Another weird thing?? (3am me is weird yes)
    let resp = ureq::get(&search_url)
        .set("User-Agent", "AetherPlayer/0.2.0 (https://github.com/ayxrc/aether)")
        .call()
        .map_err(|_| "Online lyrics network lookup failed.".to_string())?;

    let results: Vec<LrcLibResponse> = resp
        .into_json()
        .map_err(|_| "Invalid response from lyrics server.".to_string())?;

    for item in results {
        if let Some(synced) = item.synced_lyrics {
            let parsed = parse_lrc(&synced);
            if !parsed.is_empty() {
                return Ok((parsed, Some(synced), "Online (LRCLIB Synced)".to_string()));
            }
        }
        if let Some(plain) = item.plain_lyrics {
            let parsed = parse_lrc_or_plain(&plain);
            if !parsed.is_empty() {
                return Ok((parsed, None, "Online (LRCLIB Plain)".to_string()));
            }
        }
    }

    Err(format!("No online lyrics found for \"{}\".", track.title))
}

pub fn parse_lrc_or_plain(content: &str) -> Vec<LyricLine> {
    let synced = parse_lrc(content);
    if !synced.is_empty() {
        return synced;
    }

    // If the mofo isnt synced, we do this.
    content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .enumerate()
        .map(|(idx, text)| LyricLine {
            timestamp: Duration::from_secs((idx as u64) * 4),
            text: text.to_string(),
        })
        .collect()
}

pub fn parse_lrc(content: &str) -> Vec<LyricLine> {
    let mut parsed_lines = Vec::new();

    for raw_line in content.lines() {
        let mut remainder = raw_line.trim();
        if remainder.is_empty() {
            continue;
        }

        let mut timestamps = Vec::new();

        while remainder.starts_with('[') {
            if let Some(close_idx) = remainder.find(']') {
                let tag = &remainder[1..close_idx];
                if let Some(ts) = parse_timestamp_tag(tag) {
                    timestamps.push(ts);
                    remainder = &remainder[close_idx + 1..];
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        let lyric_text = remainder.trim().to_string();
        for ts in timestamps {
            parsed_lines.push(LyricLine {
                timestamp: ts,
                text: if lyric_text.is_empty() {
                    "•••".to_string()
                } else {
                    lyric_text.clone()
                },
            });
        }
    }

    parsed_lines.sort_by_key(|line| line.timestamp);
    parsed_lines
}

fn parse_timestamp_tag(tag: &str) -> Option<Duration> {
    let (min_str, sec_part) = tag.split_once(':')?;
    let minutes: u64 = min_str.trim().parse().ok()?;

    let seconds: f64 = sec_part.trim().replace(':', ".").parse().ok()?;
    if seconds < 0.0 {
        return None;
    }

    let total_secs = (minutes as f64) * 60.0 + seconds;
    Some(Duration::from_secs_f64(total_secs))
}

pub fn current_lyric_index(lyrics: &[LyricLine], elapsed: Duration) -> Option<usize> {
    if lyrics.is_empty() {
        return None;
    }
    match lyrics.binary_search_by_key(&elapsed, |line| line.timestamp) {
        Ok(idx) => Some(idx),
        Err(0) => None,
        Err(idx) => Some(idx - 1),
    }
}
