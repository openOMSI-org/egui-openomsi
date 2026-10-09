//! What a node is: an [`Element`] - its own size, its painting and what it does with events.
//! Every widget of the crate is one, and so is any widget an application writes: implement
//! the trait and add it to the tree like the built-in ones.

use crate::input::{Button, Key, Modifiers};
use crate::paint::Painter;
use crate::style::{Look, State, Theme};
use crate::{NodeId, Ui};
use emath::{Pos2, Rect, Vec2};
use std::any::Any;
use std::sync::Arc;

/// Something a node does: its size where it has content of its own, its picture, and its
/// answer to events. Containers need none of it (they are laid out by their style and paint
/// their box).
pub trait Element: Any {
    /// The theme class the element's look starts from (`"button"`, `"label"`).
    fn class(&self) -> &'static str {
        ""
    }

    /// The layout the element starts from before the classes and the node's own.
    fn layout(&self, _style: &mut taffy::Style) {}

    /// The size of the element's content (a leaf: text, an image), for the space given:
    /// `known` sizes are fixed by the layout, `available` is what there is room for.
    fn measure(&mut self, _cx: &mut MeasureCx<'_>, _known: [Option<f32>; 2], _available: [Option<f32>; 2]) -> Vec2 {
        Vec2::ZERO
    }

    /// Paint the element in `cx.rect` (its box is painted already, see [`Element::paint_box`]).
    fn paint(&mut self, _cx: &mut PaintCx<'_>) {}

    /// Whether the node's background and border are painted before [`Element::paint`].
    fn paint_box(&self) -> bool {
        true
    }

    /// Answer an event that reached the node (its own, or bubbling up from a child: see
    /// [`EventCx::target`]). True stops it there.
    fn event(&mut self, _cx: &mut EventCx<'_>, _event: &Event) -> bool {
        false
    }

    /// Whether the node takes the keyboard when clicked (and with Tab).
    fn focusable(&self) -> bool {
        false
    }

    /// Whether the pointer can land on the node (a label lets it through to what is under it).
    fn hit_test(&self) -> bool {
        true
    }

    /// The element's type, for messages about it.
    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// What a node is asked to answer.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    PointerEnter,
    PointerLeave,
    PointerMove { pos: Pos2 },
    PointerDown { pos: Pos2, button: Button },
    PointerUp { pos: Pos2, button: Button },
    /// Pressed and let go over the same node.
    Click { pos: Pos2, button: Button, count: u32 },
    /// The pointer moves with a button held, the press having been on this node.
    Drag { pos: Pos2, delta: Vec2, button: Button },
    DragEnd { pos: Pos2, button: Button },
    Wheel { delta: Vec2 },
    Key { key: Key, pressed: bool, repeat: bool, modifiers: Modifiers },
    Text(String),
    Paste(String),
    FocusGained,
    FocusLost,
}

/// Kinds of [`Event`], for the handlers a node is given ([`Ui::on`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EventKind {
    PointerEnter,
    PointerLeave,
    PointerMove,
    PointerDown,
    PointerUp,
    Click,
    Drag,
    DragEnd,
    Wheel,
    Key,
    Text,
    Paste,
    FocusGained,
    FocusLost,
}

impl Event {
    pub fn kind(&self) -> EventKind {
        match self {
            Event::PointerEnter => EventKind::PointerEnter,
            Event::PointerLeave => EventKind::PointerLeave,
            Event::PointerMove { .. } => EventKind::PointerMove,
            Event::PointerDown { .. } => EventKind::PointerDown,
            Event::PointerUp { .. } => EventKind::PointerUp,
            Event::Click { .. } => EventKind::Click,
            Event::Drag { .. } => EventKind::Drag,
            Event::DragEnd { .. } => EventKind::DragEnd,
            Event::Wheel { .. } => EventKind::Wheel,
            Event::Key { .. } => EventKind::Key,
            Event::Text(_) => EventKind::Text,
            Event::Paste(_) => EventKind::Paste,
            Event::FocusGained => EventKind::FocusGained,
            Event::FocusLost => EventKind::FocusLost,
        }
    }

    /// Whether it bubbles to the node's ancestors when not handled (enter/leave/focus do not).
    pub fn bubbles(&self) -> bool {
        !matches!(self, Event::PointerEnter | Event::PointerLeave | Event::FocusGained | Event::FocusLost)
    }
}

/// For [`Element::measure`].
pub struct MeasureCx<'a> {
    pub fonts: &'a mut epaint::Fonts,
    pub ppp: f32,
    pub look: &'a Look,
    pub theme: &'a Theme,
}

impl MeasureCx<'_> {
    /// `text` set in the node's font, wrapped at `wrap` points (None: one line).
    pub fn layout_text(&mut self, text: &str, wrap: Option<f32>) -> Arc<epaint::Galley> {
        crate::paint::layout_text(self.fonts, self.ppp, text, self.look.font_id(self.theme), self.look.color, wrap)
    }
}

/// For [`Element::paint`].
pub struct PaintCx<'a> {
    pub node: NodeId,
    /// The node's border box, in points from the window's top left.
    pub rect: Rect,
    /// Inside its padding and border.
    pub content: Rect,
    pub look: &'a Look,
    pub state: State,
    pub painter: &'a mut Painter,
    pub fonts: &'a mut epaint::Fonts,
    pub ppp: f32,
    pub theme: &'a Theme,
    /// Seconds since the [`Ui`] was made.
    pub time: f64,
    /// Ask for another frame `secs` from now (an animation, a blinking caret): 0 at once.
    pub repaint_after: &'a mut Option<f32>,
    /// Where the pointer is.
    pub pointer: Option<Pos2>,
    pub textures: &'a mut crate::icons::Textures,
}

impl PaintCx<'_> {
    /// Icon `name` centred in `rect` at `size` points, in `color`.
    pub fn icon(&mut self, name: &str, center: Pos2, size: f32, color: ecolor::Color32) {
        let px = (size * self.ppp).round().max(4.0) as u32;
        if let Some(t) = self.textures.icon(name, px) {
            let d = px as f32 / self.ppp;
            let r = Rect::from_center_size(center, Vec2::splat(d));
            // (on whole pixels: a mask drawn between them is blurred)
            let r = Rect::from_min_size(Pos2::new((r.min.x * self.ppp).round() / self.ppp, (r.min.y * self.ppp).round() / self.ppp), r.size());
            self.painter.image(t, r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), color, 0.0);
        }
    }

    pub fn layout_text(&mut self, text: &str, wrap: Option<f32>) -> Arc<epaint::Galley> {
        crate::paint::layout_text(self.fonts, self.ppp, text, self.look.font_id(self.theme), self.look.color, wrap)
    }

    pub fn repaint_after(&mut self, secs: f32) {
        *self.repaint_after = Some(self.repaint_after.map_or(secs, |s| s.min(secs)));
    }
}

/// What an element or a handler can do while it answers an event: send the application a
/// message, take or give up the keyboard, keep the pointer, change the tree after the event.
pub struct EventCx<'a> {
    /// The node answering.
    pub node: NodeId,
    /// The node the event was for (a child, while it bubbles).
    pub target: NodeId,
    pub rect: Rect,
    pub content: Rect,
    pub look: &'a Look,
    pub state: State,
    pub fonts: &'a mut epaint::Fonts,
    pub ppp: f32,
    pub theme: &'a Theme,
    pub modifiers: Modifiers,
    pub time: f64,
    pub(crate) fx: &'a mut Effects,
}

/// What the event's answers asked for, carried out once it is done.
#[derive(Default)]
pub(crate) struct Effects {
    pub messages: Vec<Box<dyn Any>>,
    pub focus: Option<Option<NodeId>>,
    pub capture: Option<Option<NodeId>>,
    pub deferred: Vec<Box<dyn FnOnce(&mut Ui)>>,
    pub repaint: bool,
    pub relayout: bool,
    pub copied: Option<String>,
    pub open_url: Option<String>,
    /// Events to send once this one is answered (a key pressed on a button clicks it).
    pub send: Vec<(NodeId, Event)>,
}

impl EventCx<'_> {
    /// A message for the application ([`Ui::drain`]).
    pub fn emit<M: Any>(&mut self, m: M) {
        self.fx.messages.push(Box::new(m));
    }

    /// Take the keyboard (`None`: give it up).
    pub fn focus(&mut self, node: Option<NodeId>) {
        self.fx.focus = Some(node);
    }

    /// Keep the pointer's events coming to this node until it is let go (a drag).
    pub fn capture(&mut self) {
        self.fx.capture = Some(Some(self.node));
    }

    pub fn release(&mut self) {
        self.fx.capture = Some(None);
    }

    /// Change the tree once the event is answered (another node's text, visibility...).
    pub fn defer(&mut self, f: impl FnOnce(&mut Ui) + 'static) {
        self.fx.deferred.push(Box::new(f));
    }

    /// The node looks different now (draw again).
    pub fn repaint(&mut self) {
        self.fx.repaint = true;
    }

    /// The node's size may have changed (lay out again).
    pub fn relayout(&mut self) {
        self.fx.relayout = true;
        self.fx.repaint = true;
    }

    /// Send `event` to `node` (and up its ancestors) once this event is answered.
    pub fn send(&mut self, node: NodeId, event: Event) {
        self.fx.send.push((node, event));
    }

    pub fn copy(&mut self, text: String) {
        self.fx.copied = Some(text);
    }

    pub fn open_url(&mut self, url: impl Into<String>) {
        self.fx.open_url = Some(url.into());
    }

    pub fn layout_text(&mut self, text: &str, wrap: Option<f32>) -> Arc<epaint::Galley> {
        crate::paint::layout_text(self.fonts, self.ppp, text, self.look.font_id(self.theme), self.look.color, wrap)
    }
}

/// A handler a node is given: called with each event of its kind that reaches the node.
pub(crate) type Handler = Box<dyn FnMut(&mut EventCx<'_>, &Event) -> bool>;
