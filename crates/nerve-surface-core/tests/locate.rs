//! `locate.rs` — finding the hub binary (spec T4 · acceptance A3).
//!
//! The rule under test is the *order*: install directories first, then a
//! `cargo install`, then whatever the user's shell would have found. First hit
//! wins, and not finding it is a fail-open state rather than an error (spec
//! Open questions 1).
//!
//! That rule is the same on every platform, so the bulk of this file injects
//! its own directory list through `with_fixed_directories` and never names a
//! real one. Two small tests at the end pin what each platform actually
//! searches; they are the only part that knows an OS.
//!
//! Determinism: a fake probe over a literal path set, an injected home, no
//! filesystem, no `PATH`, no clock, no socket.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use nerve_surface_core::locate::{BINARY, BinaryProbe, HubLocator, platform_fixed_directories};

const HOME: &str = "/Users/tester";

/// The two injected install directories, standing in for whatever a platform
/// really uses.
fn install_dirs() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/install/first"),
        PathBuf::from("/install/second"),
    ]
}

fn first() -> PathBuf {
    PathBuf::from("/install/first").join(BINARY)
}

fn second() -> PathBuf {
    PathBuf::from("/install/second").join(BINARY)
}

fn cargo() -> PathBuf {
    PathBuf::from(HOME).join(".cargo/bin").join(BINARY)
}

fn from_path() -> PathBuf {
    PathBuf::from("/Users/tester/bin").join(BINARY)
}

/// Every path the locator probed, in order — shared with the test after the
/// probe has been moved into the locator.
type Asked = Rc<RefCell<Vec<PathBuf>>>;

/// A filesystem that exists only in this file.
struct FakeProbe {
    executable: Vec<PathBuf>,
    on_path: Option<PathBuf>,
    asked: Asked,
}

impl FakeProbe {
    fn new(executable: &[PathBuf], on_path: Option<PathBuf>) -> (Self, Asked) {
        let asked: Asked = Rc::new(RefCell::new(Vec::new()));
        let probe = Self {
            executable: executable.to_vec(),
            on_path,
            asked: Rc::clone(&asked),
        };
        (probe, asked)
    }
}

impl BinaryProbe for FakeProbe {
    fn is_executable(&self, path: &Path) -> bool {
        self.asked.borrow_mut().push(path.to_path_buf());
        self.executable.iter().any(|known| known == path)
    }

    fn on_path(&self, name: &str) -> Option<PathBuf> {
        assert_eq!(name, BINARY, "PATH is searched for the hub binary only");
        self.on_path.clone()
    }
}

fn locate(executable: &[PathBuf], on_path: Option<PathBuf>) -> Option<PathBuf> {
    let (probe, _) = FakeProbe::new(executable, on_path);
    HubLocator::with_fixed_directories(probe, HOME, install_dirs()).locate()
}

fn probed(executable: &[PathBuf], on_path: Option<PathBuf>) -> Vec<PathBuf> {
    let (probe, asked) = FakeProbe::new(executable, on_path);
    let _ = HubLocator::with_fixed_directories(probe, HOME, install_dirs()).locate();

    asked.borrow().clone()
}

// ── Discovery order ─────────────────────────────────────────────────────────

/// Acceptance A3: only `PATH` answers, so `PATH` is what the surface uses.
#[test]
fn test_path_is_the_last_resort() {
    let found = locate(&[], Some(from_path()));

    assert_eq!(found, Some(from_path()));
}

#[test]
fn test_nothing_installed_locates_nothing() {
    assert_eq!(locate(&[], None), None);
}

// ── How the search asks ─────────────────────────────────────────────────────

#[test]
fn test_the_candidates_are_probed_in_order() {
    let asked = probed(&[], None);

    assert_eq!(asked, vec![first(), second(), cargo()]);
}

/// A hit short-circuits: a locator that already found the first hub must not
/// go on touching the filesystem.
#[test]
fn test_a_hit_stops_the_search() {
    let asked = probed(&[first(), second(), cargo()], Some(from_path()));

    assert_eq!(asked, vec![first()]);
}

// ── What each platform really searches ──────────────────────────────────────

#[cfg(unix)]
#[test]
fn test_unix_searches_both_homebrew_prefixes_in_order() {
    assert_eq!(
        platform_fixed_directories(),
        vec![
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
        ],
        "Apple silicon Homebrew first, Intel Homebrew second"
    );
}
