# DemonOS PhantomCore ISO: signing status and plan

**Status (2026-11-15): the ISO is unsigned.** This is a plan with TODOs only. No key has
been generated, stored, committed or published, and no ISO has been published.

## Current state (verified on `main` @ `7d4472b`)

| Item | State | Evidence |
|---|---|---|
| Secure Boot | **Not supported.** No shim, no signed GRUB / kernel, no MOK. Firmware with Secure Boot on will refuse to boot it; turn Secure Boot off to boot. | `iso/profile/profiledef.sh` `bootmodes=('bios.syslinux' 'uefi.grub')` (same on `garuda/dragonized-rebase` @ `3f9833c`); `docs/HANDOVER.md` §7; `iso/README.md` ("unsigned, and Secure Boot isn't supported, the same as releng") |
| Release signature (GPG / minisign) | **None.** | no `.sig` / `.asc` / `.minisig` produced by `iso/build-iso.sh` |
| Integrity | sha256 only, written by `iso/build-iso.sh` next to the ISO (`<iso>.sha256`) and recorded in `docs/HANDOVER.md` / `iso/README.md`. A sha256 proves the download matches the build box's file; it does not prove who built it. | `iso/build-iso.sh:101` |
| Publishing | Not published. The ISO lives only on the build box. How to deliver it (GitHub release parts or Google Drive) is still Ko's call. | `docs/HANDOVER.md` §7 |

Release notes must keep saying **"unsigned, local-only ISO; Secure Boot off"** until the
TODOs below are done.

## Plan (each step needs Ko's decision before anyone does it)

1. **TODO (Ko): detached release signature.** Choose minisign (simplest) or GPG. Ko
   generates the key **offline, on his own machine**; the secret key never goes on the
   build box, into git, CI secrets or chat. Only the public key / fingerprint is committed
   (e.g. `docs/keys/`). The build box produces `SHA256SUMS`; Ko signs that file himself
   (`minisign -Sm SHA256SUMS` or `gpg --detach-sign --armor SHA256SUMS`) and the signature is
   published beside the ISO. Users check with `minisign -Vm SHA256SUMS -p demonos.pub` (or
   `gpg --verify`) and then `sha256sum -c SHA256SUMS`.
2. **TODO (Ko): Secure Boot, or explicitly not.** Options:
   * (a) keep "Secure Boot off" and document it (current state, zero risk of key loss);
   * (b) shim + MOK: ship Microsoft-signed `shim` (`shim-signed`), sign GRUB and the kernel
     with a DemonOS MOK key (`sbsigntools` / `sbsign`), user enrols the MOK once via
     `mokutil` / MokManager. The MOK private key has the same handling rules as step 1;
   * (c) user-owned keys (`sbctl`) for installed systems only, not the live ISO.
   Whatever is chosen must be re-tested with OVMF **Secure Boot enabled**, under QEMU
   **TCG only** (never `-accel kvm` on this host), for both the archiso and Garuda profiles.
3. **TODO (lead): wire verification into the docs and test harness** once a key exists:
   `iso/README.md` gains the verify commands, and `iso/test-boot.sh` checks the signature
   before booting. Not before: there is nothing to verify yet.
4. **TODO (Ko): publishing channel** (GitHub release split parts or Google Drive). Signing
   should be done before the first public upload, so no unsigned copy circulates as "the"
   release.

## What is deliberately not done here

* No key generation, no secrets, no signing of any existing ISO.
* No change to `iso/build-iso.sh`, `profiledef.sh` or the boot loaders.
* No ISO build or upload.
