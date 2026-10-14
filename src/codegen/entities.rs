//! Entity code generation for the Rust shell.

use orbital_core::ir::{
    classify_type, render_type, OirEntity, OirField, OirType, TargetTypeRenderer, TypeMappingClass,
};
use orbital_core::str_utils::{split_to_pascal_case, to_pascal_case, to_snake_case};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

/// Generate the `src/entities/mod.rs` source for a set of entities.
pub fn generate_entities_module(
    entities: &[OirEntity],
    named_types: &std::collections::BTreeMap<String, OirType>,
) -> TokenStream {
    let mut items = TokenStream::new();

    // Named-type declarations (resolver-registered recursive aliases +
    // Phase-3D nominalized anonymous shapes) hoist first — every entity
    // struct below references them by bare name (`type_ref`), mirroring the
    // TS shell's `emit_named_type_declarations`.
    let file_hoisted = std::cell::RefCell::new(Vec::new());
    for (name, ty) in named_types {
        render_type(
            &RustRenderer {
                entity_name: "",
                field_name: "",
                context_name: name.clone(),
                hoisted: &file_hoisted,
            },
            ty,
            None,
        );
    }
    let file_hoisted_decls = file_hoisted.into_inner();
    items.extend(quote! { #(#file_hoisted_decls)* });

    for entity in entities {
        // Emit any inline enum types for enum-valued fields.
        for field in &entity.fields {
            if let Some(values) = &field.enum_values {
                if !values.is_empty() {
                    let enum_name = format_ident!(
                        "{}{}",
                        split_to_pascal_case(&entity.name),
                        to_pascal_case(&field.name)
                    );
                    let variants = values.iter().map(|v| {
                        let variant = format_ident!("{}", to_pascal_case(v));
                        quote! {
                            #[serde(rename = #v)]
                            #variant
                        }
                    });
                    items.extend(quote! {
                        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
                        #[serde(rename_all = "snake_case")]
                        pub enum #enum_name {
                            #(#variants,)*
                        }
                    });
                }
            }
        }

        let struct_name = format_ident!("{}", split_to_pascal_case(&entity.name));
        let doc = entity
            .description
            .as_deref()
            .map(|d| quote!(#[doc = #d]))
            .unwrap_or_else(|| {
                let doc = format!("{} entity.", entity.name);
                quote!(#[doc = #doc])
            });

        let hoisted = std::cell::RefCell::new(Vec::new());
        let field_tokens: Vec<TokenStream> = entity
            .fields
            .iter()
            .filter(|f| f.name != "id")
            .map(|f| generate_field(f, &entity.name, &hoisted))
            .collect();

        let id_field = quote! {
            /// Entity identifier.
            pub id: String,
        };

        // Struct/union shapes hoist as named types before the entity struct.
        let hoisted_items = hoisted.into_inner();
        items.extend(quote! { #(#hoisted_items)* });

        items.extend(quote! {
            #doc
            #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
            #[serde(rename_all = "camelCase")]
            pub struct #struct_name {
                #id_field
                #(#field_tokens)*
            }
        });
    }

    quote! {
        //! Generated entity types.

        use serde::{Deserialize, Serialize};

        #items
    }
}

/// Generate a struct field declaration from an OIR field.
fn generate_field(
    field: &OirField,
    entity_name: &str,
    hoisted: &std::cell::RefCell<Vec<TokenStream>>,
) -> TokenStream {
    let name = format_ident!("{}", to_snake_case(&field.name));
    let doc = field
        .description
        .as_deref()
        .map(|d| quote!(#[doc = #d]))
        .unwrap_or_default();

    let (ty, is_optional) = field.effective_type();
    let rust_type = map_type_to_rust(
        ty,
        entity_name,
        &field.name,
        field.enum_values.as_deref(),
        hoisted,
    );

    if is_optional {
        quote! {
            #doc
            pub #name: Option<#rust_type>,
        }
    } else {
        quote! {
            #doc
            pub #name: #rust_type,
        }
    }
}

/// The Rust leaf-renderer behind the shared `orbital_core` walker
/// ([`render_type`]). `type Out = TokenStream` so the type reference composes
/// via `quote!` with no stringify/re-parse. Context lives on `self`:
/// `entity_name`/`field_name` synthesize the field enum's `{Entity}{Field}`
/// ident (the enum *definitions* stay in `generate_entities_module`); the
/// growing `context_name` names a `Struct`/`Union`'s named type
/// (`{Entity}{Field}{Nested}…` — positional, so siblings never collide), whose
/// declaration is HOISTED into `hoisted` (emitted before the entity struct)
/// and referenced by bare name. Nested fields drop the `enum_values` sidecar:
/// a nested format-union has no field-layer enum definition to reference, so
/// it renders as `String` rather than a wrong `{Entity}{Field}` ident.
struct RustRenderer<'a> {
    entity_name: &'a str,
    field_name: &'a str,
    context_name: String,
    hoisted: &'a std::cell::RefCell<Vec<TokenStream>>,
}

impl RustRenderer<'_> {
    /// A field enum lowers to `String`/`Enum` + the `enum_values` sidecar; the
    /// reference is the `{PascalEntity}{PascalField}` ident whose `pub enum` is
    /// emitted alongside. An empty/absent sidecar falls back to `String`.
    fn enum_ident_or_string(&self, enum_values: Option<&[String]>) -> TokenStream {
        if let Some(values) = enum_values {
            if !values.is_empty() {
                let enum_name = format_ident!(
                    "{}{}",
                    split_to_pascal_case(self.entity_name),
                    to_pascal_case(self.field_name)
                );
                return quote!(#enum_name);
            }
        }
        quote!(String)
    }
}

impl TargetTypeRenderer for RustRenderer<'_> {
    type Out = TokenStream;

    fn string(&self, enum_values: Option<&[String]>) -> TokenStream {
        self.enum_ident_or_string(enum_values)
    }
    fn number(&self) -> TokenStream {
        quote!(f64)
    }
    fn integer(&self) -> TokenStream {
        quote!(i64)
    }
    fn boolean(&self) -> TokenStream {
        quote!(bool)
    }
    fn date(&self) -> TokenStream {
        quote!(String)
    }
    fn json(&self) -> TokenStream {
        quote!(serde_json::Value)
    }
    fn enum_ty(&self, _name: &str, enum_values: Option<&[String]>) -> TokenStream {
        self.enum_ident_or_string(enum_values)
    }
    fn trait_ref(&self) -> TokenStream {
        quote!(String)
    }
    fn sexpr(&self) -> TokenStream {
        quote!(serde_json::Value)
    }
    fn type_ref(&self, name: &str) -> TokenStream {
        // The named type hoists at the top of `src/entities/mod.rs` from
        // `OirModule::named_types` (resolver-registered recursive aliases AND
        // Phase-3D nominalized anonymous shapes) — the field references it by
        // bare name, mirroring the TS shell (types.rs `type_ref`). The
        // declaration is genuinely recursive (a recursive field inside it
        // renders `TypeRef` again → the same bare name).
        let name = format_ident!("{}", name);
        quote!(#name)
    }
    fn row(&self, entity: &str) -> TokenStream {
        let name = format_ident!("{}", split_to_pascal_case(entity));
        quote!(#name)
    }
    fn array(&self, _inner_ty: &OirType, inner: TokenStream) -> TokenStream {
        quote!(Vec<#inner>)
    }
    fn map(
        &self,
        key_ty: &OirType,
        _key: TokenStream,
        _value_ty: &OirType,
        value: TokenStream,
    ) -> TokenStream {
        // Only String-serializable keys map to a real `HashMap`; anything else
        // (the rendered key is discarded) collapses to a dynamic JSON value.
        match key_ty {
            OirType::String | OirType::Integer | OirType::Number => {
                quote!(std::collections::HashMap<String, #value>)
            }
            _ => quote!(serde_json::Value),
        }
    }
    fn optional(&self, _inner_ty: &OirType, inner: TokenStream) -> TokenStream {
        quote!(Option<#inner>)
    }
    fn struct_ty(&self, fields: &[OirField]) -> TokenStream {
        let name = format_ident!("{}", self.context_name);
        let field_tokens: Vec<TokenStream> = fields
            .iter()
            .map(|f| {
                let child = RustRenderer {
                    entity_name: self.entity_name,
                    field_name: self.field_name,
                    context_name: format!("{}{}", self.context_name, to_pascal_case(&f.name)),
                    hoisted: self.hoisted,
                };
                let ty = render_type(&child, &f.field_type, None);
                let fname = format_ident!("{}", to_snake_case(&f.name));
                quote! { pub #fname: #ty, }
            })
            .collect();
        self.hoisted.borrow_mut().push(quote! {
            #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
            #[serde(rename_all = "camelCase")]
            pub struct #name { #(#field_tokens)* }
        });
        quote!(#name)
    }
    fn union_ty(&self, variants: &[OirField]) -> TokenStream {
        if variants.is_empty() {
            return self.json();
        }
        let name = format_ident!("{}", self.context_name);
        // A named enum over the variant structs. `untagged` because the
        // discriminator field lives INSIDE each variant's own shape (resolved
        // there as a literal-typed struct field) — serde's `tag = "…"` would
        // need a tag field name this arm doesn't own; untagged keeps every
        // variant's structure with the wire shape unchanged.
        let variant_tokens: Vec<TokenStream> = variants
            .iter()
            .map(|v| {
                let child = RustRenderer {
                    entity_name: self.entity_name,
                    field_name: self.field_name,
                    context_name: format!("{}{}", self.context_name, to_pascal_case(&v.name)),
                    hoisted: self.hoisted,
                };
                let ty = render_type(&child, &v.field_type, None);
                let vident = format_ident!("{}", to_pascal_case(&v.name));
                quote! { #vident(#ty), }
            })
            .collect();
        self.hoisted.borrow_mut().push(quote! {
            #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
            #[serde(untagged)]
            pub enum #name { #(#variant_tokens)* }
        });
        quote!(#name)
    }

    // Classifications per `docs/Orbital_Type_Chain_Table.md` (Rust column)
    // as amended by Phase 3: Json→`serde_json::Value` is the one FAITHFUL
    // json arm (the carrier is exact); Number→f64 and the String-carried
    // domains/date are Widening; Struct/Union/File are Faithful (named
    // hoisted `pub struct` / `enum` — declared + referenced by name, not
    // erased); TypeRef is the bare hoisted name
    // — Faithful (Phase 3D: the recursion-bearing declaration is hoisted from
    // `named_types` at the top of the entities module, same as TS, so neither
    // the name nor the recursion is lost); SExpr is the PERMANENT deliberate
    // Lossy (config/knob-only,
    // no entity-data consequence — gaps §130 item 4).
    fn classify_string(&self, _enum_values: Option<&[String]>) -> TypeMappingClass {
        TypeMappingClass::Faithful
    }
    fn classify_number(&self) -> TypeMappingClass {
        TypeMappingClass::Widening
    }
    fn classify_integer(&self) -> TypeMappingClass {
        TypeMappingClass::Faithful
    }
    fn classify_boolean(&self) -> TypeMappingClass {
        TypeMappingClass::Faithful
    }
    fn classify_date(&self) -> TypeMappingClass {
        TypeMappingClass::Widening
    }
    fn classify_json(&self) -> TypeMappingClass {
        TypeMappingClass::Faithful
    }
    fn classify_node(&self) -> TypeMappingClass {
        // The json carrier preserves the pattern tree's STRUCTURE and erases
        // only the pattern TYPE — the same structure-preserved justification
        // behind the Widening class in Kotlin/Swift/Python. Rust keeps Lossy
        // for this arm per the Phase-3 disposition: a typed PatternNode
        // would ride on named_types, and until that lands the node arm
        // stays recorded as structure-erased.
        TypeMappingClass::Lossy
    }
    fn classify_row(&self, _entity: &str) -> TypeMappingClass {
        // Pair decision with `row`: the entity's own struct carries the full shape.
        TypeMappingClass::Faithful
    }
    fn classify_type_ref(&self, _name: &str) -> TypeMappingClass {
        // Pair decision with `type_ref`: the bare name's declaration is
        // hoisted from `named_types` at the top of the entities module — the
        // recursion survives — Faithful (Phase 3D; same flip TS always had).
        TypeMappingClass::Faithful
    }
    fn classify_union_ty(&self, variants: &[OirField]) -> TypeMappingClass {
        // A named enum over the hoisted variant structs carries every
        // variant's structure — Faithful (the pair decision with
        // `union_ty`); an empty union still degrades to the json carrier.
        if variants.is_empty() {
            return self.classify_json();
        }
        TypeMappingClass::Faithful
    }
    fn classify_enum_ty(&self, _name: &str, _enum_values: Option<&[String]>) -> TypeMappingClass {
        TypeMappingClass::Faithful
    }
    fn classify_trait_ref(&self) -> TypeMappingClass {
        TypeMappingClass::Widening
    }
    fn classify_sexpr(&self) -> TypeMappingClass {
        // PERMANENT deliberate Lossy — sexprs are config/knob-only, never
        // entity data, so the erasure has no at-rest consequence (§130).
        TypeMappingClass::Lossy
    }
    fn classify_array(&self, _inner_ty: &OirType, inner: TypeMappingClass) -> TypeMappingClass {
        inner
    }
    fn classify_map(
        &self,
        key_ty: &OirType,
        _key: TypeMappingClass,
        _value_ty: &OirType,
        value: TypeMappingClass,
    ) -> TypeMappingClass {
        // Mirrors `map` above: String keys keep the value's class; Integer/
        // Number keys are coerced to `String` (Widening at best); any other
        // key collapses the whole map to `serde_json::Value` — Lossy.
        match key_ty {
            OirType::String => value,
            OirType::Integer | OirType::Number => TypeMappingClass::Widening.meet(value),
            _ => TypeMappingClass::Lossy,
        }
    }
    fn classify_optional(&self, _inner_ty: &OirType, inner: TypeMappingClass) -> TypeMappingClass {
        inner
    }
    fn classify_struct_ty(&self, _fields: &[OirField]) -> TypeMappingClass {
        TypeMappingClass::Faithful
    }
}

/// Classify an `OirType` under the Rust renderer's faithfulness table
/// (Phase-1 metadata; the classification never alters rendered output).
/// Entity/field names matter only to the enum sidecar's generated ident, so
/// the classification uses placeholder names.
pub fn classify_oir_type(ty: &OirType) -> TypeMappingClass {
    let hoisted = std::cell::RefCell::new(Vec::new());
    classify_type(
        &RustRenderer {
            entity_name: "Entity",
            field_name: "field",
            context_name: "EntityField".to_string(),
            hoisted: &hoisted,
        },
        ty,
        None,
    )
}

/// Map an OIR type to a Rust type token stream via the shared walker.
fn map_type_to_rust(
    ty: &OirType,
    entity_name: &str,
    field_name: &str,
    enum_values: Option<&[String]>,
    hoisted: &std::cell::RefCell<Vec<TokenStream>>,
) -> TokenStream {
    render_type(
        &RustRenderer {
            entity_name,
            field_name,
            context_name: format!(
                "{}{}",
                split_to_pascal_case(entity_name),
                to_pascal_case(field_name)
            ),
            hoisted,
        },
        ty,
        enum_values,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbital_core::ir::{OirEntity, OirField, OirType};

    #[test]
    fn test_generate_simple_entity() {
        let mut entity = OirEntity::new("User");
        entity.fields.push(OirField::new("email", OirType::String));
        entity.fields.push(OirField::new("age", OirType::Integer));

        let tokens = generate_entities_module(&[entity], &Default::default());
        let code = tokens.to_string();
        assert!(code.contains("pub struct User"));
        assert!(code.contains("pub email : String"));
        assert!(code.contains("pub age : i64"));
    }

    #[test]
    fn a_row_renders_the_entity_struct_where_a_reference_renders_an_id() {
        let mut note = OirEntity::new("Note");
        note.fields.push(OirField::new(
            "revisions",
            OirType::array(OirType::row("Revision")),
        ));
        note.fields.push(OirField::new(
            "tagIds",
            OirType::array(OirType::reference("Tag")),
        ));
        let code =
            generate_entities_module(&[note, OirEntity::new("Revision")], &Default::default())
                .to_string();
        assert!(code.contains("pub revisions : Vec < Revision >"), "{code}");
        assert!(code.contains("pub struct Revision"), "{code}");
        assert!(code.contains("pub tag_ids : Vec < String >"), "{code}");
    }

    #[test]
    fn a_camel_case_entity_name_keeps_its_case_in_the_struct_and_every_reference() {
        let mut note = OirEntity::new("Note");
        note.fields.push(OirField::new(
            "revisions",
            OirType::array(OirType::row("NoteRevision")),
        ));
        let mut revision = OirEntity::new("NoteRevision");
        let mut kind = OirField::new("kind", OirType::String);
        kind.enum_values = Some(vec!["auto".to_string()]);
        revision.fields.push(kind);
        let code = generate_entities_module(&[note, revision], &Default::default()).to_string();
        assert!(code.contains("pub struct NoteRevision"), "{code}");
        assert!(
            code.contains("pub revisions : Vec < NoteRevision >"),
            "{code}"
        );
        assert!(code.contains("pub enum NoteRevisionKind"), "{code}");
        assert!(!code.contains("Noterevision"), "{code}");
    }

    #[test]
    fn control_a_snake_case_entity_name_still_becomes_pascal() {
        let code = generate_entities_module(&[OirEntity::new("task_item")], &Default::default())
            .to_string();
        assert!(code.contains("pub struct TaskItem"), "{code}");
    }

    #[test]
    fn test_generate_enum_field() {
        let mut entity = OirEntity::new("Task");
        let mut field = OirField::new("status", OirType::String);
        field.enum_values = Some(vec!["open".to_string(), "closed".to_string()]);
        entity.fields.push(field);

        let tokens = generate_entities_module(&[entity], &Default::default());
        let code = tokens.to_string();
        assert!(code.contains("pub enum TaskStatus"));
        assert!(code.contains("pub status : TaskStatus"));
    }

    /// Characterization matrix for the `OirType → Rust` walk (now the shared
    /// `orbital_core::render_type` + `RustRenderer`). Pins the arms the fixture
    /// byte-diff under-covers: the enum-ident synthesis (`{Entity}{Field}`),
    /// the raw-`key_ty` Map gate (String key → `HashMap`, non-String →
    /// `serde_json::Value`), `Integer→i64` vs `Number→f64`, and the collapses
    /// (`Reference`/`Trait`/`SExpr`/`TypeRef` → their carriers). The
    /// `Struct`/`Union` arms pin the Phase-2 named-type output — bare name at
    /// the field position, declaration hoisted (the old arms erased both to
    /// `serde_json::Value`). Whitespace-insensitive so it pins semantics, not
    /// `proc_macro2` spacing.
    fn rust_render(ty: &OirType, enum_values: Option<&[String]>) -> (String, Vec<String>) {
        let hoisted = std::cell::RefCell::new(Vec::new());
        let ty_str = map_type_to_rust(ty, "Post", "status", enum_values, &hoisted)
            .to_string()
            .replace(' ', "");
        let hoisted_strs: Vec<String> = hoisted
            .into_inner()
            .iter()
            .map(|t| t.to_string().replace(' ', ""))
            .collect();
        (ty_str, hoisted_strs)
    }

    fn rust_ty(ty: &OirType, enum_values: Option<&[String]>) -> String {
        rust_render(ty, enum_values).0
    }

    #[test]
    fn char_map_type_to_rust_arms() {
        assert_eq!(rust_ty(&OirType::String, None), "String");
        assert_eq!(rust_ty(&OirType::Number, None), "f64");
        assert_eq!(rust_ty(&OirType::Integer, None), "i64");
        assert_eq!(rust_ty(&OirType::Boolean, None), "bool");
        assert_eq!(rust_ty(&OirType::Date, None), "String");
        assert_eq!(rust_ty(&OirType::DateTime, None), "String");
        assert_eq!(rust_ty(&OirType::Json, None), "serde_json::Value");
        assert_eq!(rust_ty(&OirType::reference("Author"), None), "String");
        assert_eq!(rust_ty(&OirType::Trait, None), "String");
        assert_eq!(rust_ty(&OirType::SExpr, None), "serde_json::Value");
        // Struct: named hoisted `pub struct`, referenced by bare name (Phase-2
        // fix — the old arm erased the struct to `serde_json::Value`). Nested
        // structs extend the positional name (`{Entity}{Field}{Nested}`).
        let (ty, hoisted) = rust_render(
            &OirType::Struct {
                fields: vec![
                    OirField::new("name", OirType::String),
                    OirField::new(
                        "geo",
                        OirType::Struct {
                            fields: vec![OirField::new("lat", OirType::Number)],
                        },
                    ),
                ],
            },
            None,
        );
        assert_eq!(ty, "PostStatus");
        assert_eq!(
            hoisted,
            vec![
                "#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]#[serde(rename_all=\"camelCase\")]pubstructPostStatusGeo{publat:f64,}",
                "#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]#[serde(rename_all=\"camelCase\")]pubstructPostStatus{pubname:String,pubgeo:PostStatusGeo,}",
            ]
        );
        // Union: a named `#[serde(untagged)]` enum over the hoisted variant
        // structs, referenced by bare name.
        let (ty, hoisted) = rust_render(
            &OirType::Union {
                variants: vec![
                    OirField::new(
                        "book",
                        OirType::Struct {
                            fields: vec![OirField::new("kind", OirType::String)],
                        },
                    ),
                    OirField::new(
                        "movie",
                        OirType::Struct {
                            fields: vec![OirField::new("kind", OirType::String)],
                        },
                    ),
                ],
            },
            None,
        );
        assert_eq!(ty, "PostStatus");
        assert_eq!(
            hoisted,
            vec![
                "#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]#[serde(rename_all=\"camelCase\")]pubstructPostStatusBook{pubkind:String,}",
                "#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]#[serde(rename_all=\"camelCase\")]pubstructPostStatusMovie{pubkind:String,}",
                "#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]#[serde(untagged)]pubenumPostStatus{Book(PostStatusBook),Movie(PostStatusMovie),}",
            ]
        );
        // Enum-ident synthesis from the `enum_values` sidecar (entity+field).
        let vals = vec!["a".to_string(), "b".to_string()];
        assert_eq!(rust_ty(&OirType::String, Some(&vals)), "PostStatus");
        assert_eq!(rust_ty(&OirType::enum_type("X"), Some(&vals)), "PostStatus");
        // Empty sidecar falls back to String.
        assert_eq!(rust_ty(&OirType::String, Some(&[])), "String");
        // Recursive arms.
        assert_eq!(rust_ty(&OirType::array(OirType::Number), None), "Vec<f64>");
        assert_eq!(
            rust_ty(&OirType::optional(OirType::Integer), None),
            "Option<i64>"
        );
        // Map gates on the RAW key type: String-ish key → HashMap<String, V>,
        // anything else collapses to a dynamic JSON value.
        assert_eq!(
            rust_ty(&OirType::map(OirType::String, OirType::Number), None),
            "std::collections::HashMap<String,f64>"
        );
        assert_eq!(
            rust_ty(&OirType::map(OirType::Boolean, OirType::Number), None),
            "serde_json::Value"
        );
    }
}
