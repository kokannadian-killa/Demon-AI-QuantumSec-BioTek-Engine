#!/usr/bin/env bash
# Runs INSIDE the Arch build chroot (as root). Optional fast path for the Garuda build:
# rebuild the exact `lillith` pacman package that is inside an already-verified
# DemonOS ISO (same binaries + art that passed the guest checks), instead of a
# fresh cargo build (~6 GB of build trees). Lillith's sources are unchanged
# between the verified ISO's commit and this branch (see iso/garuda/README.md).
#
#   repack-lillith-from-iso.sh /build/out/<verified>.iso /build/repo
#
# Every file is checked against the sha256 in the package's own mtree (from the
# ISO's pacman database) before it is packaged. Nothing outside /build is written.
set -euo pipefail
ISO="${1:?iso}"; DEST="${2:?dest repo dir}"
W=/build/lillith-repack; rm -rf "$W"; mkdir -p "$W"
log(){ printf '[repack-lillith %(%F %T)T] %s\n' -1 "$*"; }

lba=$(xorriso -indev "$ISO" -find /demonos/x86_64/airootfs.sfs -exec report_lba -- 2>/dev/null \
      | awk -F, '/airootfs.sfs/{gsub(/ /,"",$2); print $2}')
[[ -n "$lba" ]] || { log "no /demonos/x86_64/airootfs.sfs in $ISO"; exit 2; }
off=$((lba * 2048)); log "airootfs.sfs at byte offset $off in $ISO"
db=var/lib/pacman/local/lillith-0.0.1-1
unsquashfs -q -o "$off" -d "$W/db" "$ISO" "$db" >/dev/null
awk '/^%FILES%/{f=1;next} /^$/{f=0} f && !/\/$/' "$W/db/$db/files" > "$W/files.txt"
log "$(wc -l < "$W/files.txt") files listed in the lillith package"
unsquashfs -q -o "$off" -d "$W/pkgroot" -ef "$W/files.txt" "$ISO" >/dev/null
# verify against the package mtree
zcat "$W/db/$db/mtree" | python3 -c '
import sys, hashlib, os, re
root = sys.argv[1]; bad = 0; n = 0
for line in sys.stdin:
    if not line.startswith("./") or line.startswith("./."): continue
    path, *kv = line.split(); kv = dict(x.split("=",1) for x in kv if "=" in x)
    if "sha256digest" not in kv: continue
    p = os.path.join(root, path[2:].encode().decode("unicode_escape").encode("latin1").decode())
    h = hashlib.sha256(open(p,"rb").read()).hexdigest(); n += 1
    if h != kv["sha256digest"]: bad += 1; print("MISMATCH", p)
print(f"mtree check: {n} files, {bad} mismatches"); sys.exit(1 if bad or n == 0 else 0)
' "$W/pkgroot"
install -d -o builder -g builder "$W/build" "$W/mk" "$W/src"; install -d -o builder -g builder "$DEST"
cp /build/src/linux/lillith.install "$W/build/"
cmp -s "$W/build/lillith.install" "$W/db/$db/install" || { log "lillith.install differs from the ISO's"; exit 2; }
# same metadata as linux/PKGBUILD (package() just copies the verified tree)
awk '/^%DEPENDS%/{f=1;next} /^$/{f=0} f' "$W/db/$db/desc" > "$W/depends.txt"
cat > "$W/build/PKGBUILD" <<PKG
pkgname=lillith
pkgver=0.0.1
pkgrel=1
pkgdesc="$(awk '/^%DESC%/{getline; print; exit}' "$W/db/$db/desc")"
arch=('x86_64')
url="https://github.com/kokannadian-killa/Lillith-Desktop-Avatar"
license=('MIT')
depends=($(sed "s/.*/'&'/" "$W/depends.txt" | tr '\n' ' '))
optdepends=($(awk '/^%OPTDEPENDS%/{f=1;next} /^$/{f=0} f' "$W/db/$db/desc" | sed "s/'/'\\\\''/g; s/.*/'&'/" | tr '\n' ' '))
install=lillith.install
options=(!strip !debug !lto)
package() { cp -a "$W/pkgroot/." "\$pkgdir/"; }
PKG
chown -R builder:builder "$W/build"; chmod -R a+rX "$W/pkgroot"
sudo -u builder env -i HOME=/home/builder PATH=/usr/bin LANG=C.UTF-8 PKGDEST="$DEST" \
  BUILDDIR="$W/mk" SRCDEST="$W/src" COMPRESSZST="zstd -c -T0 -6 -" \
  bash -c "cd '$W/build' && makepkg -f --noconfirm --nodeps"
rm -rf "$W/pkgroot" "$W/mk"
ls -la "$DEST"/lillith-0.0.1-1-*.pkg.tar.zst
log "done"
