//! A one-line text field: caret, selection by pointer and Shift+arrows, clipboard, a hint
//! while empty, a password mode.

use super::{Emit, emit_with, lp};
use crate::element::{Element, Event, EventCx, MeasureCx, PaintCx};
use crate::input::{Button, Key};
use emath::{Pos2, Rect, Vec2};
use epaint::Stroke;
use epaint::text::cursor::CCursor;
use std::any::Any;

pub struct TextInput {
    pub value: String,
    pub hint: String,
    pub password: bool,
    /// Caret and the other end of the selection, in characters.
    caret: usize,
    anchor: usize,
    /// How far the text is scrolled left (a long value).
    scroll: f32,
    focused_at: f64,
    on_change: Option<Emit<String>>,
    on_submit: Option<Emit<String>>,
    pub max_len: Option<usize>,
}

impl TextInput {
    pub fn new(value: impl Into<String>) -> TextInput {
        let value: String = value.into();
        let n = value.chars().count();
        TextInput { value, hint: String::new(), password: false, caret: n, anchor: n, scroll: 0.0, focused_at: 0.0, on_change: None, on_submit: None, max_len: None }
    }
    pub fn hint(mut self, h: impl Into<String>) -> TextInput {
        self.hint = h.into();
        self
    }
    pub fn password(mut self) -> TextInput {
        self.password = true;
        self
    }
    /// The message sent with the new text after each change.
    pub fn on_change<M: Any>(mut self, f: impl Fn(String) -> M + 'static) -> TextInput {
        self.on_change = Some(emit_with(f));
        self
    }
    /// The message sent with the text when Enter is pressed.
    pub fn on_submit<M: Any>(mut self, f: impl Fn(String) -> M + 'static) -> TextInput {
        self.on_submit = Some(emit_with(f));
        self
    }

    /// The text from outside: the caret goes to its end.
    pub fn set(&mut self, v: &str) {
        if self.value != v {
            self.value = v.to_owned();
            let n = self.value.chars().count();
            self.caret = self.caret.min(n);
            self.anchor = self.anchor.min(n);
        }
    }

    fn shown(&self) -> String {
        if self.password { "•".repeat(self.value.chars().count()) } else { self.value.clone() }
    }

    fn byte(&self, ch: usize) -> usize {
        self.value.char_indices().nth(ch).map(|(b, _)| b).unwrap_or(self.value.len())
    }

    fn selection(&self) -> (usize, usize) {
        (self.caret.min(self.anchor), self.caret.max(self.anchor))
    }

    fn selected(&self) -> String {
        let (a, b) = self.selection();
        self.value[self.byte(a)..self.byte(b)].to_owned()
    }

    fn delete_selection(&mut self) -> bool {
        let (a, b) = self.selection();
        if a == b {
            return false;
        }
        let (ba, bb) = (self.byte(a), self.byte(b));
        self.value.replace_range(ba..bb, "");
        self.caret = a;
        self.anchor = a;
        true
    }

    fn insert(&mut self, text: &str) {
        self.delete_selection();
        let text: String = text.chars().filter(|c| !c.is_control()).collect();
        let room = self.max_len.map(|m| m.saturating_sub(self.value.chars().count())).unwrap_or(usize::MAX);
        let text: String = text.chars().take(room).collect();
        let b = self.byte(self.caret);
        self.value.insert_str(b, &text);
        self.caret += text.chars().count();
        self.anchor = self.caret;
    }

    fn changed(&self, cx: &mut EventCx<'_>) {
        if let Some(f) = &self.on_change {
            cx.fx.messages.push(f(self.value.clone()));
        }
        cx.repaint();
    }

    /// The character boundary before/after `i` at a word's edge.
    fn word(&self, i: usize, forward: bool) -> usize {
        let chars: Vec<char> = self.value.chars().collect();
        let mut j = i;
        if forward {
            while j < chars.len() && !chars[j].is_alphanumeric() {
                j += 1;
            }
            while j < chars.len() && chars[j].is_alphanumeric() {
                j += 1;
            }
        } else {
            while j > 0 && !chars[j - 1].is_alphanumeric() {
                j -= 1;
            }
            while j > 0 && chars[j - 1].is_alphanumeric() {
                j -= 1;
            }
        }
        j
    }

    fn caret_at(&self, cx: &mut EventCx<'_>, pos: Pos2) -> usize {
        let g = cx.layout_text(&self.shown(), None);
        let x = pos.x - cx.rect.left() - pad_x(cx.rect) + self.scroll;
        g.cursor_from_pos(Vec2::new(x, g.size().y * 0.5)).index.0
    }
}

fn pad_x(_r: Rect) -> f32 {
    10.0
}

impl Element for TextInput {
    fn class(&self) -> &'static str {
        "text-input"
    }
    fn layout(&self, s: &mut taffy::Style) {
        s.padding = taffy::Rect { left: lp(10.0), right: lp(10.0), top: lp(6.0), bottom: lp(6.0) };
        s.min_size.width = taffy::Dimension::length(60.0);
    }
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _k: [Option<f32>; 2], _a: [Option<f32>; 2]) -> Vec2 {
        let g = cx.layout_text("Ag", None);
        Vec2::new(160.0, g.size().y)
    }
    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let r = cx.content;
        let shown = self.shown();
        let g = cx.layout_text(&shown, None);
        // keep the caret in view
        let caret_x = g.pos_from_cursor(CCursor::new(self.caret)).left();
        if caret_x - self.scroll > r.width() - 2.0 {
            self.scroll = caret_x - r.width() + 2.0;
        }
        if caret_x - self.scroll < 0.0 {
            self.scroll = caret_x.max(0.0);
        }
        self.scroll = self.scroll.min((g.size().x - r.width() + 2.0).max(0.0));
        let origin = Pos2::new(r.left() - self.scroll, r.center().y - g.size().y * 0.5);
        let clip = cx.rect.shrink(1.0);
        let focused = cx.state.focused;
        let sel_color = cx.theme.selection;
        let caret_color = cx.theme.caret;
        let blink = cx.theme.caret_blink;
        let hint = self.hint.clone();
        let hint_g = (self.value.is_empty() && !hint.is_empty()).then(|| {
            let mut look = cx.look.clone();
            look.color = look.color.gamma_multiply(0.45);
            crate::paint::layout_text(cx.fonts, cx.ppp, &hint, look.font_id(cx.theme), look.color, None)
        });
        let (a, b) = self.selection();
        let (pa, pb) = (g.pos_from_cursor(CCursor::new(a)), g.pos_from_cursor(CCursor::new(b)));
        let caret = g.pos_from_cursor(CCursor::new(self.caret));
        let color = cx.look.color;
        let time = cx.time;
        let since = time - self.focused_at;
        cx.painter.with_clip(clip, |p| {
            if focused && a != b {
                p.rect_filled(Rect::from_min_max(origin + Vec2::new(pa.left(), 0.0), origin + Vec2::new(pb.left(), g.size().y)), 2.0, sel_color);
            }
            if let Some(h) = hint_g {
                p.galley(Pos2::new(r.left(), r.center().y - h.size().y * 0.5), h, color);
            }
            p.galley(origin, g.clone(), color);
            let on = blink <= 0.0 || ((since / blink as f64) as i64) % 2 == 0;
            if focused && on {
                let x = origin.x + caret.left();
                p.line(Pos2::new(x, origin.y), Pos2::new(x, origin.y + g.size().y.max(14.0)), Stroke::new(1.5, caret_color));
            }
        });
        if focused && blink > 0.0 {
            let next = blink - (since as f32 % blink);
            cx.repaint_after(next);
        }
    }
    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> bool {
        if cx.target != cx.node {
            return false;
        }
        let n = self.value.chars().count();
        match event {
            Event::FocusGained => {
                self.focused_at = cx.time;
                cx.repaint();
                false
            }
            Event::FocusLost => {
                self.anchor = self.caret;
                cx.repaint();
                false
            }
            Event::PointerDown { pos, button: Button::Primary } => {
                let at = self.caret_at(cx, *pos);
                self.caret = at;
                if !cx.modifiers.shift {
                    self.anchor = at;
                }
                self.focused_at = cx.time;
                cx.capture();
                cx.repaint();
                true
            }
            Event::Drag { pos, .. } => {
                self.caret = self.caret_at(cx, *pos);
                cx.repaint();
                true
            }
            Event::Click { count: 2, .. } => {
                self.anchor = self.word(self.caret, false);
                self.caret = self.word(self.caret, true);
                cx.repaint();
                true
            }
            Event::Text(t) => {
                self.insert(t);
                self.focused_at = cx.time;
                self.changed(cx);
                true
            }
            Event::Paste(t) => {
                let line: String = t.lines().next().unwrap_or("").to_owned();
                self.insert(&line);
                self.changed(cx);
                true
            }
            Event::Key { key, pressed: true, modifiers, .. } => {
                let cmd = modifiers.command();
                let shift = modifiers.shift;
                let word = modifiers.alt || (!cfg!(target_os = "macos") && modifiers.ctrl);
                self.focused_at = cx.time;
                let moved = |to: usize, s: &mut TextInput| {
                    s.caret = to.min(n);
                    if !shift {
                        s.anchor = s.caret;
                    }
                };
                match key {
                    Key::Left => {
                        let (a, b) = self.selection();
                        let to = if a != b && !shift { a } else if word { self.word(self.caret, false) } else if cmd { 0 } else { self.caret.saturating_sub(1) };
                        moved(to, self);
                    }
                    Key::Right => {
                        let (a, b) = self.selection();
                        let to = if a != b && !shift { b } else if word { self.word(self.caret, true) } else if cmd { n } else { self.caret + 1 };
                        moved(to, self);
                    }
                    Key::Home | Key::Up => moved(0, self),
                    Key::End | Key::Down => moved(n, self),
                    Key::Backspace => {
                        if !self.delete_selection() && self.caret > 0 {
                            let from = if word || cmd { if cmd { 0 } else { self.word(self.caret, false) } } else { self.caret - 1 };
                            let (ba, bb) = (self.byte(from), self.byte(self.caret));
                            self.value.replace_range(ba..bb, "");
                            self.caret = from;
                            self.anchor = from;
                        }
                        self.changed(cx);
                    }
                    Key::Delete => {
                        if !self.delete_selection() && self.caret < n {
                            let to = if word { self.word(self.caret, true) } else { self.caret + 1 };
                            let (ba, bb) = (self.byte(self.caret), self.byte(to));
                            self.value.replace_range(ba..bb, "");
                        }
                        self.changed(cx);
                    }
                    Key::A if cmd => {
                        self.anchor = 0;
                        self.caret = n;
                    }
                    Key::C if cmd => {
                        if !self.password {
                            cx.copy(self.selected());
                        }
                    }
                    Key::X if cmd => {
                        if !self.password {
                            cx.copy(self.selected());
                            self.delete_selection();
                            self.changed(cx);
                        }
                    }
                    Key::Enter => {
                        if let Some(f) = &self.on_submit {
                            cx.fx.messages.push(f(self.value.clone()));
                        }
                    }
                    Key::Escape => {
                        cx.focus(None);
                    }
                    Key::Tab => return false,
                    _ => return !cmd,
                }
                cx.repaint();
                true
            }
            Event::Key { .. } => true,
            _ => false,
        }
    }
    fn focusable(&self) -> bool {
        true
    }
}
