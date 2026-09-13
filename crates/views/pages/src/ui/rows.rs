impl PagesView {
    fn page_button(&self, page: &crate::host::PageItem) -> Node {
        let selected = page.id == self.active_page;
        let title = if page.title.is_empty() {
            "Untitled"
        } else {
            &page.title
        };
        let label = if page.child_count > 0 {
            format!("{}{title} · {}", page.prefix, page.child_count)
        } else {
            format!("{}{title}", page.prefix)
        };
        let preset = if selected {
            ButtonPreset::Primary
        } else {
            ButtonPreset::Text
        };
        let mut node = action(
            format!("{PAGE_KEY}/page/{}", page.id),
            label,
            Message::ChoosePage(page.id.clone()),
            !self.unavailable(),
            preset,
        );
        if let Node::Button { checked, .. } = &mut node {
            *checked = Some(selected);
        }
        kit::sized(node, Some(Length::Fill), None)
    }

    fn search_result(&self, hit: &crate::host::PageSearchHit) -> Node {
        let key = format!("{PAGE_KEY}/search/{}/{}", hit.page_id, hit.block_id);
        let content = kit::column(
            format!("{key}/content"),
            [
                kit::text(format!("{key}/title"), &hit.page_title),
                kit::text(format!("{key}/kind"), &hit.kind),
                kit::text(format!("{key}/excerpt"), &hit.text),
            ],
        );
        kit::button_child(
            key,
            content,
            (!self.unavailable()).then(|| {
                slots::message(Message::OpenPageSearchHit(
                    hit.page_id.clone(),
                    hit.block_id.clone(),
                ))
            }),
            ButtonPreset::Text,
        )
    }

    fn comment_thread(&self, thread: &crate::host::PageCommentThread) -> Node {
        let key = format!("{PAGE_KEY}/thread/{}", thread.id);
        let expanded = crate::host::expanded(&self.expanded_threads, &thread.id);
        let replying = self.reply_thread == thread.id && !thread.resolved;
        let disabled = self.unavailable() || self.threads_loading;
        let mut rows = vec![
            kit::row(
                format!("{key}/header"),
                [
                    kit::text(format!("{key}/author"), &thread.author),
                    kit::text(format!("{key}/meta"), &thread.meta),
                    action(
                        format!("{key}/resolve"),
                        if thread.resolved {
                            "Reopen"
                        } else {
                            "Resolve thread"
                        },
                        Message::ResolveThreadSubmit(thread.id.clone(), !thread.resolved),
                        !disabled,
                        ButtonPreset::Text,
                    ),
                ],
            ),
            kit::text(format!("{key}/opener"), crate::host::opener_text(thread)),
        ];
        for reply in crate::host::thread_replies(thread, expanded) {
            rows.push(kit::column(
                format!("{key}/reply/{}", reply.id),
                [
                    kit::text(
                        format!("{key}/reply/{}/meta", reply.id),
                        format!("{} · {}", reply.author, reply.meta),
                    ),
                    kit::text(format!("{key}/reply/{}/body", reply.id), reply.text),
                ],
            ));
        }
        let toggle = crate::host::reply_toggle_label(thread, expanded);
        if !toggle.is_empty() {
            rows.push(named(
                action(
                    format!("{key}/replies"),
                    toggle,
                    Message::ToggleThreadReplies(thread.id.clone()),
                    !disabled,
                    ButtonPreset::Text,
                ),
                if expanded {
                    "Fewer replies"
                } else {
                    "Show every reply"
                },
            ));
        }
        if replying {
            rows.push(input(
                format!("{PAGE_KEY}/thread-reply({})", thread.id),
                "Reply…",
                &self.reply_draft,
                Message::ReplyDraftChanged,
                Some(Message::PostThreadReply(thread.id.clone())),
                disabled,
            ));
            rows.push(action(
                format!("{key}/submit"),
                "Post reply",
                Message::PostThreadReply(thread.id.clone()),
                !disabled && !self.reply_draft.trim().is_empty(),
                ButtonPreset::Primary,
            ));
        } else if !thread.resolved {
            rows.push(action(
                format!("{key}/reply"),
                "Reply to this thread",
                Message::SelectReplyThread(thread.id.clone()),
                !disabled,
                ButtonPreset::Text,
            ));
        }
        kit::padded(kit::column(key, rows), wire::Edges::all(8.))
    }
}
