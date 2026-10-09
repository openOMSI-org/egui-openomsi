//! What happened since the last frame, as the platform reports it: pointer, keys, text,
//! touches. [`Ui::input`](crate::Ui::input) takes it (the `winit` feature translates winit's
//! window events, see [`WinitInput`]).

use emath::{Pos2, Vec2};

/// A pointer button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Button {
    Primary,
    Secondary,
    Middle,
}

/// Modifier keys held.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    /// Cmd on macOS.
    pub logo: bool,
}

impl Modifiers {
    /// The platform's command key: Cmd on macOS, Ctrl elsewhere.
    pub fn command(&self) -> bool {
        if cfg!(target_os = "macos") { self.logo } else { self.ctrl }
    }
}

/// Keys the widgets act on (letters arrive as [`InputEvent::Text`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Backspace,
    Delete,
    Enter,
    Escape,
    Tab,
    Space,
    A,
    C,
    V,
    X,
    Z,
    /// Any other key, by the platform's name for it.
    Other(u32),
}

/// One thing the platform reported.
#[derive(Clone, Debug, PartialEq)]
pub enum InputEvent {
    PointerMoved(Pos2),
    PointerButton { pos: Pos2, button: Button, pressed: bool },
    PointerLeft,
    /// Scrolled by this many points (positive: content moves down/right, as a wheel turned
    /// towards the user).
    Wheel(Vec2),
    Key { key: Key, pressed: bool, repeat: bool },
    Text(String),
    /// Text from the clipboard, for the focused field (the platform read it on Ctrl+V).
    Paste(String),
    /// The window got or lost the keyboard.
    Focus(bool),
    /// A finger (the first on the screen): a tap is a click, a drag scrolls - unless what it
    /// pressed holds the pointer (a slider, a picture turned by dragging).
    Touch { pos: Pos2, phase: TouchPhase },
}

/// Where a finger is in its touch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchPhase {
    Start,
    Move,
    End,
    Cancel,
}

/// Everything since the last frame.
#[derive(Clone, Debug, Default)]
pub struct Input {
    pub events: Vec<InputEvent>,
    pub modifiers: Modifiers,
    /// Seconds since the last frame.
    pub dt: f32,
    /// The window's size in points.
    pub screen: Vec2,
    /// Physical pixels per point.
    pub pixels_per_point: f32,
}

/// What the platform should do after a frame.
#[derive(Clone, Debug, Default)]
pub struct PlatformOutput {
    pub cursor: crate::Cursor,
    /// Text to put on the clipboard (Ctrl+C, Ctrl+X in a field).
    pub copied: Option<String>,
    /// Whether a text field has the keyboard (a phone shows its keyboard), and where its
    /// caret is (for the IME's candidate window).
    pub text_focus: Option<emath::Rect>,
    /// Links clicked.
    pub open_url: Option<String>,
}

#[cfg(feature = "winit")]
pub use winit_input::WinitInput;

#[cfg(feature = "winit")]
mod winit_input {
    use super::*;
    use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
    use winit::keyboard::{KeyCode, PhysicalKey};

    /// Collects winit's window events into the next frame's [`Input`].
    #[derive(Default)]
    pub struct WinitInput {
        input: Input,
        modifiers: Modifiers,
        pointer: Option<Pos2>,
        /// The finger followed.
        finger: Option<u64>,
    }

    impl WinitInput {
        pub fn new() -> Self {
            Self::default()
        }

        /// Take one event; `scale` is physical pixels per point. True when it was input (the
        /// frame should be drawn again).
        pub fn on_event(&mut self, event: &WindowEvent, scale: f32) -> bool {
            let ev = &mut self.input.events;
            match event {
                WindowEvent::CursorMoved { position, .. } => {
                    let p = Pos2::new(position.x as f32 / scale, position.y as f32 / scale);
                    self.pointer = Some(p);
                    ev.push(InputEvent::PointerMoved(p));
                }
                WindowEvent::CursorLeft { .. } => {
                    self.pointer = None;
                    ev.push(InputEvent::PointerLeft);
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    let button = match button {
                        MouseButton::Left => Button::Primary,
                        MouseButton::Right => Button::Secondary,
                        MouseButton::Middle => Button::Middle,
                        _ => return false,
                    };
                    let pos = self.pointer.unwrap_or(Pos2::new(-1e4, -1e4));
                    ev.push(InputEvent::PointerButton { pos, button, pressed: *state == ElementState::Pressed });
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    let d = match delta {
                        MouseScrollDelta::LineDelta(x, y) => Vec2::new(*x, *y) * 40.0,
                        MouseScrollDelta::PixelDelta(p) => Vec2::new(p.x as f32, p.y as f32) / scale,
                    };
                    ev.push(InputEvent::Wheel(d));
                }
                WindowEvent::Touch(t) => {
                    // the first finger is the pointer (the others are not followed)
                    let p = Pos2::new(t.location.x as f32 / scale, t.location.y as f32 / scale);
                    let phase = match t.phase {
                        winit::event::TouchPhase::Started if self.finger.is_none() => {
                            self.finger = Some(t.id);
                            TouchPhase::Start
                        }
                        _ if self.finger != Some(t.id) => return false,
                        winit::event::TouchPhase::Started | winit::event::TouchPhase::Moved => TouchPhase::Move,
                        winit::event::TouchPhase::Ended => TouchPhase::End,
                        winit::event::TouchPhase::Cancelled => TouchPhase::Cancel,
                    };
                    if matches!(phase, TouchPhase::End | TouchPhase::Cancel) {
                        self.finger = None;
                    }
                    ev.push(InputEvent::Touch { pos: p, phase });
                }
                WindowEvent::ModifiersChanged(m) => {
                    let s = m.state();
                    self.modifiers = Modifiers { shift: s.shift_key(), ctrl: s.control_key(), alt: s.alt_key(), logo: s.super_key() };
                    return false;
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    let pressed = event.state == ElementState::Pressed;
                    if let PhysicalKey::Code(code) = event.physical_key {
                        if let Some(key) = key_of(code) {
                            ev.push(InputEvent::Key { key, pressed, repeat: event.repeat });
                        }
                    }
                    // (typed characters, not while a shortcut is held)
                    if pressed && !self.modifiers.command() && !self.modifiers.ctrl {
                        if let Some(t) = event.text.as_ref().filter(|t| t.chars().all(|c| !c.is_control())) {
                            ev.push(InputEvent::Text(t.to_string()));
                        }
                    }
                }
                WindowEvent::Ime(winit::event::Ime::Commit(t)) => ev.push(InputEvent::Text(t.clone())),
                WindowEvent::Focused(f) => ev.push(InputEvent::Focus(*f)),
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {}
                _ => return false,
            }
            true
        }

        /// An event made up by the application (a scripted test, a remote control), as if it
        /// came from the window.
        pub fn push(&mut self, event: InputEvent) {
            if let InputEvent::PointerMoved(p) | InputEvent::PointerButton { pos: p, .. } = &event {
                self.pointer = Some(*p);
            }
            self.input.events.push(event);
        }

        /// A paste the application read from its clipboard.
        pub fn paste(&mut self, text: String) {
            self.input.events.push(InputEvent::Paste(text));
        }

        /// Whether the next frame's input asks for a paste (Ctrl/Cmd+V pressed): the
        /// application reads its clipboard then and calls [`WinitInput::paste`].
        pub fn wants_paste(&self) -> bool {
            self.modifiers.command() && self.input.events.iter().any(|e| matches!(e, InputEvent::Key { key: Key::V, pressed: true, .. }))
        }

        /// The input of the frame about to be drawn.
        pub fn take(&mut self, screen: Vec2, pixels_per_point: f32, dt: f32) -> Input {
            let mut i = std::mem::take(&mut self.input);
            i.modifiers = self.modifiers;
            i.screen = screen;
            i.pixels_per_point = pixels_per_point;
            i.dt = dt;
            i
        }
    }

    fn key_of(code: KeyCode) -> Option<Key> {
        Some(match code {
            KeyCode::ArrowLeft => Key::Left,
            KeyCode::ArrowRight => Key::Right,
            KeyCode::ArrowUp => Key::Up,
            KeyCode::ArrowDown => Key::Down,
            KeyCode::Home => Key::Home,
            KeyCode::End => Key::End,
            KeyCode::PageUp => Key::PageUp,
            KeyCode::PageDown => Key::PageDown,
            KeyCode::Backspace => Key::Backspace,
            KeyCode::Delete => Key::Delete,
            KeyCode::Enter | KeyCode::NumpadEnter => Key::Enter,
            KeyCode::Escape => Key::Escape,
            KeyCode::Tab => Key::Tab,
            KeyCode::Space => Key::Space,
            KeyCode::KeyA => Key::A,
            KeyCode::KeyC => Key::C,
            KeyCode::KeyV => Key::V,
            KeyCode::KeyX => Key::X,
            KeyCode::KeyZ => Key::Z,
            other => Key::Other(other as u32),
        })
    }

    /// The winit cursor of a [`crate::Cursor`].
    pub fn cursor_icon(c: crate::Cursor) -> winit::window::CursorIcon {
        use winit::window::CursorIcon as C;
        match c {
            crate::Cursor::Default => C::Default,
            crate::Cursor::Pointer => C::Pointer,
            crate::Cursor::Text => C::Text,
            crate::Cursor::Grab => C::Grab,
            crate::Cursor::Grabbing => C::Grabbing,
            crate::Cursor::NotAllowed => C::NotAllowed,
            crate::Cursor::ResizeHorizontal => C::EwResize,
            crate::Cursor::ResizeVertical => C::NsResize,
        }
    }

    impl crate::Cursor {
        pub fn to_winit(self) -> winit::window::CursorIcon {
            cursor_icon(self)
        }
    }
}
