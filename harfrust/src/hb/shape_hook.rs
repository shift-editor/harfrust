//! Callbacks at the points in shaping where HarfBuzz sends buffer messages.

use super::buffer::{GlyphInfo, GlyphPosition};

/// An OpenType layout table whose lookups shaping applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ShapeTable {
    /// The glyph substitution table.
    Gsub,
    /// The glyph positioning table.
    Gpos,
}

/// A point in shaping that a [`ShapeHook`] is told about.
///
/// These are the points where HarfBuzz sends buffer messages while it applies
/// a layout table (`start table`, `start lookup`, `end lookup`, `end table`):
/// each lookup the shaping plan enables is reported, in the order the plan
/// applies them, whether or not it applies to the buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ShapeEvent {
    /// Shaping is about to apply `table`'s lookups.
    StartTable(ShapeTable),
    /// Shaping is about to apply the lookup at `index` in `table`'s lookup
    /// list.
    StartLookup {
        /// The table the lookup belongs to.
        table: ShapeTable,
        /// The lookup's index in the table's lookup list.
        index: u16,
    },
    /// Shaping has applied the lookup at `index` in `table`'s lookup list.
    EndLookup {
        /// The table the lookup belongs to.
        table: ShapeTable,
        /// The lookup's index in the table's lookup list.
        index: u16,
    },
    /// Shaping has applied every lookup of `table`.
    EndTable(ShapeTable),
}

impl ShapeEvent {
    /// The table the event belongs to.
    pub fn table(&self) -> ShapeTable {
        match *self {
            Self::StartTable(table) | Self::EndTable(table) => table,
            Self::StartLookup { table, .. } | Self::EndLookup { table, .. } => table,
        }
    }
}

/// Observes shaping as it applies layout tables, and may adjust positions.
///
/// Set one with [`ShapeOptions::hook`](crate::ShapeOptions::hook). Without
/// one, shaping makes no calls and pays only a branch per lookup.
pub trait ShapeHook {
    /// Called at each [`ShapeEvent`] with the buffer's glyphs as they stand.
    ///
    /// The glyphs are in logical order with the buffer's clusters, whatever
    /// the text direction: shaping reverses a right-to-left buffer only after
    /// positioning. `positions` is the buffer's positions, which the hook may
    /// change, during [`ShapeTable::Gpos`]; it is empty during
    /// [`ShapeTable::Gsub`], when the buffer holds no positions. While
    /// positioning, offsets of attached glyphs (marks, cursive joins) are
    /// relative to the glyph they attach to until shaping finishes; advances
    /// are final.
    fn event(&mut self, event: ShapeEvent, infos: &[GlyphInfo], positions: &mut [GlyphPosition]);
}
