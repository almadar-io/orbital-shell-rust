//! The one native program host: a program booted on the kernel with the native ports it asks for, the host `orb run`,
//! `cli-*` programs, `orb mcp --program`, the Node and Python add-ons and the generated Rust app all run on.

pub mod boot;
#[path = "../../browser/src/lib.rs"]
pub mod cdp;
pub mod background;
#[path = "../../../runtime/src/run.rs"]
pub mod run;
#[path = "../../../runtime/src/error.rs"]
mod error;
#[path = "../../../runtime/src/runtime.rs"]
mod runtime;
pub mod store;

pub use error::HostError;
pub use runtime::{
    EntitySummary, Host, HostConfig, OrbitalDetail, OrbitalInfo, OriginGuard, ProcessEventResult, RuntimeStats,
    SubstratePort, TraceAsk, TraceEffect, TraceEmit, TraceEntry, TraceTransition,
};
