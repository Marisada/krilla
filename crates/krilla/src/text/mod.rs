//! Text and font support.
//!
//! krilla has extensive support for OpenType fonts. It supports CFF-based as well
//! as TTF-based OpenType fonts. In addition to that, krilla also supports
//! all major tables used in color fonts, including the `SVG`, `COLR`, `sbix` and
//! `CBDT`/`EBDT` (only PNG) tables, something that, to the best of my knowledge, no other
//! Rust crates provides.
//!
//! Even better is the fact that you do not need to take care of choosing the right
//! table for drawing glyphs: All you need to do is to provide the [`Font`] object with
//! an appropriate index.

use std::fmt::Debug;
use std::hash::Hash;

use crate::graphics::paint::{Fill, Stroke};
use crate::text::cid::CIDFont;
use crate::text::type3::{CoveredGlyph, Type3Font, Type3FontMapper, Type3ID};
pub(crate) mod cid;
pub(crate) mod font;
pub(crate) mod glyph;
pub(crate) mod group;
#[cfg(feature = "simple-text")]
pub(crate) mod shape;
pub(crate) mod type3;

pub use font::*;
pub use glyph::*;
#[cfg(feature = "simple-text")]
pub use shape::TextDirection;

pub(crate) const PDF_UNITS_PER_EM: f32 = 1000.0;

impl PaintMode<'_> {
    pub(crate) fn to_owned(self) -> OwnedPaintMode {
        match self {
            PaintMode::Fill(f) => OwnedPaintMode::Fill((*f).clone()),
            PaintMode::Stroke(s) => OwnedPaintMode::Stroke((*s).clone()),
        }
    }
}

/// A wrapper enum for fills/strokes. We use that to keep track whether a Type3 font contains
/// filled or stroked outlines of a glyph.
#[derive(Debug, Clone, Copy)]
pub(crate) enum PaintMode<'a> {
    Fill(&'a Fill),
    Stroke(&'a Stroke),
}

/// A unique CID identifier.
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub(crate) struct CIDIdentifier(pub Font);

/// A unique Type3 font identifier. Type3 fonts can only hold 256 glyphs, which
/// means that we might have to create more than one Type3 font. This is why we
/// additionally store an index that indicates which specific Type3Font we are
/// referring to.
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub(crate) struct Type3Identifier(pub Font, pub Type3ID);

/// A font identifier for a PDF font.
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub(crate) enum FontIdentifier {
    Cid(CIDIdentifier),
    Type3(Type3Identifier),
}

/// The owned version of `PaintMode`.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub(crate) enum OwnedPaintMode {
    Fill(Fill),
    Stroke(Stroke),
}

impl From<Fill> for OwnedPaintMode {
    fn from(value: Fill) -> Self {
        Self::Fill(value)
    }
}

impl From<Stroke> for OwnedPaintMode {
    fn from(value: Stroke) -> Self {
        Self::Stroke(value)
    }
}

impl OwnedPaintMode {
    pub(crate) fn as_ref(&self) -> PaintMode {
        match self {
            OwnedPaintMode::Fill(f) => PaintMode::Fill(f),
            OwnedPaintMode::Stroke(s) => PaintMode::Stroke(s),
        }
    }
}

/// A container that holds all PDF fonts belonging to an OTF font.
#[derive(Debug)]
pub(crate) enum FontContainer {
    Type3(Type3FontMapper),
    CIDFont(CIDFont),
}

impl FontContainer {
    #[inline]
    pub(crate) fn font_identifier(&self, glyph: CoveredGlyph) -> Option<FontIdentifier> {
        match self {
            FontContainer::Type3(t3) => t3.id_from_glyph(&glyph.to_owned()),
            FontContainer::CIDFont(cid) => cid.get_cid(glyph.glyph_id).map(|_| cid.identifier()),
        }
    }

    #[inline]
    pub(crate) fn get_from_identifier_mut(
        &mut self,
        font_identifier: FontIdentifier,
    ) -> Option<&mut dyn PdfFont> {
        match self {
            FontContainer::Type3(t3) => {
                if let Some(t3_font) = t3.font_mut_from_id(font_identifier) {
                    Some(t3_font)
                } else {
                    None
                }
            }
            FontContainer::CIDFont(cid) => {
                if cid.identifier() == font_identifier {
                    Some(cid)
                } else {
                    None
                }
            }
        }
    }

    #[inline]
    pub(crate) fn get_from_identifier(
        &self,
        font_identifier: FontIdentifier,
    ) -> Option<&dyn PdfFont> {
        match self {
            FontContainer::Type3(t3) => {
                if let Some(t3_font) = t3.font_from_id(font_identifier) {
                    Some(t3_font)
                } else {
                    None
                }
            }
            FontContainer::CIDFont(cid) => {
                if cid.identifier() == font_identifier {
                    Some(cid)
                } else {
                    None
                }
            }
        }
    }

    #[inline]
    pub(crate) fn add_glyph(&mut self, glyph: CoveredGlyph) -> (FontIdentifier, PDFGlyph) {
        match self {
            FontContainer::Type3(t3) => {
                let (identifier, gid) = t3.add_glyph(glyph.to_owned());
                (identifier, PDFGlyph::Type3(gid))
            }
            FontContainer::CIDFont(cid_font) => {
                let cid = cid_font.add_glyph(glyph.glyph_id);
                (cid_font.identifier(), PDFGlyph::Cid(cid))
            }
        }
    }
}

pub(crate) trait PdfFont {
    fn units_per_em(&self) -> f32;
    fn font(&self) -> Font;
    fn get_codepoints(&self, pdf_glyph: PDFGlyph) -> Option<&str>;
    fn set_codepoints(
        &mut self,
        pdf_glyph: PDFGlyph,
        text: String,
        location: Option<crate::surface::Location>,
    );
    fn get_gid(&self, glyph: CoveredGlyph) -> Option<PDFGlyph>;
    fn force_fill(&self) -> bool;
}

impl PdfFont for Type3Font {
    fn units_per_em(&self) -> f32 {
        self.unit_per_em()
    }

    fn font(&self) -> Font {
        Type3Font::font(self)
    }

    #[track_caller]
    fn get_codepoints(&self, pdf_glyph: PDFGlyph) -> Option<&str> {
        match pdf_glyph {
            PDFGlyph::Type3(t3) => self.get_codepoints(t3),
            PDFGlyph::Cid(_) => panic!("attempted to pass cid to type 3 font"),
        }
    }

    #[track_caller]
    fn set_codepoints(
        &mut self,
        pdf_glyph: PDFGlyph,
        text: String,
        location: Option<crate::surface::Location>,
    ) {
        match pdf_glyph {
            PDFGlyph::Type3(t3) => self.set_codepoints(t3, text, location),
            PDFGlyph::Cid(_) => panic!("attempted to pass cid to type 3 font"),
        }
    }

    fn get_gid(&self, glyph: CoveredGlyph) -> Option<PDFGlyph> {
        self.get_gid(&glyph.to_owned()).map(PDFGlyph::Type3)
    }

    fn force_fill(&self) -> bool {
        true
    }
}

impl PdfFont for CIDFont {
    fn units_per_em(&self) -> f32 {
        self.units_per_em()
    }

    fn font(&self) -> Font {
        CIDFont::font(self)
    }

    #[track_caller]
    fn get_codepoints(&self, pdf_glyph: PDFGlyph) -> Option<&str> {
        match pdf_glyph {
            PDFGlyph::Type3(_) => panic!("attempted to pass type 3 glyph to cid font"),
            PDFGlyph::Cid(cid) => self.get_codepoints(cid),
        }
    }

    #[track_caller]
    fn set_codepoints(
        &mut self,
        pdf_glyph: PDFGlyph,
        text: String,
        location: Option<crate::surface::Location>,
    ) {
        match pdf_glyph {
            PDFGlyph::Type3(_) => panic!("attempted to pass type 3 glyph to cid font"),
            PDFGlyph::Cid(cid) => self.set_codepoints(cid, text, location),
        }
    }

    fn get_gid(&self, glyph: CoveredGlyph) -> Option<PDFGlyph> {
        self.get_cid(glyph.glyph_id).map(PDFGlyph::Cid)
    }

    fn force_fill(&self) -> bool {
        false
    }
}
