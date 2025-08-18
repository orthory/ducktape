mod comment;
mod document;
mod task;
mod tree;

use serde::Serialize;
use std::sync::Arc;

pub use comment::*;
pub use document::*;
pub use task::*;

pub struct Service<Header: Serialize, Document: Serialize, Comment: Serialize, Task: Serialize> {
    document: Arc<dyn DocumentService<Header = Header, Document = Document>>,
    comment: Arc<dyn CommentService<Comment = Comment>>,
    task: Arc<dyn TaskService<Task = Task>>,
}

impl<Header: Serialize, Document: Serialize, Comment: Serialize, Task: Serialize>
    Service<Header, Document, Comment, Task>
{
    pub fn new(
        document_service: impl DocumentService<Header = Header, Document = Document> + 'static,
        comment_service: impl CommentService<Comment = Comment> + 'static,
        task_service: impl TaskService<Task = Task> + 'static,
    ) -> Self {
        Self {
            document: Arc::new(document_service),
            comment: Arc::new(comment_service),
            task: Arc::new(task_service),
        }
    }
}
