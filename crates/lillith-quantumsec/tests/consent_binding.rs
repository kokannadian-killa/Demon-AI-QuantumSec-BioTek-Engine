//! Independent QA: consent-executor binding review (NON-EXECUTING).
//!
//! Nothing here runs `sh -c`, `install`, `systemctl` or any proposal command.
//! The tests only inspect the strings that would be handed to `sh -c`,
//! using the minimal POSIX-ish word splitter below, and the token-check logic
//! (`ConsentToken::matches`, plus `execute_with_token` on `proposal_only`
//! proposals only; that path returns before anything is spawned, see
//! `lockdown.rs:176-179`).
//!
//! History: the QA review (8a24d1a9) added `documents_current_*` tests that
//! asserted the vulnerable behaviour at 7d4472bc. The fix branch
//! (`qsec/consent-binding-fix`) flipped each one into a `fixed_*` regression
//! test covering the same case: typed argv with quoted display, approval bound
//! to a SHA-256 of the exact plan + flags, unique ids, a token without serde,
//! a random never-logged nonce, and no shell for LOCAL SIMULATION cards.
#![cfg(unix)]

use std::path::Path;

use lillith_quantumsec::consent::ConsentLog;
use lillith_quantumsec::lockdown::{
    execute_with_token, ConsentToken, LockdownError, LockdownProposal,
};
use lillith_quantumsec::{registry, report, teams, units};

// ---------------------------------------------------------- shell parsing

/// One parsed shell token.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    /// A word after quote removal.
    Word(String),
    /// A control operator (`;`, `&`, `|`, `&&`, `||`).
    Op(String),
}

/// Parse result plus "would the shell expand something" flags.
#[derive(Debug, Default)]
struct Parsed {
    toks: Vec<Tok>,
    /// An unquoted `#` at word start: the rest of the line is a comment.
    comment: Option<String>,
    /// Unquoted / double-quoted `$(`, or a backtick (command substitution).
    cmd_subst: bool,
}

/// A small POSIX-ish splitter, enough to show how `sh -c` would tokenise a
/// line: '..', "..", backslash, blanks, `;&|`, `#` comments and `$(` /
/// backtick detection. It is not a full shell parser.
fn sh_parse(s: &str) -> Parsed {
    let mut p = Parsed::default();
    let c: Vec<char> = s.chars().collect();
    let (mut i, mut cur, mut in_word) = (0usize, String::new(), false);
    let flush = |p: &mut Parsed, cur: &mut String, in_word: &mut bool| {
        if *in_word {
            p.toks.push(Tok::Word(std::mem::take(cur)));
            *in_word = false;
        }
    };
    while i < c.len() {
        let ch = c[i];
        match ch {
            ' ' | '\t' | '\n' => flush(&mut p, &mut cur, &mut in_word),
            '#' if !in_word => {
                p.comment = Some(c[i..].iter().collect());
                break;
            }
            ';' | '&' | '|' => {
                flush(&mut p, &mut cur, &mut in_word);
                let mut op = ch.to_string();
                if (ch == '&' || ch == '|') && c.get(i + 1) == Some(&ch) {
                    op.push(ch);
                    i += 1;
                }
                p.toks.push(Tok::Op(op));
            }
            '\'' => {
                in_word = true;
                i += 1;
                while i < c.len() && c[i] != '\'' {
                    cur.push(c[i]);
                    i += 1;
                }
            }
            '"' => {
                in_word = true;
                i += 1;
                while i < c.len() && c[i] != '"' {
                    if c[i] == '\\' && i + 1 < c.len() {
                        i += 1;
                    } else if c[i] == '`' || (c[i] == '$' && c.get(i + 1) == Some(&'(')) {
                        p.cmd_subst = true;
                    }
                    cur.push(c[i]);
                    i += 1;
                }
            }
            '\\' => {
                in_word = true;
                if i + 1 < c.len() {
                    i += 1;
                    cur.push(c[i]);
                }
            }
            _ => {
                if ch == '`' || (ch == '$' && c.get(i + 1) == Some(&'(')) {
                    p.cmd_subst = true;
                }
                in_word = true;
                cur.push(ch);
            }
        }
        i += 1;
    }
    flush(&mut p, &mut cur, &mut in_word);
    p
}

fn words(p: &Parsed) -> Vec<&str> {
    p.toks
        .iter()
        .filter_map(|t| match t {
            Tok::Word(w) => Some(w.as_str()),
            Tok::Op(_) => None,
        })
        .collect()
}

fn ops(p: &Parsed) -> Vec<&str> {
    p.toks
        .iter()
        .filter_map(|t| match t {
            Tok::Op(o) => Some(o.as_str()),
            Tok::Word(_) => None,
        })
        .collect()
}

#[test]
fn splitter_self_check() {
    let p = sh_parse("install -m 644 '/a b' ~/x");
    assert_eq!(words(&p), ["install", "-m", "644", "/a b", "~/x"]);
    let p = sh_parse(r#"echo "a;b" c\ d; true # tail"#);
    assert_eq!(words(&p), ["echo", "a;b", "c d", "true"]);
    assert_eq!(ops(&p), [";"]);
    assert_eq!(p.comment.as_deref(), Some("# tail"));
    assert!(sh_parse("echo $(id)").cmd_subst);
    assert!(!sh_parse("echo '$(id)'").cmd_subst);
}

/// The `install` line of the enable-unit proposal for `src`.
fn install_line(src: &str) -> String {
    let p = units::enable_quantumsec_unit_proposal(Path::new(src));
    p.commands
        .iter()
        .find(|c| c.starts_with("install "))
        .cloned()
        .expect("install line")
}

// --------------------------------------------- (i)/(ii) units.rs quoting

/// Baseline: the only path Settings passes today
/// (`tabs/quantumsec.rs:212`, `:566`) parses to the intended argv.
#[test]
fn fixed_settings_path_parses_to_intended_argv() {
    let l = install_line("/usr/lib/systemd/user/lillith-quantumsec.service");
    let p = sh_parse(&l);
    assert_eq!(
        words(&p),
        [
            "install",
            "-m",
            "644",
            "/usr/lib/systemd/user/lillith-quantumsec.service",
            "~/.config/systemd/user/lillith-quantumsec.service",
        ]
    );
    assert!(ops(&p).is_empty() && p.comment.is_none() && !p.cmd_subst);
}

/// (i) A source path with a space stays ONE operand: the typed argv keeps
/// it whole and the shown line quotes it (was units.rs:12-18).
#[test]
fn fixed_unit_path_with_space_stays_one_operand() {
    let l = install_line("/tmp/unit files/example.service");
    let w = sh_parse(&l);
    let w = words(&w);
    assert_eq!(w.len(), 5, "{l:?} -> {w:?}");
    assert_eq!(w[3], "/tmp/unit files/example.service");
    let p = units::enable_quantumsec_unit_proposal(Path::new("/tmp/unit files/example.service"));
    // Display == execution: the executed argv has the same single operand.
    assert_eq!(p.argv[1][3], "/tmp/unit files/example.service");
    assert!(p
        .detail_block()
        .contains("$ install -m 644 '/tmp/unit files/example.service' ~/.config"));
}

/// (ii) `;`, `#` and `$(..)` in the path stay literal: no extra command,
/// no comment, no substitution, and the destination operand survives.
#[test]
fn fixed_unit_path_metacharacters_stay_literal() {
    let l = install_line("/tmp/unit; printf REVIEW_ONLY; #");
    let p = sh_parse(&l);
    assert!(ops(&p).is_empty(), "{l:?}");
    assert!(p.comment.is_none(), "{l:?}");
    assert_eq!(
        words(&p),
        [
            "install",
            "-m",
            "644",
            "/tmp/unit; printf REVIEW_ONLY; #",
            "~/.config/systemd/user/lillith-quantumsec.service",
        ]
    );
    let prop =
        units::enable_quantumsec_unit_proposal(Path::new("/tmp/unit; printf REVIEW_ONLY; #"));
    assert_eq!(prop.argv[1][3], "/tmp/unit; printf REVIEW_ONLY; #");
    // And `$(..)` in a path is not command substitution.
    assert!(!sh_parse(&install_line("/tmp/$(printf x).service")).cmd_subst);
}

// ---------------------------------- (iii) token binds only to the proposal id

/// (iii) Approve P, then mutate its commands (and flags) but keep its id:
/// the token is bound to a SHA-256 of the exact plan + flags, so `matches`
/// and `execute_with_token` refuse. Uses `proposal_only` proposals, so
/// nothing spawns even on the accepted path.
#[test]
fn fixed_token_rejects_same_id_with_mutated_commands() {
    let dir = tempdir("iii");
    let log = ConsentLog::open_in(dir.join("consent")).unwrap();
    let mut approved = LockdownProposal::new("QA synthetic", vec!["true".into()]);
    approved.proposal_only = true;
    let mut tok = log.approve(&approved).unwrap();

    let mut mutated = approved.clone();
    mutated.commands = vec!["printf MUTATED_NOT_APPROVED".into()];
    mutated.summary = "different summary".into();
    assert_ne!(approved.detail_block(), mutated.detail_block());
    assert!(
        !tok.matches(&mutated),
        "token must be bound to the exact plan"
    );
    // The real gate refuses and does not consume the token.
    assert_eq!(
        execute_with_token(&mutated, &mut tok),
        Err(LockdownError::ConsentRequired)
    );
    assert!(!tok.is_consumed());
    // The unmodified approved proposal is still accepted (proposal_only => no spawn).
    assert!(tok.matches(&approved));
    assert_eq!(execute_with_token(&approved, &mut tok), Ok(Vec::new()));

    // Flags are bound too: a token approved for a "PROPOSAL, not run" card
    // does not match the same id with proposal_only=false (REAL COMMANDS).
    let mut tok2 = log.approve(&approved).unwrap();
    let mut real = approved.clone();
    real.proposal_only = false;
    assert_ne!(approved.seal_banner(), real.seal_banner());
    assert!(!tok2.matches(&real));
    // Refused before anything could spawn.
    assert_eq!(
        execute_with_token(&real, &mut tok2),
        Err(LockdownError::ConsentRequired)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ----------------------------------------- (iv) second-resolution ids

/// Calls `f` twice inside one wall-clock second (retries across a
/// second boundary).
fn same_second<T>(f: impl Fn() -> (T, u64)) -> (T, T) {
    loop {
        let (a, ua) = f();
        let (b, ub) = f();
        if ua == ub {
            return (a, b);
        }
    }
}

/// (iv) Two proposals created in the same second get distinct ids
/// (nanos + process counter + random), so a token for one never matches the
/// other.
#[test]
fn fixed_proposals_in_same_second_have_unique_ids() {
    let (a, b) = same_second(|| {
        let p = LockdownProposal::new("A", vec!["true".into()]);
        let u = p.proposed_unix;
        (p, u)
    });
    let b = LockdownProposal {
        commands: vec!["printf OTHER".into()],
        ..b
    };
    assert_ne!(a.id, b.id);
    let dir = tempdir("iv");
    let log = ConsentLog::open_in(dir.join("consent")).unwrap();
    let tok = log.approve(&a).unwrap();
    assert!(!tok.matches(&b), "approval of A must not unlock B");
    // The same holds for the unit factory and the local sim default.
    let (u1, u2) = same_second(|| {
        let p = units::enable_quantumsec_unit_proposal(Path::new("/x"));
        let u = p.proposed_unix;
        (p, u)
    });
    assert_ne!(u1.id, u2.id);
    let (s1, s2) = same_second(|| {
        let p = LockdownProposal::local_simulation_default();
        let u = p.proposed_unix;
        (p, u)
    });
    assert_ne!(s1.id, s2.id);
    let _ = std::fs::remove_dir_all(&dir);
}

// --------------------------------------------- extra review findings

/// The token is opaque, created only by `ConsentLog::approve`. It no longer
/// derives `Deserialize` (or `Serialize`), so it cannot be forged from JSON
/// or leaked into data. Checked at compile time via inherent-const probing.
#[test]
fn fixed_consent_token_not_forgeable_via_serde() {
    use std::marker::PhantomData;
    trait Fallback {
        const DESERIALIZE: bool = false;
        const SERIALIZE: bool = false;
    }
    struct ProbeDe<T>(PhantomData<T>);
    impl<T> Fallback for ProbeDe<T> {}
    impl<T: serde::de::DeserializeOwned> ProbeDe<T> {
        #[allow(dead_code)]
        const DESERIALIZE: bool = true;
    }
    struct ProbeSer<T>(PhantomData<T>);
    impl<T> Fallback for ProbeSer<T> {}
    impl<T: serde::Serialize> ProbeSer<T> {
        #[allow(dead_code)]
        const SERIALIZE: bool = true;
    }
    // Probe self-check on a type that does implement both.
    assert!(std::hint::black_box(
        ProbeDe::<LockdownProposal>::DESERIALIZE
    ));
    assert!(std::hint::black_box(
        ProbeSer::<LockdownProposal>::SERIALIZE
    ));
    assert!(!std::hint::black_box(ProbeDe::<ConsentToken>::DESERIALIZE));
    assert!(!std::hint::black_box(ProbeSer::<ConsentToken>::SERIALIZE));
}

/// The nonce is 128 bits from the OS CSPRNG: two approvals in the same
/// second on one thread differ, and the nonce is never written to the
/// consent log or shown by `Debug` (only the non-secret binding digest is).
#[test]
fn fixed_nonce_is_random_and_never_logged() {
    let dir = tempdir("nonce");
    let log = ConsentLog::open_in(dir.join("consent")).unwrap();
    let p = LockdownProposal::new("QA synthetic", vec!["true".into()]);
    let (a, b) = same_second(|| {
        let t = log.approve(&p).unwrap();
        let u = t.approved_unix;
        (t, u)
    });
    // Same proposal, same second, same thread: tokens differ only by nonce.
    assert_eq!(a.proposal_id, b.proposal_id);
    assert_eq!(a.approved_unix, b.approved_unix);
    assert_eq!(a.binding(), b.binding());
    assert_ne!(a, b, "nonces must differ");
    assert!(format!("{a:?}").contains("<redacted>"));
    // No record carries a nonce; each carries the binding digest.
    let recs = log.list_recent(1000).unwrap();
    assert!(!recs.is_empty());
    for r in &recs {
        assert!(r.token_nonce.is_empty(), "{r:?}");
        assert_eq!(r.binding, a.binding());
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// "LOCAL SIMULATION" cards are typed `echo` argv and never reach `sh -c`.
/// Interpolated data containing a `'` (registry `deploy_command` from
/// `~/.biotek/bots/registry.json`; teams `title` from
/// `~/.biotek/teams/catalog.json`) stays one literal echo argument, and the
/// shown line quotes it so it cannot break out either.
#[test]
fn fixed_local_simulation_echo_cannot_break_out() {
    let mut bot = registry::default_registry().remove(0);
    bot.deploy_command = "x'; printf BREAKOUT; echo '".into();
    let p = bot.deploy_proposal(Path::new("/tmp/qa-biotek"));
    assert!(p.local_simulation);
    assert_eq!(p.seal_banner(), "LOCAL SIMULATION ONLY");
    let i = p
        .commands
        .iter()
        .position(|c| c.contains("BREAKOUT"))
        .unwrap();
    let line = &p.commands[i];
    let parsed = sh_parse(line);
    assert!(ops(&parsed).is_empty(), "{line:?}");
    assert!(!words(&parsed).contains(&"printf"), "{line:?}");
    assert_eq!(words(&parsed).len(), 2, "{line:?}");
    assert_eq!(p.argv[i][0], "echo");
    assert!(p.argv[i][1].ends_with("x'; printf BREAKOUT; echo '"));
    assert!(p.argv.iter().all(|a| a[0] == "echo" || a[0] == "true"));

    let mut item = teams::default_catalog()
        .into_iter()
        .find(|i| i.local_simulation)
        .unwrap();
    item.title = "t'; printf BREAKOUT; echo '".into();
    let p = item.as_proposal();
    assert_eq!(p.seal_banner(), "LOCAL SIMULATION ONLY");
    let parsed = sh_parse(&p.commands[0]);
    assert!(!words(&parsed).contains(&"printf"), "{:?}", p.commands[0]);
    assert!(ops(&parsed).is_empty(), "{:?}", p.commands[0]);
    assert_eq!(p.argv[0][0], "echo");
    assert!(p.argv[0][1].ends_with("t'; printf BREAKOUT; echo '"));
    // A tampered simulation line (not plain echo/true) is refused before
    // any spawn, even with a valid approval. The `proposal_only=false`
    // simulation path is the one that would otherwise reach a shell.
    let mut bad = LockdownProposal::new("QA sim", vec!["echo 'a'; printf BREAKOUT".into()]);
    bad.local_simulation = true;
    let dir = tempdir("sim");
    let log = ConsentLog::open_in(dir.join("consent")).unwrap();
    let mut tok = log.approve(&bad).unwrap();
    assert!(matches!(
        execute_with_token(&bad, &mut tok),
        Err(LockdownError::UnsafeCommand(_))
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The Fish proposal echoes each helper line as ONE typed argv argument and
/// the shown line quotes it correctly, so the Biotek home path (from
/// `$BIOTEK_HOME`, `$AETHER_HOME` or `~`) is never split or substituted.
#[test]
fn fixed_fish_proposal_keeps_home_quoted() {
    let prof = report::DeviceProfile {
        hostname: "qa".into(),
        username: "qa".into(),
        os: "linux".into(),
        arch: "x86_64".into(),
        cpu_cores: 1,
        total_memory_mb: 1,
        biotek_home: "/tmp/qa home/$(printf X)".into(),
        generated_unix: 0,
    };
    let p = prof.fish_apply_proposal();
    assert!(p.local_simulation);
    let i = p
        .commands
        .iter()
        .position(|c| c.contains("BIOTEK_HOME"))
        .unwrap();
    let line = &p.commands[i];
    let parsed = sh_parse(line);
    assert!(!parsed.cmd_subst, "{line:?}");
    assert!(ops(&parsed).is_empty(), "{line:?}");
    assert_eq!(
        words(&parsed),
        [
            "echo",
            "[QuantumSec] FISH PROPOSAL: set -gx BIOTEK_HOME '/tmp/qa home/$(printf X)'"
        ],
        "{line:?}"
    );
    assert_eq!(
        p.argv[i],
        [
            "echo",
            "[QuantumSec] FISH PROPOSAL: set -gx BIOTEK_HOME '/tmp/qa home/$(printf X)'"
        ]
    );
}

fn tempdir(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let d = std::env::temp_dir().join(format!("qa-consent-{tag}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}
