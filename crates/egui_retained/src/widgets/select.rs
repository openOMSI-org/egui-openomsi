//! A choice of one of several: a button showing the chosen one that opens a menu over the page.

use super::{Button, Emit, Text, emit_with, lp};
use crate::element::{Element, Event, EventCx, EventKind, MeasureCx, PaintCx};
use crate::input::{Button as MouseButton, Key};
use crate::ui::{Anchor, Layer, ScrollAxes};
use crate::{NodeId, Ui};
use emath::{Pos2, Vec2};
use epaint::Stroke;
use std::any::Any;

pub struct Select {
    pub options: Vec<String>,
    pub selected: Option<usize>,
    pub placeholder: String,
    on_change: Option<Emit<usize>>,
    /// The menu while it is open (its backdrop).
    open: Option<NodeId>,
}

impl Select {
    pub fn new(options: Vec<String>, selected: Option<usize>) -> Select {
        Select { options, selected, placeholder: String::new(), on_change: None, open: None }
    }
    pub fn placeholder(mut self, p: impl Into<String>) -> Select {
        self.placeholder = p.into();
        self
    }
    /// The message sent with the index of the option chosen.
    pub fn on_change<M: Any>(mut self, f: impl Fn(usize) -> M + 'static) -> Select {
        self.on_change = Some(emit_with(f));
        self
    }
    fn label(&self) -> &str {
        self.selected.and_then(|i| self.options.get(i)).map(String::as_str).unwrap_or(&self.placeholder)
    }
}

/// What lies behind an open menu: a click on it closes the menu.
struct Backdrop;

impl Element for Backdrop {
    fn layout(&self, s: &mut taffy::Style) {
        s.position = taffy::Position::Absolute;
        s.inset = taffy::Rect { left: taffy::LengthPercentageAuto::length(0.0), top: taffy::LengthPercentageAuto::length(0.0), right: taffy::LengthPercentageAuto::length(0.0), bottom: taffy::LengthPercentageAuto::length(0.0) };
    }
    fn paint_box(&self) -> bool {
        false
    }
}

impl Element for Select {
    fn class(&self) -> &'static str {
        "select"
    }
    fn layout(&self, s: &mut taffy::Style) {
        s.padding = taffy::Rect { left: lp(10.0), right: lp(30.0), top: lp(6.0), bottom: lp(6.0) };
        s.align_items = Some(taffy::AlignItems::Center);
    }
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _k: [Option<f32>; 2], _a: [Option<f32>; 2]) -> Vec2 {
        let widest = self.options.iter().map(|o| cx.layout_text(o, None).size().x).fold(0.0f32, f32::max);
        let g = cx.layout_text(self.label(), None);
        Vec2::new(widest.max(g.size().x).min(420.0), g.size().y)
    }
    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let label = self.label().to_owned();
        let mut look = cx.look.clone();
        if self.selected.is_none() {
            look.color = look.color.gamma_multiply(0.5);
        }
        let g = crate::paint::layout_text(cx.fonts, cx.ppp, &label, look.font_id(cx.theme), look.color, None);
        let at = Pos2::new(cx.content.left(), cx.content.center().y - g.size().y * 0.5);
        let clip = cx.rect.shrink(1.0);
        let c = look.color;
        cx.painter.with_clip(clip, |p| p.galley(at, g, c));
        // the chevron
        let x = cx.rect.right() - 16.0;
        let y = cx.rect.center().y;
        let s = if self.open.is_some() { -1.0 } else { 1.0 };
        let stroke = Stroke::new(1.5, cx.look.color.gamma_multiply(0.7));
        cx.painter.add(epaint::Shape::line(vec![Pos2::new(x - 4.0, y - 2.0 * s), Pos2::new(x, y + 2.0 * s), Pos2::new(x + 4.0, y - 2.0 * s)], stroke));
    }
    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> bool {
        if cx.target != cx.node {
            return false;
        }
        let toggle = matches!(event, Event::Click { button: MouseButton::Primary, .. }) || matches!(event, Event::Key { key: Key::Enter | Key::Space, pressed: true, .. });
        if toggle {
            let node = cx.node;
            match self.open.take() {
                Some(b) => cx.defer(move |ui| ui.remove(b)),
                None => cx.defer(move |ui| open_menu(ui, node)),
            }
            cx.repaint();
            return true;
        }
        let step = match event {
            Event::Key { key: Key::Down, pressed: true, .. } => 1i64,
            Event::Key { key: Key::Up, pressed: true, .. } => -1,
            _ => return false,
        };
        if self.options.is_empty() {
            return true;
        }
        let i = self.selected.map(|i| i as i64 + step).unwrap_or(0).clamp(0, self.options.len() as i64 - 1) as usize;
        if Some(i) != self.selected {
            self.selected = Some(i);
            if let Some(f) = &self.on_change {
                cx.fx.messages.push(f(i));
            }
            cx.relayout();
        }
        true
    }
    fn focusable(&self) -> bool {
        true
    }
}

fn open_menu(ui: &mut Ui, select: NodeId) {
    let Some(s) = ui.get::<Select>(select) else { return };
    let (options, selected) = (s.options.clone(), s.selected);
    let overlay = ui.root(Layer::Overlay);
    let backdrop = ui.add(overlay, Backdrop);
    ui.on(backdrop, EventKind::PointerDown, move |cx, _| {
        if cx.target == cx.node {
            cx.defer(move |ui| close(ui, select));
        }
        cx.target == cx.node
    });
    ui.on(backdrop, EventKind::Key, move |cx, e| {
        if matches!(e, Event::Key { key: Key::Escape, pressed: true, .. }) {
            cx.defer(move |ui| close(ui, select));
            return true;
        }
        false
    });
    let menu = ui.add(backdrop, super::Div::column());
    ui.add_class(menu, "menu");
    ui.set_anchor(menu, Some((select, Anchor::Below { gap: 4.0, match_width: true })));
    ui.set_scroll(menu, ScrollAxes { x: false, y: true });
    ui.style(menu, |s| {
        s.max_size.height = taffy::Dimension::length(320.0);
        s.padding = taffy::Rect { left: lp(4.0), right: lp(4.0), top: lp(4.0), bottom: lp(4.0) };
    });
    let mut chosen = None;
    for (i, o) in options.iter().enumerate() {
        let item = ui.add(menu, Button::new(o.clone()).class("menu-item"));
        ui.style(item, |s| s.justify_content = Some(taffy::JustifyContent::FlexStart));
        if Some(i) == selected {
            ui.set_selected(item, true);
            chosen = Some(item);
        }
        ui.on(item, EventKind::Click, move |cx, _| {
            cx.defer(move |ui| choose(ui, select, i));
            true
        });
    }
    if options.is_empty() {
        ui.add(menu, Text::new("(none)"));
    }
    ui.with::<Select, _>(select, |s| s.open = Some(backdrop));
    if let Some(c) = chosen {
        ui.defer(move |ui| ui.scroll_into_view(c));
    }
}

fn close(ui: &mut Ui, select: NodeId) {
    if let Some(b) = ui.with::<Select, _>(select, |s| s.open.take()).flatten() {
        ui.remove(b);
    }
}

fn choose(ui: &mut Ui, select: NodeId, i: usize) {
    let msg = ui.with::<Select, _>(select, |s| {
        s.selected = Some(i);
        s.on_change.as_ref().map(|f| f(i))
    });
    if let Some(Some(m)) = msg {
        ui.push_message(m);
    }
    close(ui, select);
}

impl Ui {
    /// A [`Select`]'s options or choice set from outside (no message is sent).
    pub fn set_select(&mut self, node: NodeId, options: Option<Vec<String>>, selected: Option<usize>) {
        let same = self.get::<Select>(node).is_some_and(|s| options.as_ref().is_none_or(|o| *o == s.options) && s.selected == selected);
        if !same {
            self.with::<Select, _>(node, |s| {
                if let Some(o) = options {
                    s.options = o;
                }
                s.selected = selected;
            });
        }
    }

    pub(crate) fn push_message(&mut self, m: Box<dyn Any>) {
        self.emit_boxed(m);
    }
}
