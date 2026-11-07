//! Application au premier plan et liste des applications ouvertes, pour
//! les pages automatiques (une page liée à Fusion 360 s'affiche quand
//! Fusion 360 passe au premier plan).

/// Nom normalisé d'un programme : « C:\\…\\Fusion360.exe » → « fusion360.exe ».
pub fn normalize(name: &str) -> String {
    let n = name.trim().trim_matches('"');
    let n = n.rsplit(['\\', '/']).next().unwrap_or(n).to_lowercase();
    if n.is_empty() || n.ends_with(".exe") {
        n
    } else {
        format!("{n}.exe")
    }
}

/// Programme de la fenêtre au premier plan (ex. « fusion360.exe »).
pub fn foreground_exe() -> Option<String> {
    imp::foreground_exe().map(|s| normalize(&s))
}

/// Programmes qui ont une fenêtre visible, triés, sans HubForge.
pub fn running_apps() -> Vec<String> {
    let mut v: Vec<String> = imp::window_exes()
        .into_iter()
        .map(|s| normalize(&s))
        .filter(|s| !s.is_empty() && !IGNORED.contains(&s.as_str()))
        .collect();
    v.sort();
    v.dedup();
    v
}

/// Programmes à ne jamais proposer (HubForge lui-même, éléments du système).
const IGNORED: &[&str] = &[
    "hubforge.exe",
    "applicationframehost.exe",
    "textinputhost.exe",
    "shellexperiencehost.exe",
    "searchhost.exe",
    "startmenuexperiencehost.exe",
    "systemsettings.exe",
    "lockapp.exe",
];

#[cfg(windows)]
mod imp {
    use windows::core::{BOOL, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetForegroundWindow, GetWindow, GetWindowTextLengthW, GetWindowThreadProcessId,
        IsWindowVisible, GW_OWNER,
    };

    fn exe_of_window(hwnd: HWND) -> Option<String> {
        unsafe {
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 {
                return None;
            }
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = vec![0u16; 1024];
            let mut len = buf.len() as u32;
            let r = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
            let _ = CloseHandle(h);
            r.ok()?;
            Some(String::from_utf16_lossy(&buf[..len as usize]))
        }
    }

    pub fn foreground_exe() -> Option<String> {
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.0.is_null() {
            return None;
        }
        exe_of_window(hwnd)
    }

    pub fn window_exes() -> Vec<String> {
        unsafe extern "system" fn cb(hwnd: HWND, lp: LPARAM) -> BOOL {
            let list = &mut *(lp.0 as *mut Vec<HWND>);
            let visible = IsWindowVisible(hwnd).as_bool();
            let owned = GetWindow(hwnd, GW_OWNER).map(|o| !o.0.is_null()).unwrap_or(false);
            if visible && !owned && GetWindowTextLengthW(hwnd) > 0 {
                list.push(hwnd);
            }
            BOOL(1)
        }
        let mut list: Vec<HWND> = Vec::new();
        unsafe {
            let _ = EnumWindows(Some(cb), LPARAM(&mut list as *mut _ as isize));
        }
        list.into_iter().filter_map(exe_of_window).collect()
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn foreground_exe() -> Option<String> {
        None
    }

    pub fn window_exes() -> Vec<String> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::normalize;

    #[test]
    fn normalizes_names() {
        assert_eq!(normalize("C:\\Program Files\\Autodesk\\Fusion360.exe"), "fusion360.exe");
        assert_eq!(normalize("OBS64"), "obs64.exe");
        assert_eq!(normalize("\"prusa-slicer.exe\""), "prusa-slicer.exe");
        assert_eq!(normalize(""), "");
    }
}
