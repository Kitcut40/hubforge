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
    /// Variation du volume du micro
    StepMic(f32),
    /// Variation du volume d'applications
    StepApps(Vec<String>, f32),
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
    /// Couper / rétablir le son d'applications.
    ToggleAppMute(Vec<String>),
    /// Liste des sorties audio actives (noms affichés par Windows).
    ListOutputs(Sender<Vec<String>>),
    /// Change la sortie audio par défaut. Une sortie : celle-ci. Plusieurs :
    /// la suivante de la liste. Aucune : la suivante parmi toutes.
    SetOutput(Vec<String>),
}

/// État d'une cible de fader, pour les LED « état » et « VU-mètre ».
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Meter {
    /// La sortie, le micro ou l'application existe (l'appli joue ou a joué du son).
    pub present: bool,
    pub muted: bool,
    /// Niveau crête actuel (0.0 à 1.0).
    pub peak: f32,
    /// Réglage de volume actuel (0.0 à 1.0), pour la reprise douce des faders.
    pub level: Option<f32>,
}

/// Dernier état connu du son (mis à jour par `AudioCmd::Poll`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AudioState {
    /// Volume général (0.0 à 1.0), None si pas de sortie audio.
    pub master: Option<f32>,
    pub master_muted: bool,
    /// Micro coupé ? None si pas de micro.
    pub mic_muted: Option<bool>,
    /// Nom de la sortie audio par défaut (ex. « Casque (Arctis 7) »).
    pub output: Option<String>,
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

    /// Comme `meter`, mais None si cette cible n'a pas encore été mesurée.
    pub fn meter_opt(&self, target: &str) -> Option<Meter> {
        self.meters.lock().ok().and_then(|m| m.get(target).copied())
    }

    /// Dernier état connu (demander une mise à jour avec `AudioCmd::Poll`).
    pub fn state(&self) -> AudioState {
        self.state.lock().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn send(&self, cmd: AudioCmd) {
        let _ = self.tx.send(cmd);
    }

    /// Sorties audio actives.
    pub fn list_outputs(&self) -> Vec<String> {
        let (tx, rx) = mpsc::channel();
        self.send(AudioCmd::ListOutputs(tx));
        rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap_or_default()
    }

    /// Applications qui jouent (ou ont joué) du son, ex. « discord.exe ».
    pub fn list_apps(&self) -> Vec<String> {
        let (tx, rx) = mpsc::channel();
        self.send(AudioCmd::ListApps(tx));
        rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap_or_default()
    }
}

#[cfg(windows)]
#[allow(non_snake_case)]
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
        IAudioSessionManager2, IMMDevice, IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
    };
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ};
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
        /// Nom de la sortie par défaut.
        output: Option<String>,
    }

    // --- Changer la sortie par défaut ---------------------------------
    // Windows n'a pas d'API publique pour ça : comme les utilitaires connus
    // (SoundSwitch, EarTrumpet…), on passe par l'interface IPolicyConfig.

    #[windows::core::interface("f8679f50-850a-41cf-9c72-430f290290c8")]
    unsafe trait IPolicyConfig: windows::core::IUnknown {
        fn GetMixFormat(&self, id: windows::core::PCWSTR, fmt: *mut *mut core::ffi::c_void) -> windows::core::HRESULT;
        fn GetDeviceFormat(&self, id: windows::core::PCWSTR, def: i32, fmt: *mut *mut core::ffi::c_void) -> windows::core::HRESULT;
        fn ResetDeviceFormat(&self, id: windows::core::PCWSTR) -> windows::core::HRESULT;
        fn SetDeviceFormat(&self, id: windows::core::PCWSTR, a: *mut core::ffi::c_void, b: *mut core::ffi::c_void) -> windows::core::HRESULT;
        fn GetProcessingPeriod(&self, id: windows::core::PCWSTR, def: i32, a: *mut i64, b: *mut i64) -> windows::core::HRESULT;
        fn SetProcessingPeriod(&self, id: windows::core::PCWSTR, a: *mut i64) -> windows::core::HRESULT;
        fn GetShareMode(&self, id: windows::core::PCWSTR, a: *mut core::ffi::c_void) -> windows::core::HRESULT;
        fn SetShareMode(&self, id: windows::core::PCWSTR, a: *mut core::ffi::c_void) -> windows::core::HRESULT;
        fn GetPropertyValue(&self, id: windows::core::PCWSTR, k: *const core::ffi::c_void, v: *mut core::ffi::c_void) -> windows::core::HRESULT;
        fn SetPropertyValue(&self, id: windows::core::PCWSTR, k: *const core::ffi::c_void, v: *mut core::ffi::c_void) -> windows::core::HRESULT;
        fn SetDefaultEndpoint(&self, id: windows::core::PCWSTR, role: i32) -> windows::core::HRESULT;
        fn SetEndpointVisibility(&self, id: windows::core::PCWSTR, visible: i32) -> windows::core::HRESULT;
    }

    const CLSID_POLICY_CONFIG: windows::core::GUID = windows::core::GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);

    fn device_id(dev: &IMMDevice) -> Option<String> {
        unsafe {
            let p = dev.GetId().ok()?;
            let s = p.to_string().ok();
            CoTaskMemFree(Some(p.0 as *const core::ffi::c_void));
            s
        }
    }

    fn device_name(dev: &IMMDevice) -> Option<String> {
        use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
        use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
        unsafe {
            let store = dev.OpenPropertyStore(STGM_READ).ok()?;
            let v = store.GetValue(&PKEY_Device_FriendlyName).ok()?;
            let p = PropVariantToStringAlloc(&v).ok()?;
            let s = p.to_string().ok();
            CoTaskMemFree(Some(p.0 as *const core::ffi::c_void));
            s
        }
    }

    /// Sorties actives : (identifiant, nom).
    fn outputs(st: &State) -> Vec<(String, String)> {
        let mut out = Vec::new();
        unsafe {
            let Ok(coll) = st.enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) else { return out };
            for i in 0..coll.GetCount().unwrap_or(0) {
                let Ok(dev) = coll.Item(i) else { continue };
                if let (Some(id), Some(name)) = (device_id(&dev), device_name(&dev)) {
                    out.push((id, name));
                }
            }
        }
        out
    }

    fn set_output(st: &mut State, wanted: &[String]) -> windows::core::Result<()> {
        let all = outputs(st);
        let current = unsafe { st.enumerator.GetDefaultAudioEndpoint(eRender, eConsole) }.ok().and_then(|d| device_id(&d));
        let candidates: Vec<&(String, String)> = if wanted.is_empty() {
            all.iter().collect()
        } else {
            // Dans l'ordre de la liste de l'utilisateur.
            wanted
                .iter()
                .filter_map(|w| {
                    let w = w.trim().to_lowercase();
                    all.iter().find(|(_, n)| n.to_lowercase() == w).or_else(|| all.iter().find(|(_, n)| n.to_lowercase().contains(&w)))
                })
                .collect()
        };
        let Some(first) = candidates.first() else {
            log::warn!("sortie audio introuvable : {wanted:?}");
            return Ok(());
        };
        let target = match candidates.iter().position(|(id, _)| Some(id) == current.as_ref()) {
            Some(i) if candidates.len() > 1 => candidates[(i + 1) % candidates.len()],
            _ => first,
        };
        log::info!("sortie audio : {}", target.1);
        let wide: Vec<u16> = target.0.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            let pc: IPolicyConfig = CoCreateInstance(&CLSID_POLICY_CONFIG, None, CLSCTX_ALL)?;
            for role in 0..3 {
                pc.SetDefaultEndpoint(windows::core::PCWSTR(wide.as_ptr()), role).ok()?;
            }
        }
        refresh(st);
        Ok(())
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
            output: None,
        };
        let mut stamps: HashMap<String, Instant> = HashMap::new();
        while let Ok(cmd) = rx.recv() {
            if st.refreshed.elapsed() > Duration::from_secs(3) || matches!(cmd, AudioCmd::ListApps(_)) {
                refresh(&mut st);
            }
            let res = match &cmd {
                AudioCmd::SetMaster(v) => set_endpoint(&st.master, *v),
                AudioCmd::StepMaster(d) => step_endpoint(&st.master, *d),
                AudioCmd::SetMic(v) => set_endpoint(&st.mic, *v),
                AudioCmd::StepMic(d) => step_endpoint(&st.mic, *d),
                AudioCmd::StepApps(names, d) => {
                    let mut found = false;
                    for n in names {
                        for s in st.sessions.get(&n.to_lowercase()).into_iter().flatten() {
                            if let Ok(cur) = unsafe { s.vol.GetMasterVolume() } {
                                found |= unsafe { s.vol.SetMasterVolume((cur + d).clamp(0.0, 1.0), std::ptr::null()) }.is_ok();
                            }
                        }
                    }
                    if !found {
                        st.refreshed = Instant::now() - Duration::from_secs(60);
                    }
                    Ok(())
                }
                AudioCmd::ToggleMicMute => toggle_mute(&st.mic),
                AudioCmd::ListApps(reply) => {
                    let mut names: Vec<String> = st.sessions.keys().cloned().collect();
                    names.sort();
                    let _ = reply.send(names);
                    Ok(())
                }
                AudioCmd::Poll => Ok(()),
                AudioCmd::ListOutputs(reply) => {
                    let _ = reply.send(outputs(&st).into_iter().map(|(_, n)| n).collect());
                    Ok(())
                }
                AudioCmd::SetOutput(list) => set_output(&mut st, list),
                AudioCmd::Measure(targets) => {
                    // Plusieurs demandeurs (LED des faders, touches vivantes) : on
                    // garde chaque cible tant qu'elle est redemandée.
                    let now = Instant::now();
                    if let Ok(mut m) = meters.lock() {
                        for t in targets {
                            m.insert(t.clone(), measure(&st, t));
                            stamps.insert(t.clone(), now);
                        }
                        stamps.retain(|_, at| now.duration_since(*at) < Duration::from_secs(3));
                        m.retain(|k, _| stamps.contains_key(k));
                    }
                    Ok(())
                }
                AudioCmd::ToggleAppMute(names) => {
                    let sessions: Vec<&Session> = names.iter().flat_map(|n| st.sessions.get(&n.to_lowercase()).into_iter().flatten()).collect();
                    // Si une seule session a le son, on coupe tout ; sinon on rétablit tout.
                    let any_on = sessions.iter().any(|s| unsafe { s.vol.GetMute() }.map(|b| !b.as_bool()).unwrap_or(false));
                    for s in &sessions {
                        let _ = unsafe { s.vol.SetMute(any_on, std::ptr::null()) };
                    }
                    if sessions.is_empty() {
                        st.refreshed = Instant::now() - Duration::from_secs(60);
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
            if !matches!(cmd, AudioCmd::ListApps(_) | AudioCmd::SetApps(..) | AudioCmd::StepApps(..) | AudioCmd::Measure(_) | AudioCmd::ToggleAppMute(_) | AudioCmd::ListOutputs(_)) {
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
            AudioState { master, master_muted, mic_muted, output: st.output.clone() }
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
                level: unsafe { ep.GetMasterVolumeLevelScalar() }.ok(),
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
        let mut out = Meter { present: false, muted: true, peak: 0.0, level: None };
        for name in apps.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
            for s in st.sessions.get(name).into_iter().flatten() {
                let alive = unsafe { s.ctl.GetState() }.map(|state| state != AudioSessionStateExpired).unwrap_or(false);
                if !alive {
                    continue;
                }
                out.present = true;
                out.muted &= unsafe { s.vol.GetMute() }.map(|b| b.as_bool()).unwrap_or(false);
                out.peak = out.peak.max(peak(&s.meter));
                if out.level.is_none() {
                    out.level = unsafe { s.vol.GetMasterVolume() }.ok();
                }
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
        st.output = unsafe { st.enumerator.GetDefaultAudioEndpoint(eRender, eConsole) }.ok().and_then(|d| device_name(&d));
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
            match cmd {
                AudioCmd::ListApps(reply) | AudioCmd::ListOutputs(reply) => {
                    let _ = reply.send(Vec::new());
                }
                _ => {}
            }
        }
    }
}
