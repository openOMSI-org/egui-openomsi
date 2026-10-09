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

// (the workspace's pedantic style lints; the crate keeps to rustfmt and clippy's defaults)
#![allow(
    clippy::use_self,
    clippy::collapsible_if,
    clippy::needless_pass_by_value,
    clippy::too_long_first_doc_paragraph,
    clippy::unwrap_used,
    clippy::option_option,
    clippy::unnecessary_wraps,
    clippy::unused_self,
    clippy::or_fun_call,
    clippy::derive_partial_eq_without_eq,
    clippy::allow_attributes,
    clippy::explicit_iter_loop,
    clippy::match_wildcard_for_single_variants,
    clippy::single_match_else,
    clippy::bind_instead_of_map,
    clippy::option_if_let_else,
    clippy::clone_on_ref_ptr
)]

mod element;
pub mod input;
mod paint;
#[cfg(feature = "wgpu")]
pub mod render;
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
