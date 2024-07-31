use crate::object::mask::Mask;
use crate::serialize::SerializerContext;
use crate::stream::StreamBuilder;
use crate::svg::util::convert_mask_type;
use crate::svg::{group, FontContext};
use crate::util::RectExt;
use crate::FillRule;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

pub fn get_mask(
    mask: &usvg::Mask,
    serializer_context: Rc<RefCell<SerializerContext>>,
    font_context: &mut FontContext,
) -> Mask {
    // Dummy size. TODO: Improve?
    let mut stream_builder = StreamBuilder::new(serializer_context);

    if let Some(sub_usvg_mask) = mask.mask() {
        let sub_mask = get_mask(
            sub_usvg_mask,
            stream_builder.serializer_context(),
            font_context,
        );
        let mut sub_stream_builder = StreamBuilder::new(stream_builder.serializer_context());
        remaining(mask, &mut sub_stream_builder, font_context);
        let sub_stream = sub_stream_builder.finish();
        stream_builder.draw_masked(sub_mask, Arc::new(sub_stream));
    } else {
        remaining(mask, &mut stream_builder, font_context);
    };

    let stream = stream_builder.finish();

    Mask::new(Arc::new(stream), convert_mask_type(&mask.kind()))
}

fn remaining(
    mask: &usvg::Mask,
    stream_builder: &mut StreamBuilder,
    font_context: &mut FontContext,
) {
    let clip_path = mask.rect().to_rect().to_clip_path();
    stream_builder.push_clip_path(&clip_path, &FillRule::NonZero);
    group::render(mask.root(), stream_builder, font_context);
    stream_builder.pop_clip_path();
}
