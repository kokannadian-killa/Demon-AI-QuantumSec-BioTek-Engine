#!/usr/bin/env bash
# Runs INSIDE the Arch build chroot (as root), started by iso/garuda/build-garuda-iso.sh.
#   /build      = host build dir (/workspace/qsec/iso-build), read-write
#   /build/src  = this checkout, read-only
# Builds the DemonOS PhantomCore ISO on the Garuda Linux Dr460nized base from
# iso/profile-garuda (Garuda's [garuda] + [chaotic-aur] repos, Dr460nized packages,
# linux-garuda kernel) with mkarchiso.
#   1. check the Chaotic-AUR/Garuda signing key + mirrorlist in the build chroot
#   2. local repo [demonos-local]: lillith (repacked from the verified ISO, mtree-
#      checked) + lillith-llama-bin (the verified build's package)
#   3. DemonOS theme art (iso/theme/make-theme-assets.py) + Garuda-override assets
#   4. mkarchiso, with downloaded packages deleted as soon as they are installed and
#      the airootfs streamed into the squashfs (disk on this box is tight)
set -euo pipefail
SRC=/build/src
STAGE=/build/staging/profile-garuda
REPO=/build/repo
PKGCACHE=/build/pkgcache
VERIFIED_ISO=${VERIFIED_ISO:-/build/out/demonos-phantomcore-2026.10.30-x86_64.iso}
CHAOTIC_KEY=EF925EA60F33D0CB85C44AD13056513887B78AEB
export SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH:-$(date +%s)}
log(){ printf '\n[chroot-build-garuda %(%F %T)T] %s\n' -1 "$*"; }
free_mb(){ df -Pm /build | awk 'NR==2{print $4}'; }

log "1/4 Garuda / Chaotic-AUR trust in the build chroot"
pacman-key --list-keys "$CHAOTIC_KEY" | grep -q "$CHAOTIC_KEY" || { log "STOP: Chaotic-AUR key $CHAOTIC_KEY not in the build keyring"; exit 2; }
pacman -Q chaotic-keyring chaotic-mirrorlist
[[ -s /etc/pacman.d/chaotic-mirrorlist ]] || { log "STOP: /etc/pacman.d/chaotic-mirrorlist missing"; exit 2; }
pacman -Q archiso squashfs-tools python-pillow grub >/dev/null

log "2/4 [demonos-local] repo"
mkdir -p "$REPO"
id builder &>/dev/null || useradd -m -U builder
if ! ls "$REPO"/lillith-0.0.1-1-*.pkg.tar.zst >/dev/null 2>&1; then
  bash "$SRC/iso/garuda/repack-lillith-from-iso.sh" "$VERIFIED_ISO" "$REPO"
fi
cp -f /build/packages/lillith-llama-bin-*.pkg.tar.zst "$REPO/"
rm -f "$REPO"/demonos-local.*
repo-add "$REPO/demonos-local.db.tar.gz" "$REPO"/*.pkg.tar.zst
ls -la "$REPO"

log "3/4 theme art + DemonOS overrides of the Garuda branding"
T=/build/theme-out; rm -rf "$T"
python3 "$SRC/iso/theme/make-theme-assets.py" "$SRC" "$T"
G="$STAGE/grub/themes/demonos"
cp "$T/grub/background.png" "$T"/grub/select_*.png "$G/"
FONT="$SRC/crates/lillith-theme/fonts/JetBrainsMono.ttf"
for s in 16 24; do
  grub-mkfont -s "$s" -n "DemonOS Mono Regular $s" -o "$G/demonos-mono-$s.pf2" "$FONT"
  grep -aq "DemonOS Mono Regular $s" "$G/demonos-mono-$s.pf2" || { log "font name mismatch for size $s"; exit 4; }
done
A="$STAGE/airootfs"
cp "$T/syslinux/splash.png" "$STAGE/syslinux/splash.png"
cp "$T"/plymouth/*.png "$A/usr/share/plymouth/themes/demonos/"
cp "$T/sddm/background.png" "$A/usr/share/sddm/themes/demonos/background.png"
cp -r "$T/wallpaper/DemonOS/contents" "$A/usr/share/wallpapers/DemonOS/"
# GRUB theme for an installed system (/etc/default/grub GRUB_THEME, set by demonos-rebrand)
install -d "$A/usr/share/grub/themes"; cp -a "$G" "$A/usr/share/grub/themes/demonos"
# logos: Plasma start button, fastfetch, os-release LOGO=demonos (replace Garuda's)
L="$STAGE/airootfs/usr/share/plasma/look-and-feel/org.demonos.phantomcore/contents"
install -d "$A/usr/share/icons/demonos" "$A/usr/share/icons/hicolor/256x256/apps" "$L/splash/images" "$L/previews"
python3 - "$SRC" "$A" "$L" "$T" <<'PY'
import sys
from PIL import Image
src, a, l, t = sys.argv[1:5]
em = Image.open(f"{src}/branding/icons/demonos-emblem/icon-1024.png").convert("RGBA")
for name, size in (("demonos-start", 256), ("demonos-logo", 512), ("demonos-fastfetch", 512)):
    em.resize((size, size), Image.LANCZOS).save(f"{a}/usr/share/icons/demonos/{name}.png")
em.resize((256, 256), Image.LANCZOS).save(f"{a}/usr/share/icons/hicolor/256x256/apps/demonos.png")
Image.open(f"{t}/plymouth/logo.png").save(f"{l}/splash/images/logo.png")
Image.open(f"{t}/plymouth/background.png").save(f"{l}/splash/images/background.png")
w = Image.open(f"{t}/wallpaper/DemonOS/contents/images/1920x1080.png").convert("RGB")
w.resize((640, 360), Image.LANCZOS).save(f"{l}/previews/preview.png")
w.resize((1920, 1080)).save(f"{l}/previews/fullscreenpreview.jpg", quality=90)
PY
ls -la "$A/usr/share/icons/demonos" "$L/splash/images"

log "4/4 mkarchiso ($(free_mb) MB free)"
rm -rf --one-file-system /build/work
SHIM=/build/shim; rm -rf "$SHIM"; install -d "$SHIM"
install -m 755 "$SRC/iso/garuda/mksquashfs-stream.sh" "$SHIM/mksquashfs"
export DEMONOS_SQFS_STREAM=1 PATH="$SHIM:$PATH"
mkarchiso -v -r -w /build/work -o /build/out "$STAGE" &
mk_pid=$!
# Free the download cache while pacstrap runs: a package file can go once its
# local-db entry exists in the airootfs (pacman writes it after extracting).
( LDB=/build/work/x86_64/airootfs/var/lib/pacman/local; n=0
  # One pass per cache file (not one glob per installed package, which fell
  # minutes behind pacman in build 2): a file name-ver-rel-arch.pkg.tar.zst can go
  # once local/name-ver-rel/files exists in the airootfs.
  while kill -0 "$mk_pid" 2>/dev/null; do
    if [[ -d "$LDB" ]]; then
      for f in "$PKGCACHE"/*.pkg.tar.zst "$REPO"/*.pkg.tar.zst; do
        [[ -e "$f" ]] || continue
        b=${f##*/}; nv=${b%-*.pkg.tar.zst}
        [[ -f "$LDB/$nv/files" ]] || continue
        rm -f "$f" "$f.sig"; n=$((n+1))
      done
    fi
    if pgrep -x sqfstar >/dev/null || pgrep -x mksquashfs >/dev/null; then
      rm -f "$PKGCACHE"/*.pkg.tar.* "$REPO"/*.pkg.tar.* 2>/dev/null
      echo "[cache-watch] squashfs started; package cache emptied ($n removed during pacstrap); $(free_mb) MB free"; break
    fi
    sleep 3
  done ) &
rc=0; wait "$mk_pid" || rc=$?
for m in $(findmnt -rn -o TARGET | grep -E "^/build/work(/|$)" | sort -r); do umount -l "$m"; done
rm -rf --one-file-system /build/work "$SHIM"
log "mkarchiso exit $rc; $(free_mb) MB free"
ls -la /build/out
exit $rc
