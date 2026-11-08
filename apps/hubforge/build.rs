fn main() {
    build_doom();
    tauri_build::build()
}

/// Compile le moteur de Doom (doomgeneric, GPL-2.0-or-later) pour l'easter egg.
fn build_doom() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/doomgeneric");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .expect("third_party/doomgeneric manquant")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|x| x == "c").unwrap_or(false))
        .collect();
    files.sort();
    let mut b = cc::Build::new();
    b.files(&files)
        .include(&dir)
        .define("DOOMGENERIC_RESX", "320")
        .define("DOOMGENERIC_RESY", "200")
        .define("_CRT_SECURE_NO_WARNINGS", None)
        .warnings(false)
        .opt_level(2);
    if b.get_compiler().is_like_msvc() {
        b.flag("/wd4996");
    } else {
        b.flag("-w").flag("-fno-strict-aliasing").flag("-std=gnu99");
    }
    b.compile("doomgeneric");
}
