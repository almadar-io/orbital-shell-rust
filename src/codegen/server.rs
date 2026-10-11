//! Server wiring code generation for the Rust shell.
//!
//! Generates the Axum application bootstrap. The generated server reuses the
//! route handlers in `orbital_server` so event processing, state queries, and
//! health checks work out of the box.

use orbital_core::ir::OirModule;
use quote::quote;

/// Generate `src/server.rs` and `src/main.rs` for the generated application.
pub fn generate_server_files(module: &OirModule) -> Vec<(String, String)> {
    let app_name = &module.name;
    let server_rs = generate_server_rs(app_name);
    let main_rs = generate_main_rs();

    vec![
        ("src/server.rs".to_string(), server_rs),
        ("src/main.rs".to_string(), main_rs),
    ]
}

fn generate_server_rs(app_name: &str) -> String {
    let doc = format!("Axum server wiring for {}.", app_name);
    let tokens = quote! {
        #![doc = #doc]

        use anyhow::Context;
        use axum::Router;
        use orbital_server::{create_router, InMemoryAdapter, OrbitalServerRuntime};
        use std::path::Path;
        use std::sync::Arc;

        /// Create the Axum application for this generated project.
        ///
        /// The schema is loaded from `resolved.orb` in the current directory, mock
        /// data is seeded, and the resulting router is returned ready to serve.
        pub fn create_app() -> anyhow::Result<Router> {
            let schema_path = Path::new("resolved.orb");
            let schema = orbital_core::schema::parse_schema_file(schema_path)
                .with_context(|| format!("failed to load schema from {}", schema_path.display()))?;

            let persistence = Arc::new(InMemoryAdapter::new());
            let runtime = Arc::new(OrbitalServerRuntime::new(persistence));
            runtime
                .register_schema(schema.clone())
                .context("failed to register schema with runtime")?;

            // Seed sample data via the one canonical seeder so the first INIT fetch
            // returns rows (instance-less entities only; authored instances are kept).
            runtime.seed_sample(&schema, 3);

            Ok(create_router(runtime))
        }
    };
    tokens.to_string()
}

fn generate_main_rs() -> String {
    quote! {
        mod server;

        use std::net::SocketAddr;

        fn main() -> anyhow::Result<()> {
            let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
            let port = std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3030);

            let app = server::create_app()?;
            let addr: SocketAddr = format!("{}:{}", host, port).parse()?;

            println!("Server listening on http://{}", addr);
            println!("Press Ctrl+C to stop");

            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let listener = tokio::net::TcpListener::bind(addr).await?;
                axum::serve(listener, app).await?;
                Ok::<(), anyhow::Error>(())
            })?;

            Ok(())
        }
    }
    .to_string()
}
