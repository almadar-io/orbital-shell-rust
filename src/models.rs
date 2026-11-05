//! `src/generated/{mod,entities,events}.rs`: one serde struct per entity and per declared event payload, so native
//! trait code imports the same model names every shell generates (`<Trait><EVENT>Payload`).

use orbital_core::generate::GeneratedFile;
use orbital_core::ir::{
    render_default, render_type, OirEntity, OirField, OirModule, OirType, TargetLiteralRenderer,
    TargetTypeRenderer, TypeMappingClass,
};
use orbital_core::str_utils::{
    escape_double_quoted, sanitize_identifier, split_to_pascal_case, to_identifier_pascal,
};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

const MODULE_ATTRS: &str = "#![allow(dead_code, unused_imports, non_camel_case_types, non_snake_case)]\n";
const DERIVES: &str = "#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]";

const RUST_KEYWORDS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "crate", "do",
    "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "if", "impl", "in", "let", "loop",
    "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref", "return", "self",
    "Self", "static", "struct", "super", "trait", "true", "try", "type", "typeof", "unsafe",
    "unsized", "use", "virtual", "where", "while", "yield",
];

/// Names the generated files use unqualified: an entity of the same name would shadow them.
const RUST_RESERVED_TYPES: &[&str] = &[
    "Box", "Clone", "Debug", "Default", "Deserialize", "Option", "PartialEq", "Result", "Serialize",
    "String", "Vec",
];

fn rs_identifier(name: &str) -> String {
    sanitize_identifier(name, RUST_KEYWORDS)
}

fn entity_class(name: &str) -> String {
    let class = split_to_pascal_case(name);
    if RUST_RESERVED_TYPES.contains(&class.as_str()) {
        format!("{class}Entity")
    } else {
        class
    }
}

fn symbol(name: &str) -> String {
    let valid = name
        .chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_alphanumeric() || c == '_');
    if valid {
        name.to_string()
    } else {
        to_identifier_pascal(name)
    }
}

fn variant_ident(value: &str) -> String {
    let ident = to_identifier_pascal(value);
    if ident.is_empty() || ident.starts_with(|c: char| c.is_ascii_digit()) {
        format!("V{ident}")
    } else {
        ident
    }
}

struct RsTypes<'a> {
    context: String,
    hoisted: &'a RefCell<Vec<String>>,
    local: &'a BTreeSet<String>,
}

impl RsTypes<'_> {
    fn child(&self, context: String) -> RsTypes<'_> {
        RsTypes {
            context,
            hoisted: self.hoisted,
            local: self.local,
        }
    }
}

impl TargetTypeRenderer for RsTypes<'_> {
    type Out = String;

    fn string(&self, _enum_values: Option<&[String]>) -> String {
        "String".to_string()
    }
    fn number(&self) -> String {
        "f64".to_string()
    }
    fn integer(&self) -> String {
        "i64".to_string()
    }
    fn boolean(&self) -> String {
        "bool".to_string()
    }
    fn date(&self) -> String {
        "String".to_string()
    }
    fn json(&self) -> String {
        "serde_json::Value".to_string()
    }
    fn enum_ty(&self, name: &str, enum_values: Option<&[String]>) -> String {
        let Some(values) = enum_values.filter(|v| !v.is_empty()) else {
            return "String".to_string();
        };
        let name = if orbital_core::type_compat::literal_union_members(name).is_some() {
            self.context.as_str()
        } else {
            name
        };
        let variants: Vec<String> = values
            .iter()
            .map(|v| {
                format!(
                    "    #[serde(rename = \"{}\")]\n    {},",
                    escape_double_quoted(v),
                    variant_ident(v)
                )
            })
            .collect();
        let decl = format!(
            "#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\npub enum {name} {{\n{}\n}}",
            variants.join("\n")
        );
        let mut hoisted = self.hoisted.borrow_mut();
        if !hoisted.contains(&decl) {
            hoisted.push(decl);
        }
        name.to_string()
    }
    fn trait_ref(&self) -> String {
        "String".to_string()
    }
    fn sexpr(&self) -> String {
        "serde_json::Value".to_string()
    }
    fn type_ref(&self, name: &str) -> String {
        orbital_core::type_compat::type_identifier(name)
    }
    fn row(&self, entity: &str) -> String {
        if self.local.contains(entity) {
            entity_class(entity)
        } else {
            "serde_json::Value".to_string()
        }
    }
    fn array(&self, _inner_ty: &OirType, inner: String) -> String {
        format!("Vec<{inner}>")
    }
    fn map(&self, _key_ty: &OirType, key: String, _value_ty: &OirType, value: String) -> String {
        format!("std::collections::BTreeMap<{key}, {value}>")
    }
    fn optional(&self, _inner_ty: &OirType, inner: String) -> String {
        format!("Option<{inner}>")
    }
    fn struct_ty(&self, fields: &[OirField], _additional: Option<&OirType>) -> String {
        let name = self.context.clone();
        let (lines, fns) = field_lines(fields, &name, self);
        self.hoisted
            .borrow_mut()
            .push(struct_source(&name, None, &lines, &fns));
        name
    }
    fn union_ty(&self, variants: &[OirField]) -> String {
        if variants.is_empty() {
            return self.json();
        }
        let name = self.context.clone();
        let arms: Vec<String> = variants
            .iter()
            .map(|v| {
                let variant = split_to_pascal_case(&v.name);
                let struct_name = format!("{name}{variant}");
                let (lines, fns) = match &v.field_type {
                    OirType::Struct { fields, .. } => field_lines(fields, &struct_name, self),
                    other => {
                        let child = self.child(format!("{struct_name}Value"));
                        (
                            vec![format!("    pub value: {},", render_type(&child, other, None))],
                            Vec::new(),
                        )
                    }
                };
                self.hoisted
                    .borrow_mut()
                    .push(struct_source(&struct_name, None, &lines, &fns));
                format!("    {variant}({struct_name}),")
            })
            .collect();
        self.hoisted.borrow_mut().push(format!(
            "{DERIVES}\n#[serde(untagged)]\npub enum {name} {{\n{}\n}}",
            arms.join("\n")
        ));
        name
    }

    fn classify_string(&self, _enum_values: Option<&[String]>) -> TypeMappingClass {
        TypeMappingClass::Faithful
    }
    fn classify_number(&self) -> TypeMappingClass {
        TypeMappingClass::Faithful
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
        TypeMappingClass::Lossy
    }
    fn classify_enum_ty(&self, _name: &str, _enum_values: Option<&[String]>) -> TypeMappingClass {
        TypeMappingClass::Faithful
    }
    fn classify_trait_ref(&self) -> TypeMappingClass {
        TypeMappingClass::Widening
    }
    fn classify_sexpr(&self) -> TypeMappingClass {
        TypeMappingClass::Lossy
    }
    fn classify_row(&self, _entity: &str) -> TypeMappingClass {
        TypeMappingClass::Faithful
    }
    fn classify_type_ref(&self, _name: &str) -> TypeMappingClass {
        TypeMappingClass::Faithful
    }
    fn classify_array(&self, _inner_ty: &OirType, inner: TypeMappingClass) -> TypeMappingClass {
        inner
    }
    fn classify_map(
        &self,
        _key_ty: &OirType,
        key: TypeMappingClass,
        _value_ty: &OirType,
        value: TypeMappingClass,
    ) -> TypeMappingClass {
        key.meet(value)
    }
    fn classify_optional(&self, _inner_ty: &OirType, inner: TypeMappingClass) -> TypeMappingClass {
        inner
    }
    fn classify_struct_ty(
        &self,
        _fields: &[OirField],
        additional: Option<&OirType>,
    ) -> TypeMappingClass {
        if additional.is_some() {
            TypeMappingClass::Lossy
        } else {
            TypeMappingClass::Faithful
        }
    }
    fn classify_union_ty(&self, variants: &[OirField]) -> TypeMappingClass {
        if variants.is_empty() {
            self.classify_json()
        } else {
            TypeMappingClass::Faithful
        }
    }
}

struct RsLiteral;

impl TargetLiteralRenderer for RsLiteral {
    type Out = String;

    fn null(&self) -> String {
        "None".to_string()
    }
    fn bool(&self, b: bool) -> String {
        b.to_string()
    }
    fn integer(&self, i: i64) -> String {
        i.to_string()
    }
    fn number(&self, n: f64) -> String {
        let s = n.to_string();
        if s.contains('.') || s.contains('e') || s.contains('E') {
            s
        } else {
            format!("{s}.0")
        }
    }
    fn string(&self, s: &str) -> String {
        format!("\"{}\".to_string()", escape_double_quoted(s))
    }
    fn enum_member(&self, enum_name: &str, member: &str) -> String {
        format!("{enum_name}::{}", variant_ident(member))
    }
    fn array(&self, items: Vec<String>) -> String {
        format!("vec![{}]", items.join(", "))
    }
}

fn struct_source(name: &str, doc: Option<&str>, lines: &[String], fns: &[String]) -> String {
    let mut out = String::new();
    if let Some(doc) = doc {
        out.push_str(&format!("/// {}\n", doc.lines().collect::<Vec<_>>().join(" ")));
    }
    out.push_str(&format!("{DERIVES}\npub struct {name} {{\n"));
    for line in lines {
        out.push_str(line);
        out.push('\n');
    }
    out.push('}');
    for f in fns {
        out.push_str("\n\n");
        out.push_str(f);
    }
    out
}

/// The struct body lines and the default functions the lines name.
fn field_lines(fields: &[OirField], class: &str, types: &RsTypes<'_>) -> (Vec<String>, Vec<String>) {
    let mut lines = Vec::new();
    let mut fns = Vec::new();
    for f in fields {
        let (line, default_fn) = field_line(f, class, types);
        lines.push(line);
        fns.extend(default_fn);
    }
    (lines, fns)
}

fn field_line(f: &OirField, class: &str, types: &RsTypes<'_>) -> (String, Option<String>) {
    let (ty, optional) = f.effective_type();
    let child = types.child(format!("{class}{}", split_to_pascal_case(&f.name)));
    let enum_ty = f
        .enum_values
        .as_ref()
        .filter(|v| !v.is_empty() && matches!(ty, OirType::String | OirType::Enum { .. }))
        .map(|_| OirType::Enum {
            name: child.context.clone(),
        });
    let ty = enum_ty.as_ref().unwrap_or(ty);
    let rendered = render_type(&child, ty, f.enum_values.as_deref());
    let annotation = if optional {
        format!("Option<{rendered}>")
    } else {
        rendered
    };
    let ident = rs_identifier(&f.name);
    let mut attrs = Vec::new();
    if ident != f.name {
        attrs.push(format!("rename = \"{}\"", escape_double_quoted(&f.name)));
    }
    let mut default_fn = None;
    let mut comment = String::new();
    if let Some(value) = &f.default {
        match render_default(&RsLiteral, value, ty, None) {
            Ok(Some(expr)) => {
                let fn_name = format!("default_{class}_{ident}");
                let body = if optional { format!("Some({expr})") } else { expr };
                attrs.push(format!("default = \"{fn_name}\""));
                default_fn = Some(format!("fn {fn_name}() -> {annotation} {{\n    {body}\n}}"));
            }
            Ok(None) => {}
            Err(mismatch) => comment = format!("    // {}\n", mismatch.marker()),
        }
    }
    let attr = if attrs.is_empty() {
        String::new()
    } else {
        format!("    #[serde({})]\n", attrs.join(", "))
    };
    (format!("{comment}{attr}    pub {ident}: {annotation},"), default_fn)
}

fn entity_source(entity: &OirEntity, local: &BTreeSet<String>) -> String {
    let class = entity_class(&entity.name);
    let hoisted = RefCell::new(Vec::new());
    let types = RsTypes {
        context: class.clone(),
        hoisted: &hoisted,
        local,
    };
    let rest: Vec<OirField> = entity
        .fields
        .iter()
        .filter(|f| f.name != "id")
        .cloned()
        .collect();
    let (mut lines, fns) = field_lines(&rest, &class, &types);
    lines.insert(0, "    pub id: String,".to_string());
    let mut out = String::new();
    for decl in hoisted.into_inner() {
        out.push_str(&decl);
        out.push_str("\n\n");
    }
    out.push_str(&struct_source(
        &class,
        Some(entity.description.as_deref().unwrap_or(&entity.name)),
        &lines,
        &fns,
    ));
    out
}

fn local_entities(module: &OirModule) -> BTreeSet<String> {
    module.entities.iter().map(|e| e.name.clone()).collect()
}

fn prelude() -> String {
    format!("// Generated by the Orbital compiler. Do not edit.\n{MODULE_ATTRS}\nuse serde::{{Deserialize, Serialize}};\n")
}

pub fn entities_file(module: &OirModule) -> GeneratedFile {
    let local = local_entities(module);
    let file_hoisted = RefCell::new(Vec::new());
    let types = RsTypes {
        context: String::new(),
        hoisted: &file_hoisted,
        local: &local,
    };
    for (name, ty) in &module.named_types {
        render_type(&types.child(orbital_core::type_compat::type_identifier(name)), ty, None);
    }
    let mut body = String::new();
    for decl in file_hoisted.take() {
        body.push_str(&decl);
        body.push_str("\n\n");
    }
    for entity in module.distinct_entities() {
        body.push_str(&entity_source(&entity, &local));
        body.push_str("\n\n");
    }
    GeneratedFile::new(
        "src/generated/entities.rs",
        format!("{}\n{}\n", prelude(), body.trim_end()),
    )
}

fn payloads(module: &OirModule) -> BTreeMap<String, Vec<OirField>> {
    let mut out = BTreeMap::new();
    for t in &module.traits {
        for event in &t.events {
            let Some(schema) = event.payload_schema.as_ref().filter(|s| !s.is_empty()) else {
                continue;
            };
            let mut fields: Vec<OirField> = schema
                .iter()
                .map(|(k, v)| OirField::new(k.clone(), v.clone()))
                .collect();
            fields.sort_by(|a, b| a.name.cmp(&b.name));
            out.insert(format!("{}{}Payload", symbol(&t.name), symbol(&event.key)), fields);
        }
    }
    out
}

/// One `<Trait>Config` per trait that declares knobs. A knob with a usable declared default carries it; one without
/// (none, null, or an `@config.` forward) is optional.
fn trait_configs(module: &OirModule) -> BTreeMap<String, Vec<OirField>> {
    let mut out = BTreeMap::new();
    for t in &module.traits {
        let fields = t.config_fields();
        if fields.is_empty() {
            continue;
        }
        out.insert(format!("{}Config", symbol(&t.name)), fields);
    }
    out
}

/// `impl Default` building each field from its default fn, or `None` for an optional knob without one.
fn default_impl(name: &str, fields: &[OirField], types: &RsTypes<'_>) -> String {
    let inits: Vec<String> = fields
        .iter()
        .map(|f| {
            let ident = rs_identifier(&f.name);
            let value = if field_line(f, name, types).1.is_some() {
                format!("default_{name}_{ident}()")
            } else {
                "None".to_string()
            };
            format!("            {ident}: {value},")
        })
        .collect();
    format!(
        "impl Default for {name} {{\n    fn default() -> Self {{\n        Self {{\n{}\n        }}\n    }}\n}}",
        inits.join("\n")
    )
}

pub fn events_file(module: &OirModule) -> GeneratedFile {
    let local = local_entities(module);
    let hoisted = RefCell::new(Vec::new());
    let mut body = String::new();
    let configs = trait_configs(module);
    let declared = payloads(module)
        .into_iter()
        .map(|(n, f)| (n, f, false))
        .chain(configs.into_iter().map(|(n, f)| (n, f, true)));
    for (name, fields, is_config) in declared {
        let types = RsTypes {
            context: name.clone(),
            hoisted: &hoisted,
            local: &local,
        };
        let (lines, fns) = field_lines(&fields, &name, &types);
        for decl in hoisted.take() {
            body.push_str(&decl);
            body.push_str("\n\n");
        }
        body.push_str(&struct_source(&name, None, &lines, &fns));
        body.push_str("\n\n");
        if is_config {
            body.push_str(&default_impl(&name, &fields, &types));
            body.push_str("\n\n");
        }
    }
    GeneratedFile::new(
        "src/generated/events.rs",
        format!("{}use super::entities::*;\n\n{}\n", prelude(), body.trim_end()),
    )
}

pub fn mod_file() -> GeneratedFile {
    GeneratedFile::new(
        "src/generated/mod.rs",
        "// Generated by the Orbital compiler. Do not edit.\npub mod entities;\npub mod events;\n",
    )
}

pub fn files(module: &OirModule) -> Vec<GeneratedFile> {
    vec![mod_file(), entities_file(module), events_file(module)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbital_core::ir::{OirConfigFieldType, OirEvent, OirTrait, OirTypeMap, OirValue};

    pub(crate) fn module(with_trait: bool) -> OirModule {
        let mut m = OirModule::new("app", "1.0.0");
        let mut doc = OirEntity::new("Doc");
        doc.fields.push(OirField::new("id", OirType::String));
        doc.fields.push({
            let mut f = OirField::new("chunks", OirType::Array { inner: Box::new(OirType::String) });
            f.required = false;
            f.default = Some(OirValue::Array(vec![]));
            f
        });
        doc.fields.push({
            let mut f = OirField::new("status", OirType::String);
            f.enum_values = Some(vec!["open".into(), "closed".into()]);
            f
        });
        doc.fields.push(OirField::optional("note", OirType::String));
        m.entities.push(doc);
        if with_trait {
            let mut t = OirTrait::new("Chunk");
            let mut listened = OirEvent::new("CHUNK");
            let mut schema = OirTypeMap::new();
            schema.insert("text".into(), OirType::String);
            schema.insert("size".into(), OirType::Number);
            listened.payload_schema = Some(schema);
            let mut emitted = OirEvent::new("CHUNKED");
            let mut schema = OirTypeMap::new();
            schema.insert("count".into(), OirType::Integer);
            emitted.payload_schema = Some(schema);
            t.events = vec![listened, emitted, OirEvent::new("RESET")];
            m.traits.push(t);
        }
        m
    }

    #[test]
    fn entities_render_required_defaulted_enum_and_optional_fields() {
        let file = entities_file(&module(false));
        let expected = "#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]\npub struct Doc {\n    pub id: String,\n    #[serde(default = \"default_Doc_chunks\")]\n    pub chunks: Option<Vec<String>>,\n    pub status: DocStatus,\n    pub note: Option<String>,\n}\n\nfn default_Doc_chunks() -> Option<Vec<String>> {\n    Some(vec![])\n}";
        assert!(file.content.contains(expected), "{}", file.content);
        assert!(file.content.contains("pub enum DocStatus {\n    #[serde(rename = \"open\")]\n    Open,\n    #[serde(rename = \"closed\")]\n    Closed,\n}"));
    }

    #[test]
    fn a_nullable_number_field_renders_one_option() {
        let mut m = OirModule::new("app", "1.0.0");
        let mut rec = OirEntity::new("Rec");
        rec.fields.push(OirField::optional("v", OirType::optional(OirType::Number)));
        rec.fields.push(OirField::new("n", OirType::Number));
        m.entities.push(rec);
        let content = entities_file(&m).content;
        assert!(content.contains("pub v: Option<f64>,"), "{content}");
        assert!(!content.contains("Option<Option<"), "{content}");
        assert!(content.contains("pub n: f64,"), "{content}");
    }

    #[test]
    fn control_a_required_field_has_no_option_and_no_default() {
        let content = entities_file(&module(false)).content;
        assert!(!content.contains("Option<DocStatus>"));
        assert!(!content.contains("default_Doc_status"));
        assert!(!content.contains("default_Doc_note"));
    }

    fn holder_module(alias: &str) -> OirModule {
        let mut m = OirModule::new("app", "1.0.0");
        m.named_types.insert(
            alias.to_string(),
            OirType::Struct { additional: None, fields: vec![OirField::new("id", OirType::String)].into() },
        );
        let mut holder = OirEntity::new("Holder");
        holder.fields.push(OirField::new("claim", OirType::TypeRef { name: alias.to_string() }));
        m.entities.push(holder);
        m
    }

    #[test]
    fn a_behavior_qualified_alias_is_one_identifier_at_declaration_and_reference() {
        let content = entities_file(&holder_module("std-foo::Bar")).content;
        assert!(content.contains("pub struct StdFooBar {"), "{content}");
        assert!(content.contains("pub claim: StdFooBar,"), "{content}");
        assert!(!content.contains("::Bar"), "{content}");
    }

    #[test]
    fn control_an_unqualified_alias_is_spelled_as_written() {
        let content = entities_file(&holder_module("Bar")).content;
        assert!(content.contains("pub struct Bar {"), "{content}");
        assert!(content.contains("pub claim: Bar,"), "{content}");
        assert!(!content.contains("StdFoo"), "{content}");
    }

    #[test]
    fn events_render_one_payload_per_declared_payload_with_sorted_fields() {
        let content = events_file(&module(true)).content;
        assert!(content.contains("pub struct ChunkCHUNKPayload {\n    pub size: f64,\n    pub text: String,\n}"), "{content}");
        assert!(content.contains("pub struct ChunkCHUNKEDPayload {\n    pub count: i64,\n}"), "{content}");
        assert!(content.contains("use super::entities::*;"));
    }

    fn knob(ty: OirType) -> OirConfigFieldType {
        OirConfigFieldType { field_type: ty, values: None }
    }

    #[test]
    fn a_trait_with_config_knobs_declares_one_typed_config_with_defaults() {
        let mut m = OirModule::new("app", "1.0.0");
        let mut t = OirTrait::new("Game");
        let mut schema = orbital_core::ir::OirConfigSchema::new();
        schema.insert("running".into(), knob(OirType::Boolean));
        schema.insert("pointsPerKill".into(), knob(OirType::Number));
        schema.insert("label".into(), knob(OirType::String));
        t.declared_config_schema = Some(schema);
        let mut defaults = orbital_core::ir::OirObject::new();
        defaults.insert("running".into(), OirValue::Bool(true));
        defaults.insert("pointsPerKill".into(), OirValue::Number(10.0));
        t.declared_config_defaults = Some(defaults);
        m.traits.push(t);
        let expected = "#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]\npub struct GameConfig {\n    pub label: Option<String>,\n    #[serde(default = \"default_GameConfig_pointsPerKill\")]\n    pub pointsPerKill: f64,\n    #[serde(default = \"default_GameConfig_running\")]\n    pub running: bool,\n}\n\nfn default_GameConfig_pointsPerKill() -> f64 {\n    10.0\n}\n\nfn default_GameConfig_running() -> bool {\n    true\n}\n\nimpl Default for GameConfig {\n    fn default() -> Self {\n        Self {\n            label: None,\n            pointsPerKill: default_GameConfig_pointsPerKill(),\n            running: default_GameConfig_running(),\n        }\n    }\n}";
        let content = events_file(&m).content;
        assert!(content.contains(expected), "{content}");
    }

    #[test]
    fn control_a_trait_without_config_declares_no_config() {
        assert!(!events_file(&module(true)).content.contains("ChunkConfig"));
    }

    #[test]
    fn control_a_void_event_declares_no_payload_and_the_module_lists_both_files() {
        assert!(!events_file(&module(true)).content.contains("ChunkRESETPayload"));
        assert!(!events_file(&module(false)).content.contains("pub struct"));
        let listing = mod_file().content;
        assert!(listing.contains("pub mod entities;") && listing.contains("pub mod events;"));
    }

    fn payload_module(ty: OirType) -> OirModule {
        let mut m = module(true);
        let mut schema = OirTypeMap::new();
        schema.insert("reason".into(), ty);
        schema.insert("plain".into(), OirType::String);
        m.traits[0].events[1].payload_schema = Some(schema);
        m
    }

    #[test]
    fn a_literal_union_payload_field_renders_a_hoisted_serde_enum() {
        let union = OirType::Enum { name: "\"not-a-key\" | \"salt\"".into() };
        let content = events_file(&payload_module(union)).content;
        assert!(content.contains("pub enum ChunkCHUNKEDPayloadReason {\n    #[serde(rename = \"not-a-key\")]\n    NotAKey,\n    #[serde(rename = \"salt\")]\n    Salt,\n}"), "{content}");
        assert!(content.contains("pub reason: ChunkCHUNKEDPayloadReason,"), "{content}");
        assert!(!content.contains("pub enum \""), "{content}");
        assert!(content.contains("pub plain: String,"), "{content}");
    }

    #[test]
    fn control_an_array_of_one_literal_union_declares_the_enum_once() {
        let union = OirType::Array { inner: Box::new(OirType::Enum { name: "\"a\" | \"b\"".into() }) };
        let content = events_file(&payload_module(union)).content;
        assert!(content.contains("pub reason: Vec<ChunkCHUNKEDPayloadReason>,"), "{content}");
        assert_eq!(content.matches("pub enum ChunkCHUNKEDPayloadReason").count(), 1, "{content}");
    }

    #[test]
    fn control_a_plain_string_payload_field_stays_string() {
        let content = events_file(&payload_module(OirType::String)).content;
        assert!(content.contains("pub reason: String,"), "{content}");
        assert!(!content.contains("pub enum"), "{content}");
    }

    #[test]
    fn a_literal_union_entity_field_without_enum_values_renders_a_hoisted_enum() {
        let mut m = OirModule::new("app", "1.0.0");
        let mut e = OirEntity::new("Doc");
        e.fields.push(OirField::new("kind", OirType::Enum { name: "\"x\" | \"y\"".into() }));
        m.entities.push(e);
        let content = entities_file(&m).content;
        assert!(content.contains("pub enum DocKind {"), "{content}");
        assert!(content.contains("pub kind: DocKind,"), "{content}");
    }
}
