use anyhow::Result;
use serde::Serialize;

pub trait CommentService
where
    Self: Send + Sync,
{
    type Comment: Serialize;

    fn create_comment(&mut self, parent_id: String) -> Result<Self::Comment>;
    fn update_comment(&mut self, comment_id: String) -> Result<Self::Comment>;

    fn get_comments(&self) -> Result<Vec<Self::Comment>>;
    fn get_comments_for_document(&self, document_id: String) -> Result<Vec<Self::Comment>>;
}
