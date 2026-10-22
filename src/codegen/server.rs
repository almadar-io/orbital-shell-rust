//! The retired Rust HTTP adapter remains an explicit compile failure until the std server adapter lands.

use orbital_core::ir::OirModule;
use quote::quote;

pub fn generate_server_files(module: &OirModule) -> Vec<(String, String)> {
    let doc = format!("HTTP adapter for {}.", module.name);
    let server = quote! {
        #![doc = #doc]
        compile_error!("NATIVE_SERVER_UNAVAILABLE: the generated Rust HTTP adapter retired with orbital-server; the std server adapter is pending. Use the standalone terminal renderer for kernel execution.");
    }.to_string();
    let main = quote! {
        mod server;
        fn main() {}
    }.to_string();
    vec![("src/server.rs".to_string(), server), ("src/main.rs".to_string(), main)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_codegen_reports_the_retired_adapter_explicitly() {
        let module = OirModule::new("fixture", "1.0.0");
        let files = generate_server_files(&module);
        assert!(files[0].1.contains("compile_error"));
        assert!(files[0].1.contains("NATIVE_SERVER_UNAVAILABLE"));
        for (_, source) in files {
            assert!(!source.contains("orbital_server"));
            assert!(!source.contains("create_router"));
        }
    }
}
