use crate::color::{GREY_ICC_DEFLATED, SRGB_ICC_DEFLATED};
use crate::ext_g_state::ExtGState;
use crate::paint::{LinearGradient, RadialGradient};
use crate::serialize::{ObjectSerialize, PdfObject, RefAllocator, SerializeSettings};
use crate::util::NameExt;
use pdf_writer::{Chunk, Finish, Name, Ref};
use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;
use std::sync::Arc;

pub struct ResourceDictionary {
    pub color_spaces: CsResourceMapper,
    pub ext_g_state: ExtGStateResourceMapper,
}

impl ResourceDictionary {
    pub fn new() -> Self {
        Self {
            color_spaces: CsResourceMapper::new(),
            ext_g_state: ExtGStateResourceMapper::new(),
        }
    }

    pub fn register_color_space(&mut self, color_space: PdfColorSpace) -> String {
        self.color_spaces.remap_with_name(color_space)
    }

    pub fn register_ext_g_state(&mut self, ext_state: ExtGState) -> String {
        self.ext_g_state.remap_with_name(ext_state)
    }

    pub fn to_pdf_resources(
        &self,
        ref_allocator: &mut RefAllocator,
        resources: &mut pdf_writer::writers::Resources,
    ) {
        let mut color_spaces = resources.color_spaces();
        for (name, entry) in self.color_spaces.get_entries() {
            color_spaces.pair(
                name.to_pdf_name(),
                ref_allocator.cached_ref(PdfObject::PdfColorSpace(entry)),
            );
        }
        color_spaces.finish();

        let mut ext_g_states = resources.ext_g_states();

        for (name, entry) in self.ext_g_state.get_entries() {
            ext_g_states.pair(
                name.to_pdf_name(),
                ref_allocator.cached_ref(PdfObject::ExtGState(entry)),
            );
        }
        ext_g_states.finish();
    }
}

pub trait PDFResource {
    fn get_name() -> &'static str;
}

pub type CsResourceMapper = ResourceMapper<PdfColorSpace>;
pub type ExtGStateResourceMapper = ResourceMapper<ExtGState>;

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub enum PdfColorSpace {
    SRGB,
    D65Gray,
    LinearGradient(Arc<LinearGradient>),
    RadialGradient(Arc<RadialGradient>),
}

impl PDFResource for PdfColorSpace {
    fn get_name() -> &'static str {
        "cs"
    }
}

impl ObjectSerialize for PdfColorSpace {
    fn serialize_into(
        self,
        chunk: &mut Chunk,
        ref_allocator: &mut RefAllocator,
        _: &SerializeSettings,
    ) -> Ref {
        let root_ref = ref_allocator.cached_ref(PdfObject::PdfColorSpace(self.clone()));

        match self {
            PdfColorSpace::SRGB => {
                let icc_ref = ref_allocator.new_ref();
                let mut array = chunk.indirect(root_ref).array();
                array.item(Name(b"ICCBased"));
                array.item(icc_ref);
                array.finish();

                chunk
                    .icc_profile(icc_ref, &SRGB_ICC_DEFLATED)
                    .n(3)
                    .range([0.0, 1.0, 0.0, 1.0, 0.0, 1.0])
                    .filter(pdf_writer::Filter::FlateDecode);
            }
            PdfColorSpace::D65Gray => {
                chunk
                    .icc_profile(root_ref, &GREY_ICC_DEFLATED)
                    .n(1)
                    .range([0.0, 1.0])
                    .filter(pdf_writer::Filter::FlateDecode);
            }
            _ => unimplemented!(),
        }

        root_ref
    }
}

pub struct ResourceMapper<V>
where
    V: Hash + Eq,
{
    forward: BTreeMap<ResourceNumber, V>,
    backward: HashMap<V, ResourceNumber>,
    counter: ResourceNumber,
}

impl<V> ResourceMapper<V>
where
    V: Hash + Eq + Clone + PDFResource,
{
    pub fn new() -> Self {
        Self {
            forward: BTreeMap::new(),
            backward: HashMap::new(),
            counter: 1,
        }
    }

    pub fn get(&self, resource: V) -> Option<ResourceNumber> {
        self.backward.get(&resource).copied()
    }

    pub fn get_with_name(&self, resource: V) -> Option<String> {
        self.get(resource).map(|u| Self::name_from_number(u))
    }

    pub fn remap(&mut self, resource: V) -> ResourceNumber {
        let forward = &mut self.forward;
        let backward = &mut self.backward;
        let counter = &mut self.counter;

        *backward.entry(resource.clone()).or_insert_with(|| {
            let old = *counter;
            *counter += 1;
            forward.insert(old, resource.clone());
            old
        })
    }

    pub fn remap_with_name(&mut self, resource: V) -> String {
        Self::name_from_number(self.remap(resource))
    }

    fn name_from_number(num: ResourceNumber) -> String {
        format!("{}{}", V::get_name(), num)
    }

    pub fn get_entries(&self) -> impl Iterator<Item = (String, V)> + '_ {
        self.forward
            .iter()
            .map(|s| (Self::name_from_number(*s.0), s.1.clone()))
    }
}

pub type ResourceNumber = u32;

#[cfg(test)]
mod tests {
    use crate::resource::{CsResourceMapper, PdfColorSpace};
    use crate::serialize::ObjectSerialize;

    #[test]
    fn test_cs_resource_mapper() {
        let mut mapper = CsResourceMapper::new();
        assert_eq!(mapper.remap(PdfColorSpace::SRGB), 1);
        assert_eq!(mapper.remap(PdfColorSpace::D65Gray), 2);
        assert_eq!(mapper.remap(PdfColorSpace::SRGB), 1);
        assert_eq!(
            mapper.remap_with_name(PdfColorSpace::SRGB),
            String::from("cs1")
        );
        let items = mapper.get_entries().collect::<Vec<_>>();
        assert_eq!(items[0], (String::from("cs1"), PdfColorSpace::SRGB));
        assert_eq!(items[1], (String::from("cs2"), PdfColorSpace::D65Gray));
    }
}
