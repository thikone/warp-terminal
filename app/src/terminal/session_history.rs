//! Data behind the tool panel's "Session history" tab.
//!
//! The tab lists the commands run in the current session, so its source is the
//! live block list rather than the `commands` table in sqlite. That table has no
//! record of how much output a command produced, and output size is one of the
//! columns the tab shows.
//!
//! [`SessionHistoryEntry`] is a snapshot: the panel renders from a `Vec` of these
//! rather than holding the terminal model lock across a render pass.

use chrono::{DateTime, Local};

use crate::terminal::file_capture::CaptureRequest;
use crate::terminal::model::BlockId;
use crate::terminal::model::block::Block;
use crate::terminal::model::terminal_model::BlockIndex;

/// One row of the session history list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionHistoryEntry {
    /// Position in the block list. Used to address the block for scrolling and
    /// for the context menu; not stable across block-limit trimming, so it must
    /// never be used as a cache key.
    pub block_index: BlockIndex,
    /// Stable identity, safe to key caches by.
    pub block_id: BlockId,
    pub command: String,
    /// `~`-abbreviated where possible.
    pub pwd: Option<String>,
    pub started_at: Option<DateTime<Local>>,
    pub completed_at: Option<DateTime<Local>>,
    pub is_running: bool,
    pub has_failed: bool,
    /// Only meaningful when [`Self::has_failed`] is set, which is already
    /// "finished with a non-zero code".
    pub exit_code: i32,
}

/// Line and byte counts for a block's output.
///
/// Both are derived from the same materialized string, so they are computed and
/// cached together. See [`crate::terminal::view::TerminalView::block_output_size`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OutputSize {
    pub lines: usize,
    pub bytes: usize,
}

impl OutputSize {
    pub fn of(output: &str) -> Self {
        Self {
            // A trailing newline terminates the last line rather than starting a
            // new one, so an empty string is zero lines, not one.
            lines: output.lines().count(),
            bytes: output.len(),
        }
    }
}

/// The block commands the Session history panel can run by keyboard.
///
/// A closed set rather than general action dispatch: the panel is not a
/// descendant of the terminal view, so it forwards through one narrow entry
/// point. See [`crate::terminal::view::TerminalView::run_block_command_from_session_history`].
#[derive(Clone, Copy, Debug)]
pub enum SessionHistoryBlockCommand {
    Copy,
    CopyOutputs,
    Capture(CaptureRequest),
}

/// Id of the save-position element wrapping a session history row.
///
/// The row is rendered by the tool panel but the block's context menu is an
/// overlay in the terminal's element tree, so the terminal resolves this id to
/// anchor the menu beside the row. Save positions are window-scoped, which is
/// what makes the cross-view lookup work.
pub fn session_history_row_position_id(block_index: BlockIndex) -> String {
    format!("session_history_row_{block_index}")
}

/// Whether a block belongs in the session history list.
///
/// Excludes background and static/rich-content blocks, blocks whose shell
/// bootstrap has not finished (Warp's own startup block reports as unfinished),
/// and blocks with no command text.
pub fn is_session_history_command(block: &Block) -> bool {
    if block.is_background() || block.is_static() {
        return false;
    }
    if !(block.bootstrap_stage().is_done() || block.is_restored()) {
        return false;
    }
    !block.command_to_string().trim().is_empty()
}

/// Renders a duration the way a person reads a stopwatch: sub-second work in
/// milliseconds, ordinary work in seconds, long work in minutes and hours.
pub fn format_duration(seconds: f64) -> String {
    if seconds < 1.0 {
        return format!("{}ms", (seconds * 1000.0).round() as i64);
    }
    if seconds < 60.0 {
        return format!("{seconds:.1}s");
    }
    let total = seconds.round() as i64;
    let (hours, minutes, secs) = (total / 3600, (total % 3600) / 60, total % 60);
    if hours > 0 {
        format!("{hours}h {minutes:02}m")
    } else {
        format!("{minutes}m {secs:02}s")
    }
}

/// Shells report 0-255 and those read naturally as decimal. Anything larger is
/// a Windows status code, where hex is the form documentation and search results
/// use. Compared against the *unsigned* reinterpretation: see
/// [`format_exit_code_inner`].
const EXIT_CODE_HEX_THRESHOLD: u32 = 256;

/// `"exit 1"`, or `"exit -1073741819 (0xC0000005)"` on Windows.
pub fn format_exit_code(code: i32) -> String {
    format_exit_code_inner(code, cfg!(windows))
}

/// Split from [`format_exit_code`] so the formatting is testable on any host
/// rather than only where `cfg!(windows)` happens to hold.
///
/// The decimal is the signed value but the threshold and the hex both use the
/// unsigned reinterpretation: Windows status codes overflow `i32`, so
/// `0xC0000005` is `-1073741819`. Comparing the signed value would put every
/// crash code below the threshold and suppress exactly the hex worth showing.
fn format_exit_code_inner(code: i32, include_hex: bool) -> String {
    let unsigned = code as u32;
    if include_hex && unsigned >= EXIT_CODE_HEX_THRESHOLD {
        format!("exit {code} (0x{unsigned:X})")
    } else {
        format!("exit {code}")
    }
}

/// Human-readable byte count. Uses KB/MB/GB in the 1024 sense, matching what
/// file managers on every supported platform report.
pub fn format_bytes(bytes: usize) -> String {
    const UNITS: [&str; 3] = ["KB", "MB", "GB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1024.0;
    let mut unit = UNITS[0];
    for next in UNITS.into_iter().skip(1) {
        if value < 1024.0 {
            break;
        }
        value /= 1024.0;
        unit = next;
    }
    if value < 10.0 {
        format!("{value:.1} {unit}")
    } else {
        format!("{value:.0} {unit}")
    }
}

/// `"1 line"` / `"24 lines"`, `"1 match"` / `"7 matches"`.
///
/// Words ending in a sibilant take `-es`, which is why "match" is not rendered
/// as "matchs".
pub fn pluralize(count: usize, singular: &str) -> String {
    if count == 1 {
        return format!("{count} {singular}");
    }
    let needs_es = ["s", "x", "z", "ch", "sh"]
        .into_iter()
        .any(|ending| singular.ends_with(ending));
    let suffix = if needs_es { "es" } else { "s" };
    format!("{count} {singular}{suffix}")
}

#[cfg(test)]
#[path = "session_history_tests.rs"]
mod tests;
