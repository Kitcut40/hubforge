//! Contrôle du volume : général, micro et par application.
//! Sous Windows : API Core Audio (WASAPI), dans un fil dédié.

use std::collections::HashMap;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum AudioCmd {
    /// Volume général (0.0 à 1.0)
    SetMaster(f32),
    /// Variation du volume général (ex. +0.02)
    StepMaster(f32),
    /// Volume du micro par défaut (0.0 à 1.0)
    SetMic(f32),
    /// Volume des applications dont le nom d'exécutable est dans la liste
    SetApps(Vec<String>, f32),
    /// Coupe / rétablit le micro
    ToggleMicMute,
    /// Liste des applications qui ont une session audio
    ListApps(Sender<Vec<String>>),
    /// Relit le volume général et l'état des micros (touches vivantes)
    Poll,
    /// Mesure le niveau sonore et l'état (coupé, présent) de ces cibles de
    /// fader ("master", "mic", "app:a.exe,b.exe"). Résultat dans `meters()`.
    Measure(Vec<String>),
}

/// État d'une cible de fader, pour les LED « état » et « VU-mètre ».
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Meter {
    /// La sortie, le micro ou l'application existe (l'appli joue ou a joué du son).
    pub present: bool,
    pub muted: bool,
    /// Niveau crête actuel (0.0 à 1.0).
    pub peak: f32,
}

/// Dernier état connu du son (mis à jour par `AudioCmd::Poll`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct AudioState {
    /// Volume général (0.0 à 1.0), None si pas de sortie audio.
    pub master: Option<f32>,
    pub master_muted: bool,
    /// Micro coupé ? None si pas de micro.
    pub mic_muted: Option<bool>,
}

#[derive(Clone)]
pub struct Audio {
    tx: Sender<AudioCmd>,
    state: Arc<Mutex<AudioState>>,
    meters: Arc<Mutex<HashMap<String, Meter>>>,
}

impl Audio {
    pub fn start() -> Audio {
        let (tx, rx) = mpsc::channel::<AudioCmd>();
        let state = Arc::new(Mutex::new(AudioState::default()));
        let meters = Arc::new(Mutex::new(HashMap::new()));
        let shared = state.clone();
        let shared_meters = meters.clone();
        std::thread::Builder::new()
            .name("audio".into())
            .spawn(move || imp::run(rx, shared, shared_meters))
            .ok();
        Audio { tx, state, meters }
    }

    /// Dernières mesures demandées avec `AudioCmd::Measure`.
    pub fn meter(&self, target: &str) -> Meter {
        self.meters.lock().ok().and_then(|m| m.get(target).copied()).unwrap_or_default()
    }

    /// Dernier état connu (demander une mise à jour avec `AudioCmd::Poll`).
    pub fn state(&self) -> AudioState {
        self.state.lock().map(|s| *s).unwrap_or_default()
    }

    pub fn send(&self, cmd: AudioCmd) {
        let _ = self.tx.send(cmd);
    }

    /// Applications qui jouent (ou ont joué) du son, ex. « discord.exe ».
    pub fn list_apps(&self) -> Vec<String> {
        let (tx, rx) = mpsc::channel();
        self.send(AudioCmd::ListApps(tx));
        rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap_or_default()
    }
}

#[cfg(windows)]
mod imp {
    use super::{AudioCmd, AudioState, Meter};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::sync::mpsc::Receiver;
    use std::time::{Duration, Instant};
    use windows::core::{Interface, PWSTR};
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Media::Audio::Endpoints::{IAudioEndpointVolume, IAudioMeterInformation};
    use windows::Win32::Media::Audio::{
        eCapture, eConsole, eRender, AudioSessionStateExpired, IAudioSessionControl, IAudioSessionControl2,
        IAudioSessionManager2, IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator,
    };
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    struct Session {
        vol: ISimpleAudioVolume,
        ctl: IAudioSessionControl,
        meter: Option<IAudioMeterInformation>,
    }

    struct State {
        enumerator: IMMDeviceEnumerator,
        master: Option<IAudioEndpointVolume>,
        mic: Option<IAudioEndpointVolume>,
        master_meter: Option<IAudioMeterInformation>,
        mic_meter: Option<IAudioMeterInformation>,
        sessions: HashMap<String, Vec<Session>>,
        refreshed: Instant,
    }

    pub fn run(rx: Receiver<AudioCmd>, shared: Arc<Mutex<AudioState>>, meters: Arc<Mutex<HashMap<String, Meter>>>) {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        let enumerator: IMMDeviceEnumerator = match unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) } {
            Ok(e) => e,
            Err(e) => {
                log::error!("audio : initialisation impossible : {e}");
                return;
            }
        };
        let mut st = State {
            enumerator,
            master: None,
            mic: None,
            master_meter: None,
            mic_meter: None,
            sessions: HashMap::new(),
            refreshed: Instant::now() - Duration::from_secs(60),
        };
        while let Ok(cmd) = rx.recv() {
            if st.refreshed.elapsed() > Duration::from_secs(3) || matches!(cmd, AudioCmd::ListApps(_)) {
                refresh(&mut st);
            }
            let res = match &cmd {
                AudioCmd::SetMaster(v) => set_endpoint(&st.master, *v),
                AudioCmd::StepMaster(d) => step_endpoint(&st.master, *d),
                AudioCmd::SetMic(v) => set_endpoint(&st.mic, *v),
                AudioCmd::ToggleMicMute => toggle_mute(&st.mic),
                AudioCmd::ListApps(reply) => {
                    let mut names: Vec<String> = st.sessions.keys().cloned().collect();
                    names.sort();
                    let _ = reply.send(names);
                    Ok(())
                }
                AudioCmd::Poll => Ok(()),
                AudioCmd::Measure(targets) => {
                    let mut out = HashMap::new();
                    for t in targets {
                        out.insert(t.clone(), measure(&st, t));
                    }
                    if let Ok(mut m) = meters.lock() {
                        *m = out;
                    }
                    Ok(())
                }
                AudioCmd::SetApps(names, v) => {
                    let mut found = false;
                    for n in names {
                        if let Some(list) = st.sessions.get(&n.to_lowercase()) {
                            for s in list {
                                found |= unsafe { s.vol.SetMasterVolume(*v, std::ptr::null()) }.is_ok();
                            }
                        }
                    }
                    if !found {
                        // L'appli vient peut-être de démarrer : on rafraîchit au prochain coup.
                        st.refreshed = Instant::now() - Duration::from_secs(60);
                    }
                    Ok(())
                }
            };
            if let Err(e) = res {
                log::debug!("audio : {e}");
                st.refreshed = Instant::now() - Duration::from_secs(60);
            }
            if !matches!(cmd, AudioCmd::ListApps(_) | AudioCmd::SetApps(..) | AudioCmd::Measure(_)) {
                let snap = snapshot(&st);
                if let Ok(mut s) = shared.lock() {
                    *s = snap;
                }
            }
        }
    }

    fn snapshot(st: &State) -> AudioState {
        unsafe {
            let master = st.master.as_ref().and_then(|ep| ep.GetMasterVolumeLevelScalar().ok());
            let master_muted = st.master.as_ref().and_then(|ep| ep.GetMute().ok()).map(|b| b.as_bool()).unwrap_or(false);
            let mic_muted = st.mic.as_ref().and_then(|ep| ep.GetMute().ok()).map(|b| b.as_bool());
            AudioState { master, master_muted, mic_muted }
        }
    }

    fn peak(m: &Option<IAudioMeterInformation>) -> f32 {
        m.as_ref().and_then(|m| unsafe { m.GetPeakValue() }.ok()).unwrap_or(0.0)
    }

    fn endpoint_meter(ep: &Option<IAudioEndpointVolume>, m: &Option<IAudioMeterInformation>) -> Meter {
        match ep {
            Some(ep) => Meter {
                present: true,
                muted: unsafe { ep.GetMute() }.map(|b| b.as_bool()).unwrap_or(false),
                peak: peak(m),
            },
            None => Meter::default(),
        }
    }

    fn measure(st: &State, target: &str) -> Meter {
        let t = target.trim().to_lowercase();
        if t == "master" {
            return endpoint_meter(&st.master, &st.master_meter);
        }
        if t == "mic" {
            return endpoint_meter(&st.mic, &st.mic_meter);
        }
        let Some(apps) = t.strip_prefix("app:") else { return Meter::default() };
        let mut out = Meter { present: false, muted: true, peak: 0.0 };
        for name in apps.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
            for s in st.sessions.get(name).into_iter().flatten() {
                let alive = unsafe { s.ctl.GetState() }.map(|state| state != AudioSessionStateExpired).unwrap_or(false);
                if !alive {
                    continue;
                }
                out.present = true;
                out.muted &= unsafe { s.vol.GetMute() }.map(|b| b.as_bool()).unwrap_or(false);
                out.peak = out.peak.max(peak(&s.meter));
            }
        }
        if !out.present {
            out.muted = false;
        }
        out
    }

    fn set_endpoint(ep: &Option<IAudioEndpointVolume>, v: f32) -> windows::core::Result<()> {
        if let Some(ep) = ep {
            unsafe { ep.SetMasterVolumeLevelScalar(v.clamp(0.0, 1.0), std::ptr::null()) }?;
        }
        Ok(())
    }

    fn toggle_mute(ep: &Option<IAudioEndpointVolume>) -> windows::core::Result<()> {
        if let Some(ep) = ep {
            let muted = unsafe { ep.GetMute() }?.as_bool();
            unsafe { ep.SetMute(!muted, std::ptr::null()) }?;
            log::info!("micro {}", if muted { "réactivé" } else { "coupé" });
        }
        Ok(())
    }

    fn step_endpoint(ep: &Option<IAudioEndpointVolume>, d: f32) -> windows::core::Result<()> {
        if let Some(ep) = ep {
            let cur = unsafe { ep.GetMasterVolumeLevelScalar() }?;
            unsafe { ep.SetMasterVolumeLevelScalar((cur + d).clamp(0.0, 1.0), std::ptr::null()) }?;
        }
        Ok(())
    }

    fn refresh(st: &mut State) {
        st.refreshed = Instant::now();
        unsafe {
            st.master = st
                .enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .and_then(|d| d.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None))
                .ok();
            st.mic = st
                .enumerator
                .GetDefaultAudioEndpoint(eCapture, eConsole)
                .and_then(|d| d.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None))
                .ok();
            st.master_meter = st
                .enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .and_then(|d| d.Activate::<IAudioMeterInformation>(CLSCTX_ALL, None))
                .ok();
            st.mic_meter = st
                .enumerator
                .GetDefaultAudioEndpoint(eCapture, eConsole)
                .and_then(|d| d.Activate::<IAudioMeterInformation>(CLSCTX_ALL, None))
                .ok();
            st.sessions.clear();
            let Ok(dev) = st.enumerator.GetDefaultAudioEndpoint(eRender, eConsole) else { return };
            let Ok(mgr) = dev.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else { return };
            let Ok(list) = mgr.GetSessionEnumerator() else { return };
            let count = list.GetCount().unwrap_or(0);
            for i in 0..count {
                let Ok(ctl) = list.GetSession(i) else { continue };
                let Ok(ctl2) = ctl.cast::<IAudioSessionControl2>() else { continue };
                let Ok(pid) = ctl2.GetProcessId() else { continue };
                let Some(name) = process_name(pid) else { continue };
                if let Ok(vol) = ctl.cast::<ISimpleAudioVolume>() {
                    let meter = ctl.cast::<IAudioMeterInformation>().ok();
                    st.sessions.entry(name).or_default().push(Session { vol, ctl: ctl.clone(), meter });
                }
            }
        }
    }

    fn process_name(pid: u32) -> Option<String> {
        if pid == 0 {
            return None;
        }
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
            let _ = CloseHandle(h);
            if !ok {
                return None;
            }
            let full = String::from_utf16_lossy(&buf[..len as usize]);
            full.rsplit(['\\', '/']).next().map(|s| s.to_lowercase())
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::{AudioCmd, AudioState, Meter};
    use std::collections::HashMap;
    use std::sync::mpsc::Receiver;
    use std::sync::{Arc, Mutex};

    pub fn run(rx: Receiver<AudioCmd>, _shared: Arc<Mutex<AudioState>>, _meters: Arc<Mutex<HashMap<String, Meter>>>) {
        log::warn!("contrôle du volume pas encore disponible sur cette plateforme");
        while let Ok(cmd) = rx.recv() {
            if let AudioCmd::ListApps(reply) = cmd {
                let _ = reply.send(Vec::new());
            }
        }
    }
}
