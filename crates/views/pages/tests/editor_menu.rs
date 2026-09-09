use pages_view::editor::{self, Doc, EditorCursor, EditorDecision, EditorHistoryEffect, History};
use pages_view::editor_menu as menu;

fn doc(text: &str, line: usize, column: usize) -> Doc {
    Doc::new(text, EditorCursor::at(line, column))
}
fn apply(before: &Doc, decision: EditorDecision) -> Doc {
    let EditorDecision::Apply {
        patches,
        cursor,
        history,
    } = decision
    else {
        panic!("menu edit must propose an atomic Apply");
    };
    assert_eq!(history, EditorHistoryEffect::NewGroup);
    editor::apply(before, &patches, cursor)
}

#[test]
fn palette_rewrites_unicode_as_one_undo_step() {
    let before = doc("Title\n- 한글", 1, 8);
    let after = apply(&before, menu::turn(&before, 1, "code"));
    assert_eq!(after, doc("Title\n```\n한글\n```", 2, 6));
    let mut history = History::default();
    history.commit(&before, &after, EditorHistoryEffect::NewGroup, 1);
    let EditorDecision::Apply {
        patches, cursor, ..
    } = history.undo(&after).unwrap()
    else {
        panic!("undo")
    };
    assert_eq!(editor::apply(&after, &patches, cursor), before);
}

#[test]
fn todo_keeps_unicode_and_collapses_the_selection() {
    let mut before = doc("Title\n  - [ ] 👍🏽", 1, 16);
    before.cursor.selection = Some(editor::EditorPosition::new(1, 8));
    let after = apply(&before, menu::toggle_todo(&before, 1));
    assert_eq!(after, doc("Title\n  - [x] 👍🏽", 1, 6));
}

#[test]
fn plus_and_empty_palette_pick_leave_the_fence_intact() {
    let before = doc("Title\n```\ncode\n```", 2, 0);
    let after = apply(&before, menu::insert_below(&before, 2));
    assert_eq!(after, doc("Title\n```\ncode\n```\n", 4, 0));
    let picked = apply(&after, menu::turn_from_slash(&after, 4, 0, false, "h1"));
    assert_eq!(picked, doc("Title\n```\ncode\n```\n# ", 4, 2));
}

#[test]
fn block_operations_keep_fences_whole_and_title_immovable() {
    let before = doc("Title\n```\ncode\n```\npara", 2, 0);
    assert_eq!(
        apply(&before, menu::duplicate(&before, 2)),
        doc("Title\n```\ncode\n```\n```\ncode\n```\npara", 4, 0)
    );
    assert_eq!(
        apply(&before, menu::delete(&before, 2)),
        doc("Title\npara", 1, 0)
    );
    assert_eq!(
        apply(&before, menu::move_block(&before, 2, 1)),
        doc("Title\npara\n```\ncode\n```", 2, 0)
    );
    assert_eq!(menu::move_block(&before, 1, -1), EditorDecision::Noop);
    assert_eq!(menu::delete(&before, 0), EditorDecision::Noop);
}

#[test]
fn drops_use_declared_block_boundaries() {
    let before = doc("Title\n```\ncode\n```\npara", 2, 0);
    assert_eq!(menu::drop_boundaries(&before), vec![1, 4, 5]);
    assert_eq!(
        apply(&before, menu::drop_move(&before, 2, 5)),
        doc("Title\npara\n```\ncode\n```", 2, 0)
    );
    for invalid_or_own in [0, 1, 2, 4, 6] {
        assert_eq!(
            menu::drop_move(&before, 2, invalid_or_own),
            EditorDecision::Noop
        );
    }
}

#[test]
fn slash_filter_is_removed_at_utf8_byte_columns() {
    let before = doc("Title\n한글 /h", 1, 9);
    assert_eq!(
        apply(&before, menu::turn_from_slash(&before, 1, 7, true, "h2")),
        doc("Title\n## 한글 ", 1, 10)
    );
    assert_eq!(
        menu::turn_from_slash(&before, 1, 1, true, "h2"),
        EditorDecision::Noop
    );
}

#[test]
fn menu_lifecycle_filters_at_byte_columns_and_closes_on_caret_movement() {
    let slash = doc("Title\n한글 /", 1, 8);
    let mut state = menu::Menu::default();
    state.after_edit(&slash, true);
    assert_eq!(state.current(&slash).unwrap().items.len(), 12);
    let filter = doc("Title\n한글 /h", 1, 9);
    state.after_edit(&filter, false);
    let view = state.current(&filter).unwrap();
    assert_eq!(
        view.items.iter().map(|item| item.0).collect::<Vec<_>>(),
        vec!["h1", "h2", "h3"]
    );
    state.select(&filter, usize::MAX);
    assert_eq!(state.current(&filter).unwrap().selected, 2);
    state.close();
    assert!(state.current(&filter).is_none());
    state.after_edit(&slash, true);
    state.after_edit(&doc("Title\n한글 /hz", 1, 10), false);
    assert!(state.current(&filter).is_none());
}

#[test]
fn menu_lifecycle_plans_do_not_change_the_current_menu_before_commit() {
    let before = doc("Title\n```\ncode\n```", 2, 0);
    let state = menu::Menu::default();
    let (decision, opened) = state.plus(&before, 2);
    assert!(state.current(&before).is_none());
    let after = apply(&before, decision);
    assert_eq!(opened.current(&after).unwrap().items.len(), 12);
    let (decision, closed) = opened.pick(&after, "h1");
    assert!(
        opened.current(&after).is_some(),
        "a rejected pick must leave its old menu available"
    );
    let picked = apply(&after, decision);
    assert_eq!(picked.text, "Title\n```\ncode\n```\n# ");
    assert!(closed.current(&picked).is_none());
}

#[test]
fn menu_lifecycle_rejects_hidden_picks_and_turns_only_offered_blocks() {
    let before = doc("Title\n```\ncode\n```\npara", 4, 0);
    let mut state = menu::Menu::default();
    state.block(&before, 1);
    assert!(
        !state
            .current(&before)
            .unwrap()
            .items
            .iter()
            .any(|item| item.0 == "turn")
    );
    let (decision, unchanged) = state.pick(&before, "turn");
    assert_eq!(decision, EditorDecision::Noop);
    assert_eq!(unchanged, state);
    state.block(&before, 4);
    let (decision, turning) = state.pick(&before, "turn");
    assert_eq!(decision, EditorDecision::Noop);
    assert_eq!(turning.current(&before).unwrap().items.len(), 12);
    let (decision, closed) = turning.pick(&before, "h2");
    assert_eq!(
        apply(&before, decision).text,
        "Title\n```\ncode\n```\n## para"
    );
    assert!(closed.current(&before).is_none());
}
