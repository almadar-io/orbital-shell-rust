//! State-machine code generation for the Rust shell.

use crate::codegen::expr::compile_guard_with_fallback;
use orbital_core::ir::{OirState, OirTrait, OirTransitionFrom};
use orbital_core::str_utils::{to_pascal_case, to_snake_case};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

/// Generate the `src/traits/mod.rs` source and per-trait module tokens.
///
/// Returns a vec of `(file_path, content)` pairs for each trait module plus the
/// `mod.rs` entry.
pub fn generate_traits(traits: &[OirTrait]) -> Vec<(String, String)> {
    let mut files = Vec::new();
    let mut reexports = TokenStream::new();

    for trait_def in traits {
        let module_name = to_snake_case(&trait_def.name);
        let struct_name = format_ident!("{}State", to_pascal_case(&trait_def.name));
        let event_name = format_ident!("{}Event", to_pascal_case(&trait_def.name));
        let outcome_name = format_ident!("{}Outcome", to_pascal_case(&trait_def.name));
        let handler_name = format_ident!("handle_{}_event", to_snake_case(&trait_def.name));

        let state_enum = generate_state_enum(&struct_name, &trait_def.states);
        let event_enum = generate_event_enum(&event_name, &trait_def.events);
        let outcome_struct = generate_outcome_struct(&outcome_name);
        let handler_fn = generate_handler_fn(
            &handler_name,
            &struct_name,
            &event_name,
            &outcome_name,
            trait_def,
        );

        let doc = format!("Generated state machine for {}.", trait_def.name);
        let module_src = quote! {
            #![doc = #doc]

            use orbital_core::Value;

            #state_enum

            #event_enum

            #outcome_struct

            #handler_fn
        }
        .to_string();

        files.push((format!("src/traits/{}.rs", module_name), module_src));

        let mod_ident = format_ident!("{}", module_name);
        reexports.extend(quote! {
            pub mod #mod_ident;
        });
    }

    let mod_rs = quote! {
        //! Generated trait state machines and dynamic-value helpers used by
        //! native guard expressions.

        use orbital_core::Value;

        #reexports

        /// Look up a single key in a `Value` object.
        pub fn value_get<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
            value.as_object().and_then(|m| m.get(key))
        }

        /// Walk a dotted path through a `Value` object.
        pub fn value_get_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
            let mut current = value;
            for key in path {
                current = value_get(current, key)?;
            }
            Some(current)
        }
    }
    .to_string();

    files.push(("src/traits/mod.rs".to_string(), mod_rs));
    files
}

fn generate_state_enum(name: &proc_macro2::Ident, states: &[OirState]) -> TokenStream {
    let variants: Vec<_> = states
        .iter()
        .map(|s| {
            let v = format_ident!("{}", to_pascal_case(&s.name));
            quote!(#v)
        })
        .collect();

    let as_str_arms = states.iter().map(|s| {
        let v = format_ident!("{}", to_pascal_case(&s.name));
        let n = &s.name;
        quote! {
            Self::#v => #n
        }
    });

    quote! {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum #name {
            #(#variants,) *
        }

        impl #name {
            pub fn as_str(&self) -> &'static str {
                match self {
                    #(#as_str_arms,) *
                }
            }
        }

        impl std::fmt::Display for #name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    }
}

fn generate_event_enum(
    name: &proc_macro2::Ident,
    events: &[orbital_core::ir::OirEvent],
) -> TokenStream {
    let variants: Vec<_> = events
        .iter()
        .map(|e| {
            let v = format_ident!("{}", to_pascal_case(&e.key));
            quote!(#v)
        })
        .collect();

    let as_str_arms = events.iter().map(|e| {
        let v = format_ident!("{}", to_pascal_case(&e.key));
        let k = &e.key;
        quote! {
            Self::#v => #k
        }
    });

    quote! {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum #name {
            #(#variants,) *
        }

        impl #name {
            pub fn as_str(&self) -> &'static str {
                match self {
                    #(#as_str_arms,) *
                }
            }
        }
    }
}

fn generate_outcome_struct(name: &proc_macro2::Ident) -> TokenStream {
    quote! {
        #[derive(Debug, Clone)]
        pub struct #name {
            pub new_state: String,
            pub entity_data: Option<Value>,
            pub client_effects: Vec<Value>,
        }
    }
}

fn generate_handler_fn(
    handler_name: &proc_macro2::Ident,
    state_name: &proc_macro2::Ident,
    event_name: &proc_macro2::Ident,
    outcome_name: &proc_macro2::Ident,
    trait_def: &OirTrait,
) -> TokenStream {
    let match_arms = generate_match_arms(state_name, event_name, trait_def);

    quote! {
        pub fn #handler_name(
            state: #state_name,
            event: #event_name,
            payload: &Value,
            entity: &Value,
        ) -> Result<#outcome_name, String> {
            let _ = (payload, entity);
            let next_state = match (state, event) {
                #(#match_arms) *
                _ => {
                    return Ok(#outcome_name {
                        new_state: state.to_string(),
                        entity_data: None,
                        client_effects: vec![],
                    });
                }
            };

            Ok(#outcome_name {
                new_state: next_state.to_string(),
                entity_data: None,
                client_effects: vec![],
            })
        }
    }
}

fn generate_match_arms(
    state_name: &proc_macro2::Ident,
    event_name: &proc_macro2::Ident,
    trait_def: &OirTrait,
) -> Vec<TokenStream> {
    let mut arms = Vec::new();

    // Guarded transitions first so they are tried before unguarded transitions
    // for the same (state, event) pair.
    let mut transitions: Vec<_> = trait_def.transitions.iter().collect();
    transitions.sort_by_key(|t| t.guard.is_none());

    for transition in transitions {
        let event_variant = format_ident!("{}", to_pascal_case(&transition.event));
        let target_variant = format_ident!("{}", to_pascal_case(&transition.to));
        let guard = transition
            .guard
            .as_ref()
            .map(compile_guard_with_fallback)
            .map(|g| quote!(if #g));

        match &transition.from {
            OirTransitionFrom::Single(from) => {
                let from_variant = format_ident!("{}", to_pascal_case(from));
                arms.push(quote! {
                    (#state_name::#from_variant, #event_name::#event_variant) #guard => #state_name::#target_variant,
                });
            }
            OirTransitionFrom::Multiple(froms) => {
                for from in froms {
                    let from_variant = format_ident!("{}", to_pascal_case(from));
                    arms.push(quote! {
                        (#state_name::#from_variant, #event_name::#event_variant) #guard => #state_name::#target_variant,
                    });
                }
            }
            OirTransitionFrom::Any => {
                if transition.guard.is_some() {
                    // Need `state` in scope for the guard expression.
                    arms.push(quote! {
                        (state, #event_name::#event_variant) #guard => #state_name::#target_variant,
                    });
                } else {
                    arms.push(quote! {
                        (_, #event_name::#event_variant) => #state_name::#target_variant,
                    });
                }
            }
        }
    }

    arms
}
