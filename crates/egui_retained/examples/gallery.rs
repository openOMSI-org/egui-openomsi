//! Every widget in a Development Tools-like theme, drawn without a window into PNG files:
//! `cargo run -p egui_retained --example gallery --features wgpu -- out_dir`.

use egui_retained::input::Button as Mouse;
use egui_retained::widgets::{Button, Checkbox, Progress, Select, Slider, Spinner, TextInput};
use egui_retained::*;

const ACCENT: Color32 = Color32::from_rgb(0xF4, 0x7F, 0x30);

fn theme() -> Theme {
    let mut t = Theme::default();
    let text = Color32::from_gray(214);
    t.defaults.color = text;
    t.defaults.font_size = 14.0;
    t.selection = ACCENT.gamma_multiply(0.45);
    t.caret = ACCENT;
    let card = Color32::from_rgb(0x1B, 0x1E, 0x25);
    let control = Color32::from_rgb(0x2A, 0x2D, 0x35);
    let control_hover = Color32::from_rgb(0x34, 0x38, 0x41);
    t.rule("accent", Rule::visual(Visual::new().background(ACCENT)));
    t.rule("field", Rule::visual(Visual::new().background(Color32::from_rgb(0x0F, 0x11, 0x15))));
    t.rule("page", Rule::visual(Visual::new().background(Color32::from_rgb(0x13, 0x15, 0x19))));
    t.rule("sidebar", Rule::visual(Visual::new().background(Color32::from_rgb(0x17, 0x19, 0x1E))).with_layout(|s| {
        s.size.width = taffy::Dimension::length(170.0);
        s.padding = taffy::Rect::length(8.0_f32);
        s.gap = taffy::Size::length(2.0_f32);
    }));
    t.rule("card", Rule::visual(Visual::new().background(card).radius(10.0_f32)).with_layout(|s| {
        s.padding = taffy::Rect::length(14.0_f32);
        s.gap = taffy::Size::length(8.0_f32);
    }));
    t.rule("title", Rule::visual(Visual::new().font_size(22.0_f32).color(Color32::from_gray(240))));
    t.rule("strong", Rule::visual(Visual::new().color(Color32::from_gray(236))));
    t.rule("weak", Rule::visual(Visual::new().color(Color32::from_gray(120))));
    t.rule("button", Rule::visual(Visual::new().background(control).radius(6.0_f32).cursor(Cursor::Pointer).transition(0.12_f32).on_hover(Visual::new().background(control_hover)).on_press(Visual::new().background(Color32::from_rgb(0x3E, 0x42, 0x4C)))));
    t.rule("primary", Rule::visual(Visual::new().background(ACCENT).color(Color32::WHITE).radius(6.0_f32).cursor(Cursor::Pointer).transition(0.12_f32).on_hover(Visual::new().background(Color32::from_rgb(0xFF, 0x92, 0x48)))));
    t.rule("nav", Rule::visual(Visual::new().radius(6.0_f32).color(Color32::from_gray(190)).font_size(15.0_f32).cursor(Cursor::Pointer).transition(0.1_f32).on_hover(Visual::new().background(Color32::from_rgb(0x22, 0x25, 0x2C))).on_select(Visual::new().background(Color32::from_rgb(0x70, 0x3D, 0x21)).color(ACCENT))).with_layout(|s| {
        s.size.height = taffy::Dimension::length(32.0);
    }));
    t.rule("text-input", Rule::visual(Visual::new().background(Color32::from_rgb(0x0F, 0x11, 0x15)).radius(6.0_f32).cursor(Cursor::Text).border(epaint::Stroke::new(1.0, Color32::from_white_alpha(14))).on_focus(Visual::new().border(epaint::Stroke::new(1.0, ACCENT)))));
    t.rule("select", Rule::visual(Visual::new().background(control).radius(6.0_f32).cursor(Cursor::Pointer).on_hover(Visual::new().background(control_hover))));
    t.rule("menu", Rule::visual(Visual::new().background(Color32::from_rgb(0x20, 0x23, 0x2B)).radius(8.0_f32).border(epaint::Stroke::new(1.0, Color32::from_white_alpha(18)))));
    t.rule("menu-item", Rule::visual(Visual::new().radius(5.0_f32).cursor(Cursor::Pointer).on_hover(Visual::new().background(Color32::from_rgb(0x2C, 0x30, 0x39))).on_select(Visual::new().color(ACCENT))));
    t.rule("checkbox", Rule::visual(Visual::new().cursor(Cursor::Pointer)));
    t.rule("switch", Rule::visual(Visual::new().cursor(Cursor::Pointer)));
    t.rule("separator", Rule::visual(Visual::new().background(Color32::from_white_alpha(14))));
    t.rule("status", Rule::visual(Visual::new().background(Color32::from_rgb(0x13, 0x15, 0x19)).color(Color32::from_gray(150)).font_size(12.5_f32)));
    t.rule("tooltip", Rule::visual(Visual::new().background(Color32::from_rgb(0x20, 0x23, 0x2B)).radius(6.0_f32).border(epaint::Stroke::new(1.0, Color32::from_white_alpha(18)))));
    t
}

#[derive(Clone, Debug)]
enum Msg {
    #[allow(dead_code)]
    Nav(usize),
    Play,
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let mut ui = Ui::new();
    ui.set_theme(theme());
    let root = ui.root(Layer::Base);
    // header, body (sidebar + page), status bar
    let app = ui.column(root);
    ui.add_class(app, "page");
    ui.style(app, |s| s.flex_grow = 1.0);
    let header = ui.row(app);
    ui.style(header, |s| {
        s.padding = taffy::Rect::length(12.0_f32);
        s.gap = taffy::Size::length(8.0_f32);
    });
    let brand = ui.text(header, "openOMSI");
    ui.visual(brand, Visual::new().color(ACCENT).font_size(20.0_f32));
    let name = ui.text(header, "Launcher");
    ui.visual(name, Visual::new().font_size(20.0_f32).color(Color32::from_gray(240)));
    ui.spacer(header);
    ui.button(header, "Light theme", Msg::Play);
    ui.add(app, egui_retained::widgets::Separator);
    let body = ui.row(app);
    ui.style(body, |s| {
        s.flex_grow = 1.0;
        s.align_items = Some(taffy::AlignItems::Stretch);
    });
    let nav = ui.column(body);
    ui.add_class(nav, "sidebar");
    for (i, p) in ["Drive", "Multiplayer", "Profile", "Settings", "Controls", "Sessions", "Mods"].iter().enumerate() {
        let b = ui.add(nav, Button::new(*p).class("nav"));
        ui.on_click(b, Msg::Nav(i));
        if i == 0 {
            ui.set_selected(b, true);
        }
    }
    let page = ui.column(body);
    ui.set_scroll(page, ScrollAxes { x: false, y: true });
    ui.style(page, |s| {
        s.flex_grow = 1.0;
        s.padding = taffy::Rect::length(20.0_f32);
        s.gap = taffy::Size::length(12.0_f32);
    });
    let t = ui.text(page, "Drive");
    ui.add_class(t, "title");
    let c = ui.column(page);
    ui.add_class(c, "card");
    let h = ui.text(c, "Choose a bus");
    ui.add_class(h, "strong");
    let r = ui.row(c);
    ui.style(r, |s| s.gap = taffy::Size::length(8.0_f32));
    let search = ui.add(r, TextInput::new("").hint("Search buses…"));
    ui.style(search, |s| s.size.width = taffy::Dimension::length(280.0));
    let sel = ui.add(r, Select::new(vec!["MAN NL202".into(), "MAN SD202".into(), "MB O305".into()], Some(1)));
    ui.set_tooltip(sel, Some("The bus type".into()));
    ui.add(c, Checkbox::switch(true, "Favourites only"));
    ui.add(c, Checkbox::new(false, "Leave the AI buses at the depot"));
    let p = ui.paragraph(c, "Templates include an openomsi-plugin.toml manifest and a starter script. This sentence wraps at the card's width to show paragraph layout.");
    ui.add_class(p, "weak");
    let c2 = ui.column(page);
    ui.add_class(c2, "card");
    let h2 = ui.text(c2, "Day & weather");
    ui.add_class(h2, "strong");
    let row = ui.row(c2);
    ui.style(row, |s| s.gap = taffy::Size::length(12.0_f32));
    ui.text(row, "Traffic");
    let sl = ui.add(row, Slider::new(0.6, 0.0, 1.0));
    ui.style(sl, |s| s.max_size.width = taffy::Dimension::length(300.0));
    let pr = ui.row(c2);
    ui.style(pr, |s| s.gap = taffy::Size::length(12.0_f32));
    ui.add(pr, Progress { value: Some(0.42) });
    ui.add(pr, Spinner);
    let actions = ui.row(page);
    ui.style(actions, |s| s.gap = taffy::Size::length(8.0_f32));
    ui.spacer(actions);
    ui.add(actions, Button::new("Continue last game"));
    let play = ui.add(actions, Button::new("Next: Day & weather").class("primary"));
    ui.on_click(play, Msg::Play);
    let status = ui.row(app);
    ui.add_class(status, "status");
    ui.style(status, |s| s.padding = taffy::Rect::length(6.0_f32));
    let dot = ui.text(status, "●  ");
    ui.visual(dot, Visual::new().color(Color32::from_rgb(0x3A, 0xA6, 0x55)));
    ui.text(status, "Ready");

    // a GPU without a window
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).expect("adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("device");
    let mut r = egui_retained::render::Renderer::new(&device, wgpu::TextureFormat::Rgba8UnormSrgb);
    let ppp = 2.0;
    let screen = vec2(1100.0, 700.0);
    let size = [(screen.x * ppp) as u32, (screen.y * ppp) as u32];
    let input = |events: Vec<InputEvent>| Input { events, modifiers: Modifiers::default(), dt: 0.2, screen, pixels_per_point: ppp };
    let bg = wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    let shoot = |r: &mut egui_retained::render::Renderer, f: &Frame, name: &str| {
        let px = r.picture(&device, &queue, size, f, bg);
        let file = std::fs::File::create(format!("{out}/{name}")).unwrap();
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), size[0], size[1]);
        enc.set_color(png::ColorType::Rgba);
        enc.write_header().unwrap().write_image_data(&px).unwrap();
        println!("{out}/{name}");
    };
    let f = ui.run(input(vec![]));
    shoot(&mut r, &f, "gallery.png");
    // the pointer over the play button, the field typed into, the select open
    let at = ui.rect(search).center();
    let f = ui.run(input(vec![InputEvent::PointerMoved(at), InputEvent::PointerButton { pos: at, button: Mouse::Primary, pressed: true }, InputEvent::PointerButton { pos: at, button: Mouse::Primary, pressed: false }, InputEvent::Text("Solaris".into())]));
    r.draw_textures_only(&device, &queue, &f);
    let at = ui.rect(sel).center();
    let f = ui.run(input(vec![InputEvent::PointerMoved(at), InputEvent::PointerButton { pos: at, button: Mouse::Primary, pressed: true }, InputEvent::PointerButton { pos: at, button: Mouse::Primary, pressed: false }]));
    r.draw_textures_only(&device, &queue, &f);
    let f = ui.run(input(vec![]));
    r.draw_textures_only(&device, &queue, &f);
    let over = ui.rect(play).center();
    let f = ui.run(input(vec![InputEvent::PointerMoved(over)]));
    shoot(&mut r, &f, "gallery-open.png");
    let _ = ui.drain::<Msg>();
}
