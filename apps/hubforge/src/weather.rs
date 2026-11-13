//! Météo des touches vivantes, via Open-Meteo (gratuit, sans compte ni clé).
//!
//! HubForge ne contacte internet que si une touche météo existe : le fil de
//! mise à jour démarre à la première demande (`get`). La ville vient des
//! réglages (`general.weather_city`) ; ses coordonnées sont cherchées une
//! fois, puis la prévision est rafraîchie toutes les 20 minutes.

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const REFRESH: Duration = Duration::from_secs(20 * 60);
const RETRY: Duration = Duration::from_secs(2 * 60);

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Day {
    /// 0 = dimanche … 6 = samedi
    pub dow: u8,
    pub code: u8,
    pub max: f32,
    pub min: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Weather {
    /// Nom de la ville trouvée.
    pub city: String,
    pub temp: f32,
    pub code: u8,
    pub is_day: bool,
    /// Aujourd'hui puis les jours suivants.
    pub days: Vec<Day>,
}

#[derive(Default)]
struct Shared {
    /// Ville demandée dans les réglages.
    city: String,
    /// Dernière météo connue (et pour quelle ville).
    data: Option<(String, Weather)>,
    error: Option<String>,
    /// Coordonnées de la ville (cache).
    place: Option<(String, f64, f64, String)>,
    next: Option<Instant>,
    started: bool,
}

fn shared() -> &'static Mutex<Shared> {
    static S: OnceLock<Mutex<Shared>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Shared::default()))
}

/// Ville choisie dans les réglages (appelé à chaque chargement de la configuration).
pub fn set_city(city: &str) {
    let city = city.trim().to_string();
    if let Ok(mut s) = shared().lock() {
        if s.city != city {
            s.city = city;
            s.next = None; // à rafraîchir tout de suite
            s.error = None;
        }
    }
}

/// Météo de la ville des réglages, ou un message (« Choisis ta ville… »).
/// Démarre la mise à jour en arrière-plan au premier appel.
pub fn get() -> Result<Weather, String> {
    let Ok(mut s) = shared().lock() else { return Err(crate::i18n::t("Météo indisponible", "Weather unavailable").into()) };
    if s.city.is_empty() {
        return Err(crate::i18n::t("Choisis ta ville dans les réglages", "Choose your city in the settings").into());
    }
    if !s.started {
        s.started = true;
        std::thread::Builder::new().name("météo".into()).spawn(run).ok();
    }
    match (&s.data, &s.error) {
        (Some((c, w)), _) if *c == s.city => Ok(w.clone()),
        (_, Some(e)) => Err(e.clone()),
        _ => Err(crate::i18n::t("Chargement…", "Loading…").into()),
    }
}

/// Pour les tests : une météo connue, sans accès à internet.
#[cfg(test)]
pub fn inject(city: &str, w: Weather) {
    if let Ok(mut s) = shared().lock() {
        s.city = city.into();
        s.started = true;
        s.data = Some((city.into(), w));
    }
}

fn run() {
    loop {
        let job = {
            let Ok(s) = shared().lock() else { return };
            let due = s.next.map(|n| Instant::now() >= n).unwrap_or(true);
            (due && !s.city.is_empty()).then(|| (s.city.clone(), s.place.clone()))
        };
        if let Some((city, place)) = job {
            let result = fetch(&city, place);
            if let Ok(mut s) = shared().lock() {
                if s.city == city {
                    match result {
                        Ok((w, place)) => {
                            log::info!("météo {} : {:.1} °C (code {})", w.city, w.temp, w.code);
                            s.data = Some((city, w));
                            s.place = Some(place);
                            s.error = None;
                            s.next = Some(Instant::now() + REFRESH);
                        }
                        Err(e) => {
                            log::warn!("météo : {e}");
                            s.error = Some(e);
                            s.next = Some(Instant::now() + RETRY);
                        }
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(Duration::from_secs(12)).user_agent(concat!("HubForge/", env!("CARGO_PKG_VERSION"))).build()
}

/// Encode un texte pour une adresse web.
fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            b' ' => "+".into(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

type Place = (String, f64, f64, String);

fn fetch(city: &str, place: Option<Place>) -> Result<(Weather, Place), String> {
    let a = agent();
    let place = match place {
        Some(p) if p.0 == city => p,
        _ => {
            let url = format!(
                "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=fr&format=json",
                urlencode(city)
            );
            let v: serde_json::Value =
                a.get(&url).call().map_err(|_| "Pas de connexion".to_string())?.into_json().map_err(|_| "Réponse illisible".to_string())?;
            parse_place(city, &v).ok_or_else(|| format!("Ville « {city} » introuvable"))?
        }
    };
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={:.4}&longitude={:.4}&current=temperature_2m,weather_code,is_day&daily=weather_code,temperature_2m_max,temperature_2m_min&timezone=auto&forecast_days=4",
        place.1, place.2
    );
    let v: serde_json::Value =
        a.get(&url).call().map_err(|_| "Pas de connexion".to_string())?.into_json().map_err(|_| "Réponse illisible".to_string())?;
    let mut w = parse_forecast(&v).ok_or_else(|| "Réponse illisible".to_string())?;
    w.city = place.3.clone();
    Ok((w, place))
}

fn parse_place(city: &str, v: &serde_json::Value) -> Option<Place> {
    let r = v.get("results")?.as_array()?.first()?;
    Some((city.to_string(), r.get("latitude")?.as_f64()?, r.get("longitude")?.as_f64()?, r.get("name")?.as_str()?.to_string()))
}

/// Jour de la semaine d'une date « AAAA-MM-JJ » (0 = dimanche).
fn weekday(date: &str) -> Option<u8> {
    let mut it = date.split('-').map(|x| x.parse::<i64>().ok());
    let (y, m, d) = (it.next()??, it.next()??, it.next()??);
    // Jours depuis 1970 (Howard Hinnant).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some((days + 4).rem_euclid(7) as u8)
}

fn parse_forecast(v: &serde_json::Value) -> Option<Weather> {
    let cur = v.get("current")?;
    let daily = v.get("daily")?;
    let arr = |k: &str| daily.get(k).and_then(|x| x.as_array()).cloned().unwrap_or_default();
    let (time, codes, max, min) = (arr("time"), arr("weather_code"), arr("temperature_2m_max"), arr("temperature_2m_min"));
    let days = time
        .iter()
        .enumerate()
        .filter_map(|(i, t)| {
            Some(Day {
                dow: weekday(t.as_str()?)?,
                code: codes.get(i)?.as_u64()? as u8,
                max: max.get(i)?.as_f64()? as f32,
                min: min.get(i)?.as_f64()? as f32,
            })
        })
        .collect();
    Some(Weather {
        city: String::new(),
        temp: cur.get("temperature_2m")?.as_f64()? as f32,
        code: cur.get("weather_code")?.as_u64()? as u8,
        is_day: cur.get("is_day").and_then(|x| x.as_u64()).unwrap_or(1) == 1,
        days,
    })
}

/// Grandes familles de temps (codes météo WMO).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sky {
    Clear,
    Partly,
    Cloudy,
    Fog,
    Drizzle,
    Rain,
    Snow,
    Storm,
}

pub fn sky(code: u8) -> Sky {
    match code {
        0 => Sky::Clear,
        1 | 2 => Sky::Partly,
        3 => Sky::Cloudy,
        45 | 48 => Sky::Fog,
        51..=57 => Sky::Drizzle,
        61..=67 | 80..=82 => Sky::Rain,
        71..=77 | 85 | 86 => Sky::Snow,
        95..=99 => Sky::Storm,
        _ => Sky::Cloudy,
    }
}

pub fn sky_name(code: u8) -> &'static str {
    match sky(code) {
        Sky::Clear => crate::i18n::t("Ensoleillé", "Sunny"),
        Sky::Partly => crate::i18n::t("Éclaircies", "Partly cloudy"),
        Sky::Cloudy => crate::i18n::t("Couvert", "Cloudy"),
        Sky::Fog => crate::i18n::t("Brouillard", "Fog"),
        Sky::Drizzle => crate::i18n::t("Bruine", "Drizzle"),
        Sky::Rain => crate::i18n::t("Pluie", "Rain"),
        Sky::Snow => crate::i18n::t("Neige", "Snow"),
        Sky::Storm => crate::i18n::t("Orage", "Storm"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_open_meteo() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{"current":{"temperature_2m":5.3,"weather_code":2,"is_day":1},
                "daily":{"time":["2026-11-09","2026-11-10","2026-11-11"],"weather_code":[2,61,3],
                         "temperature_2m_max":[8.1,8.4,9.0],"temperature_2m_min":[2.0,0.5,3.1]}}"#,
        )
        .unwrap();
        let w = parse_forecast(&v).unwrap();
        assert_eq!(w.code, 2);
        assert!(w.is_day);
        assert_eq!(w.days.len(), 3);
        assert_eq!(w.days[0].dow, 1); // 9 novembre 2026 : lundi
        assert_eq!(sky(w.days[1].code), Sky::Rain);
        let g: serde_json::Value =
            serde_json::from_str(r#"{"results":[{"name":"Toulouse","latitude":43.6,"longitude":1.44}]}"#).unwrap();
        assert_eq!(parse_place("toulouse", &g).unwrap().3, "Toulouse");
        assert_eq!(urlencode("Saint-Étienne du"), "Saint-%C3%89tienne+du");
        assert_eq!(weekday("2026-01-01"), Some(4)); // jeudi
    }
}
