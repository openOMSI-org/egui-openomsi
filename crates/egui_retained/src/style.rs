//! How nodes look: a [`Visual`] of optional properties (with variants for the hovered,
//! pressed, focused, selected and disabled states), merged from the element's own class, the
//! node's classes in the [`Theme`] and the node's inline visual - the way CSS cascades, but
//! with plain Rust values. Layout properties are taffy's [`taffy::Style`], patched the same way.

use ecolor::Color32;
use epaint::{FontFamily, Stroke};
use std::collections::HashMap;
use std::sync::Arc;

/// Visual properties, each optional: a node's look is the merge of every `Visual` that
/// applies to it, later ones winning.
#[derive(Clone, Default, Debug)]
pub struct Visual {
    pub background: Option<Color32>,
    pub color: Option<Color32>,
    pub border: Option<Stroke>,
    pub radius: Option<f32>,
    pub font_size: Option<f32>,
    pub font: Option<FontFamily>,
    /// Text weight as a family: a theme maps `strong` to a bold face where it has one.
    pub strong: Option<bool>,
    pub opacity: Option<f32>,
    /// What the pointer over the node shows.
    pub cursor: Option<Cursor>,
    /// Seconds a change of colour takes (hover, press): 0 is at once.
    pub transition: Option<f32>,
    pub hovered: Option<Box<Visual>>,
    pub pressed: Option<Box<Visual>>,
    pub focused: Option<Box<Visual>>,
    pub selected: Option<Box<Visual>>,
    pub disabled: Option<Box<Visual>>,
}

macro_rules! setters {
    ($($name:ident: $ty:ty),* $(,)?) => {
        $(
            #[inline]
            pub fn $name(mut self, v: impl Into<$ty>) -> Self {
                self.$name = Some(v.into());
                self
            }
        )*
    };
}

impl Visual {
    pub fn new() -> Self {
        Self::default()
    }

    setters!(background: Color32, color: Color32, border: Stroke, radius: f32, font_size: f32, font: FontFamily, strong: bool, opacity: f32, cursor: Cursor, transition: f32);

    pub fn on_hover(mut self, v: Visual) -> Self {
        self.hovered = Some(Box::new(v));
        self
    }
    pub fn on_press(mut self, v: Visual) -> Self {
        self.pressed = Some(Box::new(v));
        self
    }
    pub fn on_focus(mut self, v: Visual) -> Self {
        self.focused = Some(Box::new(v));
        self
    }
    pub fn on_select(mut self, v: Visual) -> Self {
        self.selected = Some(Box::new(v));
        self
    }
    pub fn on_disable(mut self, v: Visual) -> Self {
        self.disabled = Some(Box::new(v));
        self
    }

    /// `other`'s properties over these (its state variants merged into these' too).
    pub fn merge(&mut self, other: &Visual) {
        macro_rules! take {
            ($($f:ident),*) => { $( if other.$f.is_some() { self.$f = other.$f.clone(); } )* };
        }
        take!(background, color, border, radius, font_size, font, strong, opacity, cursor, transition);
        macro_rules! nested {
            ($($f:ident),*) => {
                $(
                    if let Some(o) = &other.$f {
                        match &mut self.$f {
                            Some(s) => s.merge(o),
                            None => self.$f = Some(o.clone()),
                        }
                    }
                )*
            };
        }
        nested!(hovered, pressed, focused, selected, disabled);
    }

    /// The look in `state`: the base, then each state's variant that applies (disabled last,
    /// so a disabled node does not light up).
    pub fn resolve(&self, state: State, defaults: &Look) -> Look {
        let mut v = self.clone();
        let variants = [
            (state.selected, &self.selected),
            (state.hovered, &self.hovered),
            (state.focused, &self.focused),
            (state.pressed, &self.pressed),
            (state.disabled, &self.disabled),
        ];
        for (on, variant) in variants {
            if on {
                if let Some(o) = variant {
                    v.merge(o);
                }
            }
        }
        Look {
            background: v.background.unwrap_or(Color32::TRANSPARENT),
            color: v.color.unwrap_or(defaults.color),
            border: v.border.unwrap_or(Stroke::NONE),
            radius: v.radius.unwrap_or(0.0),
            font_size: v.font_size.unwrap_or(defaults.font_size),
            font: v.font.unwrap_or_else(|| defaults.font.clone()),
            strong: v.strong.unwrap_or(defaults.strong),
            opacity: v.opacity.unwrap_or(1.0),
            cursor: v.cursor.unwrap_or(Cursor::Default),
            transition: v.transition.unwrap_or(0.0),
        }
    }
}

/// A node's resolved look this frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    pub background: Color32,
    pub color: Color32,
    pub border: Stroke,
    pub radius: f32,
    pub font_size: f32,
    pub font: FontFamily,
    pub strong: bool,
    pub opacity: f32,
    pub cursor: Cursor,
    pub transition: f32,
}

impl Look {
    /// The font of the look (the strong family where the theme has one).
    pub fn font_id(&self, theme: &Theme) -> epaint::FontId {
        let family = if self.strong { theme.strong_family.clone().unwrap_or_else(|| self.font.clone()) } else { self.font.clone() };
        epaint::FontId::new(self.font_size, family)
    }

    /// Colours blended `t` of the way to `other` (a transition).
    pub fn lerp_colors(&self, other: &Look, t: f32) -> Look {
        let mix = |a: Color32, b: Color32| a.lerp_to_gamma(b, t);
        Look {
            background: mix(self.background, other.background),
            color: mix(self.color, other.color),
            border: Stroke::new(self.border.width + (other.border.width - self.border.width) * t, mix(self.border.color, other.border.color)),
            ..other.clone()
        }
    }
}

/// The pointer shape over a node.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Cursor {
    #[default]
    Default,
    Pointer,
    Text,
    Grab,
    Grabbing,
    NotAllowed,
    ResizeHorizontal,
    ResizeVertical,
}

/// Which interaction states a node is in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
    pub selected: bool,
    pub disabled: bool,
}

/// A patch of taffy layout properties (a class's share of the layout).
pub type LayoutPatch = Arc<dyn Fn(&mut taffy::Style) + Send + Sync>;

/// A named style: visual properties and a layout patch.
#[derive(Clone, Default)]
pub struct Rule {
    pub visual: Visual,
    pub layout: Option<LayoutPatch>,
}

impl Rule {
    pub fn visual(v: Visual) -> Rule {
        Rule { visual: v, layout: None }
    }
    pub fn with_layout(mut self, f: impl Fn(&mut taffy::Style) + Send + Sync + 'static) -> Rule {
        self.layout = Some(Arc::new(f));
        self
    }
}

/// The style rules by class name, and the defaults text falls back to. Change it and the
/// next frame restyles every node (`Ui::set_theme`).
#[derive(Clone)]
pub struct Theme {
    pub rules: HashMap<String, Rule>,
    pub defaults: Look,
    /// The family `strong` text is set in (a bold face added to the fonts).
    pub strong_family: Option<FontFamily>,
    /// Colour of the text selection and the caret.
    pub selection: Color32,
    pub caret: Color32,
    /// Seconds between caret blinks (0: never blinks).
    pub caret_blink: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            rules: HashMap::new(),
            defaults: Look {
                background: Color32::TRANSPARENT,
                color: Color32::from_gray(220),
                border: Stroke::NONE,
                radius: 0.0,
                font_size: 14.0,
                font: FontFamily::Proportional,
                strong: false,
                opacity: 1.0,
                cursor: Cursor::Default,
                transition: 0.0,
            },
            strong_family: None,
            selection: Color32::from_rgba_unmultiplied(0x60, 0x90, 0xff, 90),
            caret: Color32::WHITE,
            caret_blink: 0.53,
        }
    }
}

impl Theme {
    pub fn rule(&mut self, class: &str, rule: Rule) -> &mut Self {
        self.rules.insert(class.to_owned(), rule);
        self
    }

    /// A class's visual added to (merged over) what the theme has for it already.
    pub fn extend(&mut self, class: &str, v: Visual) -> &mut Self {
        self.rules.entry(class.to_owned()).or_default().visual.merge(&v);
        self
    }
}
