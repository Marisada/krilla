//! Drawing to a non-page context.
//!
//! In 90% of the cases, you will not have to use streams. In most cases,
//! all you need to do when using this crate is to first construct a document,
//! and then add new pages to the document and use the [`Page::surface`] method to get
//! access to the drawing context. However, there are cases when you don't want to
//! draw on the main page surface, but instead you want to create a "sub-surface"
//! where you can draw independently of the main page contents. This is what streams
//! are there for. Currently, there are only two situations where you need to do that:
//!
//! - When using masks and defining the contents of the mask.
//! - When using a pattern fill or stroke and defining the contents of the pattern.
//!
//! If you want to do any of the two above, you need to call the [`Surface::stream_builder`] method
//! of the current surface. The stream builder represents a kind of sub-context that is
//! independent of the main surface you are working with. Once you have a stream builder, you
//! can once again invoke the [`StreamBuilder::surface`] method, and use this new surface to define the contents
//! of your mask/pattern. In the end, you can call [`StreamBuilder::finish`] which will return a [`Stream`] object.
//! This [`Stream`] object contains the encoded instructions of the mask/pattern, which you can
//! then use to create new [`Pattern`]/[`Mask`] objects.
//!
//! [`Page::surface`]: crate::page::Page::surface
//! [`Surface::stream_builder`]: crate::surface::Surface::stream_builder
//! [`Pattern`]: crate::paint::Pattern
//! [`Mask`]: crate::mask::Mask
use crate::content::ContentBuilder;
use crate::resource::{ResourceDictionary, ResourceDictionaryBuilder};
use crate::serialize::SerializerContext;
use crate::surface::Surface;
use crate::util::RectWrapper;
use crate::validation::ValidationError;
use crate::SerializeSettings;
use pdf_writer::{Array, Dict, Name};
use std::borrow::Cow;
use std::ops::DerefMut;
use tiny_skia_path::{Rect, Transform};

/// A stream.
///
/// See the module description for an explanation of its purpose.
// The only reason we implement clone for this type is that in some cases,
// we might need to clone a pattern (including its stream)
#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct Stream {
    pub(crate) content: Vec<u8>,
    pub(crate) bbox: RectWrapper,
    // Important: Each object that uses a stream must ensure to pass on the validation
    // errors to the `SerializerContext` at some point. Currently, only `Mask`,
    // `TilingPattern`, `InternalPage` and `XObject` require that.
    pub(crate) validation_errors: Vec<ValidationError>,
    pub(crate) resource_dictionary: ResourceDictionary,
}

impl Stream {
    pub(crate) fn new(
        content: Vec<u8>,
        bbox: Rect,
        validation_errors: Vec<ValidationError>,
        resource_dictionary: ResourceDictionary,
    ) -> Self {
        Self {
            content,
            bbox: RectWrapper(bbox),
            validation_errors,
            resource_dictionary,
        }
    }

    pub(crate) fn empty() -> Self {
        Self {
            content: vec![],
            bbox: RectWrapper(Rect::from_xywh(0.0, 0.0, 0.0, 0.0).unwrap()),
            validation_errors: vec![],
            resource_dictionary: ResourceDictionaryBuilder::new().finish(),
        }
    }
}

/// A builder to create streams.
pub struct StreamBuilder<'a> {
    sc: &'a mut SerializerContext,
    stream: Stream,
}

impl<'a> StreamBuilder<'a> {
    pub(crate) fn new(sc: &'a mut SerializerContext) -> Self {
        Self {
            sc,
            stream: Stream::empty(),
        }
    }

    /// Get the surface of the stream builder.
    pub fn surface(&mut self) -> Surface {
        // Stream builders cannot have any tags since we always pass a dummy
        // identifier. Only main page content streams can have one.
        let finish_fn = Box::new(|stream, _| {
            self.stream = stream;
        });

        Surface::new(
            self.sc,
            ContentBuilder::new(Transform::identity()),
            None,
            finish_fn,
        )
    }

    /// Turn the stream builder into a stream.
    pub fn finish(self) -> Stream {
        self.stream
    }
}

/// A PDF stream filter.
#[derive(Debug, Copy, Clone)]
pub(crate) enum StreamFilter {
    Flate,
    AsciiHex,
    Dct,
}

impl StreamFilter {
    pub(crate) fn to_name(self) -> Name<'static> {
        match self {
            Self::AsciiHex => Name(b"ASCIIHexDecode"),
            Self::Flate => Name(b"FlateDecode"),
            Self::Dct => Name(b"DCTDecode"),
        }
    }
}

impl StreamFilter {
    pub fn can_apply(&self) -> bool {
        match self {
            StreamFilter::Flate => true,
            StreamFilter::AsciiHex => true,
            StreamFilter::Dct => false,
        }
    }

    pub fn apply(&self, content: &[u8]) -> Vec<u8> {
        match self {
            StreamFilter::Flate => deflate_encode(content),
            StreamFilter::AsciiHex => hex_encode(content),
            // Note: We don't actually encode manually with DCT, because
            // this is only used for JPEG images which are already encoded,
            // so this shouldn't be called at all.
            StreamFilter::Dct => panic!("can't apply dct decode"),
        }
    }
}

/// Allows us to keep track of the filters that a stream has and
/// apply them in an orderly fashion.
#[derive(Debug, Clone)]
pub(crate) enum StreamFilters {
    None,
    Single(StreamFilter),
    Multiple(Vec<StreamFilter>),
}

impl StreamFilters {
    pub fn add(&mut self, stream_filter: StreamFilter) {
        match self {
            StreamFilters::None => *self = StreamFilters::Single(stream_filter),
            StreamFilters::Single(cur) => {
                *self = StreamFilters::Multiple(vec![*cur, stream_filter])
            }
            StreamFilters::Multiple(cur) => cur.push(stream_filter),
        }
    }
}

pub(crate) struct FilterStream<'a> {
    content: Cow<'a, [u8]>,
    filters: StreamFilters,
}

impl<'a> FilterStream<'a> {
    fn empty(content: &'a [u8]) -> Self {
        Self {
            content: Cow::Borrowed(content),
            filters: StreamFilters::None,
        }
    }

    pub fn new_from_content_stream(
        content: &'a [u8],
        serialize_settings: &SerializeSettings,
    ) -> Self {
        let mut filter_stream = Self::empty(content);

        if serialize_settings.compress_content_streams {
            filter_stream.add_filter(StreamFilter::Flate);

            if serialize_settings.ascii_compatible {
                filter_stream.add_filter(StreamFilter::AsciiHex);
            }
        }

        filter_stream
    }

    pub fn new_from_binary_data(content: &'a [u8], serialize_settings: &SerializeSettings) -> Self {
        let mut filter_stream = Self::empty(content);
        filter_stream.add_filter(StreamFilter::Flate);

        if serialize_settings.ascii_compatible {
            filter_stream.add_filter(StreamFilter::AsciiHex);
        }

        filter_stream
    }

    pub fn new_from_jpeg_data(content: &'a [u8], serialize_settings: &SerializeSettings) -> Self {
        let mut filter_stream = Self::empty(content);
        filter_stream.add_filter(StreamFilter::Dct);

        if serialize_settings.ascii_compatible {
            filter_stream.add_filter(StreamFilter::AsciiHex);
        }

        filter_stream
    }

    pub fn new_plain(content: &'a [u8], serialize_settings: &SerializeSettings) -> Self {
        let mut filter_stream = Self::empty(content);

        if serialize_settings.ascii_compatible {
            filter_stream.add_filter(StreamFilter::AsciiHex);
        }

        filter_stream
    }

    pub fn add_filter(&mut self, filter: StreamFilter) {
        if filter.can_apply() {
            self.content = Cow::Owned(filter.apply(&self.content));
        }

        self.filters.add(filter);
    }

    pub fn encoded_data(&self) -> &[u8] {
        &self.content
    }

    pub fn write_filters<'b, T>(&self, mut dict: T)
    where
        T: DerefMut<Target = Dict<'b>>,
    {
        match &self.filters {
            StreamFilters::None => {}
            StreamFilters::Single(filter) => {
                dict.deref_mut().pair(Name(b"Filter"), filter.to_name());
            }
            StreamFilters::Multiple(filters) => {
                dict.deref_mut()
                    .insert(Name(b"Filter"))
                    .start::<Array>()
                    .items(filters.iter().map(|f| f.to_name()).rev());
            }
        }
    }
}

fn deflate_encode(data: &[u8]) -> Vec<u8> {
    const COMPRESSION_LEVEL: u8 = 6;
    miniz_oxide::deflate::compress_to_vec_zlib(data, COMPRESSION_LEVEL)
}

fn hex_encode(data: &[u8]) -> Vec<u8> {
    data.iter()
        .enumerate()
        .map(|(index, byte)| {
            let mut formatted = format!("{:02X}", byte);
            if index % 35 == 34 {
                formatted.push('\n');
            }
            formatted
        })
        .collect::<String>()
        .into_bytes()
}
