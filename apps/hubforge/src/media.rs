//! Musique en cours : titre, artiste, pochette et état de lecture du lecteur
//! que Windows affiche dans ses contrôles multimédia (Spotify, navigateur,
//! VLC, lecteur Windows…).
//!
//! Un fil dédié interroge Windows toutes les secondes, seulement à partir du
//! moment où une touche « musique en cours » existe (`get`).

use image::RgbImage;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Debug, Clone, Default)]
pub struct Media {
    pub title: String,
    pub artist: String,
    pub playing: bool,
    /// Pochette réduite en 72×72 (recadrée au centre), si le lecteur en donne une.
    pub cover: Option<Arc<RgbImage>>,
}

impl PartialEq for Media {
    fn eq(&self, o: &Media) -> bool {
        self.title == o.title && self.artist == o.artist && self.playing == o.playing && self.cover.is_some() == o.cover.is_some()
    }
}

#[derive(Default)]
struct Shared {
    started: bool,
    now: Option<Media>,
}

fn shared() -> &'static Mutex<Shared> {
    static S: OnceLock<Mutex<Shared>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Shared::default()))
}

/// Ce qui joue en ce moment (None : rien). Démarre la surveillance au premier appel.
pub fn get() -> Option<Media> {
    let Ok(mut s) = shared().lock() else { return None };
    if !s.started {
        s.started = true;
        std::thread::Builder::new().name("musique".into()).spawn(imp::run).ok();
    }
    s.now.clone()
}

#[cfg(test)]
pub fn inject(m: Option<Media>) {
    if let Ok(mut s) = shared().lock() {
        s.started = true;
        s.now = m;
    }
}

fn publish(m: Option<Media>) {
    if let Ok(mut s) = shared().lock() {
        s.now = m;
    }
}

/// Recadre une pochette au centre et la réduit en 72×72.
#[cfg_attr(not(windows), allow(dead_code))]
fn cover_from_bytes(bytes: &[u8]) -> Option<Arc<RgbImage>> {
    let img = image::load_from_memory(bytes).ok()?.to_rgb8();
    let (w, h) = img.dimensions();
    let side = w.min(h);
    let crop = image::imageops::crop_imm(&img, (w - side) / 2, (h - side) / 2, side, side).to_image();
    let s = hubforge_device::KEY_IMAGE_SIZE;
    Some(Arc::new(image::imageops::resize(&crop, s, s, image::imageops::FilterType::Lanczos3)))
}

#[cfg(windows)]
mod imp {
    use super::{cover_from_bytes, publish, Media};
    use std::time::Duration;
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSessionManager as Manager, GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
    };
    use windows::Storage::Streams::DataReader;
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

    pub fn run() {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        let mut manager: Option<Manager> = None;
        // Pochette en cache : on ne la relit que si le morceau change.
        let mut last_key = String::new();
        let mut last_cover = None;
        loop {
            if manager.is_none() {
                manager = Manager::RequestAsync().and_then(|op| op.join()).ok();
            }
            let now = manager.as_ref().and_then(|m| read(m, &mut last_key, &mut last_cover));
            publish(now);
            std::thread::sleep(Duration::from_millis(1000));
        }
    }

    fn read(m: &Manager, last_key: &mut String, last_cover: &mut Option<std::sync::Arc<image::RgbImage>>) -> Option<Media> {
        let s = m.GetCurrentSession().ok()?;
        let props = s.TryGetMediaPropertiesAsync().ok()?.join().ok()?;
        let title = props.Title().map(|t| t.to_string()).unwrap_or_default();
        let artist = props.Artist().map(|t| t.to_string()).unwrap_or_default();
        if title.is_empty() && artist.is_empty() {
            return None;
        }
        let playing = s.GetPlaybackInfo().and_then(|i| i.PlaybackStatus()).map(|st| st == Status::Playing).unwrap_or(false);
        let key = format!("{title}\u{1}{artist}");
        if key != *last_key {
            *last_key = key;
            *last_cover = props.Thumbnail().ok().and_then(|t| {
                let stream = t.OpenReadAsync().ok()?.join().ok()?;
                let size = stream.Size().ok()? as u32;
                if size == 0 || size > 8 * 1024 * 1024 {
                    return None;
                }
                let reader = DataReader::CreateDataReader(&stream).ok()?;
                reader.LoadAsync(size).ok()?.join().ok()?;
                let mut buf = vec![0u8; size as usize];
                reader.ReadBytes(&mut buf).ok()?;
                cover_from_bytes(&buf)
            });
        }
        Some(Media { title, artist, playing, cover: last_cover.clone() })
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn run() {
        // Pas de contrôles multimédia en dehors de Windows pour l'instant.
        super::publish(None);
    }
}
