use crate::backend::render::{RenderBackend, ShapeHandle};
use crate::prelude::*;
use gc_arena::{Collect, Gc, MutationContext};

type Error = Box<dyn std::error::Error>;

#[derive(Debug, Clone, Collect, Copy)]
#[collect(no_drop)]
pub struct Font<'gc>(Gc<'gc, FontData>);

#[derive(Debug, Clone, Collect)]
#[collect(require_static)]
struct FontData {
    /// The list of glyphs defined in the font.
    /// Used directly by `DefineText` tags.
    glyphs: Vec<Glyph>,

    /// A map from a Unicode code point to glyph in the `glyphs` array.
    /// Used by `DefineEditText` tags.
    code_point_to_glyph: fnv::FnvHashMap<u16, usize>,

    /// The scaling applied to the font height to render at the proper size.
    /// This depends on the DefineFont tag version.
    scale: f32,

    /// Kerning infomration.
    /// Maps from a pair of unicode code points to horizontal offset value.
    kerning_pairs: fnv::FnvHashMap<(u16, u16), Twips>,
}

impl<'gc> Font<'gc> {
    pub fn from_swf_tag(
        gc_context: MutationContext<'gc, '_>,
        renderer: &mut dyn RenderBackend,
        tag: &swf::Font,
    ) -> Result<Font<'gc>, Error> {
        let mut glyphs = vec![];
        let mut code_point_to_glyph = fnv::FnvHashMap::default();
        for swf_glyph in &tag.glyphs {
            let glyph = Glyph {
                shape: renderer.register_glyph_shape(swf_glyph),
                advance: swf_glyph.advance.unwrap_or(0),
            };
            let index = glyphs.len();
            glyphs.push(glyph);
            code_point_to_glyph.insert(swf_glyph.code, index);
        }
        let kerning_pairs: fnv::FnvHashMap<(u16, u16), Twips> = if let Some(layout) = &tag.layout {
            layout
                .kerning
                .iter()
                .map(|kerning| ((kerning.left_code, kerning.right_code), kerning.adjustment))
                .collect()
        } else {
            fnv::FnvHashMap::default()
        };
        Ok(Font(Gc::allocate(
            gc_context,
            FontData {
                glyphs,
                code_point_to_glyph,

                /// DefineFont3 stores coordinates at 20x the scale of DefineFont1/2.
                /// (SWF19 p.164)
                scale: if tag.version >= 3 { 20480.0 } else { 1024.0 },
                kerning_pairs,
            },
        )))
    }

    /// Returns whether this font contains glyph shapes.
    /// If not, this font should be rendered as a device font.
    pub fn has_glyphs(self) -> bool {
        !self.0.glyphs.is_empty()
    }

    /// Returns a glyph entry by index.
    /// Used by `Text` display objects.
    pub fn get_glyph(self, i: usize) -> Option<Glyph> {
        self.0.glyphs.get(i).cloned()
    }

    /// Returns a glyph entry by character.
    /// Used by `EditText` display objects.
    pub fn get_glyph_for_char(self, c: char) -> Option<Glyph> {
        // TODO: Properly handle UTF-16/out-of-bounds code points.
        let code_point = c as u16;
        if let Some(index) = self.0.code_point_to_glyph.get(&code_point) {
            self.get_glyph(*index)
        } else {
            None
        }
    }

    /// Given a pair of characters, applies the offset that should be applied
    /// to the advance value between these two characters.
    /// Returns 0 twips if no kerning offset exists between these two characters.
    pub fn get_kerning_offset(self, left: char, right: char) -> Twips {
        // TODO: Properly handle UTF-16/out-of-bounds code points.
        let left_code_point = left as u16;
        let right_code_point = right as u16;
        self.0
            .kerning_pairs
            .get(&(left_code_point, right_code_point))
            .cloned()
            .unwrap_or_default()
    }

    /// Returns whether this font contains kerning information.
    pub fn has_kerning_info(self) -> bool {
        !self.0.kerning_pairs.is_empty()
    }

    pub fn scale(self) -> f32 {
        self.0.scale
    }
}

#[derive(Debug, Clone)]
pub struct Glyph {
    pub shape: ShapeHandle,
    pub advance: i16,
}
