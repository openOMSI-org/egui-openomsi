//! The built-in elements: boxes, text, buttons, fields, switches, sliders, menus, pictures.
//! Each has a theme class of its name (`"button"`, `"text-input"`) to style it by, and each is
//! an ordinary [`Element`] - an application's own widgets are written the same way.

mod select;
mod text_input;

pub use select::Select;
pub use text_input::TextInput;

use crate::element::{Element, Event, EventCx, MeasureCx, PaintCx};
use crate::input::{Button as MouseButton, Key};
use crate::{NodeId, Ui};
use ecolor::Color32;
use emath::{Align2, Pos2, Rect, Vec2};
use epaint::{Stroke, TextureId};
use std::any::Any;

/// A message an element sends when its value changes: made by the application's closure.
pub type Emit<T> = Box<dyn Fn(T) -> Box<dyn Any>>;

pub(crate) fn emit_with<T, M: Any>(f: impl Fn(T) -> M + 'static) -> Emit<T> {
    Box::new(move |v| Box::new(f(v)))
}

/// Whether the element takes typed text (a phone shows its keyboard for it).
pub(crate) fn is_text_field(e: &dyn Element) -> bool {
    let a: &dyn Any = e;
    a.is::<TextInput>()
}

// --- a box -----------------------------------------------------------------------------------

/// A box that holds other nodes: laid out by its style (a flex column by default), painted
/// with its background and border.
pub struct Div {
    class: &'static str,
}

impl Div {
    pub fn new() -> Div {
        Div { class: "div" }
    }
    /// A row (children side by side).
    pub fn row() -> Div {
        Div { class: "row" }
    }
    /// A column.
    pub fn column() -> Div {
        Div { class: "column" }
    }
}

impl Default for Div {
    fn default() -> Self {
        Div::new()
    }
}

impl Element for Div {
    fn class(&self) -> &'static str {
        self.class
    }
    fn layout(&self, s: &mut taffy::Style) {
        s.display = taffy::Display::Flex;
        s.flex_direction = if self.class == "row" { taffy::FlexDirection::Row } else { taffy::FlexDirection::Column };
        if self.class == "row" {
            s.align_items = Some(taffy::AlignItems::Center);
        }
    }
    fn hit_test(&self) -> bool {
        true
    }
}

/// A box the pointer passes through (a page's layout, not something to click).
pub struct Spacer;

impl Element for Spacer {
    fn layout(&self, s: &mut taffy::Style) {
        s.flex_grow = 1.0;
    }
    fn hit_test(&self) -> bool {
        false
    }
}

// --- text --------------------------------------------------------------------------------------

/// How text sits in its box.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Text: one line, or wrapped at the box's width.
pub struct Text {
    pub text: String,
    pub wrap: bool,
    pub align: TextAlign,
    class: &'static str,
}

impl Text {
    pub fn new(text: impl Into<String>) -> Text {
        Text { text: text.into(), wrap: false, align: TextAlign::Left, class: "text" }
    }
    pub fn wrapped(text: impl Into<String>) -> Text {
        Text { wrap: true, ..Text::new(text) }
    }
    pub fn class(mut self, c: &'static str) -> Text {
        self.class = c;
        self
    }
    pub fn align(mut self, a: TextAlign) -> Text {
        self.align = a;
        self
    }
}

impl Element for Text {
    fn class(&self) -> &'static str {
        self.class
    }
    fn measure(&mut self, cx: &mut MeasureCx<'_>, known: [Option<f32>; 2], available: [Option<f32>; 2]) -> Vec2 {
        let wrap = if self.wrap { known[0].or(available[0]) } else { None };
        let g = cx.layout_text(&self.text, wrap);
        let mut s = g.size();
        if self.wrap {
            if let Some(w) = known[0] {
                s.x = w;
            }
        }
        // (an empty text keeps its line's height)
        s.y = s.y.max(cx.look.font_size * 1.2);
        s
    }
    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let wrap = self.wrap.then_some(cx.content.width());
        let g = cx.layout_text(&self.text, wrap);
        let x = match self.align {
            TextAlign::Left => cx.content.left(),
            TextAlign::Center => cx.content.center().x - g.size().x * 0.5,
            TextAlign::Right => cx.content.right() - g.size().x,
        };
        let y = cx.content.center().y - g.size().y * 0.5;
        let color = cx.look.color;
        cx.painter.galley(Pos2::new(x, y), g, color);
    }
    fn hit_test(&self) -> bool {
        false
    }
    fn paint_box(&self) -> bool {
        true
    }
}

/// A glyph of an icon font (the theme's `"icon"` class names the family): Material Icons'
/// ligature-free code points, for example.
pub struct Icon {
    pub glyph: String,
}

impl Icon {
    pub fn new(glyph: impl Into<String>) -> Icon {
        Icon { glyph: glyph.into() }
    }
}

impl Element for Icon {
    fn class(&self) -> &'static str {
        "icon"
    }
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _k: [Option<f32>; 2], _a: [Option<f32>; 2]) -> Vec2 {
        let s = cx.look.font_size;
        Vec2::splat(s)
    }
    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let g = cx.layout_text(&self.glyph, None);
        let at = cx.content.center() - g.size() * 0.5;
        let c = cx.look.color;
        cx.painter.galley(at, g, c);
    }
    fn hit_test(&self) -> bool {
        false
    }
}

// --- a button ----------------------------------------------------------------------------------

/// A button: its text (and an icon before it), clicked by the pointer, Enter or Space.
/// Give it handlers with [`Ui::on_click`].
pub struct Button {
    pub text: String,
    pub icon: Option<String>,
    pub align: TextAlign,
    class: &'static str,
}

impl Button {
    pub fn new(text: impl Into<String>) -> Button {
        Button { text: text.into(), icon: None, align: TextAlign::Center, class: "button" }
    }
    pub fn icon(mut self, glyph: impl Into<String>) -> Button {
        self.icon = Some(glyph.into());
        self
    }
    pub fn align(mut self, a: TextAlign) -> Button {
        self.align = a;
        self
    }
    /// Another class than `"button"` to start from (`"primary"`, `"nav-item"`).
    pub fn class(mut self, c: &'static str) -> Button {
        self.class = c;
        self
    }

    fn parts(&self, cx_fonts: &mut epaint::Fonts, ppp: f32, look: &crate::Look, theme: &crate::Theme) -> (Vec2, Option<std::sync::Arc<epaint::Galley>>, std::sync::Arc<epaint::Galley>) {
        let text = crate::paint::layout_text(cx_fonts, ppp, &self.text, look.font_id(theme), look.color, None);
        let icon = self.icon.as_ref().map(|g| {
            let fam = theme.rules.get("icon").and_then(|r| r.visual.font.clone()).unwrap_or_else(|| look.font.clone());
            crate::paint::layout_text(cx_fonts, ppp, g, epaint::FontId::new(look.font_size * 1.25, fam), look.color, None)
        });
        let gap = if icon.is_some() && !self.text.is_empty() { 8.0 } else { 0.0 };
        let w = text.size().x + icon.as_ref().map_or(0.0, |i| i.size().x) + gap;
        let h = text.size().y.max(icon.as_ref().map_or(0.0, |i| i.size().y));
        (Vec2::new(w, h), icon, text)
    }
}

impl Element for Button {
    fn class(&self) -> &'static str {
        self.class
    }
    fn layout(&self, s: &mut taffy::Style) {
        s.padding = taffy::Rect { left: lp(12.0), right: lp(12.0), top: lp(6.0), bottom: lp(6.0) };
        s.align_items = Some(taffy::AlignItems::Center);
        s.justify_content = Some(taffy::JustifyContent::Center);
    }
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _k: [Option<f32>; 2], _a: [Option<f32>; 2]) -> Vec2 {
        self.parts(cx.fonts, cx.ppp, cx.look, cx.theme).0
    }
    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let (size, icon, text) = self.parts(cx.fonts, cx.ppp, cx.look, cx.theme);
        let mut x = match self.align {
            TextAlign::Left => cx.content.left(),
            TextAlign::Center => cx.content.center().x - size.x * 0.5,
            TextAlign::Right => cx.content.right() - size.x,
        };
        let cy = cx.content.center().y;
        let color = cx.look.color;
        if let Some(i) = icon {
            let w = i.size().x;
            cx.painter.galley(Pos2::new(x, cy - i.size().y * 0.5), i, color);
            x += w + 8.0;
        }
        let th = text.size().y;
        cx.painter.galley(Pos2::new(x, cy - th * 0.5), text, color);
    }
    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> bool {
        if let Event::Key { key: Key::Enter | Key::Space, pressed: true, .. } = event {
            if cx.target == cx.node {
                let pos = cx.rect.center();
                cx.send(cx.node, Event::Click { pos, button: MouseButton::Primary, count: 1 });
                return true;
            }
        }
        false
    }
    fn focusable(&self) -> bool {
        true
    }
}

pub(crate) fn lp(v: f32) -> taffy::LengthPercentage {
    taffy::LengthPercentage::length(v)
}

// --- a tick box and a switch -------------------------------------------------------------------

/// A box to tick, with its text: sends its message with the new state.
pub struct Checkbox {
    pub checked: bool,
    pub text: String,
    on_change: Option<Emit<bool>>,
    switch: bool,
}

impl Checkbox {
    pub fn new(checked: bool, text: impl Into<String>) -> Checkbox {
        Checkbox { checked, text: text.into(), on_change: None, switch: false }
    }
    /// A switch (a sliding knob) instead of a tick box.
    pub fn switch(checked: bool, text: impl Into<String>) -> Checkbox {
        Checkbox { switch: true, ..Checkbox::new(checked, text) }
    }
    pub fn on_change<M: Any>(mut self, f: impl Fn(bool) -> M + 'static) -> Checkbox {
        self.on_change = Some(emit_with(f));
        self
    }
    fn mark(&self) -> Vec2 {
        if self.switch { Vec2::new(34.0, 20.0) } else { Vec2::splat(18.0) }
    }
}

impl Element for Checkbox {
    fn class(&self) -> &'static str {
        if self.switch { "switch" } else { "checkbox" }
    }
    fn layout(&self, s: &mut taffy::Style) {
        s.align_items = Some(taffy::AlignItems::Center);
    }
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _k: [Option<f32>; 2], _a: [Option<f32>; 2]) -> Vec2 {
        let m = self.mark();
        if self.text.is_empty() {
            return m;
        }
        let g = cx.layout_text(&self.text, None);
        Vec2::new(m.x + 10.0 + g.size().x, m.y.max(g.size().y))
    }
    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let m = self.mark();
        let r = Rect::from_min_size(Pos2::new(cx.content.left(), cx.content.center().y - m.y * 0.5), m);
        let accent = cx.theme.selection.to_opaque();
        let on_fill = cx.theme.rules.get("accent").and_then(|r| r.visual.background).unwrap_or(accent);
        let off = cx.theme.rules.get("field").and_then(|r| r.visual.background).unwrap_or(Color32::from_gray(50));
        if self.switch {
            cx.painter.rect_filled(r, m.y * 0.5, if self.checked { on_fill } else { off });
            let k = if self.checked { r.right() - m.y * 0.5 } else { r.left() + m.y * 0.5 };
            cx.painter.circle_filled(Pos2::new(k, r.center().y), m.y * 0.5 - 3.0, Color32::WHITE);
        } else {
            cx.painter.rect_filled(r, 4.0, if self.checked { on_fill } else { off });
            if self.checked {
                let s = Stroke::new(2.0, Color32::WHITE);
                let p = |x: f32, y: f32| Pos2::new(r.left() + x * r.width(), r.top() + y * r.height());
                cx.painter.add(epaint::Shape::line(vec![p(0.24, 0.52), p(0.43, 0.7), p(0.77, 0.32)], s));
            } else if cx.state.hovered {
                cx.painter.rect_stroke(r, 4.0, Stroke::new(1.0, cx.look.color.gamma_multiply(0.4)));
            }
        }
        if !self.text.is_empty() {
            let g = cx.layout_text(&self.text, None);
            let at = Pos2::new(r.right() + 10.0, cx.content.center().y - g.size().y * 0.5);
            let c = cx.look.color;
            cx.painter.galley(at, g, c);
        }
    }
    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> bool {
        let toggle = matches!(event, Event::Click { button: MouseButton::Primary, .. })
            || matches!(event, Event::Key { key: Key::Space | Key::Enter, pressed: true, .. }) && cx.target == cx.node;
        if toggle {
            self.checked = !self.checked;
            if let Some(f) = &self.on_change {
                cx.fx.messages.push(f(self.checked));
            }
            cx.repaint();
            return true;
        }
        false
    }
    fn focusable(&self) -> bool {
        true
    }
}

// --- a slider ----------------------------------------------------------------------------------

/// A value between `min` and `max` set by dragging (or the arrow keys).
pub struct Slider {
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
    on_change: Option<Emit<f32>>,
}

impl Slider {
    pub fn new(value: f32, min: f32, max: f32) -> Slider {
        Slider { value, min, max, step: 0.0, on_change: None }
    }
    pub fn step(mut self, s: f32) -> Slider {
        self.step = s;
        self
    }
    pub fn on_change<M: Any>(mut self, f: impl Fn(f32) -> M + 'static) -> Slider {
        self.on_change = Some(emit_with(f));
        self
    }
    fn set(&mut self, cx: &mut EventCx<'_>, v: f32) {
        let mut v = v.clamp(self.min, self.max);
        if self.step > 0.0 {
            v = self.min + ((v - self.min) / self.step).round() * self.step;
        }
        if v != self.value {
            self.value = v;
            if let Some(f) = &self.on_change {
                cx.fx.messages.push(f(v));
            }
            cx.repaint();
        }
    }
    fn at(&mut self, cx: &mut EventCx<'_>, x: f32) {
        let r = cx.rect.shrink2(Vec2::new(8.0, 0.0));
        let t = ((x - r.left()) / r.width().max(1.0)).clamp(0.0, 1.0);
        self.set(cx, self.min + t * (self.max - self.min));
    }
}

impl Element for Slider {
    fn class(&self) -> &'static str {
        "slider"
    }
    fn layout(&self, s: &mut taffy::Style) {
        s.min_size.height = taffy::Dimension::length(22.0);
        s.flex_grow = 1.0;
    }
    fn measure(&mut self, _cx: &mut MeasureCx<'_>, _k: [Option<f32>; 2], _a: [Option<f32>; 2]) -> Vec2 {
        Vec2::new(120.0, 22.0)
    }
    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let r = cx.content.shrink2(Vec2::new(8.0, 0.0));
        let t = if self.max > self.min { (self.value - self.min) / (self.max - self.min) } else { 0.0 };
        let y = r.center().y;
        let accent = cx.theme.rules.get("accent").and_then(|r| r.visual.background).unwrap_or(cx.theme.selection.to_opaque());
        let off = cx.theme.rules.get("field").and_then(|r| r.visual.background).unwrap_or(Color32::from_gray(50));
        let track = Rect::from_min_max(Pos2::new(r.left(), y - 2.0), Pos2::new(r.right(), y + 2.0));
        cx.painter.rect_filled(track, 2.0, off);
        let x = r.left() + t * r.width();
        cx.painter.rect_filled(Rect::from_min_max(track.min, Pos2::new(x, track.bottom())), 2.0, accent);
        let knob = if cx.state.pressed || cx.state.hovered { 8.0 } else { 7.0 };
        cx.painter.circle_filled(Pos2::new(x, y), knob, Color32::WHITE);
    }
    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> bool {
        let step = if self.step > 0.0 { self.step } else { (self.max - self.min) / 100.0 };
        match event {
            Event::PointerDown { pos, button: MouseButton::Primary } => {
                cx.capture();
                self.at(cx, pos.x);
                true
            }
            Event::Drag { pos, .. } => {
                self.at(cx, pos.x);
                true
            }
            Event::Key { key: Key::Left | Key::Down, pressed: true, .. } => {
                self.set(cx, self.value - step);
                true
            }
            Event::Key { key: Key::Right | Key::Up, pressed: true, .. } => {
                self.set(cx, self.value + step);
                true
            }
            _ => false,
        }
    }
    fn focusable(&self) -> bool {
        true
    }
}

// --- a picture ---------------------------------------------------------------------------------

/// A texture of the application's (a 3D picture drawn by its renderer, an icon it decoded),
/// filling the box; `aspect` keeps its proportions (cover).
pub struct Image {
    pub texture: Option<TextureId>,
    /// The texture's size in pixels (for `cover`).
    pub size: Vec2,
    pub cover: bool,
    pub tint: Color32,
    /// Text shown while there is no texture.
    pub placeholder: String,
}

impl Image {
    pub fn new(texture: Option<TextureId>, size: Vec2) -> Image {
        Image { texture, size, cover: true, tint: Color32::WHITE, placeholder: String::new() }
    }
}

impl Element for Image {
    fn class(&self) -> &'static str {
        "image"
    }
    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let r = cx.content;
        match self.texture {
            Some(t) => {
                let uv = if self.cover && self.size.x > 0.0 && self.size.y > 0.0 {
                    let (ta, ra) = (self.size.x / self.size.y, r.width() / r.height().max(1.0));
                    if ta > ra {
                        let w = ra / ta;
                        Rect::from_min_max(Pos2::new(0.5 - w * 0.5, 0.0), Pos2::new(0.5 + w * 0.5, 1.0))
                    } else {
                        let h = ta / ra;
                        Rect::from_min_max(Pos2::new(0.0, 0.5 - h * 0.5), Pos2::new(1.0, 0.5 + h * 0.5))
                    }
                } else {
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0))
                };
                let radius = cx.look.radius;
                cx.painter.image(t, r, uv, self.tint, radius);
            }
            None if !self.placeholder.is_empty() => {
                let g = cx.layout_text(&self.placeholder, None);
                let at = r.center() - g.size() * 0.5;
                let c = cx.look.color;
                cx.painter.galley(at, g, c);
            }
            None => {}
        }
    }
}

// --- progress ----------------------------------------------------------------------------------

/// A bar filled to `value` (0 to 1), or moving when the amount is not known.
pub struct Progress {
    pub value: Option<f32>,
}

impl Element for Progress {
    fn class(&self) -> &'static str {
        "progress"
    }
    fn layout(&self, s: &mut taffy::Style) {
        s.size.height = taffy::Dimension::length(4.0);
        s.flex_grow = 1.0;
    }
    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let r = cx.rect;
        let accent = cx.theme.rules.get("accent").and_then(|r| r.visual.background).unwrap_or(cx.theme.selection.to_opaque());
        let off = cx.theme.rules.get("field").and_then(|r| r.visual.background).unwrap_or(Color32::from_gray(50));
        cx.painter.rect_filled(r, r.height() * 0.5, off);
        match self.value {
            Some(v) => cx.painter.rect_filled(Rect::from_min_size(r.min, Vec2::new(r.width() * v.clamp(0.0, 1.0), r.height())), r.height() * 0.5, accent),
            None => {
                let t = (cx.time * 0.8).fract() as f32;
                let w = r.width() * 0.3;
                let x = r.left() - w + (r.width() + w) * t;
                cx.painter.with_clip(r, |p| p.rect_filled(Rect::from_min_size(Pos2::new(x, r.top()), Vec2::new(w, r.height())), r.height() * 0.5, accent));
                cx.repaint_after(0.0);
            }
        }
    }
}

/// A turning arc: something is being loaded.
pub struct Spinner;

impl Element for Spinner {
    fn class(&self) -> &'static str {
        "spinner"
    }
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _k: [Option<f32>; 2], _a: [Option<f32>; 2]) -> Vec2 {
        Vec2::splat(cx.look.font_size * 1.2)
    }
    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let a = (cx.time * 5.0) as f32;
        let r = cx.content.width().min(cx.content.height()) * 0.5 - 1.5;
        let c = cx.look.color;
        cx.painter.arc(cx.content.center(), r, a, a + 4.2, Stroke::new(2.0, c));
        cx.repaint_after(0.0);
    }
}

/// A thin line between parts.
pub struct Separator;

impl Element for Separator {
    fn class(&self) -> &'static str {
        "separator"
    }
    fn layout(&self, s: &mut taffy::Style) {
        s.size.height = taffy::Dimension::length(1.0);
        s.flex_shrink = 0.0;
    }
}

// --- convenience -------------------------------------------------------------------------------

impl Ui {
    pub fn div(&mut self, parent: NodeId) -> NodeId {
        self.add(parent, Div::new())
    }
    pub fn row(&mut self, parent: NodeId) -> NodeId {
        self.add(parent, Div::row())
    }
    pub fn column(&mut self, parent: NodeId) -> NodeId {
        self.add(parent, Div::column())
    }
    pub fn text(&mut self, parent: NodeId, text: impl Into<String>) -> NodeId {
        self.add(parent, Text::new(text))
    }
    pub fn paragraph(&mut self, parent: NodeId, text: impl Into<String>) -> NodeId {
        self.add(parent, Text::wrapped(text))
    }
    pub fn button<M: Any + Clone>(&mut self, parent: NodeId, text: impl Into<String>, message: M) -> NodeId {
        let b = self.add(parent, Button::new(text));
        self.on_click(b, message);
        b
    }
    pub fn spacer(&mut self, parent: NodeId) -> NodeId {
        self.add(parent, Spacer)
    }

    /// Change a [`Text`]'s words (laid out again only when they changed).
    pub fn set_text(&mut self, node: NodeId, text: &str) {
        let same = self.get::<Text>(node).is_some_and(|t| t.text == text) || self.get::<Button>(node).is_some_and(|b| b.text == text);
        if same {
            return;
        }
        if self.with::<Text, _>(node, |t| t.text = text.to_owned()).is_none() {
            self.with::<Button, _>(node, |b| b.text = text.to_owned());
        }
    }

    /// A [`Checkbox`]'s state set from outside (no message is sent).
    pub fn set_checked(&mut self, node: NodeId, on: bool) {
        if self.get::<Checkbox>(node).is_some_and(|c| c.checked != on) {
            self.with::<Checkbox, _>(node, |c| c.checked = on);
        }
    }

    pub fn set_slider(&mut self, node: NodeId, v: f32) {
        if self.get::<Slider>(node).is_some_and(|s| s.value != v) {
            self.with::<Slider, _>(node, |s| s.value = v);
        }
    }

    pub fn set_progress(&mut self, node: NodeId, v: Option<f32>) {
        if self.get::<Progress>(node).is_some_and(|p| p.value != v) {
            self.with::<Progress, _>(node, |p| p.value = v);
        }
    }

    pub fn set_image(&mut self, node: NodeId, texture: Option<TextureId>, size: Vec2) {
        if self.get::<Image>(node).is_some_and(|i| i.texture != texture || i.size != size) {
            self.with::<Image, _>(node, |i| {
                i.texture = texture;
                i.size = size;
            });
        } else {
            // (the texture's picture itself changed: draw again)
            self.request_repaint();
        }
    }
}

/// Where a galley sits in a rect, by alignment (for elements of an application's own).
pub fn align_in(rect: Rect, size: Vec2, align: Align2) -> Rect {
    align.align_size_within_rect(size, rect)
}
