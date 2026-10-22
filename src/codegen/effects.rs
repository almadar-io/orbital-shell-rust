//! Effect-runner code generation for the Rust shell.
//!
//! The generated runner delegates server-side effect execution to the shared
//! `Host` and exposes a typed interface that the generated
//! server can call.

use quote::quote;

/// Generate the `src/effects/mod.rs` source and the runtime-backed runner.
pub fn generate_effects_module() -> Vec<(String, String)> {
    let runner_src = quote! {
        use orbital_core::{runtime::ProcessEventResult, Value};
        use orbital_shell_rust::host::{Host, HostError};
        use std::sync::Arc;

        /// Abstraction over effect execution.
        pub trait EffectRunner: Send + Sync {
            /// Process an event, executing server-side effects and returning the
            /// runtime result (including client effects and fetched entity data).
            fn run_effects(
                &self,
                trait_name: &str,
                entity_id: &str,
                event: &str,
                payload: Value,
            ) -> Result<ProcessEventResult, HostError>;
        }

        /// Effect runner backed by the shared Orbital server runtime.
        pub struct RuntimeEffectRunner {
            runtime: Arc<Host>,
        }

        impl RuntimeEffectRunner {
            /// Wrap an existing `Host`.
            pub fn new(runtime: Arc<Host>) -> Self {
                Self { runtime }
            }
        }

        impl EffectRunner for RuntimeEffectRunner {
            fn run_effects(
                &self,
                trait_name: &str,
                entity_id: &str,
                event: &str,
                payload: Value,
            ) -> Result<ProcessEventResult, HostError> {
                self.runtime
                    .process_event(trait_name, entity_id, event, payload)
            }
        }
    }
    .to_string();

    let mod_rs = quote! {
        //! Generated effect runner that delegates execution to the Orbital
        //! server runtime.

        pub mod runner;
        pub use runner::{EffectRunner, RuntimeEffectRunner};
    }
    .to_string();

    vec![
        ("src/effects/runner.rs".to_string(), runner_src),
        ("src/effects/mod.rs".to_string(), mod_rs),
    ]
}
