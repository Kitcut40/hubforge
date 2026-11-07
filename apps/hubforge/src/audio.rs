//! Contrôle du volume : général, micro et par application.
//! Sous Windows : API Core Audio (WASAPI), dans un fil dédié.

use std::sync::mpsc::{self, Sender};

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
}

#[derive(Clone)]
pub struct Audio {
    tx: Sender<AudioCmd>,
}

impl Audio {
    pub fn start() -> Audio {
        let (tx, rx) = mpsc::channel::<AudioCmd>();
        std::thread::Builder::new()
            .name("audio".into())
            .spawn(move || imp::run(rx))
            .ok();
        Audio { tx }
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
    use super::AudioCmd;
    use std::collections::HashMap;
    use std::sync::mpsc::Receiver;
    use std::time::{Duration, Instant};
    use windows::core::{Interface, PWSTR};
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        eCapture, eConsole, eRender, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator,
        ISimpleAudioVolume, MMDeviceEnumerator,
    };
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    struct State {
        enumerator: IMMDeviceEnumerator,
        master: Option<IAudioEndpointVolume>,
        mic: Option<IAudioEndpointVolume>,
        sessions: HashMap<String, Vec<ISimpleAudioVolume>>,
        refreshed: Instant,
    }

    pub fn run(rx: Receiver<AudioCmd>) {
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
                AudioCmd::SetApps(names, v) => {
                    let mut found = false;
                    for n in names {
                        if let Some(list) = st.sessions.get(&n.to_lowercase()) {
                            for s in list {
                                found |= unsafe { s.SetMasterVolume(*v, std::ptr::null()) }.is_ok();
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
        }
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
                    st.sessions.entry(name).or_default().push(vol);
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
    use super::AudioCmd;
    use std::sync::mpsc::Receiver;

    pub fn run(rx: Receiver<AudioCmd>) {
        log::warn!("contrôle du volume pas encore disponible sur cette plateforme");
        while let Ok(cmd) = rx.recv() {
            if let AudioCmd::ListApps(reply) = cmd {
                let _ = reply.send(Vec::new());
            }
        }
    }
}
