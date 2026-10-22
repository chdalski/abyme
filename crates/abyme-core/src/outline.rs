//! A board as a compact text outline, the way the AI reads it.
//!
//! Titles, types, names, and summaries come from files that a clone or a
//! pull request can bring in, so the outline writes each of them on one
//! line and escapes every character that could break a line or hide or
//! reorder text.

use std::ops::RangeInclusive;

use abyme_model::{Board, BoardId, Element, ElementId};

use crate::apply::Snapshot;

/// The widest a column is padded to, in characters. A longer value runs past
/// its column, so one long name cannot pad every other line.
const MAX_COLUMN_WIDTH: usize = 40;

/// What the outline shows for a placement whose element file is missing.
const MISSING_MARKER: &str = "⚠ missing";

/// The characters of the Unicode general categories Cf (format), Zl (line
/// separator), and Zp (paragraph separator), as of Unicode 15.0. They are
/// invisible or move text around, so a reader cannot see them in a review.
const FORMAT_AND_SEPARATORS: [RangeInclusive<char>; 21] = [
    '\u{ad}'..='\u{ad}',
    '\u{600}'..='\u{605}',
    '\u{61c}'..='\u{61c}',
    '\u{6dd}'..='\u{6dd}',
    '\u{70f}'..='\u{70f}',
    '\u{890}'..='\u{891}',
    '\u{8e2}'..='\u{8e2}',
    '\u{180e}'..='\u{180e}',
    '\u{200b}'..='\u{200f}',
    '\u{2028}'..='\u{202e}',
    '\u{2060}'..='\u{2064}',
    '\u{2066}'..='\u{206f}',
    '\u{feff}'..='\u{feff}',
    '\u{fff9}'..='\u{fffb}',
    '\u{110bd}'..='\u{110bd}',
    '\u{110cd}'..='\u{110cd}',
    '\u{13430}'..='\u{1343f}',
    '\u{1bca0}'..='\u{1bca3}',
    '\u{1d173}'..='\u{1d17a}',
    '\u{e0001}'..='\u{e0001}',
    '\u{e0020}'..='\u{e007f}',
];

/// No board has the ID the outline was asked for.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("no board has the ID {0}")]
pub struct UnknownBoard(pub BoardId);

/// Renders `board` as an outline.
///
/// The outline has a header line with the board's ID and title, a blank
/// line, and one line per placed element, ordered by ID, with the element's
/// ID, type, name, and summary. A placement whose element is missing shows
/// its ID and `⚠ missing`.
///
/// # Errors
///
/// Returns [`UnknownBoard`] when the snapshot has no such board.
pub fn outline(snapshot: &Snapshot, board: &BoardId) -> Result<String, UnknownBoard> {
    let found = snapshot
        .boards
        .get(board)
        .ok_or_else(|| UnknownBoard(board.clone()))?;
    Ok(format!(
        "{}\n\n{}",
        header(found),
        element_list(snapshot, found.placements.keys())
    ))
}

/// Renders the elements with the given IDs as the outline's element lines.
///
/// The lines come in the given order, without a header: one line per ID,
/// each ending in a line feed, with the element's ID, type, name, and
/// summary. An ID whose element is missing shows `⚠ missing`. An ID given
/// twice is rendered twice; limiting and deduplicating is the caller's
/// business.
///
/// All element text is escaped onto its line already, so callers must not
/// escape it again.
pub fn element_list<'a>(
    snapshot: &Snapshot,
    ids: impl IntoIterator<Item = &'a ElementId>,
) -> String {
    let rows: Vec<Vec<String>> = ids
        .into_iter()
        .map(|id| row(id, snapshot.elements.get(id)))
        .collect();
    let widths = column_widths(&rows);
    let mut text = String::new();
    for row in &rows {
        text.push_str(&aligned(row, &widths));
        text.push('\n');
    }
    text
}

fn header(board: &Board) -> String {
    format!("Board: {} · {}", board.id, one_line(&board.title))
}

/// The columns of one placement's line; the last one is never padded.
fn row(id: &ElementId, element: Option<&Element>) -> Vec<String> {
    let Some(element) = element else {
        return vec![id.to_string(), MISSING_MARKER.to_owned()];
    };
    let mut columns = vec![
        id.to_string(),
        one_line(element.type_ref().as_str()),
        one_line(element.name()),
    ];
    columns.extend(element.summary().map(one_line));
    columns
}

/// The width of each column but the last of any row: its widest value, at
/// most [`MAX_COLUMN_WIDTH`].
fn column_widths(rows: &[Vec<String>]) -> Vec<usize> {
    let mut widths: Vec<usize> = Vec::new();
    for row in rows {
        let padded = row.len().saturating_sub(1);
        for (index, value) in row.iter().take(padded).enumerate() {
            let width = value.chars().count().min(MAX_COLUMN_WIDTH);
            match widths.get_mut(index) {
                Some(widest) => *widest = (*widest).max(width),
                None => widths.push(width),
            }
        }
    }
    widths
}

/// Joins a row's columns with two spaces, padding each column but the last
/// to its width.
fn aligned(row: &[String], widths: &[usize]) -> String {
    let last = row.len().saturating_sub(1);
    row.iter()
        .enumerate()
        .map(|(index, value)| match widths.get(index) {
            Some(&width) if index < last => format!("{value:<width$}"),
            _ => value.clone(),
        })
        .collect::<Vec<_>>()
        .join("  ")
}

/// Writes `text` on one line. A backslash becomes `\\`; line feed, carriage
/// return, and tab become `\n`, `\r`, and `\t`; every other control
/// character (Unicode Cc) and every character in
/// [`FORMAT_AND_SEPARATORS`] becomes `\u{…}`. Everything else stays as it
/// is.
fn one_line(text: &str) -> String {
    let mut line = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '\\' => line.push_str("\\\\"),
            '\n' => line.push_str("\\n"),
            '\r' => line.push_str("\\r"),
            '\t' => line.push_str("\\t"),
            _ if needs_escape(character) => line.extend(character.escape_unicode()),
            _ => line.push(character),
        }
    }
    line
}

/// Whether a character is a control, format, or separator character.
fn needs_escape(character: char) -> bool {
    character.is_control()
        || FORMAT_AND_SEPARATORS
            .iter()
            .any(|range| range.contains(&character))
}

#[cfg(test)]
mod tests;
