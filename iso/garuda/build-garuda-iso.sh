#!/usr/bin/env bash
# Build the DemonOS PhantomCore live ISO on the Garuda Linux Dr460nized base.
#
#   iso/garuda/build-garuda-iso.sh   # -> $BUILD/out/demonos-phantomcore-garuda-*.iso
#
# Like iso/build-iso.sh (same Arch build chroot, same bind mounts, same disk
# watcher) but for iso/profile-garuda. Lillith is not recompiled: the exact
# package from the verified DemonOS ISO is repacked and checked against its mtree
# (iso/garuda/repack-lillith-from-iso.sh), so no Git LFS art checkout is needed.
# Stops (and says so) if the Garuda / Chaotic-AUR repos are unreachable.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
BUILD="${DEMONOS_BUILD_DIR:-/workspace/qsec/iso-build}"
CHROOT_SH="${ARCH_CHROOT_SCRIPT:-$REPO/iso/setup-arch-chroot.sh}"
ROOT="${ARCH_ROOT:-/workspace/qsec/arch-root/root.x86_64}"
MODEL="${DEMONOS_MODEL:-/workspace/models/qwen2.5-0.5b-instruct-q4_k_m.gguf}"
ABORT_FREE_MB="${ABORT_FREE_MB:-1500}"
MIN_START_FREE_MB="${MIN_START_FREE_MB:-9000}"
PKGCACHE_TMPFS_MB="${PKGCACHE_TMPFS_MB:-5000}"   # whole package set is ~4.3 GB; 0 = keep the cache on disk
TS="$(date +%Y%m%d-%H%M%S)"
LOG="$BUILD/logs/build-garuda-iso-$TS.log"
mkdir -p "$BUILD/logs" "$BUILD/out"
exec > >(tee -a "$LOG") 2>&1
say(){ printf '[build-garuda-iso %(%F %T)T] %s\n' -1 "$*"; }
free_mb(){ df -Pm "$BUILD" | awk 'NR==2{print $4}'; }
say "repo $REPO @ $(git -C "$REPO" rev-parse --short HEAD 2>/dev/null || echo '?')  build dir $BUILD  log $LOG"

# ---- 1. preflight -------------------------------------------------------------
f=$(free_mb); say "free on $BUILD: ${f} MB"
(( f >= MIN_START_FREE_MB )) || { say "STOP: need >= ${MIN_START_FREE_MB} MB free to start (have ${f}). Nothing deleted."; exit 3; }
for u in https://builds.garudalinux.org/repos/garuda/x86_64/garuda.db \
         https://cdn-mirror.chaotic.cx/chaotic-aur/x86_64/chaotic-aur.db; do
  curl -fsSIL --max-time 30 "$u" >/dev/null || { say "STOP: Garuda/Chaotic-AUR repo unreachable: $u (no fallback, nothing faked)"; exit 4; }
done
say "Garuda + Chaotic-AUR repos reachable"
[[ -f "$MODEL" ]] || { say "STOP: model $MODEL not found"; exit 2; }
ls "$BUILD"/packages/lillith-llama-bin-*.pkg.tar.zst >/dev/null || { say "STOP: no lillith-llama-bin package in $BUILD/packages (run iso/build-iso.sh once)"; exit 2; }
SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-$(git -C "$REPO" log -1 --format=%ct 2>/dev/null || date +%s)}"
export SOURCE_DATE_EPOCH

# ---- 2. stage the profile ---------------------------------------------------------
sudo rm -rf "$BUILD/staging" "$BUILD/work"
mkdir -p "$BUILD/staging"
cp -a "$REPO/iso/profile-garuda" "$BUILD/staging/profile-garuda"
M="$BUILD/staging/profile-garuda/airootfs/usr/share/lillith/models"
mkdir -p "$M"
ln "$MODEL" "$M/$(basename "$MODEL")" 2>/dev/null || cp "$MODEL" "$M/"
say "staged iso/profile-garuda; model $(basename "$MODEL") ($(stat -c %s "$MODEL") bytes)"

# Signal every process rooted in the build chroot or below it (prefix match: the
# pacman that mkarchiso runs inside work/x86_64/airootfs has that as its root).
stop_chroot_procs(){
  local sig=$1 p r
  for p in /proc/[0-9]*; do
    r="$(sudo readlink "$p/root" 2>/dev/null)" || continue
    [[ "$r" == "$ROOT" || "$r" == "$ROOT"/* ]] && sudo kill "-$sig" "${p#/proc/}" 2>/dev/null
  done
  return 0
}

# ---- 3. mounts -------------------------------------------------------------------
"$CHROOT_SH" run true
cleanup(){
  set +e
  [[ -n "${WATCH_PID:-}" ]] && kill "$WATCH_PID" 2>/dev/null
  # Never delete the work dir under a still-running pacstrap/mkarchiso: stop every
  # process whose root is the build chroot OR anything below it (mkarchiso's
  # pacman runs chrooted into work/x86_64/airootfs), then lazily unmount
  # everything below work/ (proc, sys, dev, run binds) before removing it, and
  # keep rm on one filesystem so it can never walk into a bind-mounted /dev or /proc.
  stop_chroot_procs KILL
  local m
  for m in $(findmnt -rn -o TARGET | grep -E "^($ROOT/build/work|$BUILD/work)(/|$)" | sort -r); do
    sudo umount -l "$m" 2>/dev/null
  done
  sudo umount "$ROOT/build/pkgcache" 2>/dev/null
  sudo umount "$ROOT/build/src" 2>/dev/null; sudo umount "$ROOT/build" 2>/dev/null
  if findmnt -rn -o TARGET | grep -qE "^$BUILD/work(/|$)"; then
    say "cleanup: something is still mounted under $BUILD/work; NOT removing it"
  else
    sudo rm -rf --one-file-system "$BUILD/work" "$BUILD/shim" "$BUILD/lillith-repack"
  fi
}
trap cleanup EXIT
sudo mkdir -p "$ROOT/build"
mountpoint -q "$ROOT/build" || sudo mount --bind "$BUILD" "$ROOT/build"
sudo mkdir -p "$ROOT/build/src"
mountpoint -q "$ROOT/build/src" || { sudo mount --bind "$REPO" "$ROOT/build/src" && sudo mount -o remount,bind,ro "$ROOT/build/src"; }
# Package download cache in RAM (tmpfs, ${PKGCACHE_TMPFS_MB} MB cap) instead of on
# the disk: pacman downloads the whole set (~4.3 GB) before it installs ~9.5 GB,
# which ran build 2 into the low-disk stop. Already-downloaded packages are moved in.
if [[ "${PKGCACHE_TMPFS_MB}" != 0 ]] && ! mountpoint -q "$ROOT/build/pkgcache"; then
  # set the on-disk files aside first (mount propagation may show the tmpfs at
  # $BUILD/pkgcache too), then move them into the tmpfs
  sudo rm -rf "$BUILD/pkgcache.disk"
  if [[ -d "$BUILD/pkgcache" ]]; then sudo mv "$BUILD/pkgcache" "$BUILD/pkgcache.disk"; fi
  sudo mkdir -p "$BUILD/pkgcache"
  sudo mount -t tmpfs -o "size=${PKGCACHE_TMPFS_MB}m,mode=755" demonos-pkgcache "$ROOT/build/pkgcache"
  if [[ -d "$BUILD/pkgcache.disk" ]]; then
    sudo find "$BUILD/pkgcache.disk" -maxdepth 1 -name '*.pkg.tar*' -exec mv -t "$ROOT/build/pkgcache/" {} + || true
    sudo rm -rf "$BUILD/pkgcache.disk"
  fi
  say "package cache on tmpfs ($(df -Pm "$ROOT/build/pkgcache" | awk 'NR==2{print $3}') MB in it, cap ${PKGCACHE_TMPFS_MB} MB)"
fi

# ---- disk watcher -------------------------------------------------------------------
(
  low=999999
  while sleep 15; do
    f=$(free_mb); (( f < low )) && { low=$f; echo "$low" > "$BUILD/logs/garuda-min-free-$TS"; }
    if (( f < ABORT_FREE_MB )); then
      echo "[disk-watch] STOP: ${f} MB free < ${ABORT_FREE_MB} MB; stopping the build (nothing else deleted)." | tee -a "$LOG"
      stop_chroot_procs TERM; sleep 5; stop_chroot_procs KILL
      touch "$BUILD/logs/ABORTED-LOW-DISK-$TS"; exit 0
    fi
  done
) & WATCH_PID=$!

# ---- 4. build in the chroot -----------------------------------------------------------
say "running iso/garuda/chroot-build-garuda.sh in the Arch chroot"
"$CHROOT_SH" run env SOURCE_DATE_EPOCH="$SOURCE_DATE_EPOCH" /build/src/iso/garuda/chroot-build-garuda.sh

# ---- 5. result ----------------------------------------------------------------------
ISO="$(ls -t "$BUILD"/out/demonos-phantomcore-garuda-*.iso | head -n1)"
sudo chown "$(id -u):$(id -g)" "$ISO"
( cd "$(dirname "$ISO")" && sha256sum "$(basename "$ISO")" | tee "$(basename "$ISO").sha256" )
say "ISO: $ISO ($(du -h "$ISO" | cut -f1)); lowest free during build: $(cat "$BUILD/logs/garuda-min-free-$TS" 2>/dev/null) MB"
say "free on $BUILD now: $(free_mb) MB"
