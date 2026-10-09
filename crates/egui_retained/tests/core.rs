//! The retained core driven as an application drives it: input in, messages and frames out.

use egui_retained::input::Button as Mouse;
use egui_retained::widgets::{Button, Checkbox, Select, Slider, TextInput};
use egui_retained::*;

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Play,
    Name(String),
    Submit(String),
    Shadows(bool),
    Volume(f32),
    Map(usize),
}

fn input(events: Vec<InputEvent>) -> Input {
    Input { events, modifiers: Modifiers::default(), dt: 0.016, screen: vec2(800.0, 600.0), pixels_per_point: 1.0 }
}

fn click(ui: &mut Ui, p: Pos2) {
    ui.run(input(vec![
        InputEvent::PointerMoved(p),
        InputEvent::PointerButton { pos: p, button: Mouse::Primary, pressed: true },
        InputEvent::PointerButton { pos: p, button: Mouse::Primary, pressed: false },
    ]));
}

fn page(ui: &mut Ui) -> NodeId {
    let root = ui.root(Layer::Base);
    let p = ui.column(root);
    ui.style(p, |s| {
        s.padding = taffy::Rect::length(10.0_f32);
        s.gap = taffy::Size::length(8.0_f32);
        s.align_items = Some(taffy::AlignItems::FlexStart);
    });
    p
}

#[test]
fn a_click_sends_the_buttons_message() {
    let mut ui = Ui::new();
    let p = page(&mut ui);
    let b = ui.button(p, "Play", Msg::Play);
    ui.run(input(vec![]));
    let r = ui.rect(b);
    assert!(r.width() > 20.0 && r.height() > 10.0, "{r:?}");
    click(&mut ui, r.center());
    assert_eq!(ui.drain::<Msg>(), vec![Msg::Play]);
    // a press that leaves the button before it is let go is no click
    let out = r.right_bottom() + vec2(50.0, 50.0);
    ui.run(input(vec![
        InputEvent::PointerMoved(r.center()),
        InputEvent::PointerButton { pos: r.center(), button: Mouse::Primary, pressed: true },
        InputEvent::PointerMoved(out),
        InputEvent::PointerButton { pos: out, button: Mouse::Primary, pressed: false },
    ]));
    assert!(ui.drain::<Msg>().is_empty());
}

#[test]
fn keyboard_reaches_buttons_through_tab() {
    let mut ui = Ui::new();
    let p = page(&mut ui);
    ui.button(p, "Play", Msg::Play);
    ui.run(input(vec![]));
    ui.run(input(vec![InputEvent::Key { key: Key::Tab, pressed: true, repeat: false }, InputEvent::Key { key: Key::Enter, pressed: true, repeat: false }]));
    assert_eq!(ui.drain::<Msg>(), vec![Msg::Play]);
}

#[test]
fn a_field_is_typed_into_edited_and_submitted() {
    let mut ui = Ui::new();
    let p = page(&mut ui);
    let f = ui.add(p, TextInput::new("").hint("Name").on_change(Msg::Name).on_submit(Msg::Submit));
    ui.run(input(vec![]));
    let at = ui.rect(f).center();
    click(&mut ui, at);
    assert_eq!(ui.focused(), Some(f));
    let key = |k| InputEvent::Key { key: k, pressed: true, repeat: false };
    ui.run(input(vec![InputEvent::Text("Grundorf".into()), key(Key::Backspace), key(Key::Left), key(Key::Left), InputEvent::Text("X".into())]));
    assert_eq!(ui.get::<TextInput>(f).unwrap().value, "GrundXor");
    ui.run(input(vec![key(Key::Enter)]));
    let msgs = ui.drain::<Msg>();
    assert_eq!(msgs.last(), Some(&Msg::Submit("GrundXor".into())));
    assert!(msgs.contains(&Msg::Name("Grundorf".into())));
    // select all and copy
    let mut i = input(vec![key(Key::A), key(Key::C)]);
    i.modifiers = if cfg!(target_os = "macos") { Modifiers { logo: true, ..Default::default() } } else { Modifiers { ctrl: true, ..Default::default() } };
    let frame = ui.run(i);
    assert_eq!(frame.platform.copied.as_deref(), Some("GrundXor"));
    assert!(frame.platform.text_focus.is_some());
}

#[test]
fn boxes_switches_and_sliders_send_their_values() {
    let mut ui = Ui::new();
    let p = page(&mut ui);
    let c = ui.add(p, Checkbox::switch(false, "Shadows").on_change(Msg::Shadows));
    let s = ui.add(p, Slider::new(0.5, 0.0, 1.0).on_change(Msg::Volume));
    ui.style(s, |st| st.size.width = taffy::Dimension::length(200.0));
    ui.run(input(vec![]));
    let at = ui.rect(c).center();
    click(&mut ui, at);
    assert_eq!(ui.drain::<Msg>(), vec![Msg::Shadows(true)]);
    let r = ui.rect(s);
    let (a, b) = (pos2(r.center().x, r.center().y), pos2(r.right() - 8.0, r.center().y));
    ui.run(input(vec![
        InputEvent::PointerMoved(a),
        InputEvent::PointerButton { pos: a, button: Mouse::Primary, pressed: true },
        InputEvent::PointerMoved(pos2(a.x + 20.0, a.y)),
        InputEvent::PointerMoved(b),
        InputEvent::PointerButton { pos: b, button: Mouse::Primary, pressed: false },
    ]));
    let v = ui.drain::<Msg>();
    assert_eq!(v.last(), Some(&Msg::Volume(1.0)), "{v:?}");
}

#[test]
fn a_select_opens_its_menu_over_the_page_and_chooses() {
    let mut ui = Ui::new();
    let p = page(&mut ui);
    let sel = ui.add(p, Select::new(vec!["Grundorf".into(), "Berlin-Spandau".into(), "Hamburg".into()], Some(0)).on_change(Msg::Map));
    let below = ui.button(p, "Play", Msg::Play);
    ui.run(input(vec![]));
    let at = ui.rect(sel).center();
    click(&mut ui, at);
    ui.run(input(vec![]));
    ui.run(input(vec![]));
    // the menu lies over the button under the select: a click there picks an option
    let target = ui.rect(below).center();
    let item = ui.hit_name_at(target);
    click(&mut ui, target);
    let m = ui.drain::<Msg>();
    assert!(matches!(m.as_slice(), [Msg::Map(_)]), "{m:?} (hit {item:?})");
    assert!(!m.contains(&Msg::Play));
    ui.run(input(vec![]));
    // closed again: the button is reachable
    let at = ui.rect(below).center();
    click(&mut ui, at);
    assert_eq!(ui.drain::<Msg>(), vec![Msg::Play]);
}

#[test]
fn a_list_scrolls_and_clips_its_rows() {
    let mut ui = Ui::new();
    let p = page(&mut ui);
    let list = ui.column(p);
    ui.style(list, |s| s.size = taffy::Size { width: taffy::Dimension::length(200.0), height: taffy::Dimension::length(100.0) });
    ui.set_scroll(list, ScrollAxes { x: false, y: true });
    let rows: Vec<NodeId> = (0..20).map(|i| ui.add(list, Button::new(format!("Row {i}")))).collect();
    ui.run(input(vec![]));
    let first = ui.rect(rows[0]);
    ui.run(input(vec![InputEvent::PointerMoved(ui.rect(list).center()), InputEvent::Wheel(vec2(0.0, -60.0))]));
    ui.run(input(vec![]));
    assert!((ui.rect(rows[0]).top() - (first.top() - 60.0)).abs() < 0.5, "{:?} vs {:?}", ui.rect(rows[0]), first);
    assert!(ui.scroll_offset(list).y > 0.0);
}

#[test]
fn nothing_changed_draws_no_frame() {
    let mut ui = Ui::new();
    let p = page(&mut ui);
    let t = ui.text(p, "Hello");
    let f = ui.run(input(vec![]));
    assert!(!f.primitives.is_empty());
    let f = ui.run(input(vec![]));
    assert_eq!(f.repaint_after, None, "an idle interface waits for input");
    ui.set_text(t, "Hello");
    assert!(!ui.needs_repaint(), "the same text changes nothing");
    ui.set_text(t, "Bye");
    assert!(ui.needs_repaint());
}

#[test]
fn classes_cascade_and_the_nodes_own_layout_wins() {
    let mut ui = Ui::new();
    ui.add_rule("card", Rule::visual(Visual::new().background(Color32::RED).radius(10.0_f32)).with_layout(|s| s.padding = taffy::Rect::length(14.0_f32)));
    let p = page(&mut ui);
    let card = ui.column(p);
    ui.add_class(card, "card");
    ui.visual(card, Visual::new().background(Color32::BLUE));
    ui.style(card, |s| s.padding.left = taffy::LengthPercentage::length(40.0));
    let inner = ui.text(card, "x");
    ui.run(input(vec![]));
    let (c, i) = (ui.rect(card), ui.rect(inner));
    assert!((i.left() - c.left() - 40.0).abs() < 0.5, "own padding wins");
    assert!((i.top() - c.top() - 14.0).abs() < 0.5, "the class's padding where the node set none");
}

#[test]
fn a_finger_dragged_over_a_button_scrolls_and_a_tap_clicks() {
    let mut ui = Ui::new();
    let p = page(&mut ui);
    let list = ui.column(p);
    ui.set_scroll(list, ScrollAxes { x: false, y: true });
    ui.style(list, |s| s.size = taffy::Size { width: taffy::Dimension::length(200.0), height: taffy::Dimension::length(100.0) });
    let buttons: Vec<NodeId> = (0..20).map(|_| ui.button(list, "Play", Msg::Play)).collect();
    ui.run(input(vec![]));
    let at = ui.rect(buttons[1]).center();
    let touch = |pos: Pos2, phase: TouchPhase| InputEvent::Touch { pos, phase };
    // dragged up: the list moves with the finger, no click
    ui.run(input(vec![touch(at, TouchPhase::Start), touch(at - vec2(0.0, 20.0), TouchPhase::Move), touch(at - vec2(0.0, 40.0), TouchPhase::Move), touch(at - vec2(0.0, 40.0), TouchPhase::End)]));
    assert!((ui.scroll_offset(list).y - 40.0).abs() < 0.5, "{:?}", ui.scroll_offset(list));
    assert!(ui.drain::<Msg>().is_empty());
    // a tap (a finger that hardly moves) is a click
    ui.run(input(vec![]));
    let at = ui.rect(buttons[2]).center();
    ui.run(input(vec![touch(at, TouchPhase::Start), touch(at + vec2(2.0, 1.0), TouchPhase::Move), touch(at + vec2(2.0, 1.0), TouchPhase::End)]));
    assert_eq!(ui.drain::<Msg>(), vec![Msg::Play]);
}
