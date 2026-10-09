//! A retained-mode GUI core on epaint.
//!
//! The interface is a tree of nodes made once and changed in place - not described again
//! every frame. Each node is an [`Element`] (a box, text, a button, a field, or a widget of the
//! application's own), styled from a [`Theme`] of classes the way CSS cascades, laid out by
//! [taffy](https://docs.rs/taffy) (flexbox, grid) only when something changed, and painted with
//! epaint into the same triangles egui's renderers draw (`egui-wgpu`, `egui_glow`).
//!
//! Events go to the node under the pointer (or with the keyboard) and bubble up to its
//! ancestors; nodes answer them in their [`Element::event`] and in handlers given with
//! [`Ui::on`], and tell the application what happened with typed messages it drains:
//!
//! ```ignore
//! #[derive(Clone)]
//! enum Msg { Play }
//! let mut ui = Ui::new();
//! let page = ui.column(ui.root(Layer::Base));
//! ui.button(page, "Play", Msg::Play);
//! // each frame:
//! let frame = ui.run(input);
//! for m in ui.drain::<Msg>() { /* ... */ }
//! ```
//!
//! A frame is drawn only when something changed (input, an animation, the tree):
//! [`Frame::repaint_after`] says when the next one is due.

mod element;
pub mod input;
mod paint;
mod style;
mod ui;
pub mod widgets;

pub use element::{Element, Event, EventCx, EventKind, MeasureCx, PaintCx};
pub use input::{Input, InputEvent, Key, Modifiers, PlatformOutput};
pub use paint::{Painter, layout_text};
pub use style::{Cursor, LayoutPatch, Look, Rule, State, Theme, Visual};
pub use ui::{Anchor, Frame, Layer, NodeId, ScrollAxes, Ui};

pub use ecolor::{self, Color32};
pub use emath::{self, Pos2, Rect, Vec2, pos2, vec2};
pub use epaint;
pub use taffy;
