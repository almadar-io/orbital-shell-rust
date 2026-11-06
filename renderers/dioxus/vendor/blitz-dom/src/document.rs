use crate::events::handle_dom_event;
use crate::font_metrics::BlitzFontMetricsProvider;
use crate::layout::construct::{
    build_inline_layout_into, collect_layout_children, ConstructionTask, ConstructionTaskData,
    ConstructionTaskResult, ConstructionTaskResultData,
};
use crate::layout::damage::{ALL_DAMAGE, CONSTRUCT_BOX, CONSTRUCT_DESCENDENT, CONSTRUCT_FC};
use crate::mutator::ViewportMut;
use crate::net::{CssHandler, Resource, StylesheetLoader};
use crate::node::{ImageData, NodeFlags, RasterImageData, SpecialElementData, Status, TextBrush};
use crate::stylo_to_cursor_icon::stylo_to_cursor_icon;
use crate::traversal::TreeTraverser;
use crate::url::DocumentUrl;
use crate::util::ImageType;
use crate::{
    DocumentConfig, DocumentMutator, DummyHtmlParserProvider, ElementData, EventDriver,
    HtmlParserProvider, Node, NodeData, NoopEventHandler, TextNodeData, DEFAULT_CSS,
    NON_INCREMENTAL,
};
use blitz_traits::devtools::DevtoolSettings;
use blitz_traits::events::{BlitzDragTransfer, DomEvent, HitResult, UiEvent};
use blitz_traits::navigation::{DummyNavigationProvider, NavigationProvider};
use blitz_traits::net::{DummyNetProvider, NetProvider, Request, SharedProvider};
use blitz_traits::shell::{ColorScheme, DummyShellProvider, ShellProvider, Viewport};
use cursor_icon::CursorIcon;
use debug_timer::debug_timer;
use linebender_resource_handle::Blob;
use markup5ever::local_name;
use parley::{FontContext, LayoutContext};
use selectors::{matching::QuirksMode, Element};
use slab::Slab;
use std::any::Any;
use std::cell::RefCell;
use std::collections::{BTreeMap, Bound, HashMap, HashSet};
use std::ops::{Deref, DerefMut};
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::Context as TaskContext;
use style::animation::DocumentAnimationSet;
use style::attr::{AttrIdentifier, AttrValue};
use style::data::{ElementData as StyloElementData, ElementStyles};
use style::media_queries::MediaType;
use style::properties::style_structs::Font;
use style::properties::ComputedValues;
use style::queries::values::PrefersColorScheme;
use style::selector_parser::ServoElementSnapshot;
use style::servo_arc::Arc as ServoArc;
use style::values::computed::Overflow;
use style::values::GenericAtomIdent;
use style::Atom;
use style::{
    dom::{TDocument, TNode},
    media_queries::{Device, MediaList},
    selector_parser::SnapshotMap,
    shared_lock::{SharedRwLock, StylesheetGuards},
    stylesheets::{AllowImportRules, DocumentStyleSheet, Origin, Stylesheet},
    stylist::Stylist,
};
use taffy::AvailableSpace;
use url::Url;

#[cfg(feature = "parallel-construct")]
use rayon::prelude::*;

thread_local! {
    static LAYOUT_CTX: RefCell<Option<Box<LayoutContext<TextBrush>>>> = const { RefCell::new(None) };
    static FONT_CTX: RefCell<Option<Box<FontContext>>> = const { RefCell::new(None) };
}

#[cfg(feature = "incremental")]
use style::selector_parser::RestyleDamage;

/// Abstraction over wrappers around [`BaseDocument`] to allow for them all to
/// be driven by [`blitz-shell`](https://docs.rs/blitz-shell)
pub trait Document: Deref<Target = BaseDocument> + DerefMut + 'static {
    fn load_resource(&mut self, resource: Resource) {
        self.deref_mut().load_resource(resource);
    }

    /// Update the [`Document`] in response to a [`UiEvent`] (click, keypress, etc)
    fn handle_ui_event(&mut self, event: UiEvent) {
        let mut driver = EventDriver::new((*self).mutate(), NoopEventHandler);
        driver.handle_ui_event(event);
    }

    /// Poll any pending async operations, and flush changes to the underlying [`BaseDocument`]
    fn poll(&mut self, task_context: Option<TaskContext>) -> bool {
        // Default implementation does nothing
        let _ = task_context;
        false
    }

    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// Get the [`Document`]'s id
    fn id(&self) -> usize {
        self.id
    }
}

pub struct BaseDocument {
    /// ID of the document
    id: usize,

    // Config
    /// Base url for resolving linked resources (stylesheets, images, fonts, etc)
    pub(crate) url: DocumentUrl,
    // Devtool settings. Currently used to render debug overlays
    pub(crate) devtool_settings: DevtoolSettings,
    // Viewport details such as the dimensions, HiDPI scale, and zoom factor,
    pub(crate) viewport: Viewport,
    // Scroll within our viewport
    pub(crate) viewport_scroll: crate::Point<f64>,

    /// A slab-backed tree of nodes
    ///
    /// We pin the tree to a guarantee to the nodes it creates that the tree is stable in memory.
    /// There is no way to create the tree - publicly or privately - that would invalidate that invariant.
    pub(crate) nodes: Box<Slab<Node>>,

    // Stylo
    /// The Stylo engine
    pub(crate) stylist: Stylist,
    pub(crate) animations: DocumentAnimationSet,
    /// Stylo shared lock
    pub(crate) guard: SharedRwLock,
    /// Stylo invalidation map. We insert into this map prior to mutating nodes.
    pub(crate) snapshots: SnapshotMap,

    // Parley contexts
    /// A Parley font context
    pub(crate) font_ctx: Arc<Mutex<parley::FontContext>>,
    /// A Parley layout context
    pub(crate) layout_ctx: parley::LayoutContext<TextBrush>,

    /// The node which is currently hovered (if any)
    pub(crate) hover_node_id: Option<usize>,
    /// The node which is currently focussed (if any)
    pub(crate) focus_node_id: Option<usize>,
    /// The node which is currently active (if any)
    pub(crate) active_node_id: Option<usize>,
    /// The node which recieved a mousedown event (if any)
    pub(crate) mousedown_node_id: Option<usize>,
    pub(crate) drag_candidate: Option<(usize, f32, f32)>,
    pub(crate) drag_active: Option<(usize, Option<usize>, BlitzDragTransfer, bool)>,

    /// Whether there are active CSS animations/transitions (so we should re-render every frame)
    pub(crate) has_active_animations: bool,
    /// Whether there is a <canvas> element in the DOM (so we should re-render every frame)
    pub(crate) has_canvas: bool,

    /// Map of node ID's for fast lookups
    pub(crate) nodes_to_id: HashMap<String, usize>,
    /// Map of `<style>` and `<link>` node IDs to their associated stylesheet
    pub(crate) nodes_to_stylesheet: BTreeMap<usize, DocumentStyleSheet>,
    /// Stylesheets added by the useragent
    /// where the key is the hashed CSS
    pub(crate) ua_stylesheets: HashMap<String, DocumentStyleSheet>,
    /// Map from form control node ID's to their associated forms node ID's
    pub(crate) controls_to_form: HashMap<usize, usize>,
    /// Set of changed nodes for updating the accessibility tree
    pub(crate) changed_nodes: HashSet<usize>,
    /// Set of changed nodes for updating the accessibility tree
    pub(crate) deferred_construction_nodes: Vec<ConstructionTask>,

    // Service providers
    /// Network provider. Can be used to fetch assets.
    pub net_provider: Arc<dyn NetProvider<Resource>>,
    /// Navigation provider. Can be used to navigate to a new page (bubbles up the event
    /// on e.g. clicking a Link)
    pub navigation_provider: Arc<dyn NavigationProvider>,
    /// Shell provider. Can be used to request a redraw or set the cursor icon
    pub shell_provider: Arc<dyn ShellProvider>,
    /// HTML parser provider. Used to parse HTML for setInnerHTML
    pub html_parser_provider: Arc<dyn HtmlParserProvider>,
}

pub(crate) fn make_device(viewport: &Viewport, font_ctx: Arc<Mutex<FontContext>>) -> Device {
    let width = viewport.window_size.0 as f32 / viewport.scale();
    let height = viewport.window_size.1 as f32 / viewport.scale();
    let viewport_size = euclid::Size2D::new(width, height);
    let device_pixel_ratio = euclid::Scale::new(viewport.scale());

    Device::new(
        MediaType::screen(),
        selectors::matching::QuirksMode::NoQuirks,
        viewport_size,
        device_pixel_ratio,
        Box::new(BlitzFontMetricsProvider { font_ctx }),
        ComputedValues::initial_values_with_font_override(Font::initial_values()),
        match viewport.color_scheme {
            ColorScheme::Light => PrefersColorScheme::Light,
            ColorScheme::Dark => PrefersColorScheme::Dark,
        },
    )
}

impl BaseDocument {
    /// Create a new (empty) [`BaseDocument`] with the specified configuration
    pub fn new(config: DocumentConfig) -> Self {
        static ID_GENERATOR: AtomicUsize = AtomicUsize::new(1);

        let id = ID_GENERATOR.fetch_add(1, Ordering::SeqCst);

        let mut font_ctx = config.font_ctx.unwrap_or_default();
        font_ctx
            .collection
            .register_fonts(Blob::new(Arc::new(crate::BULLET_FONT) as _), None);
        font_ctx
            .collection
            .register_fonts(Blob::new(Arc::new(crate::SYMBOL_FONT) as _), None);
        let font_ctx = Arc::new(Mutex::new(font_ctx));

        let viewport = config.viewport.unwrap_or_default();
        let device = make_device(&viewport, font_ctx.clone());
        let stylist = Stylist::new(device, QuirksMode::NoQuirks);
        let snapshots = SnapshotMap::new();
        let nodes = Box::new(Slab::new());
        let guard = SharedRwLock::new();
        let nodes_to_id = HashMap::new();

        // Make sure we turn on stylo features
        style_config::set_bool("layout.flexbox.enabled", true);
        style_config::set_bool("layout.grid.enabled", true);
        style_config::set_bool("layout.legacy_layout", true);
        style_config::set_bool("layout.unimplemented", true);
        style_config::set_bool("layout.columns.enabled", true);
        // ORBITAL PATCH (blitz-dom 0.2.4, parity): enable container-type/container-name and @container.
        style_config::set_bool("layout.container-queries.enabled", true);

        let base_url = config
            .base_url
            .and_then(|url| DocumentUrl::from_str(&url).ok())
            .unwrap_or_default();

        let net_provider = config
            .net_provider
            .unwrap_or_else(|| Arc::new(DummyNetProvider));
        let navigation_provider = config
            .navigation_provider
            .unwrap_or_else(|| Arc::new(DummyNavigationProvider));
        let shell_provider = config
            .shell_provider
            .unwrap_or_else(|| Arc::new(DummyShellProvider));
        let html_parser_provider = config
            .html_parser_provider
            .unwrap_or_else(|| Arc::new(DummyHtmlParserProvider));

        let mut doc = Self {
            id,
            guard,
            nodes,
            stylist,
            animations: DocumentAnimationSet::default(),
            snapshots,
            nodes_to_id,
            viewport,
            devtool_settings: DevtoolSettings::default(),
            viewport_scroll: crate::Point::ZERO,
            url: base_url,
            ua_stylesheets: HashMap::new(),
            nodes_to_stylesheet: BTreeMap::new(),
            font_ctx,
            layout_ctx: parley::LayoutContext::new(),

            hover_node_id: None,
            focus_node_id: None,
            active_node_id: None,
            mousedown_node_id: None,
            drag_candidate: None,
            drag_active: None,
            has_active_animations: false,
            has_canvas: false,
            changed_nodes: HashSet::new(),
            deferred_construction_nodes: Vec::new(),
            controls_to_form: HashMap::new(),
            net_provider,
            navigation_provider,
            shell_provider,
            html_parser_provider,
        };

        // Initialise document with root Document node
        doc.create_node(NodeData::Document);
        doc.root_node_mut().flags.insert(NodeFlags::IS_IN_DOCUMENT);

        match config.ua_stylesheets {
            Some(stylesheets) => {
                for ss in &stylesheets {
                    doc.add_user_agent_stylesheet(ss);
                }
            }
            None => doc.add_user_agent_stylesheet(DEFAULT_CSS),
        }

        // Stylo data on the root node container is needed to render the node
        let stylo_element_data = StyloElementData {
            styles: ElementStyles {
                primary: Some(
                    ComputedValues::initial_values_with_font_override(Font::initial_values())
                        .to_arc(),
                ),
                ..Default::default()
            },
            ..Default::default()
        };
        *doc.root_node().stylo_element_data.borrow_mut() = Some(stylo_element_data);

        doc
    }

    /// Set the Document's networking provider
    pub fn set_net_provider(&mut self, net_provider: SharedProvider<Resource>) {
        self.net_provider = net_provider;
    }

    /// Set the Document's navigation provider
    pub fn set_navigation_provider(&mut self, navigation_provider: Arc<dyn NavigationProvider>) {
        self.navigation_provider = navigation_provider;
    }

    /// Set the Document's shell provider
    pub fn set_shell_provider(&mut self, shell_provider: Arc<dyn ShellProvider>) {
        self.shell_provider = shell_provider;
    }

    /// Set the Document's html parser provider
    pub fn set_html_parser_provider(&mut self, html_parser_provider: Arc<dyn HtmlParserProvider>) {
        self.html_parser_provider = html_parser_provider;
    }

    /// Set base url for resolving linked resources (stylesheets, images, fonts, etc)
    pub fn set_base_url(&mut self, url: &str) {
        self.url = DocumentUrl::from(Url::parse(url).unwrap());
    }

    pub fn guard(&self) -> &SharedRwLock {
        &self.guard
    }

    pub fn tree(&self) -> &Slab<Node> {
        &self.nodes
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn get_node(&self, node_id: usize) -> Option<&Node> {
        self.nodes.get(node_id)
    }

    pub fn get_node_mut(&mut self, node_id: usize) -> Option<&mut Node> {
        self.nodes.get_mut(node_id)
    }

    pub fn get_focussed_node_id(&self) -> Option<usize> {
        self.focus_node_id
            .or(self.try_root_element().map(|el| el.id))
    }

    pub fn mutate<'doc>(&'doc mut self) -> DocumentMutator<'doc> {
        DocumentMutator::new(self)
    }

    pub fn handle_dom_event<F: FnMut(DomEvent)>(
        &mut self,
        event: &mut DomEvent,
        dispatch_event: F,
    ) {
        handle_dom_event(self, event, dispatch_event)
    }

    pub fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    /// Find the label's bound input elements:
    /// the element id referenced by the "for" attribute of a given label element
    /// or the first input element which is nested in the label
    /// Note that although there should only be one bound element,
    /// we return all possibilities instead of just the first
    /// in order to allow the caller to decide which one is correct
    pub fn label_bound_input_element(&self, label_node_id: usize) -> Option<&Node> {
        let label_element = self.nodes[label_node_id].element_data()?;
        if let Some(target_element_dom_id) = label_element.attr(local_name!("for")) {
            TreeTraverser::new(self)
                .filter_map(|id| {
                    let node = self.get_node(id)?;
                    let element_data = node.element_data()?;
                    if element_data.name.local != local_name!("input") {
                        return None;
                    }
                    let id = element_data.id.as_ref()?;
                    if *id == *target_element_dom_id {
                        Some(node)
                    } else {
                        None
                    }
                })
                .next()
        } else {
            TreeTraverser::new_with_root(self, label_node_id)
                .filter_map(|child_id| {
                    let node = self.get_node(child_id)?;
                    let element_data = node.element_data()?;
                    if element_data.name.local == local_name!("input") {
                        Some(node)
                    } else {
                        None
                    }
                })
                .next()
        }
    }

    pub fn toggle_checkbox(el: &mut ElementData) -> bool {
        let Some(is_checked) = el.checkbox_input_checked_mut() else {
            return false;
        };
        *is_checked = !*is_checked;

        *is_checked
    }

    pub fn toggle_radio(&mut self, radio_set_name: String, target_radio_id: usize) {
        for i in 0..self.nodes.len() {
            let node = &mut self.nodes[i];
            if let Some(node_data) = node.data.downcast_element_mut() {
                if node_data.attr(local_name!("name")) == Some(&radio_set_name) {
                    let was_clicked = i == target_radio_id;
                    let Some(is_checked) = node_data.checkbox_input_checked_mut() else {
                        continue;
                    };
                    *is_checked = was_clicked;
                }
            }
        }
    }

    pub fn set_style_property(&mut self, node_id: usize, name: &str, value: &str) {
        self.nodes[node_id]
            .element_data_mut()
            .unwrap()
            .set_style_property(name, value, &self.guard, self.url.url_extra_data());
    }

    pub fn remove_style_property(&mut self, node_id: usize, name: &str) {
        self.nodes[node_id]
            .element_data_mut()
            .unwrap()
            .remove_style_property(name, &self.guard, self.url.url_extra_data());
    }

    pub fn root_node(&self) -> &Node {
        &self.nodes[0]
    }

    pub fn root_node_mut(&mut self) -> &mut Node {
        &mut self.nodes[0]
    }

    pub fn try_root_element(&self) -> Option<&Node> {
        TDocument::as_node(&self.root_node()).first_element_child()
    }

    pub fn root_element(&self) -> &Node {
        TDocument::as_node(&self.root_node())
            .first_element_child()
            .unwrap()
            .as_element()
            .unwrap()
    }

    pub fn create_node(&mut self, node_data: NodeData) -> usize {
        let slab_ptr = self.nodes.as_mut() as *mut Slab<Node>;
        let guard = self.guard.clone();

        let entry = self.nodes.vacant_entry();
        let id = entry.key();
        entry.insert(Node::new(slab_ptr, id, guard, node_data));

        // Mark the new node as changed.
        self.changed_nodes.insert(id);
        id
    }

    /// Whether the document has been mutated
    pub fn has_changes(&self) -> bool {
        !self.changed_nodes.is_empty()
    }

    pub fn create_text_node(&mut self, text: &str) -> usize {
        let content = text.to_string();
        let data = NodeData::Text(TextNodeData::new(content));
        self.create_node(data)
    }

    pub fn deep_clone_node(&mut self, node_id: usize) -> usize {
        // Load existing node
        let node = &self.nodes[node_id];
        let data = node.data.clone();
        let children = node.children.clone();

        // Create new node
        let new_node_id = self.create_node(data);

        // Recursively clone children
        let new_children: Vec<usize> = children
            .into_iter()
            .map(|child_id| self.deep_clone_node(child_id))
            .collect();
        for &child_id in &new_children {
            self.nodes[child_id].parent = Some(new_node_id);
        }
        self.nodes[new_node_id].children = new_children;

        new_node_id
    }

    pub(crate) fn remove_and_drop_pe(&mut self, node_id: usize) -> Option<Node> {
        fn remove_pe_ignoring_parent(doc: &mut BaseDocument, node_id: usize) -> Option<Node> {
            let mut node = doc.nodes.try_remove(node_id);
            if let Some(node) = &mut node {
                for &child in &node.children {
                    remove_pe_ignoring_parent(doc, child);
                }
            }
            node
        }

        let node = remove_pe_ignoring_parent(self, node_id);

        // Update child_idx values
        if let Some(parent_id) = node.as_ref().and_then(|node| node.parent) {
            let parent = &mut self.nodes[parent_id];
            parent.children.retain(|id| *id != node_id);
        }

        node
    }

    pub(crate) fn resolve_url(&self, raw: &str) -> url::Url {
        self.url.resolve_relative(raw).unwrap_or_else(|| {
            panic!(
                "to be able to resolve {raw} with the base_url: {:?}",
                *self.url
            )
        })
    }

    pub fn print_tree(&self) {
        crate::util::walk_tree(0, self.root_node());
    }

    pub fn print_subtree(&self, node_id: usize) {
        crate::util::walk_tree(0, &self.nodes[node_id]);
    }

    pub fn reload_resource_by_href(&mut self, href_to_reload: &str) {
        for &node_id in self.nodes_to_stylesheet.keys() {
            let node = &self.nodes[node_id];
            let Some(element) = node.element_data() else {
                continue;
            };

            if element.name.local == local_name!("link") {
                if let Some(href) = element.attr(local_name!("href")) {
                    // println!("Node {node_id} {href} {href_to_reload} {} {}", resolved_href.as_str(), resolved_href.as_str() == url_to_reload);
                    if href == href_to_reload {
                        let resolved_href = self.resolve_url(href);
                        self.net_provider.fetch(
                            self.id(),
                            Request::get(resolved_href.clone()),
                            Box::new(CssHandler {
                                node: node_id,
                                source_url: resolved_href,
                                guard: self.guard.clone(),
                                provider: self.net_provider.clone(),
                            }),
                        );
                    }
                }
            }
        }
    }

    pub fn process_style_element(&mut self, target_id: usize) {
        let css = self.nodes[target_id].text_content();
        let css = html_escape::decode_html_entities(&css);
        let sheet = self.make_stylesheet(&css, Origin::Author);
        self.add_stylesheet_for_node(sheet, target_id);
        // ORBITAL PATCH (blitz-dom 0.2.4, parity): a body-level `<style>` inserted after the
        // first style pass must restyle already-styled elements (and register its @keyframes),
        // as a browser does; Stylo does not track late-attached sheets.
        self.restyle_from_loaded_sheets();
    }

    pub fn remove_user_agent_stylesheet(&mut self, contents: &str) {
        if let Some(sheet) = self.ua_stylesheets.remove(contents) {
            self.stylist.remove_stylesheet(sheet, &self.guard.read());
        }
    }

    pub fn add_user_agent_stylesheet(&mut self, css: &str) {
        let sheet = self.make_stylesheet(css, Origin::UserAgent);
        self.ua_stylesheets.insert(css.to_string(), sheet.clone());
        self.stylist.append_stylesheet(sheet, &self.guard.read());
    }

    pub fn make_stylesheet(&self, css: impl AsRef<str>, origin: Origin) -> DocumentStyleSheet {
        let data = Stylesheet::from_str(
            css.as_ref(),
            self.url.url_extra_data(),
            origin,
            ServoArc::new(self.guard.wrap(MediaList::empty())),
            self.guard.clone(),
            Some(&StylesheetLoader(self.id, self.net_provider.clone())),
            None,
            QuirksMode::NoQuirks,
            AllowImportRules::Yes,
        );

        DocumentStyleSheet(ServoArc::new(data))
    }

    pub fn upsert_stylesheet_for_node(&mut self, node_id: usize) {
        let raw_styles = self.nodes[node_id].text_content();
        let sheet = self.make_stylesheet(raw_styles, Origin::Author);
        self.add_stylesheet_for_node(sheet, node_id);
    }

    pub fn add_stylesheet_for_node(&mut self, stylesheet: DocumentStyleSheet, node_id: usize) {
        let old = self.nodes_to_stylesheet.insert(node_id, stylesheet.clone());

        if let Some(old) = old {
            self.stylist.remove_stylesheet(old, &self.guard.read())
        }

        // Store data on element
        let element = &mut self.nodes[node_id].element_data_mut().unwrap();
        element.special_data = SpecialElementData::Stylesheet(stylesheet.clone());

        // TODO: Nodes could potentially get reused so ordering by node_id might be wrong.
        let insertion_point = self
            .nodes_to_stylesheet
            .range((Bound::Excluded(node_id), Bound::Unbounded))
            .next()
            .map(|(_, sheet)| sheet);

        if let Some(insertion_point) = insertion_point {
            self.stylist.insert_stylesheet_before(
                stylesheet,
                insertion_point.clone(),
                &self.guard.read(),
            )
        } else {
            self.stylist
                .append_stylesheet(stylesheet, &self.guard.read())
        }
    }

    pub fn load_resource(&mut self, resource: Resource) {
        match resource {
            Resource::Css(node_id, css) => {
                self.add_stylesheet_for_node(css, node_id);
            }
            Resource::Image(node_id, kind, width, height, image_data) => {
                let node = self.get_node_mut(node_id).unwrap();

                match kind {
                    ImageType::Image => {
                        node.element_data_mut().unwrap().special_data =
                            SpecialElementData::Image(Box::new(ImageData::Raster(
                                RasterImageData::new(width, height, image_data),
                            )));

                        // Clear layout cache
                        node.cache.clear();
                        node.insert_damage(ALL_DAMAGE);
                    }
                    ImageType::Background(idx) => {
                        if let Some(Some(bg_image)) = node
                            .element_data_mut()
                            .and_then(|el| el.background_images.get_mut(idx))
                        {
                            bg_image.status = Status::Ok;
                            bg_image.image =
                                ImageData::Raster(RasterImageData::new(width, height, image_data))
                        }
                    }
                    ImageType::Mask(idx) => {
                        if let Some(Some(mask_image)) = node
                            .element_data_mut()
                            .and_then(|el| el.mask_images.get_mut(idx))
                        {
                            mask_image.status = Status::Ok;
                            mask_image.image =
                                ImageData::Raster(RasterImageData::new(width, height, image_data))
                        }
                    }
                }
            }
            Resource::ImageError(_, _) => {}
            #[cfg(feature = "svg")]
            Resource::Svg(node_id, kind, tree) => {
                let node = self.get_node_mut(node_id).unwrap();

                match kind {
                    ImageType::Image => {
                        node.element_data_mut().unwrap().special_data =
                            SpecialElementData::Image(Box::new(ImageData::Svg(tree)));

                        // Clear layout cache
                        node.cache.clear();
                        node.insert_damage(ALL_DAMAGE);
                    }
                    ImageType::Background(idx) => {
                        if let Some(Some(bg_image)) = node
                            .element_data_mut()
                            .and_then(|el| el.background_images.get_mut(idx))
                        {
                            bg_image.status = Status::Ok;
                            bg_image.image = ImageData::Svg(tree);
                        }
                    }
                    ImageType::Mask(idx) => {
                        if let Some(Some(mask_image)) = node
                            .element_data_mut()
                            .and_then(|el| el.mask_images.get_mut(idx))
                        {
                            mask_image.status = Status::Ok;
                            mask_image.image = ImageData::Svg(tree);
                        }
                    }
                }
            }
            Resource::Font(bytes) => {
                let font = Blob::new(Arc::new(bytes));

                // TODO: Implement FontInfoOveride
                // TODO: Investigate eliminating double-box
                let mut font_ctx = self.font_ctx.lock().unwrap();
                font_ctx.collection.register_fonts(font.clone(), None);

                #[cfg(feature = "parallel-construct")]
                {
                    let doc_font_ctx = &*font_ctx;
                    rayon::broadcast(|_ctx| {
                        FONT_CTX.with_borrow_mut(|font_ctx| {
                            match font_ctx {
                                None => {
                                    println!(
                                        "Initialising FontContext for thread {:?}",
                                        std::thread::current().id()
                                    );
                                    *font_ctx = Some(Box::new(doc_font_ctx.clone()));
                                }
                                Some(font_ctx) => {
                                    font_ctx.collection.register_fonts(font.clone(), None);
                                }
                            };
                        })
                    });
                }
                drop(font_ctx);

                // TODO: see if we can only invalidate if resolved fonts may have changed
                self.invalidate_inline_contexts();
            }
            Resource::None => {
                // ORBITAL PATCH: an `@import`ed sheet finished loading into its
                // already-attached parent sheet, which Stylo does not track; restyle.
                self.restyle_from_loaded_sheets();
            }
            _ => {}
        }
    }

    // ORBITAL PATCH: remote `@import` rules arrive after the first style pass, so the
    // whole tree must be restyled once they are in place.
    fn restyle_from_loaded_sheets(&mut self) {
        self.stylist
            .force_stylesheet_origins_dirty(style::stylesheets::OriginSet::all());
        let root = self.root_element().id;
        self.nodes[root].set_restyle_hint(
            style::invalidation::element::restyle_hints::RestyleHint::restyle_subtree(),
        );
    }

    pub fn snapshot_node(&mut self, node_id: usize) {
        let node = &mut self.nodes[node_id];
        let opaque_node_id = TNode::opaque(&&*node);
        node.has_snapshot = true;
        node.snapshot_handled
            .store(false, std::sync::atomic::Ordering::SeqCst);

        // TODO: handle invalidations other than hover
        if let Some(_existing_snapshot) = self.snapshots.get_mut(&opaque_node_id) {
            // Do nothing
            // TODO: update snapshot
        } else {
            let attrs: Option<Vec<_>> = node.attrs().map(|attrs| {
                attrs
                    .iter()
                    .map(|attr| {
                        let ident = AttrIdentifier {
                            local_name: GenericAtomIdent(attr.name.local.clone()),
                            name: GenericAtomIdent(attr.name.local.clone()),
                            namespace: GenericAtomIdent(attr.name.ns.clone()),
                            prefix: None,
                        };

                        let value = if attr.name.local == local_name!("id") {
                            AttrValue::Atom(Atom::from(&*attr.value))
                        } else if attr.name.local == local_name!("class") {
                            let classes = attr
                                .value
                                .split_ascii_whitespace()
                                .map(Atom::from)
                                .collect();
                            AttrValue::TokenList(attr.value.clone(), classes)
                        } else {
                            AttrValue::String(attr.value.clone())
                        };

                        (ident, value)
                    })
                    .collect()
            });

            let changed_attrs = attrs
                .as_ref()
                .map(|attrs| attrs.iter().map(|attr| attr.0.name.clone()).collect())
                .unwrap_or_default();

            self.snapshots.insert(
                opaque_node_id,
                ServoElementSnapshot {
                    state: Some(node.element_state),
                    attrs,
                    changed_attrs,
                    class_changed: true,
                    id_changed: true,
                    other_attributes_changed: true,
                },
            );
        }
    }

    pub fn snapshot_node_and(&mut self, node_id: usize, cb: impl FnOnce(&mut Node)) {
        self.snapshot_node(node_id);
        cb(&mut self.nodes[node_id]);
    }

    /// Restyle the tree and then relayout it
    pub fn resolve(&mut self, current_time_for_animations: f64) {
        if TDocument::as_node(&&self.nodes[0])
            .first_element_child()
            .is_none()
        {
            println!("No DOM - not resolving");
            return;
        }

        let root_node_id = self.root_element().id;
        debug_timer!(timer, feature = "log_phase_times");

        // ORBITAL PATCH (blitz-dom 0.2.4, parity): style runs before layout, so container
        // queries see the previous layout's container sizes. Re-run style + layout until the
        // sizes a container reported to style match its laid-out size, capped at
        // MAX_CONTAINER_QUERY_RESTYLES extra passes to avoid oscillation.
        const MAX_CONTAINER_QUERY_RESTYLES: usize = 3;
        let mut restyles = 0;
        loop {
            // we need to resolve stylist first since it will need to drive our layout bits
            self.resolve_stylist(current_time_for_animations);
            timer.record_time("style");

            // Propagate damage flags (from mutation and restyles) up and down the tree
            #[cfg(feature = "incremental")]
            self.propagate_damage_flags(root_node_id, RestyleDamage::empty());
            #[cfg(feature = "incremental")]
            timer.record_time("damage");

            // Fix up tree for layout (insert anonymous blocks as necessary, etc)
            self.resolve_layout_children();
            self.hoist_fixed_layout_children();
            timer.record_time("construct");

            self.resolve_deferred_tasks();
            timer.record_time("pconstruct");

            // Merge stylo into taffy
            self.flush_styles_to_layout(root_node_id);
            self.resolve_fixed_viewport_sizes();
            self.neutralize_indefinite_percent_heights();
            timer.record_time("flush");

            // Next we resolve layout with the data resolved by stlist
            self.resolve_layout();
            #[cfg(feature = "svg")]
            self.reparse_unsized_svgs();
            timer.record_time("layout");

            if !self.restyle_changed_containers() {
                break;
            }
            if restyles >= MAX_CONTAINER_QUERY_RESTYLES {
                #[cfg(feature = "tracing")]
                tracing::warn!(
                    "container query sizes still changing after {MAX_CONTAINER_QUERY_RESTYLES} restyle passes; keeping the last layout"
                );
                break;
            }
            restyles += 1;
        }

        // Clear all damage
        #[cfg(feature = "incremental")]
        {
            for (_, node) in self.nodes.iter_mut() {
                node.clear_damage_mut();
            }
            timer.record_time("c_damage");
        }

        timer.print_times("Resolve: ");
    }

    // ORBITAL PATCH (blitz-dom 0.2.4, parity): flag the descendants of every size container whose
    // laid-out content box differs (on the axes its `container-type` exposes) from the size it
    // reported to the last style pass. Returns whether any restyle was requested.
    fn restyle_changed_containers(&mut self) -> bool {
        use style::values::computed::ContainerType;

        let mut changed = Vec::new();
        for (id, node) in self.nodes.iter() {
            let Some(used) = node.container_size_used.get() else {
                continue;
            };
            let Some((container_type, vertical)) = node.primary_styles().map(|style| {
                (
                    style.clone_container_type(),
                    style.writing_mode.is_vertical(),
                )
            }) else {
                node.container_size_used.set(None);
                continue;
            };
            if !container_type.is_size_container_type() {
                node.container_size_used.set(None);
                continue;
            }
            let now = node.content_box_size();
            let differs = if container_type.intersects(ContainerType::SIZE) {
                now != used
            } else if vertical {
                now[1] != used[1]
            } else {
                now[0] != used[0]
            };
            if differs {
                changed.push((id, now));
            }
        }

        let any = !changed.is_empty();
        for (id, now) in changed {
            self.nodes[id].container_size_used.set(Some(now));
            self.nodes[id].set_restyle_hint(
                style::invalidation::element::restyle_hints::RestyleHint::restyle_subtree(),
            );
        }
        any
    }

    // Takes (x, y) co-ordinates (relative to the )
    pub fn hit(&self, x: f32, y: f32) -> Option<HitResult> {
        if TDocument::as_node(&&self.nodes[0])
            .first_element_child()
            .is_none()
        {
            println!("No DOM - not resolving");
            return None;
        }

        self.root_element().hit_with_viewport_scroll(
            x,
            y,
            self.viewport_scroll.x as f32,
            self.viewport_scroll.y as f32,
        )
    }

    pub fn focus_next_node(&mut self) -> Option<usize> {
        let focussed_node_id = self.get_focussed_node_id()?;
        let id = self.next_node(&self.nodes[focussed_node_id], |node| node.is_focussable())?;
        self.set_focus_to(id);
        Some(id)
    }

    /// Clear the focussed node
    pub fn clear_focus(&mut self) {
        if let Some(id) = self.focus_node_id {
            self.snapshot_node_and(id, |node| node.blur());
            self.focus_node_id = None;
        }
    }

    pub fn set_mousedown_node_id(&mut self, node_id: Option<usize>) {
        self.mousedown_node_id = node_id;
    }

    pub fn is_dragging(&self) -> bool {
        self.drag_active.is_some()
    }
    pub fn set_focus_to(&mut self, focus_node_id: usize) -> bool {
        if Some(focus_node_id) == self.focus_node_id {
            return false;
        }

        #[cfg(feature = "tracing")]
        tracing::info!("Focussed node {focus_node_id}");

        // Remove focus from the old node
        if let Some(id) = self.focus_node_id {
            self.snapshot_node_and(id, |node| node.blur());
        }

        // Focus the new node
        self.snapshot_node_and(focus_node_id, |node| node.focus());

        self.focus_node_id = Some(focus_node_id);

        true
    }

    pub fn active_node(&mut self) -> bool {
        let Some(hover_node_id) = self.get_hover_node_id() else {
            return false;
        };

        if let Some(active_node_id) = self.active_node_id {
            if active_node_id == hover_node_id {
                return true;
            }
            self.unactive_node();
        }

        let active_node_id = Some(hover_node_id);

        let node_path = self.maybe_node_layout_ancestors(active_node_id);
        for &id in node_path.iter() {
            self.snapshot_node_and(id, |node| node.active());
        }

        self.active_node_id = active_node_id;

        true
    }

    pub fn unactive_node(&mut self) -> bool {
        let Some(active_node_id) = self.active_node_id.take() else {
            return false;
        };

        let node_path = self.maybe_node_layout_ancestors(Some(active_node_id));
        for &id in node_path.iter() {
            self.snapshot_node_and(id, |node| node.unactive());
        }

        true
    }

    pub fn set_hover_to(&mut self, x: f32, y: f32) -> bool {
        let hit = self.hit(x, y);
        let hover_node_id = hit.map(|hit| hit.node_id);
        self.set_hover_node(hover_node_id)
    }

    pub fn clear_hover(&mut self) -> bool {
        self.set_hover_node(None)
    }

    fn set_hover_node(&mut self, hover_node_id: Option<usize>) -> bool {
        // Return early if the new node is the same as the already-hovered node
        if hover_node_id == self.hover_node_id {
            return false;
        }

        let old_node_path = self.maybe_node_layout_ancestors(self.hover_node_id);
        let new_node_path = self.maybe_node_layout_ancestors(hover_node_id);
        let same_count = old_node_path
            .iter()
            .zip(&new_node_path)
            .take_while(|(o, n)| o == n)
            .count();
        for &id in old_node_path.iter().skip(same_count) {
            self.snapshot_node_and(id, |node| node.unhover());
        }
        for &id in new_node_path.iter().skip(same_count) {
            self.snapshot_node_and(id, |node| node.hover());
        }

        self.hover_node_id = hover_node_id;

        // Update the cursor
        let cursor = self.get_cursor().unwrap_or_default();
        self.shell_provider.set_cursor(cursor);

        // Request redraw
        self.shell_provider.request_redraw();

        true
    }

    pub fn get_hover_node_id(&self) -> Option<usize> {
        self.hover_node_id
    }

    pub fn set_viewport(&mut self, viewport: Viewport) {
        let scale_has_changed = viewport.scale_f64() != self.viewport.scale_f64();
        self.viewport = viewport;
        self.set_stylist_device(make_device(&self.viewport, self.font_ctx.clone()));
        self.scroll_viewport_by(0.0, 0.0); // Clamp scroll offset

        if scale_has_changed {
            self.invalidate_inline_contexts();
        }
    }

    pub fn viewport(&self) -> &Viewport {
        &self.viewport
    }

    pub fn viewport_mut(&mut self) -> ViewportMut<'_> {
        ViewportMut::new(self)
    }

    pub fn zoom_by(&mut self, increment: f32) {
        *self.viewport.zoom_mut() += increment;
        self.set_viewport(self.viewport.clone());
    }

    pub fn zoom_to(&mut self, zoom: f32) {
        *self.viewport.zoom_mut() = zoom;
        self.set_viewport(self.viewport.clone());
    }

    pub fn get_viewport(&self) -> Viewport {
        self.viewport.clone()
    }

    pub fn devtools(&self) -> &DevtoolSettings {
        &self.devtool_settings
    }

    pub fn devtools_mut(&mut self) -> &mut DevtoolSettings {
        &mut self.devtool_settings
    }

    pub fn is_animating(&self) -> bool {
        self.has_canvas | self.has_active_animations
    }

    /// Update the device and reset the stylist to process the new size
    pub fn set_stylist_device(&mut self, device: Device) {
        let origins = {
            let guard = &self.guard;
            let guards = StylesheetGuards {
                author: &guard.read(),
                ua_or_user: &guard.read(),
            };
            self.stylist.set_device(device, &guards)
        };
        self.stylist.force_stylesheet_origins_dirty(origins);
    }

    pub fn stylist_device(&mut self) -> &Device {
        self.stylist.device()
    }

    /// Ensure that the layout_children field is populated for all nodes
    pub fn resolve_layout_children(&mut self) {
        resolve_layout_children_recursive(self, self.root_node().id);

        fn resolve_layout_children_recursive(doc: &mut BaseDocument, node_id: usize) {
            let mut damage = doc.nodes[node_id].damage().unwrap_or(ALL_DAMAGE);
            let _flags = doc.nodes[node_id].flags;

            #[cfg(feature = "svg")]
            let svg_restyle = !damage.is_empty()
                && doc.nodes[node_id]
                    .data
                    .is_element_with_tag_name(&local_name!("svg"));
            #[cfg(not(feature = "svg"))]
            let svg_restyle = false;

            if NON_INCREMENTAL || svg_restyle || damage.intersects(CONSTRUCT_FC | CONSTRUCT_BOX) {
                //} || flags.contains(NodeFlags::IS_INLINE_ROOT) {
                let mut layout_children = Vec::new();
                let mut anonymous_block: Option<usize> = None;
                collect_layout_children(doc, node_id, &mut layout_children, &mut anonymous_block);

                // Recurse into newly collected layout children
                for child_id in layout_children.iter().copied() {
                    resolve_layout_children_recursive(doc, child_id);
                    doc.nodes[child_id].layout_parent.set(Some(node_id));
                    if let Some(data) = doc.nodes[child_id].stylo_element_data.get_mut() {
                        data.damage
                            .remove(CONSTRUCT_DESCENDENT | CONSTRUCT_FC | CONSTRUCT_BOX);
                    }
                }

                *doc.nodes[node_id].layout_children.borrow_mut() = Some(layout_children.clone());
                *doc.nodes[node_id].paint_children.borrow_mut() = Some(layout_children);

                damage.remove(CONSTRUCT_DESCENDENT | CONSTRUCT_FC | CONSTRUCT_BOX);
                // damage.insert(RestyleDamage::RELAYOUT | RestyleDamage::REPAINT);
            } else {
                //if damage.contains(CONSTRUCT_DESCENDENT) {
                let layout_children = doc.nodes[node_id].layout_children.borrow_mut().take();
                if let Some(layout_children) = layout_children {
                    // Recurse into previously computed layout children
                    for child_id in layout_children.iter().copied() {
                        resolve_layout_children_recursive(doc, child_id);
                        doc.nodes[child_id].layout_parent.set(Some(node_id));
                    }

                    *doc.nodes[node_id].layout_children.borrow_mut() = Some(layout_children);
                }

                // damage.remove(CONSTRUCT_DESCENDENT);
                // damage.insert(RestyleDamage::RELAYOUT | RestyleDamage::REPAINT);
            }

            doc.nodes[node_id].set_damage(damage);
        }
    }

    fn hoist_fixed_layout_children(&mut self) {
        use style::properties::generated::longhands::position::computed_value::T as Position;

        let root = self.root_element().id;
        let fixed: Vec<_> = self
            .nodes
            .iter()
            .filter_map(|(id, node)| {
                (id != root
                    && node
                        .primary_styles()
                        .is_some_and(|style| style.clone_position() == Position::Fixed)
                    && node
                        .layout_parent
                        .get()
                        .is_some_and(|parent| parent != root))
                .then_some(id)
            })
            .collect();

        for id in fixed {
            let parent = self.nodes[id].layout_parent.get().unwrap();
            if let Some(children) = self.nodes[parent].layout_children.borrow_mut().as_mut() {
                children.retain(|child| *child != id);
            }
            if let Some(children) = self.nodes[parent].paint_children.borrow_mut().as_mut() {
                children.retain(|child| *child != id);
            }
            self.nodes[id].layout_parent.set(Some(root));
            self.nodes[root]
                .layout_children
                .borrow_mut()
                .get_or_insert_with(Vec::new)
                .push(id);
            self.nodes[root]
                .paint_children
                .borrow_mut()
                .get_or_insert_with(Vec::new)
                .push(id);
        }
    }

    fn resolve_fixed_viewport_sizes(&mut self) {
        use style::properties::generated::longhands::position::computed_value::T as Position;

        let size = self.stylist.device().au_viewport_size();
        let viewport_width = size.width.to_f32_px();
        let viewport_height = size.height.to_f32_px();
        for (_, node) in self.nodes.iter_mut() {
            if !node
                .primary_styles()
                .is_some_and(|style| style.clone_position() == Position::Fixed)
            {
                continue;
            }
            let style = &mut node.style;
            if style.size.width.is_auto() {
                if let (Some(left), Some(right)) = (
                    style
                        .inset
                        .left
                        .resolve_to_option(viewport_width, crate::layout::resolve_calc_value),
                    style
                        .inset
                        .right
                        .resolve_to_option(viewport_width, crate::layout::resolve_calc_value),
                ) {
                    style.size.width =
                        taffy::Dimension::length((viewport_width - left - right).max(0.0));
                }
            }
            if style.size.height.is_auto() {
                if let (Some(top), Some(bottom)) = (
                    style
                        .inset
                        .top
                        .resolve_to_option(viewport_height, crate::layout::resolve_calc_value),
                    style
                        .inset
                        .bottom
                        .resolve_to_option(viewport_height, crate::layout::resolve_calc_value),
                ) {
                    style.size.height =
                        taffy::Dimension::length((viewport_height - top - bottom).max(0.0));
                }
            }
        }
    }

    pub fn resolve_deferred_tasks(&mut self) {
        let mut deferred_construction_nodes = std::mem::take(&mut self.deferred_construction_nodes);

        // Deduplicate deferred tasks by node_id to avoid redundant work
        deferred_construction_nodes.sort_unstable_by_key(|task| task.node_id);
        deferred_construction_nodes.dedup_by_key(|task| task.node_id);

        #[cfg(feature = "parallel-construct")]
        let iter = deferred_construction_nodes.into_par_iter();
        #[cfg(not(feature = "parallel-construct"))]
        let iter = deferred_construction_nodes.into_iter();

        let results: Vec<ConstructionTaskResult> = iter
            .map(|task: ConstructionTask| match task.data {
                ConstructionTaskData::InlineLayout(mut layout) => {
                    #[cfg(feature = "parallel-construct")]
                    let mut layout_ctx = LAYOUT_CTX
                        .take()
                        .unwrap_or_else(|| Box::new(LayoutContext::new()));
                    #[cfg(feature = "parallel-construct")]
                    let layout_ctx_mut = &mut layout_ctx;

                    #[cfg(feature = "parallel-construct")]
                    let mut font_ctx = FONT_CTX
                        .take()
                        .unwrap_or_else(|| Box::new(self.font_ctx.lock().unwrap().clone()));
                    #[cfg(feature = "parallel-construct")]
                    let font_ctx_mut = &mut font_ctx;

                    #[cfg(not(feature = "parallel-construct"))]
                    let layout_ctx_mut = &mut self.layout_ctx;
                    #[cfg(not(feature = "parallel-construct"))]
                    let font_ctx_mut = &mut *self.font_ctx.lock().unwrap();

                    layout.content_widths = None;
                    build_inline_layout_into(
                        &self.nodes,
                        layout_ctx_mut,
                        font_ctx_mut,
                        &mut layout,
                        self.viewport.scale(),
                        task.node_id,
                    );

                    #[cfg(feature = "parallel-construct")]
                    {
                        LAYOUT_CTX.set(Some(layout_ctx));
                        FONT_CTX.set(Some(font_ctx));
                    }

                    // If layout doesn't contain any inline boxes, then it is safe to populate the content_widths
                    // cache during this parallelized stage.
                    // if layout.layout.inline_boxes().is_empty() {
                    //     layout.content_widths();
                    // }

                    ConstructionTaskResult {
                        node_id: task.node_id,
                        data: ConstructionTaskResultData::InlineLayout(layout),
                    }
                }
            })
            .collect();

        for result in results {
            match result.data {
                ConstructionTaskResultData::InlineLayout(layout) => {
                    self.nodes[result.node_id].cache.clear();
                    self.nodes[result.node_id]
                        .element_data_mut()
                        .unwrap()
                        .inline_layout_data = Some(layout);
                }
            }
        }

        self.deferred_construction_nodes.clear();
    }

    /// Walk the nodes now that they're properly styled and transfer their styles to the taffy style system
    ///
    /// TODO: update taffy to use an associated type instead of slab key
    /// TODO: update taffy to support traited styles so we don't even need to rely on taffy for storage
    // ORBITAL PATCH (blitz-dom 0.2.4, parity): CSS resolves a percentage height only against
    // a containing block whose height is definite; otherwise it behaves as `auto`. Taffy
    // resolves it against the block parent's available space, stretching `h-full` cards to
    // the viewport or content-size extremes. Walk the block-container chain and turn
    // percentage heights under an auto-height block into auto. Flex/grid parents are left to
    // Taffy because their items can be definite through stretch.
    fn neutralize_indefinite_percent_heights(&mut self) {
        let root = self.root_element().id;
        let root_definite = !self.nodes[root].style.size.height.is_auto();
        let is_block = self.nodes[root].style.display == taffy::Display::Block;
        self.neutralize_percent_heights_below(root, root_definite, is_block);
    }

    fn neutralize_percent_heights_below(
        &mut self,
        id: usize,
        self_definite: bool,
        self_is_block: bool,
    ) {
        let children = self.nodes[id]
            .layout_children
            .borrow()
            .clone()
            .unwrap_or_default();
        for child in children {
            let node = &mut self.nodes[child];
            let positioned = node.style.position == taffy::Position::Absolute;
            let is_pct = |d: taffy::Dimension| {
                let raw = d.into_raw();
                raw.tag() == taffy::CompactLength::PERCENT_TAG || raw.is_calc()
            };
            if self_is_block && !self_definite && !positioned {
                if is_pct(node.style.size.height) {
                    node.style.size.height = taffy::Dimension::auto();
                }
                if is_pct(node.style.min_size.height) {
                    node.style.min_size.height = taffy::Dimension::auto();
                }
                if is_pct(node.style.max_size.height) {
                    node.style.max_size.height = taffy::Dimension::auto();
                }
            }
            let height = node.style.size.height;
            let definite = if height.is_auto() {
                !self_is_block
            } else if is_pct(height) {
                self_definite
            } else {
                true
            };
            let is_block = node.style.display == taffy::Display::Block;
            self.neutralize_percent_heights_below(child, definite, is_block);
        }
    }

    // ORBITAL PATCH (blitz-dom 0.2.4, parity): hoisted `position: fixed` boxes are laid out
    // by Taffy as absolute children of the root element, whose height is the whole document,
    // so a `bottom`/`right` inset anchors to the document end. The initial containing block
    // is the viewport: re-anchor those axes against the viewport after layout.
    fn anchor_fixed_to_viewport(&mut self) {
        use style::properties::generated::longhands::position::computed_value::T as Position;

        let size = self.stylist.device().au_viewport_size();
        let viewport_width = size.width.to_f32_px();
        let viewport_height = size.height.to_f32_px();
        for (_, node) in self.nodes.iter_mut() {
            if !node
                .primary_styles()
                .is_some_and(|style| style.clone_position() == Position::Fixed)
            {
                continue;
            }
            let inset = node.style.inset;
            let layout = &mut node.final_layout;
            if inset.top.is_auto() {
                if let Some(bottom) = inset
                    .bottom
                    .resolve_to_option(viewport_height, crate::layout::resolve_calc_value)
                {
                    layout.location.y =
                        viewport_height - bottom - layout.margin.bottom - layout.size.height;
                }
            }
            if inset.left.is_auto() {
                if let Some(right) = inset
                    .right
                    .resolve_to_option(viewport_width, crate::layout::resolve_calc_value)
                {
                    layout.location.x =
                        viewport_width - right - layout.margin.right - layout.size.width;
                }
            }
        }
    }

    // ORBITAL PATCH (blitz-dom 0.2, parity): an inline `<svg>` with no viewBox/width/height is
    // parsed by usvg against a 100x100 default; re-parse it against its laid-out content box so
    // percentage geometry and patterns cover the box. Runs only when the box differs from the
    // parsed size, so it is stable after one pass (inherent size then equals the box size).
    #[cfg(feature = "svg")]
    fn reparse_unsized_svgs(&mut self) {
        let mut todo: Vec<(usize, f32, f32)> = Vec::new();
        for (id, node) in self.nodes.iter() {
            let Some(el) = node.element_data() else {
                continue;
            };
            if el.name.local != local_name!("svg") {
                continue;
            }
            let SpecialElementData::Image(img) = &el.special_data else {
                continue;
            };
            let ImageData::Svg(tree) = &**img else {
                continue;
            };
            let sized = el.attrs().iter().any(|a| {
                let n: &str = &a.name.local;
                n.eq_ignore_ascii_case("viewBox")
                    || n.eq_ignore_ascii_case("width")
                    || n.eq_ignore_ascii_case("height")
            });
            if sized {
                continue;
            }
            let l = &node.final_layout;
            let w =
                l.size.width - l.padding.left - l.padding.right - l.border.left - l.border.right;
            let h =
                l.size.height - l.padding.top - l.padding.bottom - l.border.top - l.border.bottom;
            if w <= 0.0 || h <= 0.0 {
                continue;
            }
            let ts = tree.size();
            if (ts.width() - w).abs() < 0.5 && (ts.height() - h).abs() < 0.5 {
                continue;
            }
            todo.push((id, w, h));
        }
        for (id, w, h) in todo {
            let mut html = self.nodes[id].outer_html();
            if !html.contains("xmlns") {
                html = html.replace("<svg", "<svg xmlns=\"http://www.w3.org/2000/svg\"");
            }
            if let Ok(tree) = crate::util::parse_svg_with_default_size(html.as_bytes(), w, h) {
                if let Some(el) = self.nodes[id].element_data_mut() {
                    el.special_data = SpecialElementData::Image(Box::new(tree.into()));
                }
            }
        }
    }

    pub fn resolve_layout(&mut self) {
        let size = self.stylist.device().au_viewport_size();

        let available_space = taffy::Size {
            width: AvailableSpace::Definite(size.width.to_f32_px()),
            height: AvailableSpace::Definite(size.height.to_f32_px()),
        };

        let root_element_id = taffy::NodeId::from(self.root_element().id);

        // println!("\n\nRESOLVE LAYOUT\n===========\n");

        taffy::compute_root_layout(self, root_element_id, available_space);
        taffy::round_layout(self, root_element_id);
        self.anchor_fixed_to_viewport();

        // println!("\n\n");
        // taffy::print_tree(self, root_node_id)
    }

    pub fn get_cursor(&self) -> Option<CursorIcon> {
        // todo: cache this on the node itself
        let node = &self.nodes[self.get_hover_node_id()?];

        let style = node.primary_styles()?;
        let keyword = stylo_to_cursor_icon(style.clone_cursor().keyword);

        // Return cursor from style if it is non-auto
        if keyword != CursorIcon::Default {
            return Some(keyword);
        }

        // Return text cursor for text nodes and text inputs
        if node.is_text_node()
            || node
                .element_data()
                .is_some_and(|e| e.text_input_data().is_some())
        {
            return Some(CursorIcon::Text);
        }

        // Use "pointer" cursor if any ancestor is a link
        let mut maybe_node = Some(node);
        while let Some(node) = maybe_node {
            if node.is_link() {
                return Some(CursorIcon::Pointer);
            }

            maybe_node = node.layout_parent.get().map(|node_id| node.with(node_id));
        }

        // Else fallback to default cursor
        Some(CursorIcon::Default)
    }

    pub fn scroll_node_by(&mut self, node_id: usize, x: f64, y: f64) {
        self.scroll_node_by_has_changed(node_id, x, y);
    }

    /// Scroll a node by given x and y
    /// Will bubble scrolling up to parent node once it can no longer scroll further
    /// If we're already at the root node, bubbles scrolling up to the viewport
    pub fn scroll_node_by_has_changed(&mut self, node_id: usize, x: f64, y: f64) -> bool {
        let Some(node) = self.nodes.get_mut(node_id) else {
            return false;
        };

        let is_html_or_body = node.data.downcast_element().is_some_and(|e| {
            let tag = &e.name.local;
            tag == "html" || tag == "body"
        });

        let (can_x_scroll, can_y_scroll) = node
            .primary_styles()
            .map(|styles| {
                (
                    matches!(styles.clone_overflow_x(), Overflow::Scroll | Overflow::Auto),
                    matches!(styles.clone_overflow_y(), Overflow::Scroll | Overflow::Auto)
                        || (styles.clone_overflow_y() == Overflow::Visible && is_html_or_body),
                )
            })
            .unwrap_or((false, false));

        let initial = node.scroll_offset;
        let new_x = node.scroll_offset.x - x;
        let new_y = node.scroll_offset.y - y;

        let mut bubble_x = 0.0;
        let mut bubble_y = 0.0;

        let scroll_width = node.final_layout.scroll_width() as f64;
        let scroll_height = node.final_layout.scroll_height() as f64;

        // If we're past our scroll bounds, transfer remainder of scrolling to parent/viewport
        if !can_x_scroll {
            bubble_x = x
        } else if new_x < 0.0 {
            bubble_x = -new_x;
            node.scroll_offset.x = 0.0;
        } else if new_x > scroll_width {
            bubble_x = scroll_width - new_x;
            node.scroll_offset.x = scroll_width;
        } else {
            node.scroll_offset.x = new_x;
        }

        if !can_y_scroll {
            bubble_y = y
        } else if new_y < 0.0 {
            bubble_y = -new_y;
            node.scroll_offset.y = 0.0;
        } else if new_y > scroll_height {
            bubble_y = scroll_height - new_y;
            node.scroll_offset.y = scroll_height;
        } else {
            node.scroll_offset.y = new_y;
        }

        let has_changed = node.scroll_offset != initial;

        if bubble_x != 0.0 || bubble_y != 0.0 {
            if let Some(parent) = node.parent {
                return self.scroll_node_by_has_changed(parent, bubble_x, bubble_y) | has_changed;
            } else {
                return self.scroll_viewport_by_has_changed(bubble_x, bubble_y) | has_changed;
            }
        }

        has_changed
    }

    pub fn scroll_viewport_by(&mut self, x: f64, y: f64) {
        self.scroll_viewport_by_has_changed(x, y);
    }

    /// Scroll the viewport by the given values
    pub fn scroll_viewport_by_has_changed(&mut self, x: f64, y: f64) -> bool {
        let content_size = self.root_element().final_layout.size;
        let new_scroll = (self.viewport_scroll.x - x, self.viewport_scroll.y - y);
        let window_width = self.viewport.window_size.0 as f64 / self.viewport.scale() as f64;
        let window_height = self.viewport.window_size.1 as f64 / self.viewport.scale() as f64;

        let initial = self.viewport_scroll;
        self.viewport_scroll.x = f64::max(
            0.0,
            f64::min(new_scroll.0, content_size.width as f64 - window_width),
        );
        self.viewport_scroll.y = f64::max(
            0.0,
            f64::min(new_scroll.1, content_size.height as f64 - window_height),
        );

        self.viewport_scroll != initial
    }

    pub fn viewport_scroll(&self) -> crate::Point<f64> {
        self.viewport_scroll
    }

    pub fn set_viewport_scroll(&mut self, scroll: crate::Point<f64>) {
        self.viewport_scroll = scroll;
    }

    pub fn find_title_node(&self) -> Option<&Node> {
        TreeTraverser::new(self)
            .find(|node_id| {
                self.nodes[*node_id]
                    .data
                    .is_element_with_tag_name(&local_name!("title"))
            })
            .map(|node_id| &self.nodes[node_id])
    }

    pub(crate) fn compute_has_canvas(&self) -> bool {
        TreeTraverser::new(self).any(|node_id| {
            let node = &self.nodes[node_id];
            let Some(element) = node.element_data() else {
                return false;
            };
            if element.name.local == local_name!("canvas") && element.has_attr(local_name!("src")) {
                return true;
            }

            false
        })
    }
}

impl AsRef<BaseDocument> for BaseDocument {
    fn as_ref(&self) -> &BaseDocument {
        self
    }
}

impl AsMut<BaseDocument> for BaseDocument {
    fn as_mut(&mut self) -> &mut BaseDocument {
        self
    }
}

#[cfg(test)]
mod accessibility_change_tests {
    use super::{BaseDocument, DocumentConfig};

    #[test]
    fn creating_text_marks_document_changed() {
        let mut doc = BaseDocument::new(DocumentConfig::default());
        doc.changed_nodes.clear();
        assert!(!doc.has_changes());

        doc.create_text_node("task");
        assert!(doc.has_changes());
    }

    #[test]
    fn native_document_bundles_symbol_font_for_action_icons() {
        let doc = BaseDocument::new(DocumentConfig::default());
        let mut font_ctx = doc.font_ctx.lock().unwrap();
        assert!(font_ctx
            .collection
            .family_by_name("Noto Sans Symbols 2")
            .is_some());
    }
}
