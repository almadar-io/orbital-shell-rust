//! Service client code generation for the Rust shell.

use orbital_core::schema::{ServiceDefinition, ServiceRef};
use orbital_core::str_utils::to_pascal_case;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

/// Generate the `src/services/mod.rs` source for declared services.
pub fn generate_services_module(services: &[ServiceRef]) -> TokenStream {
    let mut items = TokenStream::new();
    let mut names: Vec<String> = Vec::new();

    for service_ref in services {
        let Some(def) = service_ref.as_inline() else {
            continue;
        };

        let name = def.name();
        names.push(name.to_string());

        match def {
            ServiceDefinition::Rest(rest) => {
                let struct_name = format_ident!("{}Client", to_pascal_case(name));
                let base_url = &rest.base_url;
                let doc = rest
                    .description
                    .as_deref()
                    .map(|d| quote!(#[doc = #d]))
                    .unwrap_or_else(|| {
                        let doc = format!("REST client for `{}`.", name);
                        quote!(#[doc = #doc])
                    });

                items.extend(quote! {
                    #doc
                    #[derive(Debug, Clone)]
                    pub struct #struct_name {
                        pub base_url: String,
                    }

                    impl #struct_name {
                        /// Create a new client with the configured base URL.
                        pub fn new() -> Self {
                            Self {
                                base_url: #base_url.to_string(),
                            }
                        }

                        /// Service name.
                        pub fn name(&self) -> &'static str {
                            #name
                        }
                    }

                    impl Default for #struct_name {
                        fn default() -> Self {
                            Self::new()
                        }
                    }
                });
            }
            ServiceDefinition::Socket(socket) => {
                let struct_name = format_ident!("{}Client", to_pascal_case(name));
                let url = &socket.url;
                let doc = socket
                    .description
                    .as_deref()
                    .map(|d| quote!(#[doc = #d]))
                    .unwrap_or_else(|| {
                        let doc = format!("Socket client for `{}`.", name);
                        quote!(#[doc = #doc])
                    });

                items.extend(quote! {
                    #doc
                    #[derive(Debug, Clone)]
                    pub struct #struct_name {
                        pub url: String,
                    }

                    impl #struct_name {
                        /// Create a new client with the configured URL.
                        pub fn new() -> Self {
                            Self {
                                url: #url.to_string(),
                            }
                        }

                        /// Service name.
                        pub fn name(&self) -> &'static str {
                            #name
                        }
                    }

                    impl Default for #struct_name {
                        fn default() -> Self {
                            Self::new()
                        }
                    }
                });
            }
            ServiceDefinition::Mcp(mcp) => {
                let struct_name = format_ident!("{}Client", to_pascal_case(name));
                let server_path = &mcp.server_path;
                let doc = mcp
                    .description
                    .as_deref()
                    .map(|d| quote!(#[doc = #d]))
                    .unwrap_or_else(|| {
                        let doc = format!("MCP client for `{}`.", name);
                        quote!(#[doc = #doc])
                    });

                items.extend(quote! {
                    #doc
                    #[derive(Debug, Clone)]
                    pub struct #struct_name {
                        pub server_path: String,
                    }

                    impl #struct_name {
                        /// Create a new client with the configured server path.
                        pub fn new() -> Self {
                            Self {
                                server_path: #server_path.to_string(),
                            }
                        }

                        /// Service name.
                        pub fn name(&self) -> &'static str {
                            #name
                        }
                    }

                    impl Default for #struct_name {
                        fn default() -> Self {
                            Self::new()
                        }
                    }
                });
            }
        }
    }

    let name_literals: Vec<TokenStream> = names.iter().map(|n| quote!(#n)).collect();

    items.extend(quote! {
        /// Return the names of all services declared in the schema.
        pub fn declared_services() -> &'static [&'static str] {
            &[#(#name_literals),*]
        }
    });

    quote! {
        //! Generated service clients.

        #items
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbital_core::schema::{RestServiceDefInner, ServiceDefinition};

    #[test]
    fn test_generate_rest_service() {
        let service = ServiceDefinition::Rest(RestServiceDefInner {
            name: "openweather".to_string(),
            description: Some("Weather API".to_string()),
            base_url: "https://api.example.com".to_string(),
            headers: None,
            auth: None,
            timeout: None,
        });
        let tokens = generate_services_module(&[ServiceRef::Inline(service)]);
        let code = tokens.to_string();
        assert!(code.contains("pub struct OpenweatherClient"));
        assert!(code.contains("pub fn declared_services"));
        assert!(code.contains("\"openweather\""));
    }
}
