//! Creating new PDF documents.
//!
//! When using krilla, the starting point is always the creation of a [`Document`]. A document
//! represents _one_ PDF document, to which you can add pages or configure them in any
//! other way you want.
//!
//! Unfortunately, creating PDFs always requires some kind of global state to keep track
//! of different aspects in the creation process, meaning that it is not possible to
//! generate multiple pages at the same time. Instead, you need to add pages separately
//! by calling the [`Document::start_page`] method, which returns a new [`Page`] object that mutably
//! borrows the global state from the document. Once the page is dropped, the global
//! state is passed back to the original document, which you can then use to add even
//! more pages.
//!
//! [`Page`]: Page

use crate::error::KrillaResult;
use crate::interchange::embed::EmbeddedFile;
use crate::interchange::metadata::{Metadata, PdfSig};
use crate::interchange::outline::Outline;
use crate::interchange::tagging::TagTree;
use crate::page::{Page, PageSettings};
use crate::serialize::{SerializeContext, SerializeSettings};

/// A PDF document.
pub struct Document {
    pub(crate) serializer_context: SerializeContext,
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

impl Document {
    /// Create a new document with default serialize settings.
    pub fn new() -> Self {
        Self {
            serializer_context: SerializeContext::new(SerializeSettings::default()),
        }
    }

    /// Create a new document with custom serialize settings.
    pub fn new_with(serialize_settings: SerializeSettings) -> Self {
        Self {
            serializer_context: SerializeContext::new(serialize_settings),
        }
    }

    /// Start a new page with default settings.
    pub fn start_page(&mut self) -> Page {
        let page_index = self.serializer_context.page_infos().iter().len();
        Page::new(
            &mut self.serializer_context,
            page_index,
            PageSettings::default(),
        )
    }

    /// Start a new page with specific page settings.
    pub fn start_page_with(&mut self, page_settings: PageSettings) -> Page {
        let page_index = self.serializer_context.page_infos().iter().len();
        Page::new(&mut self.serializer_context, page_index, page_settings)
    }

    /// Set the outline of the document.
    pub fn set_outline(&mut self, outline: Outline) {
        self.serializer_context.set_outline(outline);
    }

    /// Set the metadata of the document.
    pub fn set_metadata(&mut self, metadata: Metadata) {
        self.serializer_context.set_metadata(metadata);
    }

    /// Set the tag tree of the document.
    pub fn set_tag_tree(&mut self, tag_tree: TagTree) {
        self.serializer_context.set_tag_tree(tag_tree);
    }

    /// Set the Signer of the document.
    pub fn set_signer(&mut self, sig: PdfSig) {
        self.serializer_context.set_signer(sig);
    }

    /// Embed a new file in the PDF document.
    ///
    /// Returns `None` if the file couldn't be embedded because a file
    /// with the same name has already been embedded.
    pub fn embed_file(&mut self, file: EmbeddedFile) -> Option<()> {
        self.serializer_context.embed_file(file)
    }

    /// Attempt to export the document to a PDF file.
    pub fn finish(mut self) -> KrillaResult<Vec<u8>> {
        // Write empty page if none has been created yet.
        if self.serializer_context.page_infos().is_empty() {
            self.start_page();
        }

        Ok(self.serializer_context.finish()?.finish())
    }
}
