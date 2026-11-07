#!/usr/bin/env bash
# Fabrique l'installateur Windows de HubForge (depuis Linux ou Windows avec NSIS).
#   installer/build.sh                → compile HubForge puis l'installateur
#   SKIP_BUILD=1 installer/build.sh   → réutilise le hubforge.exe déjà compilé
# Résultat : dist/HubForge-Setup-<version>.exe (+ .sha256)
set -euo pipefail
cd "$(dirname "$0")/.."
VERSION=$(grep -m1 '^version' apps/hubforge/Cargo.toml | cut -d'"' -f2)
TARGET=x86_64-pc-windows-gnu
if [ "${SKIP_BUILD:-0}" != "1" ]; then
  touch apps/hubforge/src/main.rs
  CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc RUSTC_BOOTSTRAP=1 \
    cargo build --release -p hubforge -Zbuild-std=std,panic_abort --target $TARGET
fi
STAGE=$(mktemp -d)
cp target/$TARGET/release/hubforge.exe "$STAGE/"
DLL=$(find target/$TARGET/release -name WebView2Loader.dll -path '*x64*' | head -1)
[ -n "$DLL" ] || DLL=target/$TARGET/release/WebView2Loader.dll
cp "$DLL" "$STAGE/"
mkdir -p dist
OUT="$PWD/dist/HubForge-Setup-$VERSION.exe"
makensis -V2 -DVERSION="$VERSION" -DSRC="$STAGE" -DOUT="$OUT" installer/hubforge.nsi
( cd dist && sha256sum "HubForge-Setup-$VERSION.exe" > "HubForge-Setup-$VERSION.exe.sha256" )
rm -rf "$STAGE"
echo "✓ $OUT"
cat "dist/HubForge-Setup-$VERSION.exe.sha256"
