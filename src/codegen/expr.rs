//! OIR expression lowering for generated Rust code.

use orbital_core::ir::{OirBinding, OirBindingRoot, OirExpr};
use proc_macro2::TokenStream;
use quote::quote;

/// Attempt to compile an OIR guard expression into native Rust `bool` code.
///
/// Supported subset:
/// - Logical `and`/`or`/`not`.
/// - Equality/inequality between an `@entity`/`@payload` binding path and a
///   string/number/bool/null literal (uses dynamic `Value` comparison).
/// - Numeric comparison (`<`, `<=`, `>`, `>=`) between a binding path and a
///   number literal.
/// - Boolean literals.
///
/// Returns `None` for unsupported constructs; callers should fall back to the
/// runtime `Evaluator`.
pub fn compile_guard(expr: &OirExpr) -> Option<TokenStream> {
    match expr {
        OirExpr::Bool { value } => Some(quote!(#value)),

        OirExpr::UnaryOp { op, operand } => {
            let operand = compile_guard(operand)?;
            match op {
                orbital_core::ir::OirUnaryOp::Not => Some(quote!(!#operand)),
                orbital_core::ir::OirUnaryOp::Neg => Some(quote!(-#operand)),
            }
        }

        OirExpr::BinaryOp { op, left, right } => match op {
            orbital_core::ir::OirBinaryOp::And => {
                let l = compile_guard(left)?;
                let r = compile_guard(right)?;
                Some(quote!((#l && #r)))
            }
            orbital_core::ir::OirBinaryOp::Or => {
                let l = compile_guard(left)?;
                let r = compile_guard(right)?;
                Some(quote!((#l || #r)))
            }
            orbital_core::ir::OirBinaryOp::Eq => compile_equality(left, right, true),
            orbital_core::ir::OirBinaryOp::Ne => compile_equality(left, right, false),
            orbital_core::ir::OirBinaryOp::Lt => compile_comparison(left, right, quote!(<)),
            orbital_core::ir::OirBinaryOp::Le => compile_comparison(left, right, quote!(<=)),
            orbital_core::ir::OirBinaryOp::Gt => compile_comparison(left, right, quote!(>)),
            orbital_core::ir::OirBinaryOp::Ge => compile_comparison(left, right, quote!(>=)),
            _ => None,
        },

        _ => None,
    }
}

fn compile_equality(left: &OirExpr, right: &OirExpr, is_eq: bool) -> Option<TokenStream> {
    let (binding, literal) = if let Some(binding) = as_binding(left) {
        (binding_to_value_guard(binding)?, value_literal(right)?)
    } else if let Some(binding) = as_binding(right) {
        (binding_to_value_guard(binding)?, value_literal(left)?)
    } else {
        return None;
    };

    if is_eq {
        Some(quote! {
            #binding.map(|v| v == &#literal).unwrap_or(false)
        })
    } else {
        Some(quote! {
            #binding.map(|v| v != &#literal).unwrap_or(false)
        })
    }
}

fn compile_comparison(left: &OirExpr, right: &OirExpr, op: TokenStream) -> Option<TokenStream> {
    if let (Some(binding), Some(n)) = (as_binding(left), as_number(right)) {
        let value = binding_to_value_guard(binding)?;
        Some(quote! {
            #value.and_then(|v| v.as_number()).map(|v| v #op #n).unwrap_or(false)
        })
    } else if let (Some(n), Some(binding)) = (as_number(left), as_binding(right)) {
        let value = binding_to_value_guard(binding)?;
        Some(quote! {
            #value.and_then(|v| v.as_number()).map(|v| #n #op v).unwrap_or(false)
        })
    } else {
        None
    }
}

fn as_binding(expr: &OirExpr) -> Option<&OirBinding> {
    match expr {
        OirExpr::Binding { binding } => Some(binding),
        _ => None,
    }
}

fn as_number(expr: &OirExpr) -> Option<f64> {
    match expr {
        OirExpr::Number { value } => Some(*value),
        _ => None,
    }
}

/// Lower a supported binding to an `Option<&orbital_core::Value>` expression.
fn binding_to_value_guard(binding: &OirBinding) -> Option<TokenStream> {
    let var = match &binding.root {
        OirBindingRoot::Entity => quote!(entity),
        OirBindingRoot::Payload => quote!(payload),
        _ => return None,
    };

    if binding.path.is_empty() {
        // `entity`/`payload` handler params are already `&Value`; an extra `&`
        // would yield `Option<&&Value>` and break the `&Value == &Value` compare.
        return Some(quote!(Some(#var)));
    }

    let path: Vec<_> = binding.path.iter().map(|s| s.as_str()).collect();
    Some(quote! {
        crate::traits::value_get_path(#var, &[#(#path),*])
    })
}

/// Build an `orbital_core::Value` literal token stream from an `OirValue`.
fn value_literal(value: &OirExpr) -> Option<TokenStream> {
    match value {
        OirExpr::Null => Some(quote!(orbital_core::Value::Null)),
        OirExpr::Bool { value } => Some(quote!(orbital_core::Value::Bool(#value))),
        OirExpr::Number { value } => Some(quote!(orbital_core::Value::Number(#value))),
        OirExpr::String { value } => Some(quote!(orbital_core::Value::String(#value.to_string()))),
        _ => None,
    }
}

/// Compile a guard if possible; otherwise emit a runtime evaluator fallback.
///
/// The fallback assumes the surrounding scope has `state`, `payload`, and
/// `entity` in scope, where `state` is the generated state enum.
pub fn compile_guard_with_fallback(expr: &OirExpr) -> TokenStream {
    if let Some(tokens) = compile_guard(expr) {
        return tokens;
    }

    let json = match serde_json::to_string(expr) {
        Ok(s) => s,
        Err(_) => return quote!(false),
    };

    quote! {
        {
            let guard_json = #json;
            let guard_expr: orbital_core::ir::OirExpr =
                serde_json::from_str(guard_json).expect("failed to deserialize guard");
            let ctx = orbital_core::evaluator::BindingContextBuilder::new()
                .state(orbital_core::Value::String(state.to_string()))
                .payload(payload.clone())
                .entity(entity.clone())
                .build();
            orbital_core::evaluator::Evaluator::new()
                .evaluate_guard_oir(&guard_expr, &ctx)
                .unwrap_or(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbital_core::ir::{OirBinaryOp, OirBinding, OirBindingRoot, OirExpr};

    fn eq(left: OirExpr, right: OirExpr) -> OirExpr {
        OirExpr::BinaryOp {
            op: OirBinaryOp::Eq,
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    fn entity_root() -> OirExpr {
        OirExpr::Binding {
            binding: OirBinding::new(OirBindingRoot::Entity, vec![]),
        }
    }

    // RS-3: a whole-entity binding must lower to `Some(entity)` (Option<&Value>),
    // not `Some(&entity)` (Option<&&Value>) — else `v == &literal` fails to compile.
    #[test]
    fn whole_entity_eq_has_no_double_ref() {
        let expr = eq(
            entity_root(),
            OirExpr::String {
                value: "x".to_string(),
            },
        );
        let ts = compile_guard(&expr)
            .expect("guard should compile")
            .to_string();
        assert!(
            ts.contains("Some (entity)"),
            "expected Some(entity), got: {ts}"
        );
        assert!(
            !ts.contains("Some (& entity)"),
            "double-ref regressed: {ts}"
        );
    }

    // A sub-field binding still goes through the helper path (no double-ref either).
    #[test]
    fn sub_field_eq_uses_value_get_path() {
        let expr = eq(
            OirExpr::Binding {
                binding: OirBinding::new(OirBindingRoot::Entity, vec!["status".to_string()]),
            },
            OirExpr::String {
                value: "done".to_string(),
            },
        );
        let ts = compile_guard(&expr)
            .expect("guard should compile")
            .to_string();
        assert!(
            ts.contains("value_get_path"),
            "expected value_get_path, got: {ts}"
        );
    }

    fn binding(root: OirBindingRoot, path: &[&str]) -> OirExpr {
        OirExpr::Binding {
            binding: OirBinding::new(root, path.iter().map(|s| s.to_string()).collect()),
        }
    }

    fn num(n: f64) -> OirExpr {
        OirExpr::Number { value: n }
    }

    fn str_lit(s: &str) -> OirExpr {
        OirExpr::String {
            value: s.to_string(),
        }
    }

    fn cmp(op: OirBinaryOp, left: OirExpr, right: OirExpr) -> OirExpr {
        OirExpr::BinaryOp {
            op,
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    // Smoke test: every emitted guard must be syntactically valid Rust. Wrapping
    // in a fn with the real handler param types catches malformed codegen (the
    // class that produced RS-3) across binding/operator shapes.
    #[test]
    fn emitted_guards_are_valid_rust() {
        use OirBinaryOp::*;
        let cases = vec![
            eq(binding(OirBindingRoot::Entity, &[]), str_lit("x")),
            eq(binding(OirBindingRoot::Payload, &[]), OirExpr::Null),
            eq(
                binding(OirBindingRoot::Entity, &["status"]),
                str_lit("done"),
            ),
            cmp(Ne, binding(OirBindingRoot::Payload, &["count"]), num(0.0)),
            cmp(Lt, binding(OirBindingRoot::Entity, &["age"]), num(18.0)),
            cmp(Ge, binding(OirBindingRoot::Entity, &["score"]), num(100.0)),
            OirExpr::BinaryOp {
                op: And,
                left: Box::new(eq(binding(OirBindingRoot::Entity, &["a"]), str_lit("1"))),
                right: Box::new(cmp(Gt, binding(OirBindingRoot::Entity, &["b"]), num(2.0))),
            },
            OirExpr::UnaryOp {
                op: orbital_core::ir::OirUnaryOp::Not,
                operand: Box::new(eq(
                    binding(OirBindingRoot::Entity, &["done"]),
                    OirExpr::Bool { value: true },
                )),
            },
        ];

        for expr in cases {
            let guard = compile_guard(&expr).expect("guard should compile");
            let wrapped = quote::quote! {
                fn _probe(
                    state: &str,
                    payload: &orbital_core::Value,
                    entity: &orbital_core::Value,
                ) -> bool {
                    #guard
                }
            };
            syn::parse2::<syn::ItemFn>(wrapped.clone())
                .unwrap_or_else(|e| panic!("emitted guard is not valid Rust: {e}\n{wrapped}"));
        }
    }
}
