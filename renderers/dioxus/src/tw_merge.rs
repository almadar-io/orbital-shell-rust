//! Port of tailwind-merge 2.6.1 (`twMerge`) with the web renderer's `cn.ts` extension.
//!
//! The class-group table is generated into `tw_merge_table.rs` by
//! `scripts/gen-tw-merge-table.mjs` from the library's own default config merged with
//! the extension, so the data cannot drift from the web. Goldens in
//! `tests/fixtures/tw_merge_goldens.json` come from `scripts/tw-merge-goldens.mjs`.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

#[path = "tw_merge_table.rs"]
mod table;

#[derive(Clone, Copy)]
pub enum V {
    Any,
    ArbitraryImage,
    ArbitraryLength,
    ArbitraryNumber,
    ArbitraryPosition,
    ArbitraryShadow,
    ArbitrarySize,
    ArbitraryValue,
    Integer,
    Length,
    Number,
    Percent,
    TshirtSize,
}

pub enum Def {
    S(&'static str),
    V(V),
    T(&'static str),
    O(&'static [(&'static str, &'static [Def])]),
}

const IMPORTANT: char = '!';

fn js_number(value: &str) -> Option<f64> {
    let t = value.trim();
    if t.is_empty() {
        return Some(0.0);
    }
    for (prefix, radix) in [
        ("0x", 16),
        ("0X", 16),
        ("0o", 8),
        ("0O", 8),
        ("0b", 2),
        ("0B", 2),
    ] {
        if let Some(rest) = t.strip_prefix(prefix) {
            return u64::from_str_radix(rest, radix).ok().map(|n| n as f64);
        }
    }
    let (sign, body) = match t.as_bytes()[0] {
        b'+' => (1.0, &t[1..]),
        b'-' => (-1.0, &t[1..]),
        _ => (1.0, t),
    };
    if body == "Infinity" {
        return Some(sign * f64::INFINITY);
    }
    let b = body.as_bytes();
    let mut i = 0;
    let mut digits = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
        digits += 1;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return None;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return None;
        }
    }
    if i != b.len() {
        return None;
    }
    body.parse::<f64>().ok().map(|n| sign * n)
}

fn is_number(value: &str) -> bool {
    !value.is_empty() && js_number(value).is_some()
}

fn is_integer(value: &str) -> bool {
    !value.is_empty() && js_number(value).is_some_and(|n| n.is_finite() && n.fract() == 0.0)
}

fn is_fraction(value: &str) -> bool {
    match value.split_once('/') {
        Some((a, b)) => {
            !a.is_empty()
                && !b.is_empty()
                && a.bytes().all(|c| c.is_ascii_digit())
                && b.bytes().all(|c| c.is_ascii_digit())
        }
        None => false,
    }
}

fn is_length(value: &str) -> bool {
    is_number(value) || matches!(value, "px" | "full" | "screen") || is_fraction(value)
}

fn is_percent(value: &str) -> bool {
    value.strip_suffix('%').is_some_and(is_number)
}

fn is_tshirt(value: &str) -> bool {
    let b = value.as_bytes();
    let digits = |from: usize| b[from..].iter().take_while(|c| c.is_ascii_digit()).count();
    let mut start = digits(0);
    if start > 0 && b.get(start) == Some(&b'.') {
        let frac = digits(start + 1);
        if frac > 0 {
            start += 1 + frac;
        }
    }
    matches!(&value[start..], "xs" | "sm" | "md" | "lg" | "xl")
}

fn is_word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

fn has_unit_at(s: &[u8], i: usize) -> bool {
    let r = &s[i..];
    let starts = |p: &[u8]| r.starts_with(p);
    if starts(b"%")
        || starts(b"px")
        || starts(b"em")
        || starts(b"rem")
        || starts(b"pt")
        || starts(b"pc")
        || starts(b"in")
        || starts(b"cm")
        || starts(b"mm")
        || starts(b"cap")
        || starts(b"ch")
        || starts(b"ex")
        || starts(b"lh")
        || starts(b"rlh")
    {
        return true;
    }
    let viewport = |r: &[u8]| {
        let r = if matches!(r.first(), Some(b's' | b'd' | b'l')) && r.get(1) == Some(&b'v') {
            &r[1..]
        } else {
            r
        };
        r.first() == Some(&b'v')
            && (matches!(r.get(1), Some(b'h' | b'w' | b'i' | b'b'))
                || r[1..].starts_with(b"min")
                || r[1..].starts_with(b"max"))
    };
    if viewport(r) {
        return true;
    }
    r.starts_with(b"cq")
        && (matches!(r.get(2), Some(b'w' | b'h' | b'i' | b'b'))
            || r[2..].starts_with(b"min")
            || r[2..].starts_with(b"max"))
}

fn is_length_only(value: &str) -> bool {
    let s = value.as_bytes();
    let mut matched = value == "0";
    if !matched {
        let mut i = 0;
        while i < s.len() {
            if s[i].is_ascii_digit() {
                let mut j = i;
                while j < s.len() && s[j].is_ascii_digit() {
                    j += 1;
                }
                if has_unit_at(s, j) {
                    matched = true;
                    break;
                }
                i = j;
            } else {
                i += 1;
            }
        }
    }
    if !matched {
        for name in ["calc", "min", "max", "clamp"] {
            let mut from = 0;
            while let Some(p) = value[from..].find(name) {
                let at = from + p;
                let boundary = at == 0 || !is_word(s[at - 1]);
                let after = at + name.len();
                if boundary
                    && s.get(after) == Some(&b'(')
                    && s[after + 1..].iter().skip(1).any(|&c| c == b')')
                {
                    matched = true;
                    break;
                }
                from = at + 1;
            }
            if matched {
                break;
            }
        }
    }
    matched && !is_color_function(value)
}

fn call_body(value: &str, names: &[&str]) -> bool {
    names.iter().any(|n| {
        value
            .strip_prefix(n)
            .and_then(|r| r.strip_prefix('('))
            .is_some_and(|r| r.len() >= 2 && r.ends_with(')'))
    })
}

fn is_color_function(value: &str) -> bool {
    call_body(
        value,
        &[
            "rgb",
            "rgba",
            "hsl",
            "hsla",
            "hwb",
            "lab",
            "lch",
            "oklab",
            "oklch",
            "color-mix",
        ],
    )
}

fn is_image(value: &str) -> bool {
    call_body(
        value,
        &[
            "url",
            "image",
            "image-set",
            "cross-fade",
            "element",
            "linear-gradient",
            "radial-gradient",
            "conic-gradient",
            "repeating-linear-gradient",
            "repeating-radial-gradient",
            "repeating-conic-gradient",
        ],
    )
}

fn shadow_component(s: &[u8], first: bool) -> Option<usize> {
    let mut i = 0;
    if s.first() == Some(&b'-') {
        i = 1;
    }
    let alt1 = (|| {
        let mut j = i;
        while j < s.len() && s[j].is_ascii_digit() {
            j += 1;
        }
        let int_end = j;
        if j < s.len() && s[j] == b'.' {
            let mut k = j + 1;
            while k < s.len() && s[k].is_ascii_digit() {
                k += 1;
            }
            if k > j + 1 {
                j = k;
            } else if int_end == i {
                return None;
            }
        } else if int_end == i {
            return None;
        }
        let letters = j;
        let mut k = j;
        while k < s.len() && s[k].is_ascii_lowercase() {
            k += 1;
        }
        if k == letters {
            return None;
        }
        if first && s.get(k) != Some(&b'_') {
            return None;
        }
        Some(k)
    })();
    if alt1.is_some() {
        return alt1;
    }
    if s.get(i) == Some(&b'0') && (!first || s.get(i + 1) == Some(&b'_')) {
        return Some(i + 1);
    }
    None
}

fn is_shadow(value: &str) -> bool {
    let mut s = value.as_bytes();
    if let Some(r) = value.strip_prefix("inset_") {
        s = r.as_bytes();
    }
    match shadow_component(s, true) {
        Some(end) => shadow_component(&s[end + 1..], false).is_some(),
        None => false,
    }
}

struct Arbitrary<'a> {
    label: Option<&'a str>,
    inner: &'a str,
}

fn arbitrary(value: &str) -> Option<Arbitrary<'_>> {
    if value.len() < 3 || !value.starts_with('[') || !value.ends_with(']') {
        return None;
    }
    let body = &value[1..value.len() - 1];
    if let Some((label, rest)) = body.split_once(':') {
        if !label.is_empty()
            && !rest.is_empty()
            && label.bytes().all(|c| c.is_ascii_alphabetic() || c == b'-')
        {
            return Some(Arbitrary {
                label: Some(label),
                inner: rest,
            });
        }
    }
    Some(Arbitrary {
        label: None,
        inner: body,
    })
}

fn arbitrary_check(value: &str, labels: &[&str], test: fn(&str) -> bool) -> bool {
    match arbitrary(value) {
        Some(a) => match a.label {
            Some(l) => labels.contains(&l),
            None => test(a.inner),
        },
        None => false,
    }
}

fn never(_: &str) -> bool {
    false
}

impl V {
    fn check(self, value: &str) -> bool {
        match self {
            V::Any => true,
            V::ArbitraryImage => arbitrary_check(value, &["image", "url"], is_image),
            V::ArbitraryLength => arbitrary_check(value, &["length"], is_length_only),
            V::ArbitraryNumber => arbitrary_check(value, &["number"], is_number),
            V::ArbitraryPosition => arbitrary_check(value, &["position"], never),
            V::ArbitraryShadow => arbitrary_check(value, &[""], is_shadow),
            V::ArbitrarySize => arbitrary_check(value, &["length", "size", "percentage"], never),
            V::ArbitraryValue => arbitrary(value).is_some(),
            V::Integer => is_integer(value),
            V::Length => is_length(value),
            V::Number => is_number(value),
            V::Percent => is_percent(value),
            V::TshirtSize => is_tshirt(value),
        }
    }
}

#[derive(Default)]
struct Node {
    next: HashMap<&'static str, Node>,
    validators: Vec<(V, &'static str)>,
    group: Option<&'static str>,
}

struct Config {
    root: Node,
    conflicts: HashMap<&'static str, &'static [&'static str]>,
    conflict_modifiers: HashMap<&'static str, &'static [&'static str]>,
}

fn theme_defs(key: &str) -> &'static [Def] {
    table::THEME
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, d)| *d)
        .unwrap_or(&[])
}

fn part<'a>(node: &'a mut Node, path: &'static str) -> &'a mut Node {
    let mut cur = node;
    for p in path.split('-') {
        cur = cur.next.entry(p).or_default();
    }
    cur
}

fn process(defs: &'static [Def], node: &mut Node, group: &'static str) {
    for d in defs {
        match d {
            Def::S(s) => {
                let target = if s.is_empty() {
                    &mut *node
                } else {
                    part(node, s)
                };
                target.group = Some(group);
            }
            Def::V(v) => node.validators.push((*v, group)),
            Def::T(key) => process(theme_defs(key), node, group),
            Def::O(entries) => {
                for (key, inner) in entries.iter() {
                    process(inner, part(node, key), group);
                }
            }
        }
    }
}

fn config() -> &'static Config {
    static CONFIG: OnceLock<Config> = OnceLock::new();
    CONFIG.get_or_init(|| {
        let mut root = Node::default();
        for (group, defs) in table::CLASS_GROUPS {
            process(defs, &mut root, group);
        }
        Config {
            root,
            conflicts: table::CONFLICTS.iter().copied().collect(),
            conflict_modifiers: table::CONFLICT_MODIFIERS.iter().copied().collect(),
        }
    })
}

fn group_recursive(parts: &[&str], node: &Node) -> Option<&'static str> {
    let Some((first, rest)) = parts.split_first() else {
        return node.group;
    };
    if let Some(next) = node.next.get(first) {
        if let Some(g) = group_recursive(rest, next) {
            return Some(g);
        }
    }
    if node.validators.is_empty() {
        return None;
    }
    let joined = parts.join("-");
    node.validators
        .iter()
        .find(|(v, _)| v.check(&joined))
        .map(|(_, g)| *g)
}

fn arbitrary_property_group(class_name: &str) -> Option<String> {
    if class_name.len() < 3 || !class_name.starts_with('[') || !class_name.ends_with(']') {
        return None;
    }
    let inner = &class_name[1..class_name.len() - 1];
    let colon = inner.find(':')?;
    if colon == 0 {
        return None;
    }
    Some(format!("arbitrary..{}", &inner[..colon]))
}

enum Group {
    Static(&'static str),
    Arbitrary(String),
}

impl Group {
    fn id(&self) -> &str {
        match self {
            Group::Static(s) => s,
            Group::Arbitrary(s) => s,
        }
    }
}

fn class_group_id(class_name: &str) -> Option<Group> {
    let mut parts: Vec<&str> = class_name.split('-').collect();
    if parts[0].is_empty() && parts.len() != 1 {
        parts.remove(0);
    }
    group_recursive(&parts, &config().root)
        .map(Group::Static)
        .or_else(|| arbitrary_property_group(class_name).map(Group::Arbitrary))
}

struct Parsed<'a> {
    modifiers: Vec<&'a str>,
    important: bool,
    base: &'a str,
    postfix: Option<usize>,
}

fn parse_class_name(class_name: &str) -> Parsed<'_> {
    let b = class_name.as_bytes();
    let mut modifiers = Vec::new();
    let mut depth = 0i32;
    let mut modifier_start = 0;
    let mut postfix_pos: Option<usize> = None;
    for (index, &c) in b.iter().enumerate() {
        if depth == 0 {
            if c == b':' {
                modifiers.push(&class_name[modifier_start..index]);
                modifier_start = index + 1;
                continue;
            }
            if c == b'/' {
                postfix_pos = Some(index);
                continue;
            }
        }
        if c == b'[' {
            depth += 1;
        } else if c == b']' {
            depth -= 1;
        }
    }
    let with_important = if modifiers.is_empty() {
        class_name
    } else {
        &class_name[modifier_start..]
    };
    let important = with_important.starts_with(IMPORTANT);
    let base = if important {
        &with_important[1..]
    } else {
        with_important
    };
    let postfix = postfix_pos
        .filter(|&p| p > 0 && p > modifier_start)
        .map(|p| p - modifier_start);
    Parsed {
        modifiers,
        important,
        base,
        postfix,
    }
}

fn sort_modifiers<'a>(modifiers: &[&'a str]) -> Vec<&'a str> {
    if modifiers.len() <= 1 {
        return modifiers.to_vec();
    }
    let mut sorted = Vec::new();
    let mut unsorted: Vec<&str> = Vec::new();
    for m in modifiers {
        if m.starts_with('[') {
            unsorted.sort_unstable();
            sorted.append(&mut unsorted);
            sorted.push(m);
        } else {
            unsorted.push(m);
        }
    }
    unsorted.sort_unstable();
    sorted.append(&mut unsorted);
    sorted
}

fn floor_boundary(s: &str, mut at: usize) -> usize {
    at = at.min(s.len());
    while !s.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// Same output as the web renderer's `cn` (`twMerge(clsx(...))`) for one class string.
pub fn tw_merge(classes: &str) -> String {
    let cfg = config();
    let names: Vec<&str> = classes.split_whitespace().collect();
    let mut in_conflict: HashSet<String> = HashSet::new();
    let mut kept: Vec<&str> = Vec::new();
    for original in names.iter().rev() {
        let parsed = parse_class_name(original);
        let mut has_postfix = parsed.postfix.is_some();
        let lookup = match parsed.postfix {
            Some(p) => &parsed.base[..floor_boundary(parsed.base, p)],
            None => parsed.base,
        };
        let mut group = class_group_id(lookup);
        if group.is_none() {
            if !has_postfix {
                kept.push(original);
                continue;
            }
            group = class_group_id(parsed.base);
            if group.is_none() {
                kept.push(original);
                continue;
            }
            has_postfix = false;
        }
        let group = group.expect("checked above");
        let variant = sort_modifiers(&parsed.modifiers).join(":");
        let modifier_id = if parsed.important {
            format!("{variant}{IMPORTANT}")
        } else {
            variant
        };
        let class_id = format!("{modifier_id}{}", group.id());
        if in_conflict.contains(&class_id) {
            continue;
        }
        in_conflict.insert(class_id);
        let gid = group.id();
        if let Some(list) = cfg.conflicts.get(gid) {
            for g in list.iter() {
                in_conflict.insert(format!("{modifier_id}{g}"));
            }
        }
        if has_postfix {
            if let Some(list) = cfg.conflict_modifiers.get(gid) {
                for g in list.iter() {
                    in_conflict.insert(format!("{modifier_id}{g}"));
                }
            }
        }
        kept.push(original);
    }
    kept.reverse();
    kept.join(" ")
}

#[cfg(test)]
mod tests {
    use super::tw_merge;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Case {
        input: String,
        output: String,
    }

    #[test]
    fn matches_web_goldens() {
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("../tests/fixtures/tw_merge_goldens.json"))
                .expect("goldens parse");
        assert!(cases.len() >= 300);
        let failures: Vec<String> = cases
            .iter()
            .filter(|c| tw_merge(&c.input) != c.output)
            .map(|c| {
                format!(
                    "{:?}: want {:?}, got {:?}",
                    c.input,
                    c.output,
                    tw_merge(&c.input)
                )
            })
            .collect();
        assert!(
            failures.is_empty(),
            "{} golden mismatches:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    fn later_class_wins_in_same_group() {
        assert_eq!(tw_merge("h-button-md h-auto"), "h-auto");
        assert_eq!(tw_merge("px-4 py-1 p-2"), "p-2");
    }

    #[test]
    fn non_conflicting_classes_are_both_kept() {
        assert_eq!(tw_merge("px-4 py-1"), "px-4 py-1");
        assert_eq!(
            tw_merge("text-display-1 text-red-500"),
            "text-display-1 text-red-500"
        );
    }

    #[test]
    fn conflicts_are_scoped_to_the_same_modifiers() {
        assert_eq!(tw_merge("p-2 hover:p-4 p-3"), "hover:p-4 p-3");
        assert_eq!(tw_merge("md:p-2 p-3"), "md:p-2 p-3");
        assert_eq!(
            tw_merge("hover:focus:p-2 focus:hover:p-3"),
            "focus:hover:p-3"
        );
    }

    #[test]
    fn arbitrary_values_and_important() {
        assert_eq!(tw_merge("w-[10px] w-[20px]"), "w-[20px]");
        assert_eq!(
            tw_merge("text-[12px] text-red-500"),
            "text-[12px] text-red-500"
        );
        assert_eq!(tw_merge("!p-2 p-4"), "!p-2 p-4");
        assert_eq!(tw_merge("p-2 !p-4 !p-6"), "p-2 !p-6");
    }

    #[test]
    fn unknown_classes_pass_through() {
        assert_eq!(
            tw_merge("surface-content custom-x p-2 p-4"),
            "surface-content custom-x p-4"
        );
        assert_eq!(tw_merge("   "), "");
    }
}
