impl PagesEditorFixture {
    #[allow(clippy::assign_op_pattern)]
    pub(crate) fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Load => {
                self.source = crate::fixture::large_source();
                Task::none()
            }
            Message::DocumentArrived(item) => {
                if (item.source != self.source.reference) {
                    return Task::none();
                }
                self.load_error = item.error.to_owned();
                if (!(item.error).is_empty()) {
                    return Task::none();
                }
                self.formatting_notice = item.notice.to_owned();
                {
                    let reset = self.document.reset_revision();
                    let next = crate::fixture_source::document_editor(
                        item.text.to_owned(),
                        item.cursor.clone(),
                    );
                    self.document.replace(next, reset);
                };
                self.installed_source = item.source.clone();
                self.menu = crate::editor_binding::initial_menu();
                Task::none()
            }
            Message::Committed(next) => {
                self.formatting_notice = next.notice.to_owned();
                self.history = next.history.clone();
                self.menu = next.menu.clone();
                Task::none()
            }
            Message::DocumentTransaction(transaction) => {
                let route = transaction.apply(&mut self.document);
                route.map_or_else(Task::none, Task::done)
            }
            Message::DocumentUpdated(document) => {
                document.apply(&mut self.document);
                Task::none()
            }
        }
    }
}
