//! FONT FALLBACK AT NON-REGULAR WEIGHTS.
//!
//! cosmic-text 0.15.0's fallback iterator only looked at faces whose weight
//! matched the request EXACTLY in its fast phases (the requested family, then
//! the platform's per-script and common lists). fontdb registers a variable
//! font once, at its OS/2 default — 400 for `Geist[wght]` — and every bundled
//! fallback face is a static 400, so a run asked for at 500/600/700 missed the
//! primary family AND every listed fallback and fell into the last phase: walk
//! every face in the database in weight-distance order, re-shaping the whole
//! run against each one, until something covered the glyph. The 2026-08-16
//! chat-lag spec measured one emoji paragraph at 26us regular vs 2,963us
//! semibold from exactly that walk; ducktape#1143 dodged it for five labels by
//! asking for regular, which user content cannot do.
//!
//! `patches/cosmic-text` matches a family at its closest weight when no exact
//! one exists (the variable face then gets its `wght` set by `get_font`). The
//! probes here pin that in allocations, not microseconds: every extra
//! candidate face is one more `shape_fallback` — a fresh glyph `Vec` and a
//! shaper buffer — so the count scales with faces tried and ignores box load.
//!
//! HOST FONTS: the counts depend on what is installed (the walk is over the
//! system database), so only RATIOS between weights are asserted — a host
//! with no CJK face walks for the Hangul run at EVERY weight (about 1,000
//! allocations on this box), and the ratio pins only that a weight adds
//! nothing to it. The face
//! pin on Latin text is deterministic after the patch on any host (Geist is
//! bundled); before it, it fails only where a common-list family ships a face
//! at exactly 700 — DejaVu Sans Bold on Debian/Ubuntu, Menlo on macOS — which
//! is the hijack it documents.

use iced::advanced::graphics::text::{Paragraph, cosmic_text, font_system};
use iced::advanced::text::{LineHeight, Paragraph as _, Shaping, Text, Wrapping};
use iced::font::Weight;
use iced::{Pixels, Size};

use crate::frame_probe::{FRAMES, Phase, headless_renderer};

/// A non-regular weight may cost at most this many times the regular shape.
/// The walk-every-face path is 30-100x on this box; the patched path is ~1x,
/// and the headroom only covers the `other` walk starting from a different
/// weight-distance group on hosts where nothing on the lists covers a run.
const WEIGHT_COST_RATIO_CEILING: u64 = 2;

/// Runs that miss the primary (Latin) face and need a fallback, plus one
/// that does not — the Latin word is the face-choice pin, not a cost pin.
const RUNS: [(&str, &str); 4] = [
    ("emoji", "🎉"),
    ("symbol", "♡"),
    ("hangul", "한글"),
    ("latin", "Channel"),
];

const WEIGHTS: [(&str, Weight); 3] = [
    ("regular", Weight::Normal),
    ("semibold", Weight::Semibold),
    ("bold", Weight::Bold),
];

fn geist(weight: Weight) -> iced::Font {
    iced::Font {
        weight,
        ..iced::Font::with_name("Geist")
    }
}

/// One fresh paragraph — a new cosmic-text `Buffer`, shaped from scratch —
/// exactly as a chat row's text widget is laid out on its first frame.
fn shape(content: &str, weight: Weight) -> Paragraph {
    Paragraph::with_text(Text {
        content,
        bounds: Size::INFINITE,
        size: Pixels(13.5),
        line_height: LineHeight::default(),
        font: geist(weight),
        align_x: iced::advanced::text::Alignment::Default,
        align_y: iced::alignment::Vertical::Top,
        shaping: Shaping::Advanced,
        wrapping: Wrapping::default(),
    })
}

fn shape_allocations(run: &str, weight_name: &str, content: &str, weight: Weight) -> u64 {
    let label: &'static str = Box::leak(format!("shape {run} @{weight_name}").into_boxed_str());
    let mut phase = Phase::new(label);
    for _ in 0..FRAMES {
        phase.sample(|| shape(content, weight));
    }
    phase.report();
    phase.median_allocations()
}

/// The family each glyph of the paragraph was finally shaped with.
fn glyph_families(paragraph: &Paragraph) -> Vec<String> {
    let mut fonts = font_system().write().expect("the shared font system lock");
    let db: &cosmic_text::fontdb::Database = fonts.raw().db();
    paragraph
        .buffer()
        .layout_runs()
        .flat_map(|run| run.glyphs.iter().map(|glyph| glyph.font_id))
        .map(|id| {
            db.face(id)
                .and_then(|face| face.families.first())
                .map(|(name, _)| name.clone())
                .unwrap_or_default()
        })
        .collect()
}

#[test]
fn non_regular_weights_shape_at_the_regular_fallback_cost() {
    let _renderer = headless_renderer();
    // Measure everything first so a failure still prints the whole table.
    let table: Vec<(&str, &str, &str, u64)> = RUNS
        .into_iter()
        .flat_map(|(run, content)| {
            WEIGHTS.into_iter().map(move |(weight_name, weight)| {
                (
                    run,
                    content,
                    weight_name,
                    shape_allocations(run, weight_name, content, weight),
                )
            })
        })
        .collect();
    for (run, content, weight_name, cost) in &table {
        let regular = table
            .iter()
            .find(|(other, _, name, _)| other == run && *name == "regular")
            .map(|(_, _, _, cost)| *cost)
            .expect("the regular sample of the same run");
        assert!(
            *cost <= WEIGHT_COST_RATIO_CEILING * regular,
            "shaping {content:?} at {weight_name} cost {cost} allocations against \
             {regular} at regular — over {WEIGHT_COST_RATIO_CEILING}x, the fallback \
             lookup is walking the whole font database again instead of matching \
             the listed families at their closest weight"
        );
    }
}

#[test]
fn latin_text_at_every_weight_is_shaped_with_geist() {
    let _renderer = headless_renderer();
    for (weight_name, weight) in WEIGHTS {
        let families = glyph_families(&shape("Channel", weight));
        assert!(
            !families.is_empty() && families.iter().all(|family| family == "Geist"),
            "Latin text at {weight_name} was shaped with {families:?}: the requested \
             family must win at every weight, not the first platform fallback that \
             happens to ship a face at exactly that weight"
        );
    }
}
