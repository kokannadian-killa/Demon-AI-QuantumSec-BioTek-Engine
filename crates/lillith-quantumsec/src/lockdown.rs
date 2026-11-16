//! Lockdown proposals: exact command strings, never executed without a
//! [`ConsentToken`] issued after a blood-seal approval.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// Error from a lockdown execute attempt.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum LockdownError {
    /// No consent token, or token does not match this proposal (id, exact
    /// command plan, or flags changed since the blood seal).
    #[error("consent required: blood-seal token missing or mismatched")]
    ConsentRequired,
    /// Token was already consumed.
    #[error("consent token already consumed")]
    TokenConsumed,
    /// A proposed command refused to spawn (still never silently skipped).
    #[error("command failed to spawn: {0}")]
    Spawn(String),
    /// The proposal cannot be executed safely as displayed (for example a
    /// LOCAL SIMULATION card whose line is not a plain `echo` / `true`, or a
    /// typed argv plan whose display text no longer matches). Nothing ran.
    #[error("refused unsafe command plan: {0}")]
    UnsafeCommand(String),
}

/// A proposed lockdown: exact commands the shield wants to run.
///
/// Red team = **local simulation only**. The default proposal uses
/// clearly labeled dry-run / local-only commands (`echo`, `true`) so a
/// mistaken click cannot alter the host. Real hardening commands are
/// added only when the caller builds a custom proposal and the user
/// seals them in blood.
///
/// Proposals built with [`LockdownProposal::from_argv`] carry a typed argv
/// plan: they execute **without a shell**, and `commands` is rendered from
/// that same argv (POSIX quoting), so the text on the card is exactly what
/// runs. Legacy string-only proposals ([`LockdownProposal::new`]) still run
/// each line with `sh -c`, except LOCAL SIMULATION proposals, which never
/// reach a shell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockdownProposal {
    /// Stable, unique id (also the consent-log key).
    pub id: String,
    /// Human summary shown on the blood-seal card.
    pub summary: String,
    /// Exact command lines shown on the card. For argv proposals this is the
    /// quoted rendering of [`LockdownProposal::argv`]; for legacy proposals
    /// each line is one `sh -c` invocation if executed.
    pub commands: Vec<String>,
    /// Typed argv plan (one entry per `commands` line). Empty for legacy
    /// string-only proposals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub argv: Vec<Vec<String>>,
    /// When true, every command is labeled local-simulation / dry-run (Red team).
    pub local_simulation: bool,
    /// When true (and not a local sim), the seal shows "PROPOSAL, not run"
    /// and [`execute_with_token`] logs consent but does not spawn commands.
    #[serde(default)]
    pub proposal_only: bool,
    /// Unix epoch when proposed.
    pub proposed_unix: u64,
}

impl LockdownProposal {
    /// Default local-simulation proposal (never touches the host).
    pub fn local_simulation_default() -> Self {
        let echo = |m: &str| vec!["echo".to_string(), m.to_string()];
        let mut p = Self::from_argv(
            "LOCAL SIMULATION ONLY — QuantumSec dry-run lockdown (no host changes)",
            vec![
                echo("[QuantumSec] LOCAL SIMULATION: lockdown begin"),
                echo("[QuantumSec] LOCAL SIMULATION: would pause non-essential user units"),
                echo("[QuantumSec] LOCAL SIMULATION: lockdown end"),
                vec!["true".into()],
            ],
        );
        p.id = format!("lockdown-sim-{}", unique_id_suffix());
        p.local_simulation = true;
        p
    }

    /// Custom proposal (caller sets `local_simulation` / `proposal_only`).
    ///
    /// Legacy string form: each line is shown verbatim and, if this is a
    /// real (non-simulation, non-proposal-only) card, run with `sh -c`.
    /// Prefer [`LockdownProposal::from_argv`] whenever any part of a command
    /// comes from data (paths, JSON fields, …).
    pub fn new(summary: impl Into<String>, commands: Vec<String>) -> Self {
        Self {
            id: format!("lockdown-{}", unique_id_suffix()),
            summary: summary.into(),
            commands,
            argv: Vec::new(),
            local_simulation: false,
            proposal_only: false,
            proposed_unix: now_unix(),
        }
    }

    /// Custom proposal from typed argv (executed without a shell).
    ///
    /// `commands` is rendered from `argv` with [`render_argv`], so the card
    /// shows exactly what will run. A leading `~/` in an unquoted (plain)
    /// argument is expanded to `$HOME` at execution, matching what the shown
    /// line would mean to a POSIX shell.
    pub fn from_argv(summary: impl Into<String>, argv: Vec<Vec<String>>) -> Self {
        let mut p = Self::new(summary, argv.iter().map(|a| render_argv(a)).collect());
        p.argv = argv;
        p
    }

    /// Banner / target chip on the blood-seal card.
    pub fn seal_banner(&self) -> &'static str {
        if self.local_simulation {
            "LOCAL SIMULATION ONLY"
        } else if self.proposal_only {
            "PROPOSAL, not run"
        } else {
            "REAL COMMANDS — review carefully"
        }
    }

    /// Exact text shown on the blood-seal terminal slab.
    pub fn detail_block(&self) -> String {
        let flag = if self.local_simulation {
            "# RED TEAM = LOCAL SIMULATION ONLY\n"
        } else if self.proposal_only {
            "# PROPOSAL, not run — acknowledge only; commands are not executed\n"
        } else {
            "# REAL COMMANDS — review carefully\n"
        };
        let body = self
            .commands
            .iter()
            .map(|c| format!("$ {c}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("{flag}{body}")
    }

    /// SHA-256 (hex) over everything the blood seal shows or that changes
    /// what runs: id, summary, exact command lines, argv plan, and the
    /// `local_simulation` / `proposal_only` flags (plus `proposed_unix`).
    ///
    /// A [`ConsentToken`] records this digest at approval; execution
    /// re-hashes the proposal it is about to run and refuses on mismatch.
    pub fn binding_digest(&self) -> String {
        let mut enc = Vec::new();
        let mut field = |tag: &[u8], bytes: &[u8]| {
            enc.extend_from_slice(tag);
            enc.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
            enc.extend_from_slice(bytes);
        };
        field(b"lillith-quantumsec/consent-binding/v1", b"");
        field(b"id", self.id.as_bytes());
        field(b"summary", self.summary.as_bytes());
        field(b"local_simulation", &[self.local_simulation as u8]);
        field(b"proposal_only", &[self.proposal_only as u8]);
        field(b"proposed_unix", &self.proposed_unix.to_be_bytes());
        field(b"commands", &(self.commands.len() as u64).to_be_bytes());
        for c in &self.commands {
            field(b"cmd", c.as_bytes());
        }
        field(b"argv", &(self.argv.len() as u64).to_be_bytes());
        for a in &self.argv {
            field(b"argc", &(a.len() as u64).to_be_bytes());
            for w in a {
                field(b"arg", w.as_bytes());
            }
        }
        hex(&sha256::digest(&enc))
    }

    /// The exact steps [`execute_with_token`] would spawn, after validation.
    fn execution_plan(&self) -> Result<Vec<Step>, LockdownError> {
        let steps: Vec<Step> = if !self.argv.is_empty() {
            if self.argv.len() != self.commands.len() {
                return Err(LockdownError::UnsafeCommand(
                    "argv plan and displayed commands differ in length".into(),
                ));
            }
            for (a, c) in self.argv.iter().zip(&self.commands) {
                if a.is_empty() || a[0].is_empty() {
                    return Err(LockdownError::UnsafeCommand("empty argv".into()));
                }
                if render_argv(a) != *c {
                    return Err(LockdownError::UnsafeCommand(format!(
                        "displayed command does not match argv: {c}"
                    )));
                }
            }
            self.argv.iter().cloned().map(Step::Argv).collect()
        } else if self.local_simulation {
            // Legacy simulation lines never reach a shell: only a plain
            // `echo '<text>'` or `true` is accepted (run as argv).
            self.commands
                .iter()
                .map(|c| {
                    simple_sim_argv(c).map(Step::Argv).ok_or_else(|| {
                        LockdownError::UnsafeCommand(format!(
                            "LOCAL SIMULATION line is not a plain echo/true: {c}"
                        ))
                    })
                })
                .collect::<Result<_, _>>()?
        } else {
            self.commands.iter().cloned().map(Step::Shell).collect()
        };
        if self.local_simulation {
            for s in &steps {
                match s {
                    Step::Argv(a) if a[0] == "echo" || (a[0] == "true" && a.len() == 1) => {}
                    _ => {
                        return Err(LockdownError::UnsafeCommand(
                            "LOCAL SIMULATION may only run echo / true".into(),
                        ))
                    }
                }
            }
        }
        Ok(steps)
    }
}

/// One validated execution step.
enum Step {
    /// Typed argv, spawned directly (no shell).
    Argv(Vec<String>),
    /// Legacy caller-authored shell line, shown verbatim on a REAL card.
    Shell(String),
}

/// Token proving the user sealed a specific proposal in blood (mouse click).
///
/// Opaque: created only by [`crate::consent::ConsentLog::approve`]. It is
/// bound to [`LockdownProposal::binding_digest`] of the exact proposal shown,
/// carries a random nonce that is never logged, printed, or serialized, and
/// deliberately implements neither `Serialize` nor `Deserialize` so it cannot
/// be forged from (or leaked into) data.
#[derive(Clone, PartialEq, Eq)]
pub struct ConsentToken {
    /// Proposal id this token unlocks.
    pub proposal_id: String,
    /// Random nonce (hex, 128-bit). Secret: never logged.
    pub(crate) nonce: String,
    /// Unix epoch of approval.
    pub approved_unix: u64,
    /// SHA-256 binding digest of the approved proposal.
    pub(crate) binding: String,
    /// Consumed after one successful execute.
    pub(crate) consumed: bool,
}

impl std::fmt::Debug for ConsentToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConsentToken")
            .field("proposal_id", &self.proposal_id)
            .field("nonce", &"<redacted>")
            .field("approved_unix", &self.approved_unix)
            .field("binding", &self.binding)
            .field("consumed", &self.consumed)
            .finish()
    }
}

impl ConsentToken {
    /// Build a fresh token bound to `proposal` (crate-internal; prefer the consent log).
    pub(crate) fn issue(proposal: &LockdownProposal) -> std::io::Result<Self> {
        let mut n = [0u8; 16];
        random_bytes(&mut n)?;
        Ok(Self {
            proposal_id: proposal.id.clone(),
            nonce: hex(&n),
            approved_unix: now_unix(),
            binding: proposal.binding_digest(),
            consumed: false,
        })
    }

    /// Whether this token unlocks `proposal` (same id **and** the exact same
    /// commands / argv / flags that were sealed, and not yet consumed).
    pub fn matches(&self, proposal: &LockdownProposal) -> bool {
        !self.consumed
            && self.proposal_id == proposal.id
            && self.binding == proposal.binding_digest()
    }

    /// Whether the token has been used.
    pub fn is_consumed(&self) -> bool {
        self.consumed
    }

    /// Non-secret binding digest (safe to log; identifies what was sealed).
    pub fn binding(&self) -> &str {
        &self.binding
    }
}

/// Render one argv as a POSIX shell line (display == what runs).
///
/// Plain words (`[A-Za-z0-9_@%+=:,./-]`, plus a leading `~/`) are shown
/// bare; anything else is single-quoted with `'\''` escaping. `argv[0]`
/// containing `=` is quoted so it cannot read as an assignment.
pub fn render_argv(argv: &[String]) -> String {
    argv.iter()
        .enumerate()
        .map(|(i, a)| {
            if is_plain_word(a) && !(i == 0 && a.contains('=')) {
                a.clone()
            } else {
                format!("'{}'", a.replace('\'', r"'\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_plain_word(a: &str) -> bool {
    let rest = a.strip_prefix("~/").unwrap_or(a);
    !a.is_empty()
        && rest.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '_' | '@' | '%' | '+' | '=' | ':' | ',' | '.' | '/' | '-')
        })
}

/// Parse a legacy LOCAL SIMULATION line: exactly `true`, or
/// `echo '<text without single quotes>'`.
pub(crate) fn simple_sim_argv(line: &str) -> Option<Vec<String>> {
    let t = line.trim();
    if t == "true" {
        return Some(vec!["true".into()]);
    }
    let inner = t.strip_prefix("echo '")?.strip_suffix('\'')?;
    if inner.contains('\'') {
        return None;
    }
    Some(vec!["echo".into(), inner.into()])
}

/// Unique id suffix: `<unix secs>-<subsec nanos>-<process counter>-<random>`.
///
/// Two proposals created in the same second (or the same nanosecond) never
/// share an id.
pub(crate) fn unique_id_suffix() -> String {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let mut r = [0u8; 4];
    let rand = match random_bytes(&mut r) {
        Ok(()) => hex(&r),
        Err(_) => format!("p{}", std::process::id()),
    };
    format!("{}-{:09}-{seq}-{rand}", d.as_secs(), d.subsec_nanos())
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Fill `buf` from the OS CSPRNG.
#[cfg(unix)]
fn random_bytes(buf: &mut [u8]) -> std::io::Result<()> {
    use std::io::Read;
    std::fs::File::open("/dev/urandom")?.read_exact(buf)
}

/// Fill `buf` from std's OS-seeded SipHash keys (non-unix fallback).
#[cfg(not(unix))]
fn random_bytes(buf: &mut [u8]) -> std::io::Result<()> {
    use std::hash::{BuildHasher, Hasher};
    for chunk in buf.chunks_mut(8) {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(now_unix());
        let v = h.finish().to_le_bytes();
        chunk.copy_from_slice(&v[..chunk.len()]);
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// Expand a leading `~/` in a plain argv word to `$HOME` (what `sh` would
/// do for the displayed line). Quoted words are left literal.
fn expand_tilde(
    arg: &str,
    home: Option<&std::ffi::OsStr>,
) -> Result<std::ffi::OsString, LockdownError> {
    match arg.strip_prefix("~/") {
        Some(rest) if is_plain_word(arg) => {
            let home =
                home.ok_or_else(|| LockdownError::UnsafeCommand("HOME is not set".into()))?;
            let mut p = std::path::PathBuf::from(home);
            p.push(rest);
            Ok(p.into_os_string())
        }
        _ => Ok(arg.into()),
    }
}

/// Run the proposal's commands **only** with a matching, unconsumed token.
///
/// The token must match the proposal id **and** its binding digest (exact
/// commands, argv and flags at the time of the seal). Argv proposals spawn
/// each command directly (no shell); legacy REAL string proposals use
/// `sh -c`; LOCAL SIMULATION proposals only ever run `echo` / `true` argv.
/// On success the token is marked consumed. Without a matching token this
/// returns [`LockdownError::ConsentRequired`] and runs nothing.
///
/// [`LockdownProposal::proposal_only`] proposals consume the token and return
/// an empty output list without spawning (acknowledge / log only).
pub fn execute_with_token(
    proposal: &LockdownProposal,
    token: &mut ConsentToken,
) -> Result<Vec<std::process::Output>, LockdownError> {
    if token.proposal_id != proposal.id {
        return Err(LockdownError::ConsentRequired);
    }
    if token.consumed {
        return Err(LockdownError::TokenConsumed);
    }
    if token.binding != proposal.binding_digest() {
        return Err(LockdownError::ConsentRequired);
    }
    if proposal.proposal_only {
        token.consumed = true;
        return Ok(Vec::new());
    }
    // Validate the whole plan before spawning anything.
    let steps = proposal.execution_plan()?;
    let home = std::env::var_os("HOME");
    let mut cmds = Vec::with_capacity(steps.len());
    for step in &steps {
        let cmd = match step {
            Step::Argv(a) => {
                let mut c = std::process::Command::new(expand_tilde(&a[0], home.as_deref())?);
                for arg in &a[1..] {
                    c.arg(expand_tilde(arg, home.as_deref())?);
                }
                c
            }
            Step::Shell(line) => {
                let mut c = std::process::Command::new("sh");
                c.arg("-c").arg(line);
                c
            }
        };
        cmds.push(cmd);
    }
    let mut outs = Vec::with_capacity(cmds.len());
    for mut c in cmds {
        let out = c
            .output()
            .map_err(|e| LockdownError::Spawn(e.to_string()))?;
        outs.push(out);
    }
    token.consumed = true;
    Ok(outs)
}

/// Minimal dependency-free SHA-256 (FIPS 180-4) for consent binding.
mod sha256 {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    pub(super) fn digest(data: &[u8]) -> [u8; 32] {
        let mut h: [u32; 8] = [
            0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
            0x5be0cd19,
        ];
        let mut msg = data.to_vec();
        let bit_len = (data.len() as u64).wrapping_mul(8);
        msg.push(0x80);
        while msg.len() % 64 != 56 {
            msg.push(0);
        }
        msg.extend_from_slice(&bit_len.to_be_bytes());
        for block in msg.chunks_exact(64) {
            let mut w = [0u32; 64];
            for (i, word) in block.chunks_exact(4).enumerate() {
                w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
            }
            for i in 16..64 {
                let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
                let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
                w[i] = w[i - 16]
                    .wrapping_add(s0)
                    .wrapping_add(w[i - 7])
                    .wrapping_add(s1);
            }
            let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
            for i in 0..64 {
                let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
                let ch = (e & f) ^ (!e & g);
                let t1 = hh
                    .wrapping_add(s1)
                    .wrapping_add(ch)
                    .wrapping_add(K[i])
                    .wrapping_add(w[i]);
                let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
                let maj = (a & b) ^ (a & c) ^ (b & c);
                let t2 = s0.wrapping_add(maj);
                hh = g;
                g = f;
                f = e;
                e = d.wrapping_add(t1);
                d = c;
                c = b;
                b = a;
                a = t1.wrapping_add(t2);
            }
            for (x, y) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
                *x = x.wrapping_add(y);
            }
        }
        let mut out = [0u8; 32];
        for (i, v) in h.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tok(p: &LockdownProposal) -> ConsentToken {
        ConsentToken::issue(p).unwrap()
    }

    #[test]
    fn refuses_without_token() {
        let p = LockdownProposal::local_simulation_default();
        let other = LockdownProposal::local_simulation_default();
        let mut bad = tok(&other);
        assert_eq!(
            execute_with_token(&p, &mut bad).unwrap_err(),
            LockdownError::ConsentRequired
        );
    }

    #[test]
    fn runs_local_sim_with_matching_token() {
        let p = LockdownProposal::local_simulation_default();
        let mut tok = tok(&p);
        let outs = execute_with_token(&p, &mut tok).unwrap();
        assert_eq!(outs.len(), p.commands.len());
        assert!(tok.consumed);
        assert_eq!(
            execute_with_token(&p, &mut tok).unwrap_err(),
            LockdownError::TokenConsumed
        );
    }

    #[test]
    fn detail_block_labels_simulation() {
        let p = LockdownProposal::local_simulation_default();
        let d = p.detail_block();
        assert!(d.contains("LOCAL SIMULATION"));
        assert!(d.contains("$ echo"));
        assert!(d.contains("$ echo '[QuantumSec] LOCAL SIMULATION: lockdown begin'"));
    }

    #[test]
    fn proposal_only_skips_spawn() {
        let mut p = LockdownProposal::new(
            "Blue proposal",
            vec!["false".into()], // would fail if spawned
        );
        p.proposal_only = true;
        assert_eq!(p.seal_banner(), "PROPOSAL, not run");
        let mut tok = tok(&p);
        let outs = execute_with_token(&p, &mut tok).unwrap();
        assert!(outs.is_empty());
        assert!(tok.consumed);
    }

    #[test]
    fn sha256_known_vectors() {
        assert_eq!(
            hex(&sha256::digest(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex(&sha256::digest(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex(&sha256::digest(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn render_argv_quotes_untrusted_words() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            render_argv(&a(&["mkdir", "-p", "~/.config/x"])),
            "mkdir -p ~/.config/x"
        );
        assert_eq!(render_argv(&a(&["echo", "a b"])), "echo 'a b'");
        assert_eq!(render_argv(&a(&["echo", "it's"])), r"echo 'it'\''s'");
        assert_eq!(render_argv(&a(&["echo", ""])), "echo ''");
        assert_eq!(render_argv(&a(&["A=b", "c=d"])), "'A=b' c=d");
        assert_eq!(render_argv(&a(&["x", "~/a b"])), "x '~/a b'");
    }

    #[test]
    fn tilde_expands_only_plain_words() {
        let home = std::ffi::OsStr::new("/h");
        assert_eq!(expand_tilde("~/.config", Some(home)).unwrap(), "/h/.config");
        assert_eq!(expand_tilde("~/a b", Some(home)).unwrap(), "~/a b");
        assert_eq!(expand_tilde("/x/~/y", Some(home)).unwrap(), "/x/~/y");
        assert!(expand_tilde("~/x", None).is_err());
    }

    #[test]
    fn argv_display_mismatch_is_refused() {
        let mut p = LockdownProposal::from_argv("x", vec![vec!["echo".into(), "hi".into()]]);
        p.commands[0] = "echo hello".into();
        let mut t = tok(&p);
        assert!(matches!(
            execute_with_token(&p, &mut t),
            Err(LockdownError::UnsafeCommand(_))
        ));
        assert!(!t.consumed);
    }

    #[test]
    fn local_simulation_never_reaches_a_shell() {
        let mut p = LockdownProposal::new("sim", vec!["echo 'ok'; touch /tmp/never".into()]);
        p.local_simulation = true;
        let mut t = tok(&p);
        assert!(matches!(
            execute_with_token(&p, &mut t),
            Err(LockdownError::UnsafeCommand(_))
        ));
        // A typed non-echo argv is refused on a simulation card too.
        let mut p =
            LockdownProposal::from_argv("sim", vec![vec!["rm".into(), "-rf".into(), "/x".into()]]);
        p.local_simulation = true;
        let mut t = tok(&p);
        assert!(matches!(
            execute_with_token(&p, &mut t),
            Err(LockdownError::UnsafeCommand(_))
        ));
        // Plain legacy echo/true lines still run (as argv).
        let mut p =
            LockdownProposal::new("sim", vec!["echo '[QuantumSec] ok'".into(), "true".into()]);
        p.local_simulation = true;
        let mut t = tok(&p);
        let outs = execute_with_token(&p, &mut t).unwrap();
        assert_eq!(outs.len(), 2);
        assert_eq!(
            String::from_utf8_lossy(&outs[0].stdout),
            "[QuantumSec] ok\n"
        );
    }

    #[test]
    fn binding_covers_flags_and_commands() {
        let p = LockdownProposal::from_argv("x", vec![vec!["true".into()]]);
        let base = p.binding_digest();
        let mut q = p.clone();
        q.proposal_only = true;
        assert_ne!(q.binding_digest(), base);
        let mut q = p.clone();
        q.local_simulation = true;
        assert_ne!(q.binding_digest(), base);
        let mut q = p.clone();
        q.argv[0].push("x".into());
        assert_ne!(q.binding_digest(), base);
        let mut q = p.clone();
        q.summary.push('!');
        assert_ne!(q.binding_digest(), base);
        assert_eq!(p.clone().binding_digest(), base);
    }

    #[test]
    fn token_debug_redacts_nonce() {
        let p = LockdownProposal::local_simulation_default();
        let t = tok(&p);
        assert_eq!(t.nonce.len(), 32);
        let d = format!("{t:?}");
        assert!(!d.contains(&t.nonce));
        assert!(d.contains("<redacted>"));
    }

    #[test]
    fn ids_are_unique_in_a_burst() {
        let ids: std::collections::HashSet<_> = (0..500)
            .map(|_| LockdownProposal::new("x", vec![]).id)
            .collect();
        assert_eq!(ids.len(), 500);
    }
}
