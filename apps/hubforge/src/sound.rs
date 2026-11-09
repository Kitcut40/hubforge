//! Soundboard : jouer des sons sur une touche.
//!
//! Les sons sont lus dans un fil à part (la sortie audio de `rodio` doit
//! rester dans le fil qui l'a créée). Chaque son peut être joué sur la sortie
//! principale et, si on l'a choisie, sur une seconde sortie : par exemple un
//! câble audio virtuel pour que les sons partent aussi dans Discord ou OBS.

use rodio::cpal::traits::HostTrait;
use rodio::{Decoder, DeviceTrait, OutputStream, OutputStreamHandle, Sink};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

pub enum SoundCmd {
    /// Jouer `path` (volume 0.0–1.0). `id` identifie la touche, pour
    /// arrêter ou recommencer son son au prochain appui.
    Play { id: String, path: PathBuf, volume: f32, mode: String },
    StopAll,
    /// Choisir les sorties (nom du périphérique, vide = sortie par défaut ; `second` vide = aucune).
    Devices { main: String, second: String },
}

#[derive(Clone)]
pub struct Sounds {
    tx: Sender<SoundCmd>,
}

impl Sounds {
    pub fn new() -> Sounds {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new().name("sons".into()).spawn(move || run(rx)).ok();
        Sounds { tx }
    }

    pub fn send(&self, cmd: SoundCmd) {
        let _ = self.tx.send(cmd);
    }
}

/// Extensions de fichiers son acceptées.
pub const EXTENSIONS: &[&str] = &["mp3", "wav", "ogg", "flac", "m4a", "aac"];

struct Output {
    _stream: OutputStream,
    handle: OutputStreamHandle,
}

fn open(name: &str) -> Option<Output> {
    let host = rodio::cpal::default_host();
    let device = if name.is_empty() {
        host.default_output_device()
    } else {
        host.output_devices().ok()?.find(|d| d.name().map(|n| n == name).unwrap_or(false))
    };
    let device = match device {
        Some(d) => d,
        None => {
            log::warn!("sortie audio « {name} » introuvable : sortie par défaut");
            host.default_output_device()?
        }
    };
    match OutputStream::try_from_device(&device) {
        Ok((stream, handle)) => Some(Output { _stream: stream, handle }),
        Err(e) => {
            log::warn!("sortie audio impossible à ouvrir : {e}");
            None
        }
    }
}

fn run(rx: Receiver<SoundCmd>) {
    let mut main_name = String::new();
    let mut second_name = String::new();
    let mut main: Option<Output> = None;
    let mut second: Option<Output> = None;
    let mut playing: HashMap<String, Vec<Sink>> = HashMap::new();
    while let Ok(cmd) = rx.recv() {
        playing.retain(|_, sinks| {
            sinks.retain(|s| !s.empty());
            !sinks.is_empty()
        });
        match cmd {
            SoundCmd::Devices { main: m, second: s } => {
                if m != main_name {
                    main = None;
                    main_name = m;
                }
                if s != second_name {
                    second = None;
                    second_name = s;
                }
            }
            SoundCmd::StopAll => {
                for (_, sinks) in playing.drain() {
                    for s in sinks {
                        s.stop();
                    }
                }
            }
            SoundCmd::Play { id, path, volume, mode } => {
                let already = playing.remove(&id);
                if let Some(sinks) = &already {
                    if mode != "overlap" {
                        for s in sinks {
                            s.stop();
                        }
                    }
                    // Mode « toggle » : rappuyer pendant la lecture arrête le son.
                    if mode == "toggle" {
                        continue;
                    }
                }
                let mut sinks = if mode == "overlap" { already.unwrap_or_default() } else { Vec::new() };
                if main.is_none() {
                    main = open(&main_name);
                }
                if second.is_none() && !second_name.is_empty() {
                    second = open(&second_name);
                }
                for out in [main.as_ref(), second.as_ref()].into_iter().flatten() {
                    let file = match std::fs::File::open(&path) {
                        Ok(f) => f,
                        Err(e) => {
                            log::warn!("son introuvable « {} » : {e}", path.display());
                            break;
                        }
                    };
                    let source = match Decoder::new(std::io::BufReader::new(file)) {
                        Ok(s) => s,
                        Err(e) => {
                            log::warn!("son illisible « {} » : {e}", path.display());
                            break;
                        }
                    };
                    match Sink::try_new(&out.handle) {
                        Ok(sink) => {
                            sink.set_volume(volume.clamp(0.0, 2.0));
                            sink.append(source);
                            sinks.push(sink);
                        }
                        Err(e) => log::warn!("lecture impossible : {e}"),
                    }
                }
                if !sinks.is_empty() {
                    playing.insert(id, sinks);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    /// Un petit WAV fabriqué ici se décode (les décodeurs sont bien compilés).
    #[test]
    fn decodes_wav() {
        let samples: Vec<i16> = (0..4410).map(|i| ((i as f32 * 0.06).sin() * 8000.0) as i16).collect();
        let data_len = (samples.len() * 2) as u32;
        let mut w = Vec::new();
        w.extend_from_slice(b"RIFF");
        w.extend_from_slice(&(36 + data_len).to_le_bytes());
        w.extend_from_slice(b"WAVEfmt ");
        w.extend_from_slice(&16u32.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes()); // PCM
        w.extend_from_slice(&1u16.to_le_bytes()); // mono
        w.extend_from_slice(&44100u32.to_le_bytes());
        w.extend_from_slice(&(44100u32 * 2).to_le_bytes());
        w.extend_from_slice(&2u16.to_le_bytes());
        w.extend_from_slice(&16u16.to_le_bytes());
        w.extend_from_slice(b"data");
        w.extend_from_slice(&data_len.to_le_bytes());
        for s in &samples {
            w.extend_from_slice(&s.to_le_bytes());
        }
        let d = rodio::Decoder::new(std::io::Cursor::new(w)).expect("wav décodé");
        assert_eq!(rodio::Source::sample_rate(&d), 44100);
        assert!(d.count() > 4000);
    }
}
