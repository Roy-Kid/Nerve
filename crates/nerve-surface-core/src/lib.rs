//! Everything a Nerve surface does that is not drawing.
//!
//! The macOS menu bar, the tmux sidebar, the VS Code extension and the Windows
//! tray are peers: each consumes `GET /v1/jobs` and `GET /v1/stream`, none owns
//! state, none knows the others (CLAUDE.md invariant 7). What they share is
//! everything *between* the socket and the pixels — decoding a frame, deciding
//! what a job's status is, counting, filtering, finding and starting a hub — and
//! that is this crate.
//!
//! It deliberately has no terminal library, no GUI toolkit and no async
//! runtime. Every port is blocking and pull-based so the surface owns
//! scheduling, and every derivation is a pure function a test can call without
//! an OS. That is what lets the same status rules be verified on a Mac and run
//! on Windows.

pub mod display;
pub mod filter;
pub mod frame;
pub mod group;
pub mod hub;
pub mod instance;
pub mod jobpath;
pub mod launch;
pub mod locate;
pub mod log;
pub mod machine;
pub mod notify;
pub mod palette;
pub mod ribbon;
pub mod sort;
pub mod status;
pub mod store;
pub mod stream;
pub mod tally;
#[cfg(feature = "testkit")]
pub mod testkit;
