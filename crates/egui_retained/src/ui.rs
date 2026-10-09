//! The retained tree and the frame: nodes made once and changed in place, laid out by taffy
//! when something changed, hit-tested, sent their events, and painted.

use crate::element::{Effects, Element, Event, EventCx, EventKind, Handler, MeasureCx, PaintCx};
use crate::input::{Button, Input, InputEvent, Key, Modifiers, PlatformOutput};
use crate::paint::Painter;
use crate::style::{Cursor, Look, Rule, State, Theme, Visual};
use ecolor::Color32;
use emath::{Pos2, Rect, Vec2};
use epaint::{ClippedPrimitive, Stroke, TextureId};
use std::any::Any;
use taffy::prelude::{AvailableSpace, Size, TaffyTree};

/// A node of the tree. Stays valid until the node is removed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub(crate) taffy::NodeId);

/// Where a node is painted: the page, then what opens over it (menus, dialogs), then tips.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Layer {
    Base = 0,
    Overlay = 1,
    Tooltip = 2,
}

const LAYERS: usize = 3;

/// Which way a scrolling node scrolls.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScrollAxes {
    pub x: bool,
    pub y: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Scroll {
    pub axes: ScrollAxes,
    pub offset: Vec2,
    /// How far it can scroll (content beyond the box).
    pub max: Vec2,
    /// When the bar was last used (it fades out).
    pub active_at: f64,
}

/// Where an anchored node (a menu) stands relative to its anchor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Anchor {
    /// Under the anchor, left edges together.
    Below { gap: f32, match_width: bool },
    /// Above it.
    Above { gap: f32 },
    /// To its right.
    Right { gap: f32 },
    /// Centred in the window (a dialog).
    Center,
}

pub(crate) struct NodeData {
    pub element: Box<dyn Element>,
    pub classes: Vec<String>,
    pub visual: Visual,
    /// The layout as the node was given it (the element's, then its own changes), and the
    /// element's alone: classes patch the element's, and what the node changed wins over them.
    pub layout: taffy::Style,
    pub base_layout: taffy::Style,
    pub state: State,
    pub handlers: Vec<(EventKind, Handler)>,
    pub scroll: Option<Scroll>,
    pub look: Look,
    /// A transition under way: the look it started from and when.
    pub from: Option<(Look, f64)>,
    pub rect: Rect,
    pub clip: Rect,
    pub layer: Layer,
    pub visible: bool,
    pub tooltip: Option<String>,
    pub name: Option<String>,
    pub anchor: Option<(NodeId, Anchor)>,
    /// Paint the children before (`false`) or after its own painting.
    pub restyle: bool,
}

/// The output of a frame.
pub struct Frame {
    pub primitives: Vec<ClippedPrimitive>,
    pub textures: epaint::textures::TexturesDelta,
    pub platform: PlatformOutput,
    /// Draw again this many seconds from now (an animation, a caret), or only on input.
    pub repaint_after: Option<f32>,
    pub pixels_per_point: f32,
}

/// The whole interface: the tree, its fonts and theme, and the state between frames.
pub struct Ui {
    pub(crate) tree: TaffyTree<NodeData>,
    roots: [taffy::NodeId; LAYERS],
    pub(crate) fonts: epaint::Fonts,
    theme: Theme,
    messages: Vec<Box<dyn Any>>,
    hovered: Option<NodeId>,
    pressed: Option<(NodeId, Button, Pos2)>,
    dragging: bool,
    focused: Option<NodeId>,
    captured: Option<NodeId>,
    pointer: Option<Pos2>,
    last_click: Option<(NodeId, f64, u32)>,
    /// The nodes in the order they were painted, with what of them is seen (hit-testing
    /// walks it from the top).
    order: Vec<(NodeId, Rect)>,
    time: f64,
    screen: Vec2,
    ppp: f32,
    modifiers: Modifiers,
    layout_dirty: bool,
    style_dirty: bool,
    repaint: bool,
    deferred: Vec<Box<dyn FnOnce(&mut Ui)>>,
    tooltip: Option<(NodeId, f64)>,
    tex_manager: epaint::textures::TextureManager,
    font_texture: Option<TextureId>,
    tess: epaint::TessellationOptions,
    output: PlatformOutput,
    animating: Option<f32>,
}

/// The root of each layer: the window's size, its children laid out over each other.
fn root_style() -> taffy::Style {
    taffy::Style {
        size: Size { width: taffy::Dimension::percent(1.0), height: taffy::Dimension::percent(1.0) },
        flex_direction: taffy::FlexDirection::Column,
        ..Default::default()
    }
}

struct Root;
impl Element for Root {
    fn hit_test(&self) -> bool {
        false
    }
    fn paint_box(&self) -> bool {
        false
    }
}

impl Ui {
    /// A new interface with epaint's default fonts and an empty theme.
    pub fn new() -> Ui {
        Self::with_fonts(epaint::text::FontDefinitions::default())
    }

    pub fn with_fonts(fonts: epaint::text::FontDefinitions) -> Ui {
        let mut tree: TaffyTree<NodeData> = TaffyTree::new();
        let theme = Theme::default();
        let mut make_root = |layer: Layer| {
            let mut s = root_style();
            if layer != Layer::Base {
                // (over the page, laid out on their own: absolutely placed children)
                s.position = taffy::Position::Absolute;
            }
            tree.new_leaf_with_context(s.clone(), NodeData::new(Box::new(Root), s, layer, &theme)).expect("root")
        };
        let roots = [make_root(Layer::Base), make_root(Layer::Overlay), make_root(Layer::Tooltip)];
        let options = epaint::text::TextOptions::default();
        let mut tex_manager = epaint::textures::TextureManager::default();
        let fonts = epaint::Fonts::new(options, fonts);
        let font_texture = Some(tex_manager.alloc("fonts".into(), epaint::ImageData::Color(std::sync::Arc::new(fonts.image())), epaint::textures::TextureOptions::LINEAR));
        Ui {
            tree,
            roots,
            fonts,
            theme,
            messages: Vec::new(),
            hovered: None,
            pressed: None,
            dragging: false,
            focused: None,
            captured: None,
            pointer: None,
            last_click: None,
            order: Vec::new(),
            time: 0.0,
            screen: Vec2::new(800.0, 600.0),
            ppp: 1.0,
            modifiers: Modifiers::default(),
            layout_dirty: true,
            style_dirty: true,
            repaint: true,
            deferred: Vec::new(),
            tooltip: None,
            tex_manager,
            font_texture,
            tess: epaint::TessellationOptions::default(),
            output: PlatformOutput::default(),
            animating: None,
        }
    }

    // --- the tree --------------------------------------------------------------------------

    /// The root of a layer: add the page under `root(Layer::Base)`.
    pub fn root(&self, layer: Layer) -> NodeId {
        NodeId(self.roots[layer as usize])
    }

    /// A new node of `element`, last child of `parent`.
    pub fn add(&mut self, parent: NodeId, element: impl Element) -> NodeId {
        let layer = self.data(parent).map(|d| d.layer).unwrap_or(Layer::Base);
        let mut style = taffy::Style::default();
        element.layout(&mut style);
        let data = NodeData::new(Box::new(element), style.clone(), layer, &self.theme);
        let id = self.tree.new_leaf_with_context(style, data).expect("taffy node");
        let _ = self.tree.add_child(parent.0, id);
        self.style_dirty = true;
        self.layout_dirty = true;
        NodeId(id)
    }

    /// A new node of `element` at `index` among `parent`'s children.
    pub fn insert(&mut self, parent: NodeId, index: usize, element: impl Element) -> NodeId {
        let id = self.add(parent, element);
        let _ = self.tree.remove_child(parent.0, id.0);
        let n = self.tree.children(parent.0).map(|c| c.len()).unwrap_or(0);
        let _ = self.tree.insert_child_at_index(parent.0, index.min(n), id.0);
        id
    }

    /// Remove the node and everything under it.
    pub fn remove(&mut self, node: NodeId) {
        if self.roots.contains(&node.0) {
            return;
        }
        for c in self.children(node) {
            self.remove(c);
        }
        for slot in [&mut self.hovered, &mut self.focused, &mut self.captured] {
            if *slot == Some(node) {
                *slot = None;
            }
        }
        if self.pressed.map(|p| p.0) == Some(node) {
            self.pressed = None;
        }
        if let Some(parent) = self.tree.parent(node.0) {
            let _ = self.tree.remove_child(parent, node.0);
        }
        let _ = self.tree.remove(node.0);
        self.order.retain(|(n, _)| *n != node);
        self.layout_dirty = true;
        self.repaint = true;
    }

    /// Remove every child of `node` (a list filled again).
    pub fn clear(&mut self, node: NodeId) {
        for c in self.children(node) {
            self.remove(c);
        }
    }

    pub fn children(&self, node: NodeId) -> Vec<NodeId> {
        self.tree.children(node.0).unwrap_or_default().into_iter().map(NodeId).collect()
    }

    pub fn parent(&self, node: NodeId) -> Option<NodeId> {
        self.tree.parent(node.0).map(NodeId)
    }

    /// Whether the node is still in the tree.
    pub fn exists(&self, node: NodeId) -> bool {
        self.data(node).is_some()
    }

    pub(crate) fn data(&self, node: NodeId) -> Option<&NodeData> {
        self.tree.get_node_context(node.0)
    }

    pub(crate) fn data_mut(&mut self, node: NodeId) -> Option<&mut NodeData> {
        self.tree.get_node_context_mut(node.0)
    }

    /// The node's element, to read (a field's text, a box ticked).
    pub fn get<T: Element>(&self, node: NodeId) -> Option<&T> {
        let e: &dyn Any = &*self.data(node)?.element;
        e.downcast_ref::<T>()
    }

    /// Change the node's element; it is laid out and painted again.
    pub fn with<T: Element, R>(&mut self, node: NodeId, f: impl FnOnce(&mut T) -> R) -> Option<R> {
        let d = self.data_mut(node)?;
        let e: &mut dyn Any = &mut *d.element;
        let r = f(e.downcast_mut::<T>()?);
        let _ = self.tree.mark_dirty(node.0);
        self.layout_dirty = true;
        self.repaint = true;
        Some(r)
    }

    /// The node's layout, changed by `f` (its size, padding, flex...).
    pub fn style(&mut self, node: NodeId, f: impl FnOnce(&mut taffy::Style)) -> &mut Self {
        if let Some(d) = self.data_mut(node) {
            f(&mut d.layout);
            d.restyle = true;
        }
        self.style_dirty = true;
        self
    }

    /// The node's own visual properties, merged over its classes'.
    pub fn visual(&mut self, node: NodeId, v: Visual) -> &mut Self {
        if let Some(d) = self.data_mut(node) {
            d.visual.merge(&v);
            d.restyle = true;
        }
        self.style_dirty = true;
        self
    }

    pub fn add_class(&mut self, node: NodeId, class: &str) -> &mut Self {
        if let Some(d) = self.data_mut(node) {
            if !d.classes.iter().any(|c| c == class) {
                d.classes.push(class.to_owned());
                d.restyle = true;
            }
        }
        self.style_dirty = true;
        self
    }

    pub fn remove_class(&mut self, node: NodeId, class: &str) -> &mut Self {
        if let Some(d) = self.data_mut(node) {
            d.classes.retain(|c| c != class);
            d.restyle = true;
        }
        self.style_dirty = true;
        self
    }

    pub fn set_class(&mut self, node: NodeId, class: &str, on: bool) -> &mut Self {
        if on { self.add_class(node, class) } else { self.remove_class(node, class) }
    }

    pub fn has_class(&self, node: NodeId, class: &str) -> bool {
        self.data(node).is_some_and(|d| d.classes.iter().any(|c| c == class))
    }

    /// Shown or left out of the layout altogether.
    pub fn set_visible(&mut self, node: NodeId, on: bool) -> &mut Self {
        if let Some(d) = self.data_mut(node) {
            if d.visible != on {
                d.visible = on;
                d.restyle = true;
            }
        }
        self.style_dirty = true;
        self
    }

    pub fn visible(&self, node: NodeId) -> bool {
        self.data(node).is_some_and(|d| d.visible)
    }

    pub fn set_selected(&mut self, node: NodeId, on: bool) -> &mut Self {
        if let Some(d) = self.data_mut(node) {
            if d.state.selected != on {
                d.state.selected = on;
                d.restyle = true;
            }
        }
        self.style_dirty = true;
        self
    }

    pub fn set_disabled(&mut self, node: NodeId, on: bool) -> &mut Self {
        if let Some(d) = self.data_mut(node) {
            if d.state.disabled != on {
                d.state.disabled = on;
                d.restyle = true;
            }
        }
        self.style_dirty = true;
        self
    }

    pub fn state(&self, node: NodeId) -> State {
        self.data(node).map(|d| d.state).unwrap_or_default()
    }

    /// A tip shown while the pointer rests on the node.
    pub fn set_tooltip(&mut self, node: NodeId, text: Option<String>) -> &mut Self {
        if let Some(d) = self.data_mut(node) {
            d.tooltip = text;
        }
        self
    }

    /// A name to find the node by (tests, scripts): [`Ui::find`].
    pub fn set_name(&mut self, node: NodeId, name: &str) -> &mut Self {
        if let Some(d) = self.data_mut(node) {
            d.name = Some(name.to_owned());
        }
        self
    }

    pub fn find(&self, name: &str) -> Option<NodeId> {
        self.walk_all().into_iter().find(|n| self.data(*n).and_then(|d| d.name.as_deref()) == Some(name))
    }

    /// Let the node scroll its content (the overflow) along these axes.
    pub fn set_scroll(&mut self, node: NodeId, axes: ScrollAxes) -> &mut Self {
        if let Some(d) = self.data_mut(node) {
            d.scroll = (axes.x || axes.y).then(|| Scroll { axes, ..Default::default() });
            d.restyle = true;
        }
        self.style_dirty = true;
        self
    }

    pub fn scroll_offset(&self, node: NodeId) -> Vec2 {
        self.data(node).and_then(|d| d.scroll).map(|s| s.offset).unwrap_or_default()
    }

    pub fn set_scroll_offset(&mut self, node: NodeId, offset: Vec2) {
        if let Some(s) = self.data_mut(node).and_then(|d| d.scroll.as_mut()) {
            s.offset = offset.max(Vec2::ZERO).min(s.max);
        }
        self.layout_dirty = true;
        self.repaint = true;
    }

    /// Keep the node next to `anchor` (a menu under its button), in an overlay layer.
    pub fn set_anchor(&mut self, node: NodeId, anchor: Option<(NodeId, Anchor)>) -> &mut Self {
        if let Some(d) = self.data_mut(node) {
            d.anchor = anchor;
            d.layout.position = taffy::Position::Absolute;
            d.restyle = true;
        }
        self.style_dirty = true;
        self
    }

    /// Where the node was laid out last (its border box, in points).
    pub fn rect(&self, node: NodeId) -> Rect {
        self.data(node).map(|d| d.rect).unwrap_or(Rect::NOTHING)
    }

    /// Call `handler` with every event of `kind` that reaches the node (its own, or bubbling
    /// up from its children). Return true to stop it there.
    pub fn on(&mut self, node: NodeId, kind: EventKind, handler: impl FnMut(&mut EventCx<'_>, &Event) -> bool + 'static) -> &mut Self {
        if let Some(d) = self.data_mut(node) {
            d.handlers.push((kind, Box::new(handler)));
        }
        self
    }

    /// Send `message` when the node is clicked (the primary button).
    pub fn on_click<M: Any + Clone>(&mut self, node: NodeId, message: M) -> &mut Self {
        self.on(node, EventKind::Click, move |cx, e| {
            if matches!(e, Event::Click { button: Button::Primary, .. }) && !cx.state.disabled {
                cx.emit(message.clone());
                true
            } else {
                false
            }
        })
    }

    // --- the application's side -----------------------------------------------------------

    /// Messages of type `M` the interface sent since the last call.
    pub fn drain<M: Any>(&mut self) -> Vec<M> {
        let mut out = Vec::new();
        let mut keep = Vec::new();
        for m in std::mem::take(&mut self.messages) {
            match m.downcast::<M>() {
                Ok(m) => out.push(*m),
                Err(m) => keep.push(m),
            }
        }
        self.messages = keep;
        out
    }

    /// Send a message as if the interface had (an application's own widgets).
    pub fn emit<M: Any>(&mut self, m: M) {
        self.messages.push(Box::new(m));
    }

    pub(crate) fn emit_boxed(&mut self, m: Box<dyn Any>) {
        self.messages.push(m);
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// A new theme: every node is styled again.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        for n in self.walk_all() {
            if let Some(d) = self.data_mut(n) {
                d.restyle = true;
            }
        }
        self.style_dirty = true;
    }

    pub fn theme_mut(&mut self, f: impl FnOnce(&mut Theme)) {
        let mut t = self.theme.clone();
        f(&mut t);
        self.set_theme(t);
    }

    pub fn add_rule(&mut self, class: &str, rule: Rule) {
        self.theme_mut(|t| {
            t.rule(class, rule);
        });
    }

    /// Give the keyboard to `node` (None: nobody).
    pub fn focus(&mut self, node: Option<NodeId>) {
        self.set_focus(node);
    }

    pub fn focused(&self) -> Option<NodeId> {
        self.focused
    }

    pub fn hovered(&self) -> Option<NodeId> {
        self.hovered
    }

    /// The frame should be drawn again (something the tree does not know of changed).
    pub fn request_repaint(&mut self) {
        self.repaint = true;
    }

    /// Whether something changed since the last frame (draw one).
    pub fn needs_repaint(&self) -> bool {
        self.repaint || self.layout_dirty || self.style_dirty || !self.deferred.is_empty()
    }

    /// Fonts, to add faces to (then [`Ui::set_fonts`]).
    pub fn set_fonts(&mut self, defs: epaint::text::FontDefinitions) {
        self.fonts = epaint::Fonts::new(epaint::text::TextOptions::default(), defs);
        if let Some(id) = self.font_texture {
            self.tex_manager.set(id, epaint::ImageDelta::full(epaint::ImageData::Color(std::sync::Arc::new(self.fonts.image())), epaint::textures::TextureOptions::LINEAR));
        }
        for n in self.walk_all() {
            let _ = self.tree.mark_dirty(n.0);
        }
        self.layout_dirty = true;
        self.repaint = true;
    }

    /// Change the tree after the current event or frame (from outside a handler too).
    pub fn defer(&mut self, f: impl FnOnce(&mut Ui) + 'static) {
        self.deferred.push(Box::new(f));
    }

    /// The text `text` would take in the theme's default font at `size` (for an
    /// application's own layout decisions).
    pub fn text_size(&mut self, text: &str, size: f32) -> Vec2 {
        let font = epaint::FontId::new(size, self.theme.defaults.font.clone());
        crate::paint::layout_text(&mut self.fonts, self.ppp, text, font, Color32::WHITE, None).size()
    }

    // --- a frame --------------------------------------------------------------------------

    /// Take the input since the last frame, lay out what changed and paint everything.
    pub fn run(&mut self, input: Input) -> Frame {
        self.time += input.dt.max(0.0) as f64;
        self.modifiers = input.modifiers;
        if input.screen != self.screen || input.pixels_per_point != self.ppp {
            self.screen = input.screen;
            if input.pixels_per_point != self.ppp {
                self.ppp = input.pixels_per_point.max(0.25);
                for n in self.walk_all() {
                    let _ = self.tree.mark_dirty(n.0);
                }
            }
            self.layout_dirty = true;
            self.repaint = true;
        }
        self.fonts.begin_pass(epaint::text::TextOptions::default());
        self.output = PlatformOutput::default();
        // (the layout of the last frame is what the pointer was over)
        self.ensure_layout();
        for ev in input.events {
            self.input_event(ev);
        }
        self.run_deferred();
        self.ensure_layout();
        self.update_hover_from_pointer();
        self.tooltip_tick();
        let (shapes, repaint_after) = self.paint();
        self.repaint = false;
        // the fonts' atlas, then shapes to triangles
        if let Some(delta) = self.fonts.font_image_delta() {
            if let Some(id) = self.font_texture {
                self.tex_manager.set(id, delta);
            }
        }
        let font_size = self.fonts.font_image_size();
        let prepared = self.fonts.texture_atlas().prepared_discs();
        let primitives = epaint::tessellator::Tessellator::new(self.ppp, self.tess, font_size, prepared).tessellate_shapes(shapes);
        let mut platform = std::mem::take(&mut self.output);
        platform.cursor = self.cursor();
        if let Some(f) = self.focused.filter(|f| self.data(*f).is_some_and(|d| d.element.focusable() && crate::widgets::is_text_field(&*d.element))) {
            platform.text_focus = Some(self.rect(f));
        }
        let anim = self.animating.take();
        let mut after = match (repaint_after, anim) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        if self.needs_repaint() {
            after = Some(0.0);
        }
        Frame { primitives, textures: self.tex_manager.take_delta(), platform, repaint_after: after, pixels_per_point: self.ppp }
    }

    fn run_deferred(&mut self) {
        for _ in 0..8 {
            let d = std::mem::take(&mut self.deferred);
            if d.is_empty() {
                break;
            }
            for f in d {
                f(self);
            }
        }
    }

    fn cursor(&self) -> Cursor {
        if let Some((n, _, _)) = self.pressed {
            if self.dragging {
                if let Some(d) = self.data(n) {
                    if d.look.cursor == Cursor::Grab {
                        return Cursor::Grabbing;
                    }
                }
            }
        }
        let mut n = self.hovered;
        while let Some(id) = n {
            let Some(d) = self.data(id) else { break };
            if d.state.disabled {
                return Cursor::Default;
            }
            if d.look.cursor != Cursor::Default {
                return d.look.cursor;
            }
            n = self.parent(id);
        }
        Cursor::Default
    }

    // --- styling and layout ---------------------------------------------------------------

    fn walk_all(&self) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut stack: Vec<taffy::NodeId> = self.roots.to_vec();
        while let Some(n) = stack.pop() {
            out.push(NodeId(n));
            if let Ok(c) = self.tree.children(n) {
                stack.extend(c);
            }
        }
        out
    }

    fn restyle(&mut self) {
        if !self.style_dirty {
            return;
        }
        self.style_dirty = false;
        let theme = self.theme.clone();
        for n in self.walk_all() {
            let Some(d) = self.tree.get_node_context_mut(n.0) else { continue };
            if !d.restyle {
                continue;
            }
            d.restyle = false;
            let (visual, layout) = cascade(&theme, &*d.element, &d.classes, &d.visual, &d.base_layout, &d.layout, d.visible, d.scroll.map(|s| s.axes));
            let target = visual.resolve(d.state, &theme.defaults);
            if target != d.look {
                let size_changed = target.font_size != d.look.font_size || target.font != d.look.font || target.strong != d.look.strong;
                // (a node not drawn yet starts in its look: no fade in from the theme's defaults)
                let shown = d.rect != Rect::NOTHING;
                if target.transition > 0.0 && shown && !size_changed {
                    d.from = Some((d.look.clone(), self.time));
                }
                d.look = target;
                if size_changed {
                    let _ = self.tree.mark_dirty(n.0);
                }
            }
            let _ = self.tree.set_style(n.0, layout);
            self.layout_dirty = true;
            self.repaint = true;
        }
    }

    fn ensure_layout(&mut self) {
        self.restyle();
        if !self.layout_dirty {
            return;
        }
        self.layout_dirty = false;
        self.repaint = true;
        self.place_anchored();
        let avail = Size { width: AvailableSpace::Definite(self.screen.x), height: AvailableSpace::Definite(self.screen.y) };
        let (fonts, theme, ppp) = (&mut self.fonts, &self.theme, self.ppp);
        for root in self.roots {
            let _ = self.tree.compute_layout_with_measure(root, avail, |known, available, _id, ctx, _style| {
                let Some(d) = ctx else { return Size::ZERO };
                let mut cx = MeasureCx { fonts: &mut *fonts, ppp, look: &d.look, theme };
                let def = |a: AvailableSpace| match a {
                    AvailableSpace::Definite(v) => Some(v),
                    AvailableSpace::MinContent => Some(0.0),
                    AvailableSpace::MaxContent => None,
                };
                let s = d.element.measure(&mut cx, [known.width, known.height], [def(available.width), def(available.height)]);
                Size { width: s.x, height: s.y }
            });
        }
        // absolute rectangles, scroll ranges and what of each node is seen
        let screen = Rect::from_min_size(Pos2::ZERO, self.screen);
        for (k, root) in self.roots.into_iter().enumerate() {
            let _ = k;
            self.place(root, Pos2::ZERO, screen);
        }
    }

    fn place(&mut self, node: taffy::NodeId, origin: Pos2, clip: Rect) {
        let Ok(l) = self.tree.layout(node).cloned() else { return };
        let rect = Rect::from_min_size(origin + Vec2::new(l.location.x, l.location.y), Vec2::new(l.size.width, l.size.height));
        let mut child_clip = clip;
        let mut offset = Vec2::ZERO;
        if let Some(d) = self.tree.get_node_context_mut(node) {
            d.rect = rect;
            d.clip = clip.intersect(rect);
            if let Some(s) = d.scroll.as_mut() {
                let inner = Vec2::new(l.size.width - l.border.left - l.border.right, l.size.height - l.border.top - l.border.bottom);
                let content = Vec2::new(l.content_size.width, l.content_size.height);
                s.max = Vec2::new(if s.axes.x { (content.x - inner.x).max(0.0) } else { 0.0 }, if s.axes.y { (content.y - inner.y).max(0.0) } else { 0.0 });
                s.offset = s.offset.max(Vec2::ZERO).min(s.max);
                offset = s.offset;
                child_clip = clip.intersect(Rect::from_min_max(rect.min + Vec2::new(l.border.left, l.border.top), rect.max - Vec2::new(l.border.right, l.border.bottom)));
            }
            if !d.visible {
                return;
            }
        }
        for c in self.tree.children(node).unwrap_or_default() {
            self.place(c, rect.min - offset, child_clip);
        }
    }

    /// Anchored nodes (menus) put next to their anchors' last places before the layout.
    fn place_anchored(&mut self) {
        let screen = self.screen;
        for n in self.walk_all() {
            let Some((anchor, how)) = self.data(n).and_then(|d| d.anchor) else { continue };
            let a = self.rect(anchor);
            let size = self.data(n).map(|d| d.rect.size()).unwrap_or_default();
            let (x, y, w) = match how {
                Anchor::Below { gap, match_width } => {
                    let y = if a.bottom() + gap + size.y > screen.y && a.top() - gap - size.y > 0.0 { a.top() - gap - size.y } else { a.bottom() + gap };
                    (a.left(), y, match_width.then_some(a.width()))
                }
                Anchor::Above { gap } => (a.left(), a.top() - gap - size.y, None),
                Anchor::Right { gap } => (a.right() + gap, a.top(), None),
                Anchor::Center => ((screen.x - size.x) * 0.5, (screen.y - size.y) * 0.5, None),
            };
            let x = x.min(screen.x - size.x - 4.0).max(4.0);
            let y = y.max(4.0);
            if let Some(d) = self.data_mut(n) {
                d.layout.inset = taffy::Rect { left: taffy::LengthPercentageAuto::length(x), top: taffy::LengthPercentageAuto::length(y), right: taffy::LengthPercentageAuto::auto(), bottom: taffy::LengthPercentageAuto::auto() };
                if let Some(w) = w {
                    d.layout.min_size.width = taffy::Dimension::length(w);
                }
                d.restyle = true;
            }
        }
        self.style_dirty = true;
        self.restyle();
        // (the second pass of a newly opened menu: its own size is known after one layout)
    }

    // --- events -----------------------------------------------------------------------------

    fn hit(&self, p: Pos2) -> Option<NodeId> {
        self.order.iter().rev().find(|(n, clip)| clip.contains(p) && self.data(*n).is_some_and(|d| d.visible && d.element.hit_test())).map(|(n, _)| *n)
    }

    fn update_hover_from_pointer(&mut self) {
        let over = if self.captured.is_some() { self.captured } else { self.pointer.and_then(|p| self.hit(p)) };
        self.set_hover(over);
    }

    fn set_hover(&mut self, node: Option<NodeId>) {
        if node == self.hovered {
            return;
        }
        // leaving the old node and its ancestors that are not the new one's
        let old_chain = self.ancestors(self.hovered);
        let new_chain = self.ancestors(node);
        for n in &old_chain {
            if !new_chain.contains(n) {
                self.set_state(*n, |s| s.hovered = false);
                self.dispatch_to(*n, *n, &Event::PointerLeave);
            }
        }
        for n in new_chain.iter().rev() {
            if !old_chain.contains(n) {
                self.set_state(*n, |s| s.hovered = true);
                self.dispatch_to(*n, *n, &Event::PointerEnter);
            }
        }
        self.hovered = node;
        self.tooltip = node.map(|n| (n, self.time));
        self.repaint = true;
    }

    fn ancestors(&self, node: Option<NodeId>) -> Vec<NodeId> {
        let mut v = Vec::new();
        let mut n = node;
        while let Some(id) = n {
            if self.roots.contains(&id.0) {
                break;
            }
            v.push(id);
            n = self.parent(id);
        }
        v
    }

    fn set_state(&mut self, node: NodeId, f: impl FnOnce(&mut State)) {
        if let Some(d) = self.data_mut(node) {
            let before = d.state;
            f(&mut d.state);
            if d.state != before {
                d.restyle = true;
                self.style_dirty = true;
            }
        }
    }

    fn set_focus(&mut self, node: Option<NodeId>) {
        let node = node.filter(|n| self.data(*n).is_some_and(|d| d.element.focusable() && !d.state.disabled));
        if node == self.focused {
            return;
        }
        if let Some(old) = self.focused.take() {
            self.set_state(old, |s| s.focused = false);
            self.dispatch_to(old, old, &Event::FocusLost);
        }
        if let Some(new) = node {
            self.focused = Some(new);
            self.set_state(new, |s| s.focused = true);
            self.dispatch_to(new, new, &Event::FocusGained);
        }
        self.repaint = true;
    }

    fn input_event(&mut self, ev: InputEvent) {
        self.repaint = true;
        match ev {
            InputEvent::PointerMoved(p) => {
                let delta = self.pointer.map(|o| p - o).unwrap_or_default();
                self.pointer = Some(p);
                self.update_hover_from_pointer();
                if let Some((n, button, at)) = self.pressed {
                    if !self.dragging && (p - at).length() > 3.0 {
                        self.dragging = true;
                    }
                    if self.dragging {
                        self.dispatch(n, &Event::Drag { pos: p, delta, button });
                    }
                }
                if let Some(h) = self.captured.or(self.hovered) {
                    self.dispatch(h, &Event::PointerMove { pos: p });
                }
            }
            InputEvent::PointerLeft => {
                self.pointer = None;
                if self.captured.is_none() {
                    self.set_hover(None);
                }
            }
            InputEvent::PointerButton { pos, button, pressed } => {
                self.pointer = Some(pos);
                self.update_hover_from_pointer();
                let target = self.captured.or(self.hovered);
                if pressed {
                    // the keyboard goes with the click (to nothing, on the background)
                    let focus = target.and_then(|t| self.ancestors(Some(t)).into_iter().find(|n| self.data(*n).is_some_and(|d| d.element.focusable())));
                    self.set_focus(focus);
                    if let Some(t) = target {
                        self.pressed = Some((t, button, pos));
                        self.dragging = false;
                        for n in self.ancestors(Some(t)) {
                            self.set_state(n, |s| s.pressed = true);
                        }
                        self.dispatch(t, &Event::PointerDown { pos, button });
                    }
                } else {
                    let pressed_on = self.pressed.take();
                    if let Some((p, _, _)) = pressed_on {
                        for n in self.ancestors(Some(p)) {
                            self.set_state(n, |s| s.pressed = false);
                        }
                    }
                    if let Some(t) = target {
                        self.dispatch(t, &Event::PointerUp { pos, button });
                    }
                    if let Some((p, b, _)) = pressed_on {
                        if self.dragging {
                            self.dispatch(p, &Event::DragEnd { pos, button: b });
                        } else if b == button && target.is_some_and(|t| t == p || self.ancestors(Some(t)).contains(&p)) {
                            let count = match self.last_click {
                                Some((n, at, c)) if n == p && self.time - at < 0.4 => c + 1,
                                _ => 1,
                            };
                            self.last_click = Some((p, self.time, count));
                            self.dispatch(p, &Event::Click { pos, button, count });
                        }
                    }
                    self.dragging = false;
                    self.captured = None;
                    self.update_hover_from_pointer();
                }
            }
            InputEvent::Wheel(delta) => {
                if let Some(h) = self.hovered.or_else(|| self.pointer.and_then(|p| self.hit(p))) {
                    if !self.dispatch(h, &Event::Wheel { delta }) {
                        self.scroll_by(h, delta);
                    }
                }
            }
            InputEvent::Key { key, pressed, repeat } => {
                let ev = Event::Key { key, pressed, repeat, modifiers: self.modifiers };
                let handled = match self.focused {
                    Some(f) => self.dispatch(f, &ev),
                    None => false,
                };
                if !handled && pressed {
                    match key {
                        Key::Tab => self.focus_next(!self.modifiers.shift),
                        Key::Escape => self.set_focus(None),
                        // the root's handlers: an application's shortcuts
                        _ => {
                            let root = NodeId(self.roots[0]);
                            self.dispatch_to(root, root, &ev);
                        }
                    }
                }
            }
            InputEvent::Text(t) => {
                if let Some(f) = self.focused {
                    self.dispatch(f, &Event::Text(t));
                }
            }
            InputEvent::Paste(t) => {
                if let Some(f) = self.focused {
                    self.dispatch(f, &Event::Paste(t));
                }
            }
            InputEvent::Focus(false) => {
                self.pressed = None;
                self.captured = None;
            }
            InputEvent::Focus(true) => {}
        }
    }

    /// Scroll the nearest scrolling ancestor of `node` that can move that way.
    fn scroll_by(&mut self, node: NodeId, delta: Vec2) {
        let mut n = Some(node);
        while let Some(id) = n {
            let time = self.time;
            if let Some(s) = self.data_mut(id).and_then(|d| d.scroll.as_mut()) {
                let d = Vec2::new(if s.axes.x { -delta.x } else { 0.0 }, if s.axes.y { -delta.y } else { 0.0 });
                // (a vertical wheel scrolls a list that only goes sideways)
                let d = if !s.axes.y && s.axes.x && d.x == 0.0 { Vec2::new(-delta.y, 0.0) } else { d };
                let to = (s.offset + d).max(Vec2::ZERO).min(s.max);
                if to != s.offset {
                    s.offset = to;
                    s.active_at = time;
                    self.layout_dirty = true;
                    self.repaint = true;
                    return;
                }
            }
            n = self.parent(id);
        }
    }

    /// Scroll so that `node` is seen within its scrolling ancestors.
    pub fn scroll_into_view(&mut self, node: NodeId) {
        let r = self.rect(node);
        let mut n = self.parent(node);
        while let Some(id) = n {
            let rect = self.rect(id);
            if let Some(s) = self.data_mut(id).and_then(|d| d.scroll.as_mut()) {
                let mut off = s.offset;
                if r.bottom() > rect.bottom() {
                    off.y += r.bottom() - rect.bottom();
                }
                if r.top() < rect.top() {
                    off.y -= rect.top() - r.top();
                }
                s.offset = off.max(Vec2::ZERO).min(s.max);
                self.layout_dirty = true;
            }
            n = self.parent(id);
        }
    }

    fn focus_next(&mut self, forward: bool) {
        let list: Vec<NodeId> = self.order.iter().map(|(n, _)| *n).filter(|n| self.data(*n).is_some_and(|d| d.visible && d.element.focusable() && !d.state.disabled)).collect();
        if list.is_empty() {
            return;
        }
        let i = self.focused.and_then(|f| list.iter().position(|n| *n == f));
        let next = match (i, forward) {
            (Some(i), true) => (i + 1) % list.len(),
            (Some(i), false) => (i + list.len() - 1) % list.len(),
            (None, true) => 0,
            (None, false) => list.len() - 1,
        };
        self.set_focus(Some(list[next]));
        self.scroll_into_view(list[next]);
    }

    /// Send `ev` to `target` and up its ancestors until one handles it. True when handled.
    fn dispatch(&mut self, target: NodeId, ev: &Event) -> bool {
        if !ev.bubbles() {
            return self.dispatch_to(target, target, ev);
        }
        for n in self.ancestors(Some(target)).into_iter().chain(std::iter::once(NodeId(self.roots[0]))) {
            if self.dispatch_to(n, target, ev) {
                return true;
            }
        }
        false
    }

    /// `ev` to one node: its element, then its handlers.
    fn dispatch_to(&mut self, node: NodeId, target: NodeId, ev: &Event) -> bool {
        let mut fx = Effects::default();
        let (theme, time, modifiers, ppp) = (&self.theme, self.time, self.modifiers, self.ppp);
        let fonts = &mut self.fonts;
        let Some(d) = self.tree.get_node_context_mut(node.0) else { return false };
        if d.state.disabled && !matches!(ev, Event::PointerEnter | Event::PointerLeave | Event::FocusLost) {
            return false;
        }
        let content = d.rect;
        let mut cx = EventCx { node, target, rect: d.rect, content, look: &d.look, state: d.state, fonts, ppp, theme, modifiers, time, fx: &mut fx };
        let mut handled = d.element.event(&mut cx, ev);
        if !handled {
            let kind = ev.kind();
            for (k, h) in d.handlers.iter_mut() {
                if *k == kind && h(&mut cx, ev) {
                    handled = true;
                    break;
                }
            }
        }
        self.apply(node, fx);
        handled
    }

    fn apply(&mut self, node: NodeId, fx: Effects) {
        self.messages.extend(fx.messages);
        if let Some(c) = fx.capture {
            self.captured = c;
        }
        if fx.relayout {
            let _ = self.tree.mark_dirty(node.0);
            self.layout_dirty = true;
        }
        if fx.repaint {
            self.repaint = true;
        }
        if fx.copied.is_some() {
            self.output.copied = fx.copied;
        }
        if fx.open_url.is_some() {
            self.output.open_url = fx.open_url;
        }
        self.deferred.extend(fx.deferred);
        if let Some(f) = fx.focus {
            self.set_focus(f);
        }
        for (n, e) in fx.send {
            self.dispatch(n, &e);
        }
    }

    fn tooltip_tick(&mut self) {
        // (shown after the pointer rests 0.6 s; see `paint`)
        if let Some((n, at)) = self.tooltip {
            if self.data(n).and_then(|d| d.tooltip.as_ref()).is_some() && self.time - at < 0.6 {
                let left = (0.6 - (self.time - at)) as f32;
                self.animating = Some(self.animating.map_or(left, |a| a.min(left)));
            }
        }
    }

    // --- painting ---------------------------------------------------------------------------

    fn paint(&mut self) -> (Vec<epaint::ClippedShape>, Option<f32>) {
        let mut painter = Painter::new(LAYERS);
        let mut repaint_after: Option<f32> = None;
        self.order.clear();
        for (k, root) in self.roots.into_iter().enumerate() {
            self.paint_node(root, k, &mut painter, &mut repaint_after);
        }
        // the tip of the node the pointer rests on
        if let Some((n, at)) = self.tooltip {
            if self.time - at >= 0.6 && self.pressed.is_none() {
                if let (Some(text), Some(p)) = (self.data(n).and_then(|d| d.tooltip.clone()), self.pointer) {
                    let font = epaint::FontId::new(self.theme.defaults.font_size * 0.9, self.theme.defaults.font.clone());
                    let g = crate::paint::layout_text(&mut self.fonts, self.ppp, &text, font, self.theme.defaults.color, Some(320.0));
                    let size = g.size() + Vec2::new(16.0, 10.0);
                    let mut at = p + Vec2::new(12.0, 18.0);
                    at.x = at.x.min(self.screen.x - size.x - 4.0);
                    if at.y + size.y > self.screen.y {
                        at.y = p.y - size.y - 8.0;
                    }
                    let r = Rect::from_min_size(at, size);
                    let tip = self.theme.rules.get("tooltip").map(|r| r.visual.clone()).unwrap_or_default();
                    painter.set(Layer::Tooltip as usize, Rect::EVERYTHING);
                    painter.rect_filled(r, tip.radius.unwrap_or(6.0), tip.background.unwrap_or(Color32::from_gray(30)));
                    if let Some(b) = tip.border {
                        painter.rect_stroke(r, tip.radius.unwrap_or(6.0), b);
                    }
                    painter.galley(r.min + Vec2::new(8.0, 5.0), g, self.theme.defaults.color);
                }
            }
        }
        (painter.finish(), repaint_after)
    }

    fn paint_node(&mut self, node: taffy::NodeId, layer: usize, painter: &mut Painter, repaint_after: &mut Option<f32>) {
        let time = self.time;
        let pointer = self.pointer;
        let (theme, ppp) = (&self.theme, self.ppp);
        let fonts = &mut self.fonts;
        let Ok(layout) = self.tree.layout(node).cloned() else { return };
        let Some(d) = self.tree.get_node_context_mut(node) else { return };
        if !d.visible || !d.clip.is_positive() && d.scroll.is_none() && d.rect.area() > 0.0 {
            if !d.visible {
                return;
            }
        }
        // the colours on their way from the last look to this one
        let look = match &d.from {
            Some((from, at)) if d.look.transition > 0.0 => {
                let t = ((time - at) as f32 / d.look.transition).clamp(0.0, 1.0);
                if t >= 1.0 {
                    d.from = None;
                    d.look.clone()
                } else {
                    *repaint_after = Some(0.0);
                    from.lerp_colors(&d.look, ease(t))
                }
            }
            _ => d.look.clone(),
        };
        let look = if look.opacity < 1.0 { fade(&look) } else { look };
        painter.set(layer, d.clip);
        if d.element.paint_box() {
            painter.rect_filled(d.rect, look.radius, look.background);
            painter.rect_stroke(d.rect, look.radius, look.border);
        }
        let content = Rect::from_min_max(
            d.rect.min + Vec2::new(layout.border.left + layout.padding.left, layout.border.top + layout.padding.top),
            d.rect.max - Vec2::new(layout.border.right + layout.padding.right, layout.border.bottom + layout.padding.bottom),
        );
        let mut cx = PaintCx { node: NodeId(node), rect: d.rect, content, look: &look, state: d.state, painter, fonts, ppp, theme, time, repaint_after, pointer };
        d.element.paint(&mut cx);
        let (rect, clip, scroll) = (d.rect, d.clip, d.scroll);
        self.order.push((NodeId(node), clip));
        for c in self.tree.children(node).unwrap_or_default() {
            self.paint_node(c, layer, painter, repaint_after);
        }
        // the scroll bar over the content
        if let Some(s) = scroll {
            if s.max.y > 0.5 {
                let seen = rect.height();
                let total = seen + s.max.y;
                let h = (seen * seen / total).max(24.0);
                let y = rect.top() + (seen - h) * (s.offset.y / s.max.y);
                let bar = Rect::from_min_size(Pos2::new(rect.right() - 6.0, y + 2.0), Vec2::new(4.0, h - 4.0));
                let a = (1.0 - ((time - s.active_at) as f32 - 0.8).max(0.0) / 0.4).clamp(0.35, 1.0);
                painter.set(layer, clip);
                painter.rect_filled(bar, 2.0, self.theme.defaults.color.gamma_multiply(0.35 * a));
                if a > 0.35 && a < 1.0 {
                    *repaint_after = Some(0.0);
                }
            }
        }
    }

    // --- textures of the application --------------------------------------------------------

    /// A texture the application registered with its renderer (`egui_wgpu::Renderer::
    /// register_native_texture` gives the id), for [`crate::widgets::Image`].
    pub fn texture_id(&self, user: u64) -> TextureId {
        TextureId::User(user)
    }

    /// Physical pixels per point of the last frame.
    pub fn pixels_per_point(&self) -> f32 {
        self.ppp
    }

    pub fn screen_size(&self) -> Vec2 {
        self.screen
    }

    pub fn time(&self) -> f64 {
        self.time
    }

    pub fn pointer(&self) -> Option<Pos2> {
        self.pointer
    }

    /// The node the pointer would land on at `p` (tests, scripts).
    pub fn hit_at(&self, p: Pos2) -> Option<NodeId> {
        self.hit(p)
    }

    /// What is at `p`, by its element's type name (tests).
    pub fn hit_name_at(&self, p: Pos2) -> Option<&'static str> {
        self.hit(p).and_then(|n| self.data(n)).map(|d| d.element.type_name())
    }
}

impl Default for Ui {
    fn default() -> Self {
        Ui::new()
    }
}

impl NodeData {
    fn new(element: Box<dyn Element>, layout: taffy::Style, layer: Layer, theme: &Theme) -> NodeData {
        NodeData {
            element,
            classes: Vec::new(),
            visual: Visual::default(),
            base_layout: layout.clone(),
            layout,
            state: State::default(),
            handlers: Vec::new(),
            scroll: None,
            look: theme.defaults.clone(),
            from: None,
            rect: Rect::NOTHING,
            clip: Rect::NOTHING,
            layer,
            visible: true,
            tooltip: None,
            name: None,
            anchor: None,
            restyle: true,
        }
    }
}

/// The node's visual and layout: the element's class, the node's classes in order, then its
/// own; hidden nodes leave the layout, scrolling ones clip their overflow.
#[allow(clippy::too_many_arguments)]
fn cascade(theme: &Theme, element: &dyn Element, classes: &[String], own: &Visual, base: &taffy::Style, layout: &taffy::Style, visible: bool, scroll: Option<crate::ui::ScrollAxes>) -> (Visual, taffy::Style) {
    let mut v = Visual::default();
    let mut l = base.clone();
    let own_class = element.class();
    let apply = |name: &str, v: &mut Visual, l: &mut taffy::Style| {
        if let Some(r) = theme.rules.get(name) {
            v.merge(&r.visual);
            if let Some(p) = &r.layout {
                p(l);
            }
        }
    };
    if !own_class.is_empty() {
        apply(own_class, &mut v, &mut l);
    }
    for c in classes {
        apply(c, &mut v, &mut l);
    }
    v.merge(own);
    // the node's own layout wins over its classes': every property it changed from the
    // element's
    macro_rules! own {
        ($($f:ident),*) => { $( if layout.$f != base.$f { l.$f = layout.$f.clone(); } )* };
    }
    own!(display, box_sizing, position, aspect_ratio, align_items, align_self, justify_items, justify_self, align_content, justify_content, flex_direction, flex_wrap, flex_basis, flex_grow, flex_shrink, grid_template_rows, grid_template_columns, grid_auto_rows, grid_auto_columns, grid_auto_flow, grid_row, grid_column);
    // (by side: padding.left set on the node leaves the class's padding.top)
    macro_rules! own_parts {
        ($($f:ident: $($p:ident),*);* $(;)?) => { $( $( if layout.$f.$p != base.$f.$p { l.$f.$p = layout.$f.$p; } )* )* };
    }
    own_parts!(inset: left, right, top, bottom; margin: left, right, top, bottom; padding: left, right, top, bottom; border: left, right, top, bottom; size: width, height; min_size: width, height; max_size: width, height; gap: width, height; overflow: x, y);
    if !visible {
        l.display = taffy::Display::None;
    }
    if let Some(a) = scroll {
        l.overflow = taffy::Point { x: if a.x { taffy::Overflow::Scroll } else { taffy::Overflow::Hidden }, y: if a.y { taffy::Overflow::Scroll } else { taffy::Overflow::Hidden } };
        l.scrollbar_width = 0.0;
    }
    (v, l)
}

fn ease(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

fn fade(l: &Look) -> Look {
    let a = l.opacity;
    Look { background: l.background.gamma_multiply(a), color: l.color.gamma_multiply(a), border: Stroke::new(l.border.width, l.border.color.gamma_multiply(a)), ..l.clone() }
}
