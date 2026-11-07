//! Récupère l'icône d'une application (ou d'un dossier / fichier) et
//! l'enregistre en PNG dans le dossier `icons/` de la configuration.

use std::path::Path;

/// Retourne le chemin relatif de l'icône enregistrée (ex. "icons/app-notepad.png").
pub fn extract_to(target: &str, cfg_dir: &Path) -> Result<String, String> {
    let t = target.trim();
    if t.is_empty() || t.starts_with("http://") || t.starts_with("https://") {
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
    use windows::Win32::Storage::FileSystem::{SearchPathW, FILE_FLAGS_AND_ATTRIBUTES};
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
}
