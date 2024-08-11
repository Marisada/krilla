use crate::object::mask::Mask;
use crate::serialize::{Object, RegisterableObject, SerializerContext};
use pdf_writer::types::BlendMode;
use pdf_writer::{Chunk, Finish, Name, Ref};
use std::sync::Arc;
use tiny_skia_path::NormalizedF32;

#[derive(Debug, Hash, PartialEq, Eq, Default, Clone)]
struct Repr {
    non_stroking_alpha: Option<NormalizedF32>,
    stroking_alpha: Option<NormalizedF32>,
    blend_mode: Option<BlendMode>,
    mask: Option<Arc<Mask>>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Default)]
pub struct ExtGState(Arc<Repr>);

impl ExtGState {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn stroking_alpha(mut self, stroking_alpha: NormalizedF32) -> Self {
        Arc::make_mut(&mut self.0).stroking_alpha = Some(stroking_alpha);
        self
    }

    #[must_use]
    pub fn non_stroking_alpha(mut self, non_stroking_alpha: NormalizedF32) -> Self {
        Arc::make_mut(&mut self.0).non_stroking_alpha = Some(non_stroking_alpha);
        self
    }

    #[must_use]
    pub fn blend_mode(mut self, blend_mode: BlendMode) -> Self {
        Arc::make_mut(&mut self.0).blend_mode = Some(blend_mode);
        self
    }

    #[must_use]
    pub fn mask(mut self, mask: Mask) -> Self {
        Arc::make_mut(&mut self.0).mask = Some(Arc::new(mask));
        self
    }

    pub fn empty(&self) -> bool {
        self.0.mask.is_none()
            && self.0.stroking_alpha.is_none()
            && self.0.non_stroking_alpha.is_none()
            && self.0.blend_mode.is_none()
    }

    pub fn has_mask(&self) -> bool {
        self.0.mask.is_some()
    }

    pub fn combine(&mut self, other: &ExtGState) {
        if let Some(stroking_alpha) = other.0.stroking_alpha {
            Arc::make_mut(&mut self.0).stroking_alpha = Some(stroking_alpha);
        }

        if let Some(non_stroking_alpha) = other.0.non_stroking_alpha {
            Arc::make_mut(&mut self.0).non_stroking_alpha = Some(non_stroking_alpha);
        }

        if let Some(blend_mode) = other.0.blend_mode {
            Arc::make_mut(&mut self.0).blend_mode = Some(blend_mode);
        }

        if let Some(mask) = other.0.mask.clone() {
            Arc::make_mut(&mut self.0).mask = Some(mask);
        }
    }
}

impl Object for ExtGState {
    fn serialize_into(self, sc: &mut SerializerContext) -> (Ref, Chunk) {
        let root_ref = sc.new_ref();
        let mut chunk = Chunk::new();

        // TODO: Avoid mask being cloned here?
        let mask_ref = self
            .0
            .mask
            .clone()
            .map(|ma| sc.add(Arc::unwrap_or_clone(ma)));

        let mut ext_st = chunk.ext_graphics(root_ref);
        if let Some(nsa) = self.0.non_stroking_alpha {
            ext_st.non_stroking_alpha(nsa.get());
        }

        if let Some(sa) = self.0.stroking_alpha {
            ext_st.stroking_alpha(sa.get());
        }

        if let Some(bm) = self.0.blend_mode {
            ext_st.blend_mode(bm);
        }

        if let Some(mask_ref) = mask_ref {
            ext_st.pair(Name(b"SMask"), mask_ref);
        }

        ext_st.finish();

        (root_ref, chunk)
    }
}

impl RegisterableObject for ExtGState {}

#[cfg(test)]
mod tests {
    use crate::object::ext_g_state::ExtGState;
    use crate::object::mask::Mask;
    use crate::serialize::{Object, SerializeSettings, SerializerContext};
    use crate::stream::Stream;
    use crate::test_utils::check_snapshot;
    use crate::MaskType;
    use fontdb::Database;
    use pdf_writer::types::BlendMode;
    use usvg::NormalizedF32;

    #[test]
    pub fn empty() {
        let mut sc = SerializerContext::new_unit_test();
        let ext_state = ExtGState::new();
        sc.add(ext_state);
        check_snapshot("ext_g_state/empty", sc.finish(&Database::new()).as_bytes());
    }

    #[test]
    pub fn default_values() {
        let mut sc = SerializerContext::new_unit_test();
        let ext_state = ExtGState::new()
            .non_stroking_alpha(NormalizedF32::ONE)
            .stroking_alpha(NormalizedF32::ONE)
            .blend_mode(BlendMode::Normal);
        sc.add(ext_state);
        check_snapshot(
            "ext_g_state/default_values",
            sc.finish(&Database::new()).as_bytes(),
        );
    }

    #[test]
    pub fn all_set() {
        let mut sc = SerializerContext::new_unit_test();
        let mask = Mask::new(Stream::empty(), MaskType::Luminosity);
        let ext_state = ExtGState::new()
            .non_stroking_alpha(NormalizedF32::new(0.4).unwrap())
            .stroking_alpha(NormalizedF32::new(0.6).unwrap())
            .blend_mode(BlendMode::Difference)
            .mask(mask);
        sc.add(ext_state);
        check_snapshot(
            "ext_g_state/all_set",
            sc.finish(&Database::new()).as_bytes(),
        );
    }
}
