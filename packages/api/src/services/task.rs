use anyhow::Result;
use serde::Serialize;

pub trait TaskService
where
    Self: Send + Sync,
{
    type Task: Serialize;

    fn create_task(&mut self, document_id: String) -> Result<Self::Task>;
    fn update_task(&mut self, task_id: String) -> Result<Self::Task>;

    fn get_tasks(&self) -> Result<Vec<Self::Task>>;
    fn get_tasks_for_document(&self, document_id: String) -> Result<Vec<Self::Task>>;
}
