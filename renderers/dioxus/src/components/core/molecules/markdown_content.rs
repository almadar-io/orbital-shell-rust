//! Native Dioxus renderer for the shared pattern contract.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{merge_class_name, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;
use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag};

/// Dioxus renderer for the `markdown-content` pattern.
pub struct MarkdownContentRenderer;

impl MarkdownContentRenderer {
    const NAME: &str = "markdown-content";
}

const PROSE_VARS: [(&str, &str); 17] = [
    ("color", "var(--color-foreground)"),
    ("--tw-prose-body", "var(--color-foreground)"),
    ("--tw-prose-headings", "var(--color-foreground)"),
    ("--tw-prose-lead", "var(--color-muted-foreground)"),
    ("--tw-prose-links", "var(--color-primary)"),
    ("--tw-prose-bold", "var(--color-foreground)"),
    ("--tw-prose-counters", "var(--color-muted-foreground)"),
    ("--tw-prose-bullets", "var(--color-muted-foreground)"),
    ("--tw-prose-hr", "var(--color-border)"),
    ("--tw-prose-quotes", "var(--color-foreground)"),
    ("--tw-prose-quote-borders", "var(--color-primary)"),
    ("--tw-prose-captions", "var(--color-muted-foreground)"),
    ("--tw-prose-code", "var(--color-foreground)"),
    ("--tw-prose-pre-code", "var(--color-foreground)"),
    ("--tw-prose-pre-bg", "var(--color-muted)"),
    ("--tw-prose-th-borders", "var(--color-border)"),
    ("--tw-prose-td-borders", "var(--color-border)"),
];

#[derive(Debug, Clone, PartialEq)]
enum Kind {
    Heading(u8),
    Para,
    Quote,
    List(Option<u64>),
    Item,
    CodeBlock(Option<String>),
    Strong,
    Em,
    Del,
    Link(String, String),
    Image(String, String),
    Table(Vec<Alignment>),
    Head,
    Row,
    Cell,
    Transparent,
}

#[derive(Debug, Clone, PartialEq)]
enum Node {
    Text(String),
    Code(String),
    SoftBreak,
    HardBreak,
    Rule,
    Task(bool),
    Elem(Kind, Vec<Node>),
}

fn heading_number(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn start_kind(tag: Tag) -> Kind {
    match tag {
        Tag::Heading { level, .. } => Kind::Heading(heading_number(level)),
        Tag::Paragraph => Kind::Para,
        Tag::BlockQuote(_) => Kind::Quote,
        Tag::List(start) => Kind::List(start),
        Tag::Item => Kind::Item,
        Tag::CodeBlock(CodeBlockKind::Fenced(info)) => {
            let language = info.split_whitespace().next().unwrap_or("");
            Kind::CodeBlock((!language.is_empty()).then(|| language.to_string()))
        }
        Tag::CodeBlock(CodeBlockKind::Indented) => Kind::CodeBlock(None),
        Tag::Strong => Kind::Strong,
        Tag::Emphasis => Kind::Em,
        Tag::Strikethrough => Kind::Del,
        Tag::Link {
            dest_url, title, ..
        } => Kind::Link(dest_url.to_string(), title.to_string()),
        Tag::Image {
            dest_url, title, ..
        } => Kind::Image(dest_url.to_string(), title.to_string()),
        Tag::Table(alignments) => Kind::Table(alignments),
        Tag::TableHead => Kind::Head,
        Tag::TableRow => Kind::Row,
        Tag::TableCell => Kind::Cell,
        _ => Kind::Transparent,
    }
}

fn push_node(stack: &mut [(Option<Kind>, Vec<Node>)], node: Node) {
    if let Some(frame) = stack.last_mut() {
        frame.1.push(node);
    }
}

fn markdown_tree(content: &str) -> Vec<Node> {
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut stack: Vec<(Option<Kind>, Vec<Node>)> = vec![(None, Vec::new())];
    for event in Parser::new_ext(content, options) {
        match event {
            Event::Start(tag) => stack.push((Some(start_kind(tag)), Vec::new())),
            Event::End(_) => {
                if stack.len() > 1 {
                    if let Some((Some(kind), children)) = stack.pop() {
                        push_node(&mut stack, Node::Elem(kind, children));
                    }
                }
            }
            Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => {
                push_node(&mut stack, Node::Text(text.to_string()))
            }
            Event::Code(code) => push_node(&mut stack, Node::Code(code.to_string())),
            Event::InlineMath(math) | Event::DisplayMath(math) => {
                push_node(&mut stack, Node::Text(math.to_string()))
            }
            Event::SoftBreak => push_node(&mut stack, Node::SoftBreak),
            Event::HardBreak => push_node(&mut stack, Node::HardBreak),
            Event::Rule => push_node(&mut stack, Node::Rule),
            Event::TaskListMarker(done) => push_node(&mut stack, Node::Task(done)),
            Event::FootnoteReference(label) => push_node(&mut stack, Node::Text(label.to_string())),
        }
    }
    stack.pop().map(|frame| frame.1).unwrap_or_default()
}

const INLINE_CODE_STYLE: &str = "background-color: var(--color-muted); color: var(--color-foreground); border: 1px solid var(--color-border); padding: 0.125rem 0.375rem; border-radius: 0.25rem; font-size: 0.875em; font-family: ui-monospace, monospace;";

const BLOCK_CODE_STYLE: &str = "background-color: var(--color-muted); color: var(--color-foreground); padding: 0.75rem 1rem; border-radius: 0.5rem; font-family: ui-monospace, monospace; font-size: 0.875em; white-space: pre; overflow-x: auto;";

fn plain_text(items: &[Node]) -> String {
    let mut out = String::new();
    for item in items {
        match item {
            Node::Text(text) | Node::Code(text) => out.push_str(text),
            Node::SoftBreak | Node::HardBreak => out.push('\n'),
            Node::Elem(_, children) => out.push_str(&plain_text(children)),
            Node::Rule | Node::Task(_) => {}
        }
    }
    out
}

fn nodes(items: &[Node]) -> Element {
    rsx! {
        for item in items.iter() {
            {node(item)}
        }
    }
}

fn cell_style(align: Option<&Alignment>) -> Option<&'static str> {
    match align {
        Some(Alignment::Left) => Some("text-align: left;"),
        Some(Alignment::Center) => Some("text-align: center;"),
        Some(Alignment::Right) => Some("text-align: right;"),
        _ => None,
    }
}

fn cells(items: &[Node], alignments: &[Alignment], header: bool) -> Element {
    rsx! {
        for (index, item) in items.iter().enumerate() {
            if let Node::Elem(_, children) = item {
                if header {
                    th {
                        class: "border border-[var(--color-border)] bg-[var(--color-muted)] px-4 py-2 text-left font-semibold",
                        style: cell_style(alignments.get(index)),
                        {nodes(children)}
                    }
                } else {
                    td {
                        class: "border border-[var(--color-border)] px-4 py-2",
                        style: cell_style(alignments.get(index)),
                        {nodes(children)}
                    }
                }
            }
        }
    }
}

fn table(alignments: &[Alignment], children: &[Node]) -> Element {
    let rows: Vec<&Vec<Node>> = children
        .iter()
        .filter_map(|child| match child {
            Node::Elem(Kind::Row, cells) => Some(cells),
            _ => None,
        })
        .collect();
    rsx! {
        div { class: "overflow-x-auto my-4",
            table { class: "min-w-full border-collapse border border-[var(--color-border)]",
                for child in children.iter() {
                    if let Node::Elem(Kind::Head, head) = child {
                        thead {
                            tr { {cells(head, alignments, true)} }
                        }
                    }
                }
                if !rows.is_empty() {
                    tbody {
                        for row in rows.iter() {
                            tr { {cells(row, alignments, false)} }
                        }
                    }
                }
            }
        }
    }
}

fn node(item: &Node) -> Element {
    match item {
        Node::Text(text) => rsx! { "{text}" },
        Node::Code(code) => rsx! { code { style: INLINE_CODE_STYLE, "{code}" } },
        Node::SoftBreak => rsx! { "\n" },
        Node::HardBreak => rsx! { br {} },
        Node::Rule => rsx! { hr {} },
        Node::Task(done) => rsx! { input { r#type: "checkbox", disabled: true, checked: *done } },
        Node::Elem(kind, children) => match kind {
            Kind::Heading(1) => rsx! { h1 { {nodes(children)} } },
            Kind::Heading(2) => rsx! { h2 { {nodes(children)} } },
            Kind::Heading(3) => rsx! { h3 { {nodes(children)} } },
            Kind::Heading(4) => rsx! { h4 { {nodes(children)} } },
            Kind::Heading(5) => rsx! { h5 { {nodes(children)} } },
            Kind::Heading(_) => rsx! { h6 { {nodes(children)} } },
            Kind::Para | Kind::Transparent => rsx! { p { {nodes(children)} } },
            Kind::Quote => rsx! {
                blockquote {
                    class: "border-l-heavy border-[var(--color-primary)] pl-4 italic text-[var(--color-foreground)] my-4",
                    {nodes(children)}
                }
            },
            Kind::List(Some(start)) => {
                let start = (*start != 1).then(|| start.to_string());
                rsx! { ol { start: start, {nodes(children)} } }
            }
            Kind::List(None) => rsx! { ul { {nodes(children)} } },
            Kind::Item => rsx! { li { {nodes(children)} } },
            Kind::CodeBlock(language) => {
                let code = plain_text(children);
                let code = code.strip_suffix('\n').unwrap_or(&code).to_string();
                let class = language.as_ref().map(|l| format!("language-{l}"));
                rsx! {
                    pre { style: BLOCK_CODE_STYLE,
                        code { class: class, "{code}" }
                    }
                }
            }
            Kind::Strong => rsx! { strong { {nodes(children)} } },
            Kind::Em => rsx! { em { {nodes(children)} } },
            Kind::Del => rsx! { del { {nodes(children)} } },
            Kind::Link(href, title) => {
                let external = href.starts_with("http");
                let target = external.then_some("_blank");
                let rel = external.then_some("noopener noreferrer");
                let title = (!title.is_empty()).then(|| title.clone());
                rsx! {
                    a {
                        class: "text-[var(--color-primary)] hover:underline",
                        href: "{href}",
                        title: title,
                        target: target,
                        rel: rel,
                        {nodes(children)}
                    }
                }
            }
            Kind::Image(src, title) => {
                let alt = plain_text(children);
                let title = (!title.is_empty()).then(|| title.clone());
                rsx! { img { src: "{src}", alt: "{alt}", title: title } }
            }
            Kind::Table(alignments) => table(alignments, children),
            Kind::Head | Kind::Row | Kind::Cell => nodes(children),
        },
    }
}

impl DioxusPatternRenderer for MarkdownContentRenderer {
    fn pattern_name(&self) -> &str {
        Self::NAME
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        _view: &DioxusView<'_>,
        _registry: &DioxusPatternRegistry,
    ) -> Element {
        let content = get_string_or(props, "content", "");
        let direction = get_string_or(props, "direction", "ltr");
        let mut style = PROSE_VARS
            .iter()
            .map(|(name, value)| format!("{name}: {value};"))
            .collect::<Vec<_>>()
            .join(" ");
        style.push_str(&format!(" direction: {direction};"));
        let class = merge_class_name(
            "bg-transparent rounded-none shadow-none prose max-w-none",
            props,
        );
        let tree = markdown_tree(&content);
        rsx! {
            div { class: "{class}", style: "{style}", ..shared_root_attrs(props),
                {nodes(&tree)}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn elem(kind: Kind, children: Vec<Node>) -> Node {
        Node::Elem(kind, children)
    }

    fn text(value: &str) -> Node {
        Node::Text(value.to_string())
    }

    #[test]
    fn nested_lists_keep_structure() {
        let tree = markdown_tree("- a\n  - b\n- c\n");
        let inner = elem(Kind::List(None), vec![elem(Kind::Item, vec![text("b")])]);
        let expected = vec![elem(
            Kind::List(None),
            vec![
                elem(Kind::Item, vec![text("a"), inner]),
                elem(Kind::Item, vec![text("c")]),
            ],
        )];
        assert_eq!(tree, expected);
    }

    #[test]
    fn gfm_table_strikethrough_and_tasks() {
        let tree = markdown_tree("| a | b |\n|:-|-:|\n| 1 | 2 |\n\n~~x~~\n\n- [x] done\n");
        assert!(matches!(
            &tree[0],
            Node::Elem(Kind::Table(a), _) if a == &vec![Alignment::Left, Alignment::Right]
        ));
        assert_eq!(
            tree[1],
            elem(Kind::Para, vec![elem(Kind::Del, vec![text("x")])])
        );
        assert_eq!(
            tree[2],
            elem(
                Kind::List(None),
                vec![elem(Kind::Item, vec![Node::Task(true), text("done")])]
            )
        );
    }

    #[test]
    fn fenced_code_carries_language_and_html_stays_text() {
        let tree = markdown_tree("```rust\nfn a() {}\n```\n\n<b>x</b>\n");
        assert_eq!(
            tree[0],
            elem(
                Kind::CodeBlock(Some("rust".into())),
                vec![text("fn a() {}\n")]
            )
        );
        assert!(!format!("{:?}", tree[1]).contains("Strong"));
    }
}
