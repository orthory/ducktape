use std::io::Read;

/// Section is a trait that represents a section of a document.
///
/// All sections must implement the `Section` trait.
pub trait Section
where
    Self: Sized,
{
    fn try_match<R: Read>(
        document: &mut document::DocumentBuffer<R>,
    ) -> anyhow::Result<Option<Self>>;
}
