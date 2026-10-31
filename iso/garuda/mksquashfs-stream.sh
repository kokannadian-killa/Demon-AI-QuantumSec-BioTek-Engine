#!/usr/bin/env bash
# Disk-saving stand-in for `mksquashfs SRC DEST -noappend OPTS...`, put first on
# PATH only for the Garuda ISO build (chroot-build-garuda.sh). It streams SRC as a
# POSIX tar (xattrs/ACLs/numeric owners, so file capabilities survive) into
# sqfstar and deletes each file once it is in the stream, so the ~9 GB airootfs
# and the ~5 GB squashfs never sit on the disk at the same time.
# Anything other than a directory source falls through to the real mksquashfs.
set -euo pipefail
REAL=/usr/bin/mksquashfs
SRC="${1:-}"; DEST="${2:-}"
[[ -d "$SRC" && -n "$DEST" && "${DEMONOS_SQFS_STREAM:-0}" == 1 ]] || exec "$REAL" "$@"
shift 2
opts=()
for a in "$@"; do [[ "$a" == -noappend ]] || opts+=("$a"); done
before=$(find "$SRC" -mindepth 1 | wc -l)
echo "[sqfs-stream] $SRC -> $DEST: $before entries, file caps $(getcap -r "$SRC" 2>/dev/null | wc -l); options: ${opts[*]}"
rm -f "$DEST"
mapfile -t top < <(cd "$SRC" && find . -mindepth 1 -maxdepth 1 -printf '%P\n' | LC_ALL=C sort)
set -o pipefail
tar --xattrs --xattrs-include='*' --acls --numeric-owner --format=posix --sparse \
    --remove-files -C "$SRC" -cf - -- "${top[@]}" \
  | /usr/bin/sqfstar "${opts[@]}" -root-mode 0755 "$DEST"
left=$(find "$SRC" -mindepth 1 | wc -l)
echo "[sqfs-stream] done; $left entries left in $SRC (expected 0)"
# verify: entry count and capabilities inside the image
inside=$(( $(unsquashfs -lln "$DEST" | grep -c '^[-dlcbps]') - 1 ))
echo "[sqfs-stream] image lists $inside entries (source had $before + root)"
(( inside == before )) || { echo "[sqfs-stream] ERROR: entries missing from image"; exit 7; }
