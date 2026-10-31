use std::fs;
use std::path::PathBuf;

use harfrust::{
    FontRef, GlyphBuffer, GlyphInfo, GlyphPosition, ShapeEvent, ShapeHook, ShapeOptions,
    ShapeTable, ShaperData, UnicodeBuffer,
};

/// Roboto: GSUB ligatures and GPOS kerning and mark attachment.
fn font_data() -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("benches")
        .join("fonts")
        .join("Roboto-Regular.ttf");
    fs::read(path).expect("failed to read test font")
}

fn shape(text: &str, hook: Option<&mut dyn ShapeHook>) -> GlyphBuffer {
    let data = font_data();
    let font = FontRef::new(&data).unwrap();
    let shaper_data = ShaperData::new(&font);
    let shaper = shaper_data.shaper(&font).build();
    let mut buffer = UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.guess_segment_properties();
    shaper.shape(buffer, ShapeOptions::new().hook(hook))
}

fn positions(buffer: &GlyphBuffer) -> Vec<(u32, i32, i32, i32)> {
    buffer
        .glyph_infos()
        .iter()
        .zip(buffer.glyph_positions())
        .map(|(info, pos)| (info.glyph_id, pos.x_advance, pos.x_offset, pos.y_offset))
        .collect()
}

/// Records every event, with each glyph's x advance at it.
#[derive(Default)]
struct Recorder {
    events: Vec<(ShapeEvent, usize, Vec<i32>)>,
}

impl ShapeHook for Recorder {
    fn event(&mut self, event: ShapeEvent, infos: &[GlyphInfo], positions: &mut [GlyphPosition]) {
        let advances = positions.iter().map(|pos| pos.x_advance).collect();
        self.events.push((event, infos.len(), advances));
    }
}

#[test]
fn a_hook_that_changes_nothing_leaves_the_output_unchanged() {
    let text = "AVAWaffle fi\u{0301}";
    let mut recorder = Recorder::default();

    let with_hook = shape(text, Some(&mut recorder));
    let without = shape(text, None);

    assert!(!recorder.events.is_empty());
    assert_eq!(positions(&with_hook), positions(&without));
}

#[test]
fn events_bracket_each_table_and_lookup_in_order() {
    let mut recorder = Recorder::default();
    shape("AVAWaffle", Some(&mut recorder));
    let events: Vec<ShapeEvent> = recorder.events.iter().map(|(event, ..)| *event).collect();

    assert_eq!(
        events.first(),
        Some(&ShapeEvent::StartTable(ShapeTable::Gsub))
    );
    assert_eq!(events.last(), Some(&ShapeEvent::EndTable(ShapeTable::Gpos)));
    let end_gsub = events
        .iter()
        .position(|event| *event == ShapeEvent::EndTable(ShapeTable::Gsub))
        .unwrap();
    assert_eq!(
        events[end_gsub + 1],
        ShapeEvent::StartTable(ShapeTable::Gpos)
    );

    let mut open = None;
    let mut gpos_lookups = 0;
    for event in &events {
        match *event {
            ShapeEvent::StartLookup { table, index } => {
                assert_eq!(open, None, "lookups do not nest");
                open = Some((table, index));
                if table == ShapeTable::Gpos {
                    gpos_lookups += 1;
                }
            }
            ShapeEvent::EndLookup { table, index } => {
                assert_eq!(open.take(), Some((table, index)));
            }
            ShapeEvent::StartTable(_) | ShapeEvent::EndTable(_) => assert_eq!(open, None),
            _ => {}
        }
    }
    assert!(gpos_lookups > 0);
}

#[test]
fn positions_are_given_only_while_positioning() {
    let mut recorder = Recorder::default();
    shape("AVA", Some(&mut recorder));

    for (event, glyphs, advances) in &recorder.events {
        match event.table() {
            ShapeTable::Gsub => assert!(advances.is_empty()),
            ShapeTable::Gpos => assert_eq!(advances.len(), *glyphs),
            _ => unreachable!(),
        }
    }
}

#[test]
fn a_lookup_changes_positions_between_its_start_and_end_only() {
    let mut recorder = Recorder::default();
    shape("AVAV", Some(&mut recorder));
    let gpos: Vec<&(ShapeEvent, usize, Vec<i32>)> = recorder
        .events
        .iter()
        .filter(|(event, ..)| event.table() == ShapeTable::Gpos)
        .collect();

    // Nothing moves between one lookup's end and the next one's start.
    for pair in gpos.windows(2) {
        if matches!(pair[0].0, ShapeEvent::EndLookup { .. })
            && matches!(pair[1].0, ShapeEvent::StartLookup { .. })
        {
            assert_eq!(pair[0].2, pair[1].2);
        }
    }
    // Kerning is a lookup that moves something.
    let moved = gpos.windows(2).any(|pair| {
        matches!(pair[0].0, ShapeEvent::StartLookup { .. })
            && matches!(pair[1].0, ShapeEvent::EndLookup { .. })
            && pair[0].2 != pair[1].2
    });
    assert!(moved);
}

/// Adds to the first glyph's advance once, at one event.
struct Nudge {
    at: ShapeEvent,
    by: i32,
}

impl ShapeHook for Nudge {
    fn event(&mut self, event: ShapeEvent, _: &[GlyphInfo], positions: &mut [GlyphPosition]) {
        if event == self.at {
            positions[0].x_advance += self.by;
        }
    }
}

#[test]
fn a_hook_can_position_glyphs_once_shaping_has_positioned_them() {
    let without = shape("AV", None);
    let mut nudge = Nudge {
        at: ShapeEvent::EndTable(ShapeTable::Gpos),
        by: 100,
    };

    let with_nudge = shape("AV", Some(&mut nudge));

    let advance = |buffer: &GlyphBuffer| buffer.glyph_positions()[0].x_advance;
    assert_eq!(advance(&with_nudge), advance(&without) + 100);
}
