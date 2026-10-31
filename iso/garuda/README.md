# DemonOS PhantomCore on Garuda Linux Dr460nized (KDE)

`iso/profile-garuda` is the DemonOS PhantomCore live ISO rebased from plain
Arch (`iso/profile`, archiso releng) onto **Garuda Linux Dr460nized**. Garuda's real
repositories, packages, kernel and desktop layout are used. DemonOS branding is
applied on top, and the full Lillith + QuantumSec + BioTek system ships unchanged.
`iso/profile` and its verified ISO are kept as they are.

## Approach (and why)

* **mkarchiso with Garuda's package set, not garuda-tools/buildiso.** The Arch
  ISO build path (`iso/setup-arch-chroot.sh`, `mkarchiso`, `test-boot.sh`,
  `guest-check.py`) is already verified on this box. garuda-tools' `buildiso`
  expects its own host setup and the multi-squashfs *miso* live layout. So the
  Dr460nized **package list** is taken from Garuda's own iso-profiles and
  installed from Garuda's own repos. mkarchiso builds the live medium (archiso
  boot, one squashfs).
* **Real Garuda sources, signature-checked.** `[garuda]` and `[chaotic-aur]` are
  configured as in garuda-tools' `pacman-default.conf` (`[garuda]` before
  `[core]`, `[chaotic-aur]` last, both `Include = /etc/pacman.d/chaotic-mirrorlist`).
  Signatures are checked against the Chaotic-AUR key
  `EF925EA60F33D0CB85C44AD13056513887B78AEB` (fetched over HTTPS and fully
  trusted in the build chroot's keyring). `chaotic-keyring` and
  `chaotic-mirrorlist` come from that repo. `SigLevel = Required DatabaseOptional`
  applies to every repo except the local `[demonos-local]`.
* **Branding that keeps winning.** Garuda's `garuda-hooks` and
  `garuda-config-agent` rewrite `os-release`, `lsb-release`, `/etc/issue` and
  `/etc/default/grub` back to "Garuda" on every pacman transaction. DemonOS ships
  its own pacman hook,
  `/usr/share/libalpm/hooks/zzzz50-demonos-branding.hook` → `/usr/local/bin/demonos-rebrand`.
  It sorts after all of Garuda's hooks, so DemonOS wins at build time and after
  every later `pacman -Syu` in the live or installed system. The script is
  idempotent, and it saves Garuda's originals under
  `/usr/share/demonos/garuda-originals/`.
* **Lillith isn't recompiled.** `repack-lillith-from-iso.sh` rebuilds the exact
  `lillith-0.0.1-1` package from the verified Arch ISO. Each of its 582 files is
  checked against the sha256 in that package's own mtree (0 mismatches), so the
  avatar, shield, teams, reports, consent gates, bot registry, units and art are
  byte-identical to the ones that passed the 17 guest checks.
  `lillith-llama-bin` is the verified build's package.

## Garuda sources

| what | source |
|---|---|
| package list | `gitlab.com/garuda-linux/tools/iso-profiles` @ `760f824c`, `garuda/dr460nized` (Packages-Root, -Desktop-Common, -Desktop, -Mhwd, -Live) |
| repo layout, default kernel | `gitlab.com/garuda-linux/tools/garuda-tools` @ `57325fab` (`pacman-default.conf`; `KERNEL=linux-garuda`) |
| packages | `[garuda]` and `[chaotic-aur]` (via `chaotic-mirrorlist`), plus Arch `[core]`/`[extra]`/`[multilib]` |
| examples installed | `garuda-dr460nized 5.0.4`, `garuda-common-settings 2.14.0`, `garuda-hooks 2.16.0`, `garuda-config-agent 1.1.0`, `garuda-fish-config`, `garuda-toolbox 6.3.0`, `garuda-setup-assistant`, `garuda-wallpapers`, `garuda-icons`, `linux-garuda 7.3.1-1`, `firedragon`, `kvantum`, `beautyline`, `chaotic-keyring`, `chaotic-mirrorlist`; 1313 packages in total |

Left out of Garuda's list, with the reasons (also in the header of `packages.x86_64`):
* `calamares-garuda`, `calamares-branding-garuda`: tied to miso and Garuda branding.
* `garuda-tools-base-git`, `garuda-live-systemd`: miso live plumbing (archiso does the live boot).
* `grub-theme-garuda-live`, `grub-theme-garuda-dr460nized`: Garuda trademark art, replaced by the DemonOS theme.
* `garuda-dracut-support`, `dracut`: with dracut present, garuda-hooks skips mkinitcpio, but the archiso live initramfs needs mkinitcpio-archiso.
* `linux-garuda-headers`, `broadcom-wl-dkms`: disk.
* the NVIDIA profile.
* `virtualbox-guest-utils-nox`: conflicts with Garuda's `virtualbox-guest-utils`.

## What DemonOS replaces (Garuda trademark / branding items)

| Garuda item | DemonOS replacement |
|---|---|
| `os-release` NAME/ID "Garuda Linux", `lsb-release` "Garuda Linux Temeraire", `/etc/issue` | `NAME="DemonOS PhantomCore"`, `ID=demonos`, `ID_LIKE="garuda arch"`, `VARIANT="Demon-AI-QuantumSec-BioTek-Engine"`; `DISTRIB_ID=DemonOS` |
| GRUB theme `garuda` (+ live theme), `GRUB_DISTRIBUTOR="Garuda"` | DemonOS GRUB theme (live menu + `/usr/share/grub/themes/demonos` for installs), `GRUB_DISTRIBUTOR="DemonOS"` |
| Garuda Plymouth / boot splash | Plymouth theme `demonos` ("AWAKENING THE CORE") |
| SDDM greeter (Garuda: plasma-login-manager / Maldrakor greeter wallpaper) | SDDM with theme `demonos` (autologin `demon`, Wayland) |
| Plasma look-and-feel `Dr460nized` (Maldrakor wallpaper, Dr460nized splash, `distributor-logo-garuda` start icon) | look-and-feel `org.demonos.phantomcore`: Garuda's own Dr460nized panel + dock templates, Kvantum, Sweet window decorations, BeautyLine icons, with DemonOS colours, wallpaper, lock-screen wallpaper, splash and start icon; Lillith pinned in the dock |
| fastfetch logo `/usr/share/icons/garuda/dr460nized-fastfetch.png` | `/usr/share/fastfetch/presets/demonos.jsonc` (Garuda's module list, DemonOS logo); the fish greeting is DemonOS's |
| Garuda Setup Assistant autostart (network software installs on first login) | not autostarted on the live medium (the Lillith awakening dialog is the first-run flow); `garuda-setup-assistant` is still installed |

Garuda's own packages (`garuda-toolbox`, `garuda-wallpapers`, `garuda-icons`,
the Dr460nized themes and so on) stay installed, unmodified and under their own
licenses (mostly GPL-3.0-or-later; see `/usr/share/licenses/<pkg>`). DemonOS doesn't
modify Garuda's files in place, except the config files that Garuda's own hooks
rewrite (listed above). Garuda's originals are kept in
`/usr/share/demonos/garuda-originals/`. The DemonOS look-and-feel is
GPL-3.0-or-later and credits Garuda's Dr460nized layout templates, which it loads
by name. "Garuda Linux" and the Garuda logo are the Garuda project's marks. This
ISO doesn't present itself as Garuda Linux; "Garuda Dr460nized base" is used only to
describe what it is built from.

## Differences from Garuda's live ISO (documented, on purpose)

* The live boot is archiso's (`archisobasedir=demonos`, one `airootfs.sfs`), not miso. There is no Calamares installer.
* Networking is the tested DemonOS setup (systemd-networkd, iwd, resolved, with LLMNR and mDNS off). `avahi` and NetworkManager are installed but not enabled. Garuda's `profile.conf` service list isn't adopted.
* `plasma-login-manager` (pulled in by `garuda-dr460nized`) is installed but not enabled. `display-manager.service` is SDDM.
* garuda-hooks' `99-update-grub.hook` is masked *only during the build* by `/etc/pacman.d/hooks/99-update-grub.hook`. Otherwise it would run grub-mkconfig/os-prober against the build host's disks. releng's `zzzz99` hook deletes the mask, so the live system keeps Garuda's real hook.

## Build

```sh
sudo iso/setup-arch-chroot.sh            # once (same chroot as iso/build-iso.sh)
# the chroot also needs the Chaotic-AUR key + chaotic-keyring/chaotic-mirrorlist
# (see "Garuda sources"), and the verified Arch ISO in $BUILD/out for the lillith repack
nohup iso/garuda/build-garuda-iso.sh > build-garuda.out 2>&1 &
```

1. `build-garuda-iso.sh` (host) runs a preflight: free disk, `garuda.db` and `chaotic-aur.db` reachable (**if either is unreachable it stops and says so; nothing is faked**), the model, and the llama package. It then stages `iso/profile-garuda` with the model hardlinked in, bind-mounts, starts the same low-disk watcher as `build-iso.sh` (`ABORT_FREE_MB=1500`) and runs:
2. `chroot-build-garuda.sh` (chroot) checks the key and mirrorlist, repacks and mtree-checks lillith, runs `repo-add`, generates the theme art (`iso/theme/make-theme-assets.py`) plus the DemonOS logo, start icon, fastfetch logo, LnF splash and GRUB theme for installs, and runs `mkarchiso -v -r`.
3. Disk savers (this box had about 12 GB free):
   * Each downloaded package file is deleted as soon as its local-db entry appears in the airootfs.
   * `mksquashfs-stream.sh` is put first on `PATH` (as `mksquashfs`). It streams the airootfs as a POSIX tar (with xattrs/ACLs/numeric owners, so file capabilities survive) into `sqfstar`, deleting files once they are in the stream, and then checks that the image has exactly as many entries as the source had.
   * `work/` is removed at the end.

## Result (2026-10-30)

* `demonos-phantomcore-garuda-2026.10.30-x86_64.iso`
* 5,753,348,096 bytes (5.4 GiB)
* sha256 `aa6606ecfeb71aea4f4d45761d6913c46931106b5a6bb4b84ed289c8b3e282a7`
* built 18:04 MDT
* airootfs.sfs is 5.44 GB, so it is a multi-extent ISO 9660 file (ISO level 3, which Linux reads fine)
* the lowest free disk during the build was 3956 MB

Boot-test results: see "Garuda boot proof" in `docs/HANDOVER.md`.
