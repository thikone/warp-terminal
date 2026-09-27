//! The tool panel's "Session history" tab.
//!
//! Lists the commands run in the active session with their working directory,
//! how long they took and how much output they produced. The source is the live
//! block list of the focused terminal, not the `commands` table in sqlite: that
//! table has no record of output size.
//!
//! Because the source is live, the list is per-session and per-tab, and blocks
//! trimmed by the block limit fall out of it.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

use chrono::Local;
use pathfinder_color::ColorU;
use pathfinder_geometry::vector::{Vector2F, vec2f};
use regex::{Regex, RegexBuilder};
use warpui::elements::{
    Border, ChildAnchor, ChildView, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox,
    Container, CornerRadius, CrossAxisAlignment, Element, Fill, Flex, Hoverable, MainAxisSize,
    MouseStateHandle, OffsetPositioning, ParentAnchor, ParentElement, ParentOffsetBounds, Radius,
    SavePosition, ScrollTarget, ScrollToPositionMode, ScrollbarWidth, Shrinkable, Stack, Text,
};
use warpui::event::ModifiersState;
use warpui::platform::Cursor;
use warpui::ui_components::components::{UiComponent, UiComponentStyles};
use warpui::{
    AppContext, Entity, EntityId, SingletonEntity, TypedActionView, View, ViewContext, ViewHandle,
    WeakViewHandle,
};

use crate::appearance::Appearance;
use crate::editor::{
    EditorView, PropagateAndNoOpNavigationKeys, SingleLineEditorOptions, TextOptions,
};
use crate::pane_group::PaneGroup;
use crate::search_bar::SearchBar;
// Distinct from `warpui::elements::Fill`, which the scrollable takes.
use crate::terminal::model::BlockId;
use crate::terminal::model::terminal_model::BlockIndex;
use crate::terminal::session_history::{
    OutputSize, SessionHistoryBlockCommand, SessionHistoryEntry, format_bytes, format_duration,
    format_exit_code, pluralize, session_history_row_position_id,
};
use crate::terminal::view::{Event as TerminalViewEvent, TerminalView};
use crate::themes::theme::Fill as ThemeFill;
use crate::ui_components::icons::Icon;
use crate::view_components::find::{CASE_SENSITIVE_TOOLTIP, REGEX_TOGGLE_TOOLTIP};

/// How often the list re-reads the block list while the tab is showing. Also
/// paces the running command's ticking duration and growing size.
const REFRESH_INTERVAL: Duration = Duration::from_millis(500);

const SEARCH_PLACEHOLDER: &str = "Filter commands";
const TOGGLE_ICON_SIZE: f32 = 14.;
const TOGGLE_ICON_PADDING: f32 = 4.;
const TOGGLE_ICON_SPACING: f32 = 4.;
const TOGGLE_CORNER_RADIUS: f32 = 4.;
/// Cached output sizes are pruned back to the current session past this many
/// entries. Well above any realistic block count for one session.
const SIZE_CACHE_LIMIT: usize = 20_000;
/// How many unmeasured blocks one refresh may measure. Bounds the cost of
/// opening the tab on a session with a long scrollback.
const MEASURE_BUDGET_PER_REFRESH: usize = 200;
const COMMAND_FONT_SIZE: f32 = 14.;
const DETAIL_FONT_SIZE: f32 = 12.;
const ROW_VERTICAL_PADDING: f32 = 5.;
const ROW_HORIZONTAL_PADDING: f32 = 6.;
const FAILED_STRIPE_WIDTH: f32 = 2.;
/// Matches the 10% wash the block list paints over a failed block.
const FAILED_BACKGROUND_OPACITY: u8 = 10;

/// Which parts of a row a filter query is matched against. Independent toggles;
/// at least one is always active.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterScope {
    Command,
    Directory,
    Output,
}

#[derive(Clone, Debug)]
pub enum SessionHistoryAction {
    RowClicked {
        block_id: BlockId,
        modifiers: ModifiersState,
    },
    RowDoubleClicked(BlockId),
    RowRightClicked {
        block_id: BlockId,
        position_in_row: Vector2F,
    },
    /// Forwarded from a keybinding while the panel is focused.
    BlockCommand(SessionHistoryBlockCommand),
    ToggleScope(FilterScope),
    ToggleRegex,
    ToggleCaseSensitive,
    ToggleFilterOnSelect,
    ToggleFindOnSelect,
}

/// A history entry with its output size resolved and, when the output scope is
/// active, the number of matches the current query has inside it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Row {
    entry: SessionHistoryEntry,
    /// `None` until the block's output has been measured; see the budget in
    /// [`SessionHistoryView::refresh`].
    size: Option<OutputSize>,
    output_matches: Option<usize>,
}

#[derive(Default)]
struct ToggleStates {
    command: MouseStateHandle,
    directory: MouseStateHandle,
    output: MouseStateHandle,
    regex: MouseStateHandle,
    case_sensitive: MouseStateHandle,
    filter_on_select: MouseStateHandle,
    find_on_select: MouseStateHandle,
}

pub struct SessionHistoryView {
    search_bar: ViewHandle<SearchBar>,
    search_editor: ViewHandle<EditorView>,
    scroll_state: ClippedScrollStateHandle,

    scope_command: bool,
    scope_directory: bool,
    scope_output: bool,
    regex_enabled: bool,
    case_sensitive: bool,
    filter_on_select: bool,
    find_on_select: bool,

    toggle_states: ToggleStates,
    row_states: RefCell<HashMap<BlockId, MouseStateHandle>>,

    rows: Vec<Row>,
    /// Keyed by [`BlockId`] rather than index: block indices shift when the
    /// block limit trims the front of the list.
    size_cache: HashMap<BlockId, OutputSize>,
    /// Output match counts for the current query, dropped whenever the query or
    /// its options change.
    output_match_cache: HashMap<BlockId, usize>,
    cached_query: Option<String>,
    /// Last row's block, so a newly started command can be scrolled into view
    /// without the list fighting the user's scrolling the rest of the time.
    last_row_id: Option<BlockId>,

    active_pane_group: Option<WeakViewHandle<PaneGroup>>,
    is_active: bool,

    /// Mirrors the terminal's block selection.
    selected: HashSet<BlockIndex>,
    /// Terminals already subscribed to. `subscribe_to_view` cannot be undone, so
    /// each terminal is subscribed at most once; the subscriptions die with the
    /// terminal rather than accumulating.
    subscribed_terminals: HashSet<EntityId>,
}

impl SessionHistoryView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let search_editor = {
            let appearance = Appearance::as_ref(ctx);
            let options = SingleLineEditorOptions {
                text: TextOptions {
                    font_size_override: Some(appearance.ui_font_size()),
                    ..Default::default()
                },
                propagate_and_no_op_vertical_navigation_keys:
                    PropagateAndNoOpNavigationKeys::Always,
                ..Default::default()
            };
            ctx.add_typed_action_view(|ctx| EditorView::single_line(options, ctx))
        };
        search_editor.update(ctx, |editor, ctx| {
            editor.set_placeholder_text(SEARCH_PLACEHOLDER, ctx);
        });
        ctx.subscribe_to_view(&search_editor, |me, _, _event, ctx| {
            // Any edit invalidates the output match counts, which are keyed to
            // the query that produced them.
            me.output_match_cache.clear();
            me.cached_query = None;
            me.refresh(ctx);
        });

        let search_bar = ctx.add_typed_action_view(|_| SearchBar::new(search_editor.clone()));

        Self::start_refresh_tick(ctx);

        Self {
            search_bar,
            search_editor,
            scroll_state: Default::default(),
            scope_command: true,
            scope_directory: true,
            // Output matching materializes every block's text, so it is opt-in.
            scope_output: false,
            regex_enabled: false,
            case_sensitive: false,
            filter_on_select: false,
            find_on_select: false,
            toggle_states: Default::default(),
            row_states: Default::default(),
            rows: Vec::new(),
            size_cache: HashMap::new(),
            output_match_cache: HashMap::new(),
            cached_query: None,
            last_row_id: None,
            active_pane_group: None,
            is_active: false,
            selected: HashSet::new(),
            subscribed_terminals: HashSet::new(),
        }
    }

    /// Subscribes to the active terminal's selection changes, once per terminal.
    ///
    /// The callback is guarded on the emitter being the active terminal, so a
    /// background pane's selection never drives the panel.
    fn subscribe_to_active_terminal(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(terminal_view) = self.terminal_view(ctx) else {
            return;
        };
        if !self.subscribed_terminals.insert(terminal_view.id()) {
            return;
        }
        ctx.subscribe_to_view(&terminal_view, |me, emitter, event, ctx| {
            if !matches!(event, TerminalViewEvent::SelectedBlocksChanged) {
                return;
            }
            if me.terminal_view(ctx).map(|active| active.id()) != Some(emitter.id()) {
                return;
            }
            me.sync_selection_from_terminal(ctx);
        });
    }

    /// Re-reads the terminal's selection and brings the newly selected row into
    /// view. Never applies the on-select filter/find toggles: those belong to an
    /// explicit click in the panel, so selecting a block in the terminal does not
    /// silently filter its own output.
    fn sync_selection_from_terminal(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(terminal_view) = self.terminal_view(ctx) else {
            return;
        };
        let (selected, tail) = {
            let terminal = terminal_view.as_ref(ctx);
            (
                terminal.selected_block_indices(),
                terminal.selected_blocks_tail(),
            )
        };
        let selected: HashSet<BlockIndex> = selected.into_iter().collect();
        if selected == self.selected {
            return;
        }
        self.selected = selected;

        // A no-op when the row is already visible, which is what keeps a
        // selection made on-screen from jumping the list.
        if let Some(tail) = tail {
            self.scroll_state.scroll_to_position(ScrollTarget {
                position_id: session_history_row_position_id(tail),
                mode: ScrollToPositionMode::FullyIntoView,
            });
        }
        ctx.notify();
    }

    /// The block list has no change notification reachable from out here, so the
    /// list polls instead. It only does work while the tab is the visible one.
    fn start_refresh_tick(ctx: &mut ViewContext<Self>) {
        ctx.spawn_stream_local(
            async_io::Timer::interval(REFRESH_INTERVAL),
            |view: &mut Self, _, ctx| {
                if view.is_active {
                    view.refresh(ctx);
                }
            },
            |_view, _ctx| {},
        );
    }

    pub fn set_active_pane_group(
        &mut self,
        pane_group: Option<WeakViewHandle<PaneGroup>>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.active_pane_group = pane_group;
        self.subscribe_to_active_terminal(ctx);
        self.sync_selection_from_terminal(ctx);
        // Sizes are keyed by block id, so they stay valid across a tab switch;
        // the rows themselves must be rebuilt from the new session.
        self.refresh(ctx);
    }

    pub fn set_is_active(&mut self, is_active: bool, ctx: &mut ViewContext<Self>) {
        if self.is_active == is_active {
            return;
        }
        self.is_active = is_active;
        if is_active {
            self.subscribe_to_active_terminal(ctx);
            self.sync_selection_from_terminal(ctx);
            self.refresh(ctx);
        }
    }

    fn terminal_view(&self, ctx: &AppContext) -> Option<ViewHandle<TerminalView>> {
        self.active_pane_group
            .as_ref()
            .and_then(|pane_group| pane_group.upgrade(ctx))
            .and_then(|pane_group| pane_group.as_ref(ctx).focused_session_view(ctx))
    }

    fn query(&self, ctx: &AppContext) -> String {
        self.search_editor.as_ref(ctx).buffer_text(ctx)
    }

    /// Builds a matcher for the current query and toggle state. `None` means the
    /// query is empty or is an invalid regex, in which case nothing is filtered
    /// out -- a half-typed pattern should not blank the list.
    fn matcher(&self, query: &str) -> Option<Matcher> {
        if query.is_empty() {
            return None;
        }
        if self.regex_enabled {
            RegexBuilder::new(query)
                .case_insensitive(!self.case_sensitive)
                .build()
                .ok()
                .map(Matcher::Regex)
        } else if self.case_sensitive {
            Some(Matcher::Literal(query.to_string()))
        } else {
            Some(Matcher::LiteralInsensitive(query.to_lowercase()))
        }
    }

    /// Re-reads the block list, resolves output sizes and applies the filter.
    fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(terminal_view) = self.terminal_view(ctx) else {
            if !self.rows.is_empty() {
                self.rows.clear();
                ctx.notify();
            }
            return;
        };

        let entries = terminal_view.as_ref(ctx).session_history_entries();
        let query = self.query(ctx);
        let matcher = self.matcher(&query);
        let want_output_matches = self.scope_output && matcher.is_some();

        if self.cached_query.as_deref() != Some(query.as_str()) {
            self.output_match_cache.clear();
            self.cached_query = Some(query.clone());
        }

        let mut rows = Vec::with_capacity(entries.len());
        let mut live_ids: HashSet<BlockId> = HashSet::with_capacity(entries.len());
        for entry in entries {
            live_ids.insert(entry.block_id.clone());
            // A finished block's output never changes, so its size is measured
            // once and cached. The running block is re-measured every tick.
            let size = if entry.is_running {
                terminal_view
                    .as_ref(ctx)
                    .block_output_size(entry.block_index)
            } else {
                self.size_cache.get(&entry.block_id).copied()
            };

            let output_matches = if want_output_matches {
                Some(self.output_match_count(&terminal_view, &entry, matcher.as_ref(), ctx))
            } else {
                None
            };

            if let Some(matcher) = matcher.as_ref()
                && !self.entry_matches(&entry, matcher, output_matches)
            {
                continue;
            }

            rows.push(Row {
                entry,
                size,
                output_matches,
            });
        }

        // Measuring a block means materializing its text, so opening the tab on
        // a long session must not measure everything at once. Fill in a bounded
        // number of unmeasured rows per tick, newest first: those are the ones
        // on screen, since the list is scrolled to the bottom.
        let mut budget = MEASURE_BUDGET_PER_REFRESH;
        for row in rows.iter_mut().rev() {
            if budget == 0 {
                break;
            }
            if row.size.is_some() {
                continue;
            }
            budget -= 1;
            let measured = terminal_view
                .as_ref(ctx)
                .block_output_size(row.entry.block_index)
                .unwrap_or_default();
            self.size_cache.insert(row.entry.block_id.clone(), measured);
            row.size = Some(measured);
        }

        // Sizes are kept across tab switches so returning to a tab is instant,
        // which means the cache accumulates blocks from sessions that are long
        // gone. Prune back to the current session once it grows unreasonable.
        if self.size_cache.len() > SIZE_CACHE_LIMIT {
            self.size_cache
                .retain(|block_id, _| live_ids.contains(block_id));
        }

        // Follow the newest command as it starts. `scroll_to_position` is a
        // no-op when the row is already visible, so this only moves the list
        // when a new command has actually pushed past the bottom.
        let newest_id = rows.last().map(|row| row.entry.block_id.clone());
        if newest_id != self.last_row_id {
            if let Some(row) = rows.last() {
                self.scroll_state.scroll_to_position(ScrollTarget {
                    position_id: session_history_row_position_id(row.entry.block_index),
                    mode: ScrollToPositionMode::FullyIntoView,
                });
            }
            self.last_row_id = newest_id;
        }

        // The refresh tick runs while the tab is open whether or not anything
        // changed, so only repaint when the rendered data actually differs.
        if self.rows != rows {
            self.rows = rows;
            ctx.notify();
        }
    }

    fn output_match_count(
        &mut self,
        terminal_view: &ViewHandle<TerminalView>,
        entry: &SessionHistoryEntry,
        matcher: Option<&Matcher>,
        ctx: &AppContext,
    ) -> usize {
        // A running block's output keeps growing, so its count cannot be cached.
        if !entry.is_running
            && let Some(cached) = self.output_match_cache.get(&entry.block_id)
        {
            return *cached;
        }
        let Some(matcher) = matcher else {
            return 0;
        };
        let count = terminal_view
            .as_ref(ctx)
            .block_output_text(entry.block_index)
            .map(|text| matcher.count(&text))
            .unwrap_or(0);
        if !entry.is_running {
            self.output_match_cache
                .insert(entry.block_id.clone(), count);
        }
        count
    }

    fn entry_matches(
        &self,
        entry: &SessionHistoryEntry,
        matcher: &Matcher,
        output_matches: Option<usize>,
    ) -> bool {
        if self.scope_command && matcher.is_match(&entry.command) {
            return true;
        }
        if self.scope_directory
            && let Some(pwd) = entry.pwd.as_deref()
            && matcher.is_match(pwd)
        {
            return true;
        }
        self.scope_output && output_matches.is_some_and(|count| count > 0)
    }

    fn scope_enabled(&self, scope: FilterScope) -> bool {
        match scope {
            FilterScope::Command => self.scope_command,
            FilterScope::Directory => self.scope_directory,
            FilterScope::Output => self.scope_output,
        }
    }

    /// Toggles a scope, refusing to turn off the last active one so a query can
    /// always match something.
    fn toggle_scope(&mut self, scope: FilterScope, ctx: &mut ViewContext<Self>) {
        let active = [self.scope_command, self.scope_directory, self.scope_output]
            .into_iter()
            .filter(|on| *on)
            .count();
        if self.scope_enabled(scope) && active <= 1 {
            return;
        }
        match scope {
            FilterScope::Command => self.scope_command = !self.scope_command,
            FilterScope::Directory => self.scope_directory = !self.scope_directory,
            FilterScope::Output => self.scope_output = !self.scope_output,
        }
        self.output_match_cache.clear();
        self.refresh(ctx);
    }

    fn block_index_for(&self, block_id: &BlockId) -> Option<BlockIndex> {
        self.rows
            .iter()
            .find(|row| &row.entry.block_id == block_id)
            .map(|row| row.entry.block_index)
    }

    /// Left-click: reveal the block, then apply whichever on-select toggles are
    /// enabled.
    ///
    /// With both toggles on the filter is applied without its toolbar, so the
    /// find bar is the only thing that opens.
    fn activate_row(
        &mut self,
        block_id: &BlockId,
        modifiers: ModifiersState,
        ctx: &mut ViewContext<Self>,
    ) {
        let (Some(block_index), Some(terminal_view)) =
            (self.block_index_for(block_id), self.terminal_view(ctx))
        else {
            return;
        };
        let query = self.query(ctx);
        let (regex, case_sensitive) = (self.regex_enabled, self.case_sensitive);
        let (filter_on_select, find_on_select) = (self.filter_on_select, self.find_on_select);
        // A modified click is building a selection, not asking to inspect one
        // block, so the on-select toggles stay out of it.
        let is_plain_click = !(modifiers.shift || modifiers.ctrl || modifiers.cmd);

        terminal_view.update(ctx, |terminal, ctx| {
            terminal.select_block_from_session_history(
                block_index,
                modifiers.ctrl,
                modifiers.cmd,
                modifiers.shift,
                ctx,
            );
            if query.is_empty() || !is_plain_click {
                return;
            }
            if filter_on_select {
                terminal.filter_block_from_session_history(
                    block_index,
                    &query,
                    regex,
                    case_sensitive,
                    // Both toggles on: filter silently and let the find bar be
                    // the only surface that appears.
                    !find_on_select,
                    ctx,
                );
            }
            if find_on_select {
                terminal.find_in_block_from_session_history(
                    block_index,
                    &query,
                    regex,
                    case_sensitive,
                    ctx,
                );
            }
        });
    }

    /// Double-click hands the keyboard to the terminal, for carrying on in the
    /// block list. A single click deliberately leaves focus in the panel.
    fn focus_terminal_from_row(&mut self, block_id: &BlockId, ctx: &mut ViewContext<Self>) {
        let (Some(block_index), Some(pane_group), Some(terminal_view)) = (
            self.block_index_for(block_id),
            self.active_pane_group
                .as_ref()
                .and_then(|group| group.upgrade(ctx)),
            self.terminal_view(ctx),
        ) else {
            return;
        };
        let terminal_id = terminal_view.id();
        terminal_view.update(ctx, |terminal, ctx| {
            terminal.reveal_block_from_session_history(block_index, ctx);
        });
        pane_group.update(ctx, |group, ctx| {
            group.focus_terminal_view(terminal_id, ctx);
        });
    }

    /// Runs a block command forwarded from a keybinding while the panel is
    /// focused. Acts on the shared selection, so it matches what the rows show.
    fn run_block_command(
        &mut self,
        command: SessionHistoryBlockCommand,
        ctx: &mut ViewContext<Self>,
    ) {
        let Some(terminal_view) = self.terminal_view(ctx) else {
            return;
        };
        terminal_view.update(ctx, |terminal, ctx| {
            terminal.run_block_command_from_session_history(command, ctx);
        });
    }

    /// Right-click: same reveal, then the block's own menu beside the row.
    fn open_row_context_menu(
        &mut self,
        block_id: &BlockId,
        position_in_row: Vector2F,
        ctx: &mut ViewContext<Self>,
    ) {
        let (Some(block_index), Some(terminal_view)) =
            (self.block_index_for(block_id), self.terminal_view(ctx))
        else {
            return;
        };
        terminal_view.update(ctx, |terminal, ctx| {
            terminal.open_session_history_context_menu(block_index, position_in_row, ctx);
        });
    }

    fn row_mouse_state(&self, block_id: &BlockId) -> MouseStateHandle {
        self.row_states
            .borrow_mut()
            .entry(block_id.clone())
            .or_default()
            .clone()
    }

    /// Mirrors the find bar's option toggles: an accent border and selection
    /// background when on, so the state reads without hovering.
    fn render_toggle(
        &self,
        icon: Icon,
        active: bool,
        mouse_state: MouseStateHandle,
        tooltip: &'static str,
        action: SessionHistoryAction,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        Hoverable::new(mouse_state, |state| {
            let theme = appearance.theme();
            let (border, background) = if active {
                (
                    Border::all(1.).with_border_fill(theme.accent()),
                    theme.find_bar_button_selection_color(),
                )
            } else if state.is_hovered() {
                let hover = theme.foreground_button_color();
                (Border::all(1.).with_border_fill(hover), hover)
            } else {
                let transparent = ThemeFill::Solid(ColorU::transparent_black());
                (Border::all(1.).with_border_fill(transparent), transparent)
            };
            let icon_color = if active {
                theme.active_ui_text_color()
            } else {
                theme.nonactive_ui_text_color()
            };

            let button = Container::new(
                ConstrainedBox::new(icon.to_warpui_icon(icon_color).finish())
                    .with_height(TOGGLE_ICON_SIZE)
                    .with_width(TOGGLE_ICON_SIZE)
                    .finish(),
            )
            .with_uniform_padding(TOGGLE_ICON_PADDING)
            .with_margin_right(TOGGLE_ICON_SPACING)
            .with_border(border)
            .with_background(background)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(TOGGLE_CORNER_RADIUS)))
            .finish();

            let mut stack = Stack::new().with_child(button);
            if state.is_hovered() {
                stack.add_positioned_overlay_child(
                    appearance
                        .ui_builder()
                        .tool_tip(tooltip.to_string())
                        .build()
                        .finish(),
                    OffsetPositioning::offset_from_parent(
                        vec2f(0., -5.),
                        ParentOffsetBounds::Unbounded,
                        ParentAnchor::TopMiddle,
                        ChildAnchor::BottomMiddle,
                    ),
                );
            }
            stack.finish()
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
        })
        .finish()
    }

    fn render_toggle_bar(&self, appearance: &Appearance) -> Box<dyn Element> {
        let toggles = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(self.render_toggle(
                Icon::Terminal,
                self.scope_command,
                self.toggle_states.command.clone(),
                "Match command",
                SessionHistoryAction::ToggleScope(FilterScope::Command),
                appearance,
            ))
            .with_child(self.render_toggle(
                Icon::Folder,
                self.scope_directory,
                self.toggle_states.directory.clone(),
                "Match working directory",
                SessionHistoryAction::ToggleScope(FilterScope::Directory),
                appearance,
            ))
            .with_child(self.render_toggle(
                Icon::TextBlock,
                self.scope_output,
                self.toggle_states.output.clone(),
                "Match output (slower)",
                SessionHistoryAction::ToggleScope(FilterScope::Output),
                appearance,
            ))
            .with_child(self.render_toggle(
                Icon::Regex,
                self.regex_enabled,
                self.toggle_states.regex.clone(),
                REGEX_TOGGLE_TOOLTIP,
                SessionHistoryAction::ToggleRegex,
                appearance,
            ))
            .with_child(self.render_toggle(
                Icon::CaseSensitivity,
                self.case_sensitive,
                self.toggle_states.case_sensitive.clone(),
                CASE_SENSITIVE_TOOLTIP,
                SessionHistoryAction::ToggleCaseSensitive,
                appearance,
            ))
            .with_child(self.render_toggle(
                Icon::FilterLines,
                self.filter_on_select,
                self.toggle_states.filter_on_select.clone(),
                "Filter the block's output when selected",
                SessionHistoryAction::ToggleFilterOnSelect,
                appearance,
            ))
            .with_child(self.render_toggle(
                Icon::Search,
                self.find_on_select,
                self.toggle_states.find_on_select.clone(),
                "Find within the block when selected",
                SessionHistoryAction::ToggleFindOnSelect,
                appearance,
            ))
            .finish();

        Container::new(toggles).with_padding_bottom(4.).finish()
    }

    fn render_row(&self, row: &Row, appearance: &Appearance) -> Box<dyn Element> {
        let theme = appearance.theme();
        let entry = &row.entry;
        let block_id = entry.block_id.clone();

        let detail_color = theme.sub_text_color(theme.background());
        let command = Text::new_inline(
            entry.command.clone(),
            appearance.ui_font_family(),
            COMMAND_FONT_SIZE,
        )
        .with_color(theme.main_text_color(theme.background()).into())
        .finish();

        let mut text_column = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_child(command);

        // Skipped rather than rendered empty: a block with no recorded
        // directory would otherwise leave an unexplained gap.
        if let Some(pwd) = entry.pwd.as_deref() {
            text_column.add_child(
                Text::new_inline(
                    pwd.to_string(),
                    appearance.ui_font_family(),
                    DETAIL_FONT_SIZE,
                )
                .with_color(detail_color.into())
                .finish(),
            );
        }

        text_column.add_child(
            Text::new_inline(
                self.detail_line(row),
                appearance.ui_font_family(),
                DETAIL_FONT_SIZE,
            )
            .with_color(detail_color.into())
            .finish(),
        );
        let text_column = text_column.finish();

        let mut content = Container::new(text_column)
            .with_padding_top(ROW_VERTICAL_PADDING)
            .with_padding_bottom(ROW_VERTICAL_PADDING)
            .with_padding_left(ROW_HORIZONTAL_PADDING)
            .with_padding_right(ROW_HORIZONTAL_PADDING);

        // Failed rows get the same treatment the block list paints on a failed
        // block: a left stripe at full strength over a 10% wash.
        if entry.has_failed {
            let failed = theme.failed_block_color();
            content = content
                .with_background(failed.clone().with_opacity(FAILED_BACKGROUND_OPACITY))
                .with_border(Border::left(FAILED_STRIPE_WIDTH).with_border_fill(failed));
        }

        // Selection wins over the failure wash, as it does in the block list;
        // the red stripe stays, so a failed row still reads as failed.
        if self.selected.contains(&entry.block_index) {
            content = content.with_background(theme.block_selection_color());
        }

        let position_id = session_history_row_position_id(entry.block_index);
        let content = content.finish();
        let hoverable = Hoverable::new(self.row_mouse_state(&entry.block_id), move |_| content)
            .with_cursor(Cursor::PointingHand)
            .on_click_with_modifiers({
                let block_id = block_id.clone();
                move |ctx, _, _, modifiers| {
                    ctx.dispatch_typed_action(SessionHistoryAction::RowClicked {
                        block_id: block_id.clone(),
                        modifiers,
                    });
                }
            })
            .on_double_click({
                let block_id = block_id.clone();
                move |ctx, _, _| {
                    ctx.dispatch_typed_action(SessionHistoryAction::RowDoubleClicked(
                        block_id.clone(),
                    ));
                }
            })
            .on_right_click({
                let block_id = block_id.clone();
                let position_id = position_id.clone();
                move |ctx, _, position| {
                    let Some(row_bounds) = ctx.element_position_by_id(&position_id) else {
                        return;
                    };
                    // Express the cursor relative to the row so the terminal can
                    // offset the menu from the row's saved position.
                    ctx.dispatch_typed_action(SessionHistoryAction::RowRightClicked {
                        block_id: block_id.clone(),
                        position_in_row: position - row_bounds.origin(),
                    });
                }
            })
            .finish();

        SavePosition::new(hoverable, &position_id).finish()
    }

    /// `1m 04s · exit 101 (0x65) · 612 lines · 38.6 KB · 7 matches`
    fn detail_line(&self, row: &Row) -> String {
        let entry = &row.entry;
        let mut parts = vec![self.duration_text(row)];
        // `has_failed` is already "finished with a non-zero code", so it cannot
        // disagree with the red styling on the row.
        if entry.has_failed {
            parts.push(format_exit_code(entry.exit_code));
        }
        // Omitted rather than shown as zero while the measurement is pending.
        if let Some(size) = row.size {
            parts.push(pluralize(size.lines, "line"));
            parts.push(format_bytes(size.bytes));
        }
        if let Some(matches) = row.output_matches {
            parts.push(pluralize(matches, "match"));
        }
        parts.join(" · ")
    }

    fn duration_text(&self, row: &Row) -> String {
        let entry = &row.entry;
        let Some(started) = entry.started_at else {
            return if entry.is_running {
                "running".to_string()
            } else {
                "--".to_string()
            };
        };
        let end = entry.completed_at.unwrap_or_else(Local::now);
        let seconds = ((end - started).num_milliseconds() as f64 / 1000.0).max(0.);
        if entry.is_running {
            format!("running {}", format_duration(seconds))
        } else {
            format_duration(seconds)
        }
    }
}

/// Query matcher shared by the command, directory and output scopes.
enum Matcher {
    Regex(Regex),
    Literal(String),
    LiteralInsensitive(String),
}

impl Matcher {
    fn is_match(&self, haystack: &str) -> bool {
        match self {
            Self::Regex(regex) => regex.is_match(haystack),
            Self::Literal(needle) => haystack.contains(needle.as_str()),
            Self::LiteralInsensitive(needle) => haystack.to_lowercase().contains(needle.as_str()),
        }
    }

    fn count(&self, haystack: &str) -> usize {
        match self {
            Self::Regex(regex) => regex.find_iter(haystack).count(),
            Self::Literal(needle) => haystack.matches(needle.as_str()).count(),
            Self::LiteralInsensitive(needle) => {
                haystack.to_lowercase().matches(needle.as_str()).count()
            }
        }
    }
}

impl Entity for SessionHistoryView {
    type Event = ();
}

impl View for SessionHistoryView {
    fn ui_name() -> &'static str {
        "SessionHistoryView"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);

        let mut column = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        column.add_child(
            Container::new(ChildView::new(&self.search_bar).finish())
                .with_padding_bottom(4.)
                .finish(),
        );
        column.add_child(self.render_toggle_bar(appearance));

        let body: Box<dyn Element> = if self.rows.is_empty() {
            let message = if self.query(app).is_empty() {
                "No commands yet."
            } else {
                "No commands match."
            };
            let theme = appearance.theme();
            Container::new(
                appearance
                    .ui_builder()
                    .paragraph(message)
                    .with_style(UiComponentStyles {
                        font_color: Some(theme.sub_text_color(theme.background()).into_solid()),
                        ..Default::default()
                    })
                    .build()
                    .finish(),
            )
            .with_uniform_padding(8.)
            .finish()
        } else {
            let mut list = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
            for row in &self.rows {
                list.add_child(self.render_row(row, appearance));
            }
            let theme = appearance.theme();
            ClippedScrollable::vertical(
                self.scroll_state.clone(),
                list.finish(),
                ScrollbarWidth::Auto,
                theme.disabled_text_color(theme.background()).into(),
                theme.main_text_color(theme.background()).into(),
                Fill::None,
            )
            .finish()
        };
        column.add_child(Shrinkable::new(1.0, body).finish());

        Container::new(column.with_main_axis_size(MainAxisSize::Max).finish())
            .with_padding_left(4.)
            .with_padding_right(4.)
            .finish()
    }
}

impl TypedActionView for SessionHistoryView {
    type Action = SessionHistoryAction;

    fn handle_action(&mut self, action: &SessionHistoryAction, ctx: &mut ViewContext<Self>) {
        match action {
            SessionHistoryAction::RowClicked {
                block_id,
                modifiers,
            } => self.activate_row(block_id, *modifiers, ctx),
            SessionHistoryAction::RowDoubleClicked(block_id) => {
                self.focus_terminal_from_row(block_id, ctx)
            }
            SessionHistoryAction::BlockCommand(command) => self.run_block_command(*command, ctx),
            SessionHistoryAction::RowRightClicked {
                block_id,
                position_in_row,
            } => self.open_row_context_menu(block_id, *position_in_row, ctx),
            SessionHistoryAction::ToggleScope(scope) => self.toggle_scope(*scope, ctx),
            SessionHistoryAction::ToggleRegex => {
                self.regex_enabled = !self.regex_enabled;
                self.output_match_cache.clear();
                self.refresh(ctx);
            }
            SessionHistoryAction::ToggleCaseSensitive => {
                self.case_sensitive = !self.case_sensitive;
                self.output_match_cache.clear();
                self.refresh(ctx);
            }
            SessionHistoryAction::ToggleFilterOnSelect => {
                self.filter_on_select = !self.filter_on_select;
                ctx.notify();
            }
            SessionHistoryAction::ToggleFindOnSelect => {
                self.find_on_select = !self.find_on_select;
                ctx.notify();
            }
        }
    }
}
