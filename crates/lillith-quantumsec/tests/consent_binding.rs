//! Independent QA: consent-executor binding review (NON-EXECUTING).
//!
//! Nothing here runs `sh -c`, `install`, `systemctl` or any proposal command.
//! The tests only inspect the strings that would be handed to `sh -c`,
//! using the minimal POSIX-ish word splitter below, and the token-check logic
//! (`ConsentToken::matches`, plus `execute_with_token` on `proposal_only`
//! proposals only; that path returns before anything is spawned, see
//! `lockdown.rs:176-179`).
//!
//! Convention: every `documents_current_*` test asserts the CURRENT
//! (vulnerable) behaviour at 7d4472bc, so it passes today. When the
//! production code is fixed (quoted operands or typed argv, approval bound to
//! the exact execution plan, a unique id, a non-forgeable token), the test
//! fails. At that point flip it into a regression test for the fix. These are
//! review findings, not confirmed exploits; reachability is in the QA report.
#![cfg(unix)]

use std::path::Path;

use lillith_quantumsec::consent::ConsentLog;
use lillith_quantumsec::lockdown::{execute_with_token, ConsentToken, LockdownProposal};
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

/// (i) A source path with a space becomes two operands (units.rs:12-18).
#[test]
fn documents_current_unit_path_with_space_splits_into_two_operands() {
    let l = install_line("/tmp/unit files/example.service");
    let w = sh_parse(&l);
    let w = words(&w);
    assert_eq!(w.len(), 6, "{l:?} -> {w:?}");
    assert_eq!(&w[3..5], ["/tmp/unit", "files/example.service"]);
    // The banner shows the same text, so display == execution, but the
    // shell meaning differs from the displayed "one path" intent.
    assert!(units::enable_quantumsec_unit_proposal(Path::new("/tmp/unit files/example.service"))
        .detail_block()
        .contains("$ install -m 644 /tmp/unit files/example.service ~/.config"));
}

/// (ii) `;` and `#` change the shell meaning: a second command appears and
/// the destination operand is commented out.
#[test]
fn documents_current_unit_path_metacharacters_change_shell_meaning() {
    let l = install_line("/tmp/unit; printf REVIEW_ONLY; #");
    let p = sh_parse(&l);
    assert_eq!(ops(&p), [";", ";"], "{l:?}");
    assert_eq!(words(&p), ["install", "-m", "644", "/tmp/unit", "printf", "REVIEW_ONLY"]);
    assert!(p
        .comment
        .as_deref()
        .is_some_and(|c| c.contains("~/.config/systemd/user/lillith-quantumsec.service")));
    // And `$(..)` in a path would be command substitution.
    assert!(sh_parse(&install_line("/tmp/$(printf x).service")).cmd_subst);
}

// ---------------------------------- (iii) token binds only to the proposal id

/// (iii) Approve P, then mutate its commands (and flags) but keep its id:
/// the token check accepts it. `matches` and `execute_with_token` compare
/// only `proposal_id` + `consumed` (lockdown.rs:136-138, 170-175). Uses the
/// `proposal_only` early return (lockdown.rs:176-179), so nothing spawns.
#[test]
fn documents_current_token_accepts_same_id_with_mutated_commands() {
    let dir = tempdir("iii");
    let log = ConsentLog::open_in(dir.join("consent")).unwrap();
    let mut approved = LockdownProposal::new("QA synthetic", vec!["true".into()]);
    approved.proposal_only = true;
    let mut tok = log.approve(&approved).unwrap();

    let mut mutated = approved.clone();
    mutated.commands = vec!["printf MUTATED_NOT_APPROVED".into()];
    mutated.summary = "different summary".into();
    assert_ne!(approved.detail_block(), mutated.detail_block());
    assert!(tok.matches(&mutated), "token bound to bytes? (fixed)");
    // The real gate accepts it. proposal_only => no spawn.
    assert_eq!(execute_with_token(&mutated, &mut tok), Ok(Vec::new()));

    // Flags are not bound either: a token approved for a "PROPOSAL, not run"
    // card also matches the same id with proposal_only=false (REAL COMMANDS).
    let tok2 = log.approve(&approved).unwrap();
    let mut real = approved.clone();
    real.proposal_only = false;
    assert_ne!(approved.seal_banner(), real.seal_banner());
    assert!(tok2.matches(&real));
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

/// (iv) Two proposals created in the same second share an id
/// (lockdown.rs:70 `lockdown-{now_unix}`), so a token for one matches the
/// other.
#[test]
fn documents_current_proposals_in_same_second_collide_on_id() {
    let (a, b) = same_second(|| {
        let p = LockdownProposal::new("A", vec!["true".into()]);
        let u = p.proposed_unix;
        (p, u)
    });
    let b = LockdownProposal {
        commands: vec!["printf OTHER".into()],
        ..b
    };
    assert_eq!(a.id, b.id);
    let dir = tempdir("iv");
    let log = ConsentLog::open_in(dir.join("consent")).unwrap();
    let tok = log.approve(&a).unwrap();
    assert!(tok.matches(&b), "approval of A unlocks B");
    // The same holds for the unit factory (units.rs:22) and the local sim
    // default (lockdown.rs:50).
    let (u1, u2) = same_second(|| {
        let p = units::enable_quantumsec_unit_proposal(Path::new("/x"));
        let u = p.proposed_unix;
        (p, u)
    });
    assert_eq!(u1.id, u2.id);
    let _ = std::fs::remove_dir_all(&dir);
}

// --------------------------------------------- extra review findings

/// The token is meant to be opaque, "created only by ConsentLog::approve"
/// (lockdown.rs:111). It derives `Deserialize` with a private `consumed`
/// field (lockdown.rs:112-122), so any caller can forge one from JSON
/// without approving anything.
#[test]
fn documents_current_consent_token_forgeable_via_serde() {
    let p = LockdownProposal::new("QA synthetic", vec!["true".into()]);
    let json = format!(
        r#"{{"proposal_id":"{}","nonce":"0","approved_unix":0,"consumed":false}}"#,
        p.id
    );
    let forged: ConsentToken = serde_json::from_str(&json).unwrap();
    assert!(forged.matches(&p));
}

/// The nonce is a SipHash (fixed keys) of the second, thread id and pid
/// (lockdown.rs:148-156). Two approvals in the same second on one thread
/// get the same nonce. It is not random, and it is written to the consent
/// log (consent.rs:99).
#[test]
fn documents_current_nonce_is_deterministic_within_a_second() {
    let dir = tempdir("nonce");
    let log = ConsentLog::open_in(dir.join("consent")).unwrap();
    let p = LockdownProposal::new("QA synthetic", vec!["true".into()]);
    let (a, b) = same_second(|| {
        let t = log.approve(&p).unwrap();
        let u = t.approved_unix;
        (t, u)
    });
    assert_eq!(a.nonce, b.nonce);
    let _ = std::fs::remove_dir_all(&dir);
}

/// "LOCAL SIMULATION" cards still go to `sh -c`
/// (`execute_with_token` ignores `local_simulation`, lockdown.rs:176-188).
/// They are safe only while every line stays a harmless `echo '..'`.
/// Interpolated data that contains a `'` escapes the echo:
/// registry.rs:98-102 (`deploy_command`, from same-user
/// `~/.biotek/bots/registry.json`, registry.rs:168-173) and teams.rs:110-114
/// (`title`, from `~/.biotek/teams/catalog.json`, teams.rs:204-209).
#[test]
fn documents_current_local_simulation_echo_breakout() {
    let mut bot = registry::default_registry().remove(0);
    bot.deploy_command = "x'; printf BREAKOUT; echo '".into();
    let p = bot.deploy_proposal(Path::new("/tmp/qa-biotek"));
    assert!(p.local_simulation);
    assert_eq!(p.seal_banner(), "LOCAL SIMULATION ONLY");
    let line = p.commands.iter().find(|c| c.contains("BREAKOUT")).unwrap();
    let parsed = sh_parse(line);
    assert!(ops(&parsed).contains(&";"), "{line:?}");
    assert!(words(&parsed).contains(&"printf"), "{line:?}");

    let mut item = teams::default_catalog()
        .into_iter()
        .find(|i| i.local_simulation)
        .unwrap();
    item.title = "t'; printf BREAKOUT; echo '".into();
    let p = item.as_proposal();
    assert_eq!(p.seal_banner(), "LOCAL SIMULATION ONLY");
    let parsed = sh_parse(&p.commands[0]);
    assert!(words(&parsed).contains(&"printf"), "{:?}", p.commands[0]);
}

/// report.rs:134 wraps a line that is already single-quoted
/// (`shell_single_quote`, report.rs:59-62, 172-174) in another `echo '..'`.
/// The nesting puts the Biotek home path OUTSIDE quotes. That path comes from
/// `$BIOTEK_HOME`, `$AETHER_HOME` or `~` (lillith-setup biotek.rs:54-71),
/// so a space splits it and `$(..)` would be substituted.
#[test]
fn documents_current_fish_proposal_nested_quotes_unquote_home() {
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
    let line = p.commands.iter().find(|c| c.contains("BIOTEK_HOME")).unwrap();
    let parsed = sh_parse(line);
    assert!(parsed.cmd_subst, "{line:?}");
    assert!(words(&parsed).iter().any(|w| *w == "home/$(printf"), "{line:?} -> {:?}", words(&parsed));
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
