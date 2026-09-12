#[allow(warnings, clippy::all)]
mod __ice_group_app_update {
    use super::*;
    impl super::PagesEditorFixture {
        #[allow(clippy::assign_op_pattern)]
        pub(super) fn __update(
            &mut self,
            message: __PagesEditorFixtureMessage,
        ) -> ::ducktape_view_guest::Task<__PagesEditorFixtureMessage> {
            let __task = match message {
                __PagesEditorFixtureMessage::Load => (|| {
                    self.source = crate::fixture::large_source();
                    ::ducktape_view_guest::Task::none()
                })(),
                __PagesEditorFixtureMessage::DocumentArrived(item) => (|| {
                    let _ = &item;
                    if (item.source != self.source.reference) {
                        return ::ducktape_view_guest::Task::none();
                    }
                    self.load_error = item.error.to_owned();
                    if (!(item.error).is_empty()) {
                        return ::ducktape_view_guest::Task::none();
                    }
                    self.formatting_notice = item.notice.to_owned();
                    {
                        let __reset = self.document.reset_revision();
                        let __next = crate::fixture_source::document_editor(
                            item.text.to_owned(),
                            item.cursor.clone(),
                        );
                        self.document.replace(__next, __reset);
                    };
                    self.installed_source = item.source.clone();
                    self.menu = crate::editor_binding::initial_menu();
                    ::ducktape_view_guest::Task::none()
                })(),
                __PagesEditorFixtureMessage::Committed(next) => (|| {
                    let _ = &next;
                    self.formatting_notice = next.notice.to_owned();
                    self.history = next.history.clone();
                    self.menu = next.menu.clone();
                    ::ducktape_view_guest::Task::none()
                })(),
                __PagesEditorFixtureMessage::__0T646f63756d656e74(__transaction) => {
                    let __route = __transaction.apply(&mut self.document);
                    __route.map_or_else(
                        ::ducktape_view_guest::Task::none,
                        ::ducktape_view_guest::Task::done,
                    )
                }
                __PagesEditorFixtureMessage::__EditDocument(__document) => {
                    __document.apply(&mut self.document);
                    ::ducktape_view_guest::Task::none()
                }
            };
            __task
        }
    }
}
