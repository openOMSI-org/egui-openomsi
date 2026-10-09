//! Icons by name, rasterised by the application (SVGs, an icon font) at the size they are
//! drawn and kept as textures, tinted with the node's colour.

use epaint::TextureId;
use std::collections::HashMap;

/// An icon's alpha mask: `size` x `size` bytes, 0 transparent.
pub type IconSource = Box<dyn Fn(&str, u32) -> Option<Vec<u8>>>;

/// The textures of the frame the [`crate::Ui`] manages: the fonts' atlas and the icons.
pub struct Textures {
    pub(crate) manager: epaint::textures::TextureManager,
    source: Option<IconSource>,
    icons: HashMap<(String, u32), Option<TextureId>>,
}

impl Textures {
    pub(crate) fn new(manager: epaint::textures::TextureManager) -> Textures {
        Textures { manager, source: None, icons: HashMap::new() }
    }

    pub(crate) fn set_source(&mut self, s: IconSource) {
        self.source = Some(s);
        for id in self.icons.drain().filter_map(|(_, t)| t) {
            self.manager.free(id);
        }
    }

    /// The texture of icon `name` at `px` pixels a side (made the first time).
    pub fn icon(&mut self, name: &str, px: u32) -> Option<TextureId> {
        let px = px.clamp(4, 512);
        if let Some(t) = self.icons.get(&(name.to_owned(), px)) {
            return *t;
        }
        let alpha = self.source.as_ref().and_then(|s| s(name, px)).filter(|a| a.len() == (px * px) as usize);
        let id = alpha.map(|a| {
            let pixels: Vec<ecolor::Color32> = a.iter().map(|&v| ecolor::Color32::from_white_alpha(v)).collect();
            let img = epaint::ColorImage::new([px as usize, px as usize], pixels);
            self.manager.alloc(format!("icon {name} {px}"), epaint::ImageData::Color(std::sync::Arc::new(img)), epaint::textures::TextureOptions::LINEAR)
        });
        self.icons.insert((name.to_owned(), px), id);
        id
    }
}
