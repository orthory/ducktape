use super::*;

/// THE ONE VIRTUAL LIST IN THIS APP THAT PREPENDS MUST BE KEYED.
///
/// `chat_scrolled` fires the older page automatically inside the last tenth of
/// the scrollback and `prepend_history` merges up to 256 rows AHEAD of the
/// timeline. An unkeyed virtual column diffs its children by index, so every
/// one of those rows hands its measured height to its neighbour: the rows below
/// the viewport are re-estimated at the 44px placeholder, the content height
/// moves, and an `anchor-y=end` offset — a fixed distance from the BOTTOM —
/// lands on entirely different messages. The reader gets thrown backwards
/// mid-sentence, once per page, for as long as she keeps reading upwards.
#[test]
fn the_message_timeline_virtualizes_under_an_end_anchored_scroll() {
    let chat = inlined(include_str!("../../../crates/views/chat/src/ui/chat.ice"));
    // Only the rows the viewport can see are laid out, which is what lets the
    // timeline hold a whole channel without paying a text layout per row — and
    // `by=message.view_key` is what makes per-row state and per-row MEASUREMENT
    // follow the message through prepends AND optimistic confirmation instead
    // of following the slot it happened to occupy.
    let timeline = chat
        .split_once("component MessageTimeline")
        .expect("the message timeline component")
        .1
        .split_once("keyed message in messages by=message.view_key w=fill gap=3.0 virtual-row=44.0")
        .expect("the message timeline is a KEYED virtual-row column");
    // That is only correct under an end-anchored scroll: measuring a row ABOVE
    // the viewport moves everything below it, and a bottom-anchored offset is
    // what carries the visible rows along with it. The two travel together —
    // the thread rail's own scroll sits further down the file, past the split.
    // `h=shrink` is the composer-anchored height: the virtual column reports a
    // whole-list estimate, so a long timeline still hits the box's cap.
    assert!(
        chat.contains("scroll #message-stream dir=vertical w=fill h=shrink anchor-y=end auto=!history_view")
    );
    // The page controls stay OUTSIDE the keyed column. A keyed column repeats
    // one template over one list; a button folded into that list is a row whose
    // arrival and departure shift every index below it — the same defect one
    // level up, and `has_older_history` flips on every page.
    assert!(chat.contains("col w=fill gap=3.0 pr=6.0"));
    assert!(chat.contains("button \"Load older messages\""));
    // THE COPY RANGE JOINS THE PER-ROW KEY. A quiet row's memo has to notice
    // the range's ends moving, or shift-clicking down a channel would tint the
    // rows the reader is dragging over and leave every cached row behind it
    // untinted. Everything else about the key is unchanged: the range is three
    // scalars, not a list, so a cached row still reads nothing expensive.
    assert!(
        timeline.1.contains(
            "lazy message, copy_anchor_seq, copy_head_seq, copy_surface as cached_message"
        )
    );
}

/// A MESSAGE LINE IS ONE PARAGRAPH, NOT A FLEX OF TOKENS (#1096).
///
/// ducktape-ui#639 lets a `for` expand spans inside `rich-text`, so the whole
/// span list — literal runs, mentions, links — lowers into ONE native
/// paragraph widget: real word wrapping, native selection across the line,
/// and `link=` on the widget's own route instead of a per-token button. The
/// template cannot branch, so the arm choice is DATA: exactly one `ChatSpan`
/// text field per run (the chat client's `span_arm`), and this sweep pins the
/// template that meets it.
#[test]
fn the_message_line_is_one_rich_text_paragraph() {
    let rich_body = inlined(include_str!("../ui/components/richbody.ice"));
    let rich_line = rich_body
        .split_once("component RichLine")
        .expect("the message line component")
        .1;
    let rich_line = rich_line
        .split_once("\ncomponent ")
        .map_or(rich_line, |(body, _)| body);
    // ONE paragraph, expanded by the widget's own `for` — no wrapping flex of
    // per-token `text` widgets, and no per-token link button.
    assert!(rich_line.contains(
        "rich-text w=fill size=size line-h=1.55 wrap=word-or-glyph color=accent_fg \
         -> emit(open_message_link, _)"
    ));
    assert!(rich_line.contains("for span in block.spans"));
    // The prose scale is the BODY's to set: a chat row reads at 13.5.
    assert!(rich_body.contains("RichBody blocks=message.blocks size=13.5"));
    assert!(
        !rich_line.contains("flex") && !rich_line.contains("button"),
        "a token widget beside the paragraph is the #1071 workaround back"
    );
    // The plate is the mention's token, and ONLY the mention's — a posted URL
    // is a destination, not a person.
    let plated: Vec<&str> = rich_line
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("span ") && line.contains("bg="))
        .collect();
    // The plate is ALSO a destination: `link=` carries the account the
    // mention names, so the widget's own link route opens it on a click and
    // shows the pointer on hover.
    assert_eq!(
        plated,
        [
            "span span.mention link=span.mention_link bg=brand_bg px=1.0 r=4.0 font=medium \
             color=brand"
        ],
        "the mention arm alone wears a plate that leaves prose whitespace visible"
    );
    let forge = inlined(include_str!("../../../crates/views/forge/src/ui/kit.ice"));
    assert!(
        forge.contains(plated[0]),
        "Forge uses the same mention spacing"
    );
    // And the underline is the link's rule alone — it marks a destination,
    // not an emphasis (ducktape-ui#604).
    let underlined: Vec<&str> = rich_line
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("span ") && line.contains(" underline"))
        .collect();
    assert_eq!(
        underlined,
        ["span span.link_text link=span.link underline font=medium color=brand"],
        "the link arm alone draws the rule"
    );
    // It hands off through the SAME external-URL route the page renderer's
    // link press takes — one mechanism for one act, not a second one here.
    let handlers = include_str!("../ui/handlers/chat.ice");
    assert!(handlers.contains("on open_message_link(url)"));
    assert!(handlers.contains(
        "run every open_external_url(url) -> external_url_opened _ | external_url_failed _"
    ));
}

#[test]
fn the_mention_plate_leaves_space_before_and_after_the_token() {
    use iced::advanced::graphics::text::Paragraph;
    use iced::advanced::text::{LineHeight, Paragraph as _, Shaping, Span, Text, Wrapping};

    let _renderer = crate::frame_probe::headless_renderer();
    let source = include_str!("../ui/components/richbody.ice");
    let padding: f32 = source
        .lines()
        .find(|line| line.trim_start().starts_with("span span.mention "))
        .and_then(|line| line.split_once("px="))
        .and_then(|(_, value)| value.split_whitespace().next())
        .expect("the mention plate declares its paint padding")
        .parse()
        .unwrap();
    let blocks = chat::client::paragraph_blocks("before<@3>after");
    let spans: Vec<Span<'_, ()>> = blocks[0]
        .spans
        .iter()
        .map(|run| {
            if run.mention.is_empty() {
                Span::new(run.plain.as_str())
            } else {
                Span::new(run.mention.as_str()).font(iced::Font {
                    weight: iced::font::Weight::Medium,
                    ..iced::Font::with_name("Geist")
                })
            }
        })
        .collect();
    let paragraph = Paragraph::with_spans(Text {
        content: spans.as_slice(),
        bounds: iced::Size::INFINITE,
        size: iced::Pixels(13.5),
        line_height: LineHeight::Relative(1.55),
        font: iced::Font::with_name("Geist"),
        align_x: iced::advanced::text::Alignment::Default,
        align_y: iced::alignment::Vertical::Top,
        shaping: Shaping::Advanced,
        wrapping: Wrapping::WordOrGlyph,
    });
    let before = paragraph.span_bounds(0)[0];
    let mention = paragraph.span_bounds(2)[0];
    let after = paragraph.span_bounds(4)[0];
    // Native rich text expands the painted plate without advancing glyphs.
    let leading_gap = mention.x - padding - (before.x + before.width);
    let trailing_gap = after.x - (mention.x + mention.width + padding);
    assert!(leading_gap >= 1.5, "leading gap: {leading_gap}px");
    assert!(trailing_gap >= 1.5, "trailing gap: {trailing_gap}px");
}

/// `· edited` ANNOTATES A MESSAGE, SO IT RIDES THE MESSAGE.
///
/// It lived inside the `show_author` run header, so in a run of five messages
/// only the first could ever say it had been edited — and runs are most of a
/// busy channel. A message's text changing under its readers with no mark
/// anywhere on the row silently spends the one integrity signal this product
/// has. The thread root drew a header and still never carried it at all.
#[test]
fn the_edited_marker_reaches_every_row_it_annotates() {
    let components = inlined(include_str!(
        "../../../crates/views/chat/src/ui/components.ice"
    ));
    let marker = "text \"· edited\" size=11.0 wrap=none font=code_medium @text-muted";
    assert_eq!(
        components.matches(marker).count(),
        3,
        "the run header, the continuation row, and the thread root each carry it"
    );
    assert!(
        components.contains(&format!(
            "if message.edited && !message.show_author\n          {marker}"
        )),
        "a continuation row trails its own marker under the body"
    );
    let parent = components
        .split_once("component ThreadParentBlock")
        .expect("the thread root block")
        .1;
    assert!(parent.contains(&format!("if message.edited\n            {marker}")));
}
