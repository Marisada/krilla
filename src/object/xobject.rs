use crate::serialize::{Object, RegisterableObject, SerializerContext};
use crate::stream::Stream;
use crate::util::RectExt;
use pdf_writer::{Chunk, Finish, Name, Ref};
use std::sync::Arc;
use tiny_skia_path::Rect;

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct XObject {
    stream: Stream,
    isolated: bool,
    transparency_group_color_space: bool,
    custom_bbox: Option<Rect>,
}

impl XObject {
    pub fn new(
        stream: Stream,
        isolated: bool,
        transparency_group_color_space: bool,
        custom_bbox: Option<Rect>,
    ) -> Self {
        XObject {
            stream,
            isolated,
            transparency_group_color_space,
            custom_bbox,
        }
    }

    pub fn bbox(&self) -> Rect {
        self.custom_bbox.unwrap_or(self.stream.bbox)
    }
}

impl Object for XObject {
    fn serialize_into(self, sc: &mut SerializerContext) -> (Ref, Chunk) {
        let srgb_ref = sc.srgb();

        let root_ref = sc.new_ref();
        let mut chunk = Chunk::new();

        let mut x_object = chunk.form_xobject(root_ref, &self.stream.content);
        self.stream
            .resource_dictionary
            .to_pdf_resources(sc, &mut x_object.resources());
        x_object.bbox(self.custom_bbox.unwrap_or(self.stream.bbox).to_pdf_rect());

        if self.isolated || self.transparency_group_color_space {
            let mut group = x_object.group();
            let transparency = group.transparency();

            if self.isolated {
                transparency.isolated(self.isolated);
            }

            if self.transparency_group_color_space {
                transparency.pair(Name(b"CS"), srgb_ref);
            }

            transparency.finish();
            group.finish();
        }

        x_object.finish();

        (root_ref, chunk)
    }
}

impl RegisterableObject for XObject {}
