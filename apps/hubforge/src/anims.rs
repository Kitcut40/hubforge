//! Touches animées intégrées (dessins originaux du projet HubForge, même licence).
//! Une touche les utilise avec l'icône « anim:<id> ».

/// (identifiant, nom affiché, GIF 72×72)
pub const ANIMS: &[(&str, &str, &[u8])] = &[
    ("live", "En direct", include_bytes!("../../../assets/anim/live.gif")),
    ("rec", "Enregistrement", include_bytes!("../../../assets/anim/rec.gif")),
    ("onair", "On Air", include_bytes!("../../../assets/anim/onair.gif")),
    ("equalizer", "Égaliseur", include_bytes!("../../../assets/anim/equalizer.gif")),
    ("wave", "Onde sonore", include_bytes!("../../../assets/anim/wave.gif")),
    ("loader", "Chargement", include_bytes!("../../../assets/anim/loader.gif")),
    ("heartbeat", "Battement de cœur", include_bytes!("../../../assets/anim/heartbeat.gif")),
    ("radar", "Radar", include_bytes!("../../../assets/anim/radar.gif")),
    ("bell", "Notification", include_bytes!("../../../assets/anim/bell.gif")),
    ("rainbow", "Arc-en-ciel", include_bytes!("../../../assets/anim/rainbow.gif")),
    ("sparkles", "Scintillements", include_bytes!("../../../assets/anim/sparkles.gif")),
    ("fire", "Flamme", include_bytes!("../../../assets/anim/fire.gif")),
    ("rocket", "Fusée", include_bytes!("../../../assets/anim/rocket.gif")),
    ("coffee", "Pause café", include_bytes!("../../../assets/anim/coffee.gif")),
    ("rain", "Pluie", include_bytes!("../../../assets/anim/rain.gif")),
    ("matrix", "Pluie de code", include_bytes!("../../../assets/anim/matrix.gif")),
];

pub fn get(id: &str) -> Option<&'static [u8]> {
    ANIMS.iter().find(|(i, _, _)| *i == id).map(|(_, _, b)| *b)
}
