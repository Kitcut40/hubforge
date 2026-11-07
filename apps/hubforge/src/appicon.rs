//! Récupère l'icône et le nom d'une application (ou d'un dossier / fichier).
//! L'icône est enregistrée en PNG dans le dossier `icons/` de la configuration.

use std::path::Path;

/// Retourne le chemin relatif de l'icône enregistrée (ex. "icons/app-notepad.png").
pub fn extract_to(target: &str, cfg_dir: &Path) -> Result<String, String> {
    let t = target.trim();
    if t.is_empty() || is_url(t) {
        return Err("pas d'icône pour une adresse web".into());
    }
    let rgba = imp::icon_rgba(t)?;
    let img = trim(rgba);
    let stem: String = Path::new(t)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "appli".into())
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let dir = cfg_dir.join("icons");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let name = format!("app-{}.png", stem.to_lowercase());
    img.save(dir.join(&name)).map_err(|e| e.to_string())?;
    Ok(format!("icons/{name}"))
}

/// Nom lisible d'une application, d'un raccourci, d'un dossier ou d'un fichier.
/// Pour un programme : la description de l'exe (ex. « Bloc-notes »,
/// « Google Chrome »), sinon son nom de fichier.
pub fn app_name(target: &str) -> Option<String> {
    let t = target.trim();
    if t.is_empty() || is_url(t) {
        return None;
    }
    let name = imp::display_name(t).unwrap_or_else(|| fallback_name(t));
    let name = name.trim().to_string();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn is_url(t: &str) -> bool {
    let l = t.to_lowercase();
    l.starts_with("http://") || l.starts_with("https://")
}

/// « C:\\…\\fusion360.exe » → « Fusion360 » ; « D:\\Photos » → « Photos ».
fn fallback_name(t: &str) -> String {
    // Découpe à la main : « \\ » n'est pas un séparateur sous Linux.
    let last = t.trim_end_matches(['\\', '/']).rsplit(['\\', '/']).next().unwrap_or(t).to_string();
    let lower = last.to_lowercase();
    let s = match lower.rfind('.') {
        Some(i) if matches!(&lower[i + 1..], "exe" | "lnk" | "bat" | "cmd" | "msc") => last[..i].to_string(),
        _ => last,
    };
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => s,
    }
}

/// Retire les bords transparents (les petites icônes sont souvent placées
/// dans le coin d'une grande image vide).
fn trim(img: image::RgbaImage) -> image::RgbaImage {
    let (w, h) = img.dimensions();
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for (x, y, p) in img.enumerate_pixels() {
        if p.0[3] > 8 {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    if x1 < x0 || y1 < y0 {
        return img;
    }
    image::imageops::crop_imm(&img, x0, y0, x1 - x0 + 1, y1 - y0 + 1).to_image()
}

#[cfg(windows)]
mod imp {
    use windows::core::{Interface, PCWSTR};
    use windows::Win32::Graphics::Gdi::{
        DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
        DIB_RGB_COLORS, HGDIOBJ,
    };
    use windows::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, SearchPathW, VerQueryValueW, FILE_FLAGS_AND_ATTRIBUTES,
    };
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
    use windows::Win32::System::Environment::ExpandEnvironmentStringsW;
    use windows::Win32::UI::Controls::{IImageList, ILD_TRANSPARENT};
    use windows::Win32::UI::Shell::{SHGetFileInfoW, SHGetImageList, SHFILEINFOW, SHGFI_SYSICONINDEX, SHIL_JUMBO};
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Résout « notepad.exe », « %WINDIR%\\… » ou un chemin complet.
    fn resolve(target: &str) -> String {
        let w = wide(target);
        let mut buf = vec![0u16; 1024];
        let n = unsafe { ExpandEnvironmentStringsW(PCWSTR(w.as_ptr()), Some(&mut buf)) } as usize;
        let expanded = if n > 0 { String::from_utf16_lossy(&buf[..n - 1]) } else { target.to_string() };
        if std::path::Path::new(&expanded).exists() {
            return expanded;
        }
        let w = wide(&expanded);
        let mut out = vec![0u16; 1024];
        let n = unsafe { SearchPathW(PCWSTR::null(), PCWSTR(w.as_ptr()), PCWSTR::null(), Some(&mut out), None) } as usize;
        if n > 0 && n < out.len() {
            String::from_utf16_lossy(&out[..n])
        } else {
            expanded
        }
    }

    /// Nom à afficher : description de l'exe, sinon nom du produit,
    /// sinon nom du raccourci / dossier / fichier.
    pub fn display_name(target: &str) -> Option<String> {
        let path = resolve(target);
        let p = std::path::Path::new(&path);
        let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if p.is_dir() || ext == "lnk" || !p.exists() {
            return None; // le nom du dossier / raccourci convient
        }
        if ext != "exe" {
            return None;
        }
        version_string(&path, "FileDescription")
            .filter(|s| !s.trim().is_empty() && !s.to_lowercase().ends_with(".exe"))
            .or_else(|| version_string(&path, "ProductName"))
            .filter(|s| !s.trim().is_empty())
    }

    fn version_string(path: &str, key: &str) -> Option<String> {
        unsafe {
            let w = wide(path);
            let size = GetFileVersionInfoSizeW(PCWSTR(w.as_ptr()), None);
            if size == 0 {
                return None;
            }
            let mut data = vec![0u8; size as usize];
            GetFileVersionInfoW(PCWSTR(w.as_ptr()), None, size, data.as_mut_ptr() as *mut _).ok()?;

            // Langue et page de code déclarées par le fichier (la première).
            let mut ptr: *mut core::ffi::c_void = std::ptr::null_mut();
            let mut len: u32 = 0;
            let q = wide("\\VarFileInfo\\Translation");
            let mut candidates: Vec<String> = Vec::new();
            if VerQueryValueW(data.as_ptr() as *const _, PCWSTR(q.as_ptr()), &mut ptr, &mut len).as_bool() && len >= 4 {
                let pairs = std::slice::from_raw_parts(ptr as *const u16, (len / 2) as usize);
                for c in pairs.chunks_exact(2) {
                    candidates.push(format!("{:04x}{:04x}", c[0], c[1]));
                }
            }
            // Valeurs courantes si la table de traduction est absente ou fausse.
            candidates.extend(["040c04b0", "040904b0", "040904e4", "000004b0"].iter().map(|s| s.to_string()));

            for lang in candidates {
                let q = wide(&format!("\\StringFileInfo\\{lang}\\{key}"));
                let mut p: *mut core::ffi::c_void = std::ptr::null_mut();
                let mut n: u32 = 0;
                if VerQueryValueW(data.as_ptr() as *const _, PCWSTR(q.as_ptr()), &mut p, &mut n).as_bool() && n > 1 {
                    let s = std::slice::from_raw_parts(p as *const u16, n as usize);
                    let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
                    let v = String::from_utf16_lossy(&s[..end]).trim().to_string();
                    if !v.is_empty() {
                        return Some(v);
                    }
                }
            }
            None
        }
    }

    pub fn icon_rgba(target: &str) -> Result<image::RgbaImage, String> {
        let path = resolve(target);
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let w = wide(&path);
            let mut info = SHFILEINFOW::default();
            let r = SHGetFileInfoW(
                PCWSTR(w.as_ptr()),
                FILE_FLAGS_AND_ATTRIBUTES(0),
                Some(&mut info),
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_SYSICONINDEX,
            );
            if r == 0 {
                return Err(format!("introuvable : {path}"));
            }
            let list: IImageList = SHGetImageList(SHIL_JUMBO as i32).map_err(|e| e.to_string())?;
            let hicon = list.GetIcon(info.iIcon, ILD_TRANSPARENT.0).map_err(|e| e.to_string())?;
            let mut ii = ICONINFO::default();
            GetIconInfo(hicon, &mut ii).map_err(|e| e.to_string())?;
            let mut bm = BITMAP::default();
            GetObjectW(
                HGDIOBJ(ii.hbmColor.0),
                std::mem::size_of::<BITMAP>() as i32,
                Some(&mut bm as *mut _ as *mut _),
            );
            let (w, h) = (bm.bmWidth.max(1) as u32, bm.bmHeight.max(1) as u32);
            let mut bmi = BITMAPINFO::default();
            bmi.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w as i32,
                biHeight: -(h as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            };
            let mut px = vec![0u8; (w * h * 4) as usize];
            let dc = GetDC(None);
            let lines = GetDIBits(dc, ii.hbmColor, 0, h, Some(px.as_mut_ptr() as *mut _), &mut bmi, DIB_RGB_COLORS);
            ReleaseDC(None, dc);
            let _ = DeleteObject(HGDIOBJ(ii.hbmColor.0));
            let _ = DeleteObject(HGDIOBJ(ii.hbmMask.0));
            let _ = DestroyIcon(hicon);
            let _ = list.cast::<IImageList>();
            if lines == 0 {
                return Err("lecture de l'icône impossible".into());
            }
            // BGRA → RGBA ; anciennes icônes sans transparence : alpha opaque.
            let no_alpha = px.chunks_exact(4).all(|p| p[3] == 0);
            for p in px.chunks_exact_mut(4) {
                p.swap(0, 2);
                if no_alpha {
                    p[3] = 255;
                }
            }
            image::RgbaImage::from_raw(w, h, px).ok_or_else(|| "image invalide".into())
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn icon_rgba(_target: &str) -> Result<image::RgbaImage, String> {
        Err("récupération d'icône disponible uniquement sous Windows".into())
    }

    pub fn display_name(_target: &str) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_names() {
        assert_eq!(fallback_name("C:\\Program Files\\Autodesk\\fusion360.exe"), "Fusion360");
        assert_eq!(fallback_name("D:\\Photos\\"), "Photos");
        assert_eq!(fallback_name("C:\\Users\\benja\\Desktop\\OBS Studio.lnk"), "OBS Studio");
        assert_eq!(fallback_name("notes.txt"), "Notes.txt");
        assert_eq!(app_name("https://github.com"), None);
    }
}
