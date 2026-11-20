# Biotek Engine terminology map (pre-Biotek name → Biotek Engine)

Branch: `biotek/terminology` (off `quantumsec/integration` @ `e0248a6`).
Scope: Lillith OS + QuantumSec (this repo). Rules: additive, non-breaking,
nothing deleted, legacy names kept as read fallbacks / deprecated aliases.

## 0. Engine artifact status (read this first)

**No authoritative Biotek Engine artifact is pinned yet.** Nothing in this repo,
its ISO profile (`iso/`, archiso on `main`; the Garuda profile on
`garuda/dragonized-rebase` likewise) or the built ISO contains the upstream
`biotekengine` binary, its `gui/`, its units, or its config. `git grep -i
biotekengine -- iso linux bundle tools` is empty on `main` @ `7d4472b` and on
`garuda/dragonized-rebase` @ `3f9833c`. The ISO ships Lillith + QuantumSec and the
`~/.biotek` data home only; it does **not** contain "the full Biotek Engine".

The only pinned local runtime is **llama.cpp `b11829`** (commit
`184f6536024668e8908ab8c86033d56708e62b40`, MIT), sha256-locked in
`bundle/llama/llama.lock`, packaged as `lillith-llama-bin` and served on
`127.0.0.1:8080` (loopback only).

> **TODO (Ko's call, not picked here):** to integrate the engine, name all three:
> 1. **Source/artifact pin**: an exact `kokannadian-killa/BoiTekEngine` commit, or a
>    release tarball/binary with its sha256.
> 2. **License**: upstream is marked "PROPRIETARY"; written permission to
>    redistribute it inside the ISO (or a licence file to ship).
> 3. **Runtime contract**: which commands Lillith may run (defensive / read-only
>    only, see §7.8), the listener ports (host side per §8: `127.0.0.1:18080` GUI,
>    `:18765` WebSocket, `:9090` metrics), work dir `%h/.biotek/engine`, and how the
>    approval gate maps onto the blood-seal consent dialog.
>
> Until those exist, docs, release notes and the ISO must not claim the full engine
> is integrated.

## 1. Reference material found

| Source | What it shows |
|---|---|
| GitHub `kokannadian-killa/BoiTekEngine` (public, Rust, last push 2026-07-07; note the repo name is spelled **BoiTek**) | The existing Biotek Engine project. Crate/binary `biotekengine`, brand `BIOTEKENGINE`, config `config/biotekengine.toml`, systemd `biotekengine.service` + `biotekengine-backup.{service,timer}`, env prefix `BIOTEKENGINE_` (see §4), user dot-dir `~/.biotekengine/` / CWD `.biotekengine/`, metrics prefix `biotekengine_*`, GUI `gui/biotekengine.html` (+ a legacy pre-Biotek-named GUI page), ports 8080 (static GUI) / 8765 (WebSocket) / 9090 (Prometheus). Its message bus module ("Nervous System") still carries an upstream pre-Biotek name; renaming it is upstream's call. |
| `docs/PRESERVATION_MANIFEST.md` §12 (this repo) | Phase 0 data-home spec, renamed by Ko from the pre-Biotek data home to `~/.biotek/`. |
| `~/.biotek/README.md` on the dev box (Phase 0) | Same layout as §12. The pre-Biotek Phase 0 original also exists there (same layout, left in place). |
| `lillith/grok_history*.md` (dev box notes) | "BIOTEKENGINE, a mycelial organism desktop icon (Rust core, quantum branches, self-edits KEEP/REVERT logged)". |

**2026-11-19 (release/demonos-phantomcore-2026-11-19, on Ko's instruction "change
every mention to biotek"):** every remaining pre-Biotek mention in code, docs, comments,
data tags and prototype file references was renamed to Biotek (case-preserving). The old
literal survives **only** in the clearly marked `LEGACY-COMPAT` block of
`crates/lillith-setup/src/biotek.rs` (legacy dir name, legacy env var, old migration
marker, two `#[deprecated]` aliases), so existing installs keep migrating and old
environments keep working. A case-insensitive `rg` for the old name outside that file and the ISO guest check
(which looks for the literal on purpose) is empty.

## 2. Casing rules

| Context | Form | Example |
|---|---|---|
| Prose / UI copy (this project) | **Biotek Engine** | "Biotek Engine data home" |
| Upstream engine brand (BoiTekEngine repo) | `BIOTEKENGINE` | banner, Docker labels |
| Lillith-side identifiers, paths, modules | `biotek` (snake/kebab) | `lillith_setup::biotek`, `~/.biotek` |
| Lillith-side env vars | `BIOTEK_` prefix | `BIOTEK_HOME` |
| Talking to the upstream engine process | its own names, unchanged | `BIOTEKENGINE_CONFIG`, `biotekengine.service`, `biotekengine_*` metrics |
| Rust types (new) | `Biotek…` | `MigrationReport` lives in `biotek` |
| Decorative UI labels (terminal style) | `BIOTEK_…` | `BIOTEK_FLUX` |

Never write "BioTek", "Bio-Tek" or "BoiTek" in new code (the GitHub repo name typo is
left alone; it is not ours to rename here).

## 3. Old → new

| Old | New | Status / compat |
|---|---|---|
| legacy pre-Biotek home (`biotek::LEGACY_DIR_NAME`, Phase 0 mandate) | `~/.biotek/` | primary = `~/.biotek`; reads fall back to the legacy home; one-time copy-only migration (§5) |
| legacy home env var (`biotek::LEGACY_HOME_ENV`; implied by the mandate) | `$BIOTEK_HOME` | still honoured as a deprecated fallback |
| pre-Biotek env prefix (none existed) | `BIOTEK_*` | reserve `BIOTEK_` for Lillith/QuantumSec; do not reuse `BIOTEKENGINE_` (upstream engine's) |
| old-name home fn (mandate wording) | `biotek::biotek_home()` | old name kept in the LEGACY-COMPAT block, `#[deprecated]`, returns the Biotek home |
| old flux GUI label (settings GUI, decorative flux graph) | `BIOTEK_FLUX` | label text only; graph unchanged |
| "old pre-Biotek key `num_ctx`" (`[llm]`) | `ctx_size` | unchanged behaviour: `num_ctx` still read as alias, `ctx_size` wins (wording updated only) |
| systemd unit names | none to rename | Lillith units are `lillith-*`; no pre-Biotek-named unit exists. Future QuantumSec unit: `lillith-quantumsec.service` (user unit), not `biotek*` (that namespace belongs to the upstream engine) |
| ISO / archiso scripts | none exist yet | new ISO code must use `~/.biotek` via `lillith_setup::biotek` semantics (§6) |
| Config keys | none with the pre-Biotek prefix | new QuantumSec settings go in `~/.biotek/config/*.toml`, never in `~/.config/lillith/config.toml` |
| File / dir names | none carried the pre-Biotek name | no `git mv` needed |

## 4. Data home layout (`~/.biotek/`, matches Phase 0 exactly)

```
~/.biotek/                 ($BIOTEK_HOME, else the legacy env var [deprecated], else ~/.biotek)
├── README.md
├── config/                QuantumSec / teams / swarm settings
├── logs/                  shield + exercise logs (rotated)
├── memory/                agent-loop scratch (lillith-memory stays in ~/.local/share/lillith)
├── reports/               personalization Markdown reports
├── bots/registry/         main bot manifests
├── bots/swarm/            anon (Tor-aware) swarm manifests
├── consent/               append-only consent log
├── quantumsec/            proton status, lockdown state, callbacks
└── teams/{red,blue,purple}/
```

`lillith_setup::biotek::LAYOUT` is this list; `ensure_layout_in()` creates it.

### Differences vs the upstream Biotek Engine (BoiTekEngine)

The upstream engine has **no** `~/.biotek` concept. It uses:

| Upstream | Where |
|---|---|
| `./data`, `./keys/vault_master.key` (relative to CWD) | `config/biotekengine.toml` `[cortex] data_dir`, `[vault] key_path` |
| `/var/lib/biotekengine/{data,logs,backups}` | systemd units (system service, user `biotekengine`) |
| `/etc/biotekengine/biotekengine.toml`, `/etc/biotekengine/environment` | systemd units |
| `/data/{vault,cortex,search,backups,snapshots,logs}` | Docker image |
| `~/.biotekengine/models/` (`$HOME`-relative) | `src/model_registry.rs` (GGUF model registry) |
| `.biotekengine/{reports,probability,probability/registry,lab/registry}` (CWD-relative) | `report_generator.rs`, `probability_store.rs`, `lab_forge.rs`, `swarm/scribe.rs` |

So the upstream user-level dot-dir is **`.biotekengine`**, not `.biotek`.
Lillith keeps `~/.biotek` (Ko's Phase 0 decision); the two must not be merged
silently. Options for the Architect: (a) keep both and point Lillith's model
scanner at `~/.biotekengine/models` as an extra scan path, or (b) run the engine
with CWD `~/.biotek/engine` so its relative `.biotekengine/…` lands under
`~/.biotek/engine/.biotekengine/…`.

`src/main.rs` hard-codes `Config::load("config/biotekengine.toml")`.
`BIOTEKENGINE_DATA_DIR` / `BIOTEKENGINE_CONFIG` are set by its units, Dockerfile,
compose and k8s but no source file reads them. Env vars the code does read:
`BIOTEKENGINE_HARDWARE_EMULATOR` (`hardware_interface.rs`),
`BIOTEKENGINE_WS_RATE_LIMIT_PER_SEC` / `BIOTEKENGINE_WS_RATE_BURST`
(`rate_limiter.rs`), plus `RUST_LOG`, `HOME` / `USERPROFILE`. If the engine is bundled into the ISO, run it
with `WorkingDirectory` set so `./config` and `./data` resolve, e.g. a user unit
with `WorkingDirectory=%h/.biotek/engine` (proposal, not implemented here).

## 4b. Split: Lillith data home vs upstream engine paths

Two resolvers in `lillith_setup::biotek`, deliberately separate:

| | Lillith data home | Upstream Biotek Engine (`biotekengine`) |
|---|---|---|
| Resolver | `biotek_home()` | `engine_paths()` → `EnginePaths` |
| Root | `$BIOTEK_HOME` → legacy env var (deprecated) → `~/.biotek` | work dir `<biotek home>/engine` (= `%h/.biotek/engine` in a user unit) |
| Config | `~/.biotek/config/*.toml` | `$BIOTEKENGINE_CONFIG` → `<work_dir>/config/biotekengine.toml` |
| Data | layout in §4 | `$BIOTEKENGINE_DATA_DIR` → `<work_dir>/data` |
| Models | Lillith's own GGUF scan paths, **plus `~/.biotekengine/models`** (added last in `scan::default_scan_dirs`; older paths unchanged) | `~/.biotekengine/models` (`$HOME`-relative upstream) |
| Engine state | n/a | `<work_dir>/.biotekengine/{reports,probability,lab/registry}` (`state_dir`, `reports_dir()`) |
| Names | `biotek`, `BIOTEK_*` | unchanged upstream: `BIOTEKENGINE_*`, `biotekengine.service`, `biotekengine_*` metrics |

`BIOTEKENGINE_DATA_DIR` / `BIOTEKENGINE_CONFIG` are set by the upstream units,
Dockerfile, compose and k8s files but **upstream code does not read them**. Lillith
honours them (absolute values only) so its view matches the unit's intent. Since the
engine always loads `config/biotekengine.toml` from its CWD,
`EnginePaths::config_needs_placing()` is true when the override points elsewhere; the
launcher must then copy or symlink the file into `<work_dir>/config/`. Nothing in
Lillith creates or writes engine paths.

## 5. Fallback / migration behaviour (`crates/lillith-setup/src/biotek.rs`)

* `biotek_home()`: `$BIOTEK_HOME` → `LEGACY_HOME_ENV` (deprecated) → `~/.biotek`. Only absolute, non-empty env values count.
* `legacy_home()`: `~/<LEGACY_DIR_NAME>` (read fallback / migration source only; never written).
* `resolve_read(rel)`: `~/.biotek/<rel>` if it exists, else the legacy home's `<rel>` if it exists, else `~/.biotek/<rel>`. All writes go to `~/.biotek`.
* `migrate_legacy_home()` (old name kept as a deprecated alias): one-time, **copy-only**. Copies every regular file from the legacy home that is missing in `~/.biotek`; never overwrites, never deletes, skips symlinks; writes `~/.biotek/.migrated-from-legacy-home` and is a no-op afterwards (the marker name written by earlier builds, `LEGACY_MIGRATION_MARKER`, is also honoured); then creates the layout. The legacy home is left untouched.
* *(Earlier snapshot: "nothing calls these yet".)* Now wired: the desktop start (`lillith` with no subcommand, `run_overlay` in `apps/lillith/src/main.rs`) calls `migrate_legacy_home()` once before anything else reads `~/.biotek`. It is copy-only, a no-op after the marker exists, logs what it copied, and never stops the app on error. QuantumSec (`shield.rs`, `consent.rs`, the settings tab) reads through `biotek_home()`.
* Model scan: `~/.biotekengine/models` is scanned as an extra directory (option (a) in §4); every older scan path is kept, in the same order.

## 6. Renamed on 2026-11-19 (previously left as provenance)

| Occurrence | Now |
|---|---|
| Python prototype file references (`*_animator.py`, `*_gesture_map.py`, `*_avatar_window.py`, `*_tts.py`, `*_stt.py`, …) in `docs/PLAN.md`, `docs/FEATURES.md`, `crates/lillith-anim/src/{lib,gestures}.rs`, `crates/lillith-sense/src/kwin.rs`, `assets/rules.toml` | `biotek_*` (Ko: change every mention). The prototype files themselves are outside this repo. |
| `status = "PY (…)"` provenance tags in `assets/rules.toml` | `"PY (biotek)"` (free-text tag; nothing parses it) |
| "… Companion" brand pass and the name option in `docs/FEATURES.md` / `docs/PLAN.md` | "Biotek Companion" / "Biotek" |
| `docs/PRESERVATION_MANIFEST.md` §12 notes | reworded without the old literal |
| Upstream BoiTekEngine module/GUI names | described generically; not in this repo, upstream's call |

Remaining literal (by design): only the `LEGACY-COMPAT` block in
`crates/lillith-setup/src/biotek.rs`, plus the ISO guest check that asserts no
legacy path or unit exists in the live system.

## 7. Biotek Engine integration points (for the final Lillith + QuantumSec + ISO build)

1. **Data home**: `lillith_setup::biotek` (this branch) is the single resolver for `~/.biotek`.
2. **Engine binary** `biotekengine` (Rust, clap CLI): `init <scope>`, `health`, `report`, `pending` / `approve <id>` / `deny <id>` (approval gate → map onto Lillith's consent / blood-seal dialog and `~/.biotek/consent/`), `gui`, `backup`, `search`, `model list`.
3. **WebSocket** `:8765`: JSON frames `{"type": "metrics"|"state"|"holo_update"|"event", "data": …}`; a QuantumSec panel can subscribe read-only. **Must be localhost-only** (see §8).
4. **Prometheus** `:9090/metrics`, prefix `biotekengine_` (e.g. `biotekengine_pending_approvals`, `biotekengine_swarm_agents_active`). **Must be localhost-only.** Note: in the upstream `main.rs` we read, `MetricsExporter` is defined but never started, so `:9090` may not be served until upstream wires it.
5. **Static GUI** `:8080` serves `gui/biotekengine.html`. **Clashes with Lillith's llama-server default** `127.0.0.1:8080` (see §8).
6. **LLM**: engine uses Ollama `http://localhost:11434`, model `dolphin3.0-llama3.2-3b`; Lillith's Ollama host defaults to the same port, so one Ollama can serve both.
7. **systemd**: upstream ships a hardened *system* unit (`User=biotekengine`, `/var/lib/biotekengine`). For the ISO prefer a user unit or keep the system unit but do not mix data with `~/.biotek`.
8. **Legal/scope**: upstream is "PROPRIETARY" and pentest-oriented (`swarm <target>`, `predict-*`, `pos-audit`). Lillith's red team is LOCAL SIMULATION ONLY; only defensive/read-only engine commands should be wired without explicit consent.

## 8. Network integration notes (no engine code changed)

Upstream hard-codes all three listeners on `0.0.0.0` (`gui_server.rs`, `websocket_server.rs`)
with fixed ports (`with_port(8080)`, `8765`) and has no CLI flag or env var for host/port.

| Port | Upstream | Required in Lillith OS |
|---|---|---|
| 8765 WebSocket | `0.0.0.0:8765` | localhost only |
| 9090 Prometheus | documented, `0.0.0.0` | localhost only |
| 8080 static GUI | `0.0.0.0:8080` | move off 8080; **leave Lillith's llama-server default `127.0.0.1:8080` alone** |

Recommended fix (unit / launch level only):

* **Container launch (preferred, no code change):** publish only to loopback and remap the GUI,
  e.g. `-p 127.0.0.1:18080:8080 -p 127.0.0.1:8765:8765 -p 127.0.0.1:9090:9090`
  (compose: `"127.0.0.1:18080:8080"` etc.). The engine keeps listening on 8080 inside its
  own network namespace, so there is no clash with llama-server, and nothing is reachable off-box.
  `18080` is a suggestion; pick any free loopback port and record it in `~/.biotek/config/`.
* **Bare systemd unit (`biotekengine.service` or a user unit with `WorkingDirectory=%h/.biotek/engine`):**
  add `IPAddressAllow=localhost` + `IPAddressDeny=any` so 8765 / 9090 / 8080 only answer loopback
  peers. This does **not** free port 8080: a bare process cannot be moved off 8080 without an
  upstream change. So under a bare unit either run the engine in its own netns (e.g.
  `PrivateNetwork=yes` plus a `systemd-socket-proxyd` unit on `127.0.0.1:18080`, `:8765`, `:9090`,
  which also cuts its Ollama access unless proxied), or do not run `biotekengine gui` while
  llama-server is up. Commands other than `gui` open no listeners.
* Upstream follow-up (not done here, BoiTekEngine untouched): a host/port flag or env for the three
  listeners would make the bare-unit path trivial.

Constants for these live in `lillith_setup::biotek` (`ENGINE_GUI_PORT`, `ENGINE_WS_PORT`,
`ENGINE_METRICS_PORT`, `ENGINE_UNIT`, `ENGINE_BIN`, `ENGINE_METRICS_PREFIX`).

### 8a. Port clash resolution (host side, loopback only)

Two clashes exist: engine GUI `:8080` vs llama-server `127.0.0.1:8080`, and engine
WebSocket `:8765` vs `lillith-py-api.service` (`[sidecar] port`, default 8765). Lillith's
services **keep** their ports. The engine is published on other loopback ports:

| Engine listener | Engine side (fixed upstream) | Host side default | Override |
|---|---|---|---|
| static GUI | 8080 | `127.0.0.1:18080` | `BIOTEK_ENGINE_GUI_PORT` |
| WebSocket | 8765 | `127.0.0.1:18765` | `BIOTEK_ENGINE_WS_PORT` |
| Prometheus | 9090 | `127.0.0.1:9090` | `BIOTEK_ENGINE_METRICS_PORT` |

`lillith_setup::biotek::engine_ports()` returns these (`EnginePorts`), refuses 8080 / 8765 /
11434 and duplicates, and `EnginePorts::publish_args()` gives the loopback-only container
flags (`-p 127.0.0.1:18080:8080 …`). Host is always `ENGINE_BIND_HOST` = `127.0.0.1`. Nothing
launches the engine yet (see §0); this is the contract a launcher must use.

Bind audit (`rg 0.0.0.0` on `main` @ `7d4472b`): no Lillith listener binds `0.0.0.0`. Every hit
is a test asserting refusal (`lillith-mind/src/server.rs`, `llm.rs`, `lillith-setup/src/launch.rs`,
`local.rs`, `apps/lillith/src/chat.rs`), the ISO guest check that llama-server is *not* on
`0.0.0.0:8080`, or a doc/comment. `lillith-py-api.service` runs `serve --host 127.0.0.1` with
`IPAddressDeny=any` / `IPAddressAllow=localhost`; llama-server is launched with
`--host 127.0.0.1` (`launch.rs` `LOOPBACK_HOST`).
