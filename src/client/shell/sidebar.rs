use super::render::{display_width, put_text};
use super::*;
use ratatui::{
    text::Line,
    widgets::{Paragraph, Widget},
};

fn workspace_selection_background(palette: &Palette) -> ratatui::style::Color {
    if palette.selection_bg == ratatui::style::Color::Reset {
        palette.active_row_bg
    } else {
        palette.selection_bg
    }
}

pub(in crate::client::shell) fn workspace_active_background(
    palette: &Palette,
    navigating: bool,
) -> ratatui::style::Color {
    // The fallback cursor shares the active-row color; only fill the cursor while navigating.
    if navigating && palette.selection_bg == ratatui::style::Color::Reset {
        palette.sidebar_bg
    } else {
        palette.active_row_bg
    }
}

const SIDEBAR_COLLAPSE_WIDTH: u16 = 12;
const SIDEBAR_FIND_MIN_WIDTH: u16 = 6;
const HEADER_BUTTON_WIDTH: u16 = 2;
const HEADER_TITLE_WIDTH: u16 = 8;

fn sidebar_button_background(palette: &Palette) -> ratatui::style::Color {
    if palette.surface0 == ratatui::style::Color::Reset {
        palette.active_row_bg
    } else {
        palette.surface0
    }
}

/// Draws the "« Collapse" and "Find" buttons on the bottom row of an expanded sidebar.
pub(in crate::client::shell) fn render_sidebar_collapse_button(
    buffer: &mut Buffer,
    area: Rect,
    palette: &Palette,
    hits: &mut ShellHitMap,
) {
    hits.sidebar_toggle = Rect::default();
    if area.width < 8 || area.height < 4 {
        return;
    }
    let available = area.width.saturating_sub(3);
    let collapse_width = if available >= SIDEBAR_COLLAPSE_WIDTH + 1 + SIDEBAR_FIND_MIN_WIDTH {
        SIDEBAR_COLLAPSE_WIDTH
    } else {
        available.min(16)
    };
    let y = area.bottom() - 1;
    hits.sidebar_toggle = Rect::new(area.x + 1, y, collapse_width, 1);
    render_sidebar_button(buffer, hits.sidebar_toggle, "« Collapse", palette);
    let find_x = hits.sidebar_toggle.right() + 1;
    let find_width = (area.x + 1 + available).saturating_sub(find_x).min(8);
    if find_width >= SIDEBAR_FIND_MIN_WIDTH {
        let find = Rect::new(find_x, y, find_width, 1);
        render_sidebar_button(buffer, find, "Find", palette);
        hits.sidebar_buttons.push((find, SidebarButton::Find));
    }
}

fn render_sidebar_button(buffer: &mut Buffer, button: Rect, label: &str, palette: &Palette) {
    let style = Style::default()
        .fg(palette.text)
        .bg(sidebar_button_background(palette))
        .add_modifier(Modifier::BOLD);
    buffer.set_style(button, style);
    put_text(
        buffer,
        button.x + button.width.saturating_sub(display_width(label)) / 2,
        button.y,
        button.width,
        label,
        style,
    );
}

/// Draws small glyph buttons right-aligned so the last one ends at `end_x`.
/// Skipped when the title on the left would be crowded out.
pub(in crate::client::shell) fn render_header_buttons(
    buffer: &mut Buffer,
    row: Rect,
    end_x: u16,
    buttons: &[(&str, SidebarButton)],
    palette: &Palette,
    hits: &mut ShellHitMap,
) {
    let width = HEADER_BUTTON_WIDTH * buttons.len() as u16;
    let start = end_x.saturating_sub(width);
    if end_x > row.right() || start < row.x + HEADER_TITLE_WIDTH {
        return;
    }
    for (index, (glyph, button)) in buttons.iter().enumerate() {
        let rect = Rect::new(
            start + HEADER_BUTTON_WIDTH * index as u16,
            row.y,
            HEADER_BUTTON_WIDTH,
            1,
        );
        put_text(
            buffer,
            rect.x,
            rect.y,
            1,
            glyph,
            Style::default()
                .fg(palette.text)
                .add_modifier(Modifier::BOLD),
        );
        hits.sidebar_buttons.push((rect, *button));
    }
}

/// Draws "+ new" on the spaces title row, left of the scroll buttons. Returns false
/// (leaving the footer button in charge) when the row is too narrow for it.
fn render_new_workspace_header_button(
    buffer: &mut Buffer,
    header: Rect,
    palette: &Palette,
    hits: &mut ShellHitMap,
) -> bool {
    const LABEL: &str = "+ new";
    let width = display_width(LABEL);
    let buttons = HEADER_BUTTON_WIDTH * WORKSPACE_SCROLL_BUTTONS.len() as u16;
    let x = header.right().saturating_sub(buttons + 1 + width);
    if header.right() < buttons + 1 + width || x < header.x + HEADER_TITLE_WIDTH {
        return false;
    }
    let rect = Rect::new(x, header.y, width, 1);
    put_text(
        buffer,
        rect.x,
        rect.y,
        rect.width,
        LABEL,
        Style::default()
            .fg(palette.accent)
            .add_modifier(Modifier::BOLD),
    );
    hits.new_workspace = rect;
    true
}

pub(in crate::client::shell) const WORKSPACE_SCROLL_BUTTONS: [(&str, SidebarButton); 2] = [
    ("▲", SidebarButton::WorkspaceUp),
    ("▼", SidebarButton::WorkspaceDown),
];

/// Draws the expand button on the bottom row of the compact collapsed sidebar.
pub(in crate::client::shell) fn render_sidebar_expand_button(
    buffer: &mut Buffer,
    area: Rect,
    workspace_area: Rect,
    attention: bool,
    palette: &Palette,
    hits: &mut ShellHitMap,
) {
    hits.sidebar_toggle = if area.is_empty() || workspace_area.width == 0 {
        Rect::default()
    } else {
        // Short sidebars have no agent section, so the last row is a workspace row:
        // keep that row clickable by covering a single cell there.
        let (x, width) = if area.height < 7 {
            (workspace_area.x + workspace_area.width / 2, 1)
        } else {
            (workspace_area.x, workspace_area.width.min(3))
        };
        Rect::new(x, area.bottom().saturating_sub(1), width, 1)
    };
    let button = hits.sidebar_toggle;
    if button.is_empty() {
        return;
    }
    let style = Style::default()
        .fg(if attention {
            palette.accent
        } else {
            palette.text
        })
        .bg(sidebar_button_background(palette))
        .add_modifier(Modifier::BOLD);
    buffer.set_style(button, style);
    put_text(buffer, button.x + button.width / 2, button.y, 1, "»", style);
}

pub(in crate::client::shell) fn collapsed_sidebar_sections(
    area: Rect,
) -> (Rect, Option<u16>, Rect) {
    let content = Rect::new(area.x, area.y, area.width.saturating_sub(1), area.height);
    if content.is_empty() {
        return (Rect::default(), None, Rect::default());
    }
    if content.height < 7 {
        return (content, None, Rect::default());
    }
    let workspace_height = content.height.div_ceil(2);
    let divider_y = content.y + workspace_height;
    let detail_height = content.height.saturating_sub(workspace_height + 1);
    (
        Rect::new(content.x, content.y, content.width, workspace_height),
        Some(divider_y),
        Rect::new(content.x, divider_y + 1, content.width, detail_height),
    )
}

pub(crate) fn render_collapsed_sidebar(
    buffer: &mut Buffer,
    area: Rect,
    snapshot: &ClientShellSnapshot,
    config: &ClientShellConfig,
    selected_workspace_id: Option<&str>,
    hits: &mut ShellHitMap,
) {
    let palette = &config.palette;
    let selection_background = super::mobile::accent_tint(palette, true)
        .unwrap_or_else(|| workspace_selection_background(palette));
    let active_background = super::mobile::accent_tint(palette, false)
        .unwrap_or_else(|| workspace_active_background(palette, selected_workspace_id.is_some()));
    render_sidebar_background(buffer, area, palette);
    let (workspace_area, divider_y, detail_area) = collapsed_sidebar_sections(area);
    for (index, workspace) in snapshot
        .workspaces
        .iter()
        .take(workspace_area.height as usize)
        .enumerate()
    {
        let rect = Rect::new(
            workspace_area.x,
            workspace_area.y + index as u16,
            workspace_area.width,
            1,
        );
        let selected = selected_workspace_id == Some(workspace.workspace_id.as_str());
        if selected {
            buffer.set_style(rect, Style::default().bg(selection_background));
        } else if workspace.focused {
            buffer.set_style(rect, Style::default().bg(active_background));
        }
        let number_style = if selected {
            Style::default()
                .fg(palette.overlay1)
                .bg(selection_background)
        } else if workspace.focused {
            Style::default().fg(palette.text).bg(active_background)
        } else {
            Style::default().fg(palette.overlay0)
        };
        put_text(
            buffer,
            rect.x,
            rect.y,
            rect.width.min(2),
            &format!("{:<2}", index + 1),
            number_style,
        );
        let status = workspace.agent_status;
        put_text(
            buffer,
            rect.x.saturating_add(2),
            rect.y,
            rect.width.saturating_sub(2),
            status_icon(status, config.status_indicators),
            Style::default().fg(status_color(status, palette)),
        );
        hits.workspaces.push(WorkspaceHit {
            rect,
            endpoint_id: ClientEndpointId::Local,
            workspace_id: workspace.workspace_id.clone(),
            indented: false,
            group_toggle: None,
        });
    }

    if let Some(divider_y) = divider_y {
        put_text(
            buffer,
            workspace_area.x,
            divider_y,
            workspace_area.width,
            &"─".repeat(workspace_area.width as usize),
            Style::default().fg(palette.surface_dim),
        );
    }

    let detail_content = Rect::new(
        detail_area.x,
        detail_area.y,
        detail_area.width,
        detail_area.height.saturating_sub(1),
    );
    for (index, pane_id) in super::ordered_agent_pane_ids(snapshot, config.agent_panel_sort)
        .into_iter()
        .take(detail_content.height as usize)
        .enumerate()
    {
        let Some(agent) = snapshot
            .agents
            .iter()
            .find(|agent| agent.pane_id == pane_id)
        else {
            continue;
        };
        let rect = Rect::new(
            detail_content.x,
            detail_content.y + index as u16,
            detail_content.width,
            1,
        );
        if agent.focused {
            buffer.set_style(rect, Style::default().bg(palette.active_row_bg));
        }
        put_text(
            buffer,
            rect.x,
            rect.y,
            rect.width.min(2),
            &format!("{:<2}", index + 1),
            Style::default().fg(if agent.focused {
                palette.text
            } else {
                palette.overlay0
            }),
        );
        put_text(
            buffer,
            rect.x.saturating_add(2),
            rect.y,
            rect.width.saturating_sub(2),
            status_icon(agent.agent_status, config.status_indicators),
            Style::default().fg(status_color(agent.agent_status, palette)),
        );
        hits.agents.push((rect, pane_id));
    }
    render_sidebar_expand_button(
        buffer,
        area,
        workspace_area,
        super::super::global_menu::global_menu_attention(snapshot),
        palette,
        hits,
    );
}

pub(crate) fn render_sidebar(
    buffer: &mut Buffer,
    area: Rect,
    snapshot: &ClientShellSnapshot,
    config: &ClientShellConfig,
    state: &mut ShellRenderState<'_>,
    hits: &mut ShellHitMap,
) {
    let palette = &config.palette;
    render_sidebar_background(buffer, area, palette);
    hits.sidebar_divider = if area.is_empty() {
        Rect::default()
    } else {
        Rect::new(area.right().saturating_sub(1), area.y, 1, area.height)
    };
    let (workspace_area, detail_area) =
        crate::ui::expanded_sidebar_sections(area, state.sidebar_section_split);
    hits.sidebar_section_divider =
        crate::ui::sidebar_section_divider_rect(area, state.sidebar_section_split);
    put_text(
        buffer,
        workspace_area.x,
        workspace_area.y,
        workspace_area.width,
        " spaces",
        Style::default()
            .fg(palette.overlay0)
            .add_modifier(Modifier::BOLD),
    );
    put_text(
        buffer,
        workspace_area.x,
        workspace_area.y + 1,
        workspace_area.width,
        &"─".repeat(workspace_area.width as usize),
        Style::default().fg(palette.overlay0),
    );
    let mut new_in_header = false;
    if config.mouse_capture {
        let header = Rect::new(workspace_area.x, workspace_area.y, workspace_area.width, 1);
        render_header_buttons(
            buffer,
            header,
            workspace_area.right(),
            &WORKSPACE_SCROLL_BUTTONS,
            palette,
            hits,
        );
        new_in_header = render_new_workspace_header_button(buffer, header, palette, hits);
    }

    let entries = workspace_entries(snapshot, state.collapsed_groups);
    let body = Rect::new(
        workspace_area.x,
        workspace_area.y.saturating_add(WORKSPACE_HEADER_ROWS),
        workspace_area.width,
        workspace_area
            .height
            .saturating_sub(WORKSPACE_HEADER_ROWS + 1),
    );
    hits.workspace_body = body;
    let row_heights = entries
        .iter()
        .map(|entry| {
            snapshot
                .workspaces
                .get(entry.index)
                .map(|workspace| {
                    workspace_rows(
                        snapshot,
                        workspace,
                        displayed_workspace_status(snapshot, workspace, state.collapsed_groups),
                        entry.indented,
                        &config.spaces,
                    )
                    .len()
                    .max(1)
                    .min(u16::MAX as usize) as u16
                })
                .unwrap_or(1)
        })
        .collect::<Vec<_>>();
    let gaps = entries
        .iter()
        .enumerate()
        .map(|(index, _)| {
            entries
                .get(index + 1)
                .map_or(0, |next| u16::from(!next.indented) * config.spaces.row_gap)
        })
        .collect::<Vec<_>>();
    let mut metrics = super::scroll::list_scroll_metrics(
        &row_heights,
        &gaps,
        body.height,
        *state.workspace_scroll,
    );
    if !body.is_empty() && std::mem::take(state.reveal_focused_workspace) {
        if let Some(target) = entries
            .iter()
            .position(|entry| snapshot.workspaces[entry.index].focused)
        {
            *state.workspace_scroll = super::scroll::list_scroll_start_to_reveal(
                &row_heights,
                &gaps,
                body.height,
                *state.workspace_scroll,
                target,
            );
            metrics = super::scroll::list_scroll_metrics(
                &row_heights,
                &gaps,
                body.height,
                *state.workspace_scroll,
            );
        }
    }
    hits.workspace_max_scroll = metrics.max_offset_from_bottom;
    hits.workspace_scroll_metrics = Some(metrics);
    *state.workspace_scroll = metrics
        .max_offset_from_bottom
        .saturating_sub(metrics.offset_from_bottom);
    let show_scrollbar = metrics.max_offset_from_bottom > 0 && body.width > 1;
    let content_width = body.width.saturating_sub(u16::from(show_scrollbar));
    let mut y = body.y;
    for (entry_position, entry) in entries.iter().enumerate().skip(*state.workspace_scroll) {
        let Some(workspace) = snapshot.workspaces.get(entry.index) else {
            continue;
        };
        let status = displayed_workspace_status(snapshot, workspace, state.collapsed_groups);
        let rows = workspace_rows(snapshot, workspace, status, entry.indented, &config.spaces);
        let row_height = (rows.len().max(1).min(u16::MAX as usize) as u16).min(body.height);
        if y.saturating_add(row_height) > body.bottom() {
            break;
        }
        let rect = Rect::new(body.x, y, content_width, row_height);
        let selected = state.selected_workspace_id.is_some_and(|target| {
            target.matches(state.active_endpoint_id, &workspace.workspace_id)
        });
        let dragged = state.dragged_workspace_id == Some(workspace.workspace_id.as_str());
        if selected {
            buffer.set_style(rect, Style::default().bg(palette.selection_bg));
        } else if dragged {
            buffer.set_style(rect, Style::default().bg(palette.surface1));
        } else if workspace.focused {
            buffer.set_style(rect, Style::default().bg(palette.active_row_bg));
        }
        let tab_position = workspace_tab_position(snapshot, workspace);
        render_workspace_rows(
            buffer,
            rect,
            status,
            config.status_indicators,
            entry,
            rows,
            WorkspaceRowState {
                focused: workspace.focused,
                selected,
                navigating: state.selected_workspace_id.is_some(),
                dragged,
                index: entry_position,
            },
            tab_position.as_deref(),
            palette,
        );
        let group_toggle = render_parent_group_toggle(
            buffer,
            rect,
            snapshot,
            entry.index,
            state.collapsed_groups,
            palette,
        );
        hits.workspaces.push(WorkspaceHit {
            rect,
            endpoint_id: ClientEndpointId::Local,
            workspace_id: workspace.workspace_id.clone(),
            indented: entry.indented,
            group_toggle,
        });
        let gap = entries
            .get(entry_position + 1)
            .map_or(0, |next| u16::from(!next.indented) * config.spaces.row_gap);
        y = y.saturating_add(row_height + gap);
    }

    if show_scrollbar {
        let track = Rect::new(body.right().saturating_sub(1), body.y, 1, body.height);
        hits.workspace_scrollbar = track;
        super::scroll::render_list_scrollbar(buffer, track, metrics, palette);
    }

    if let Some(row) = state.workspace_drop_indicator_row.filter(|row| {
        *row >= workspace_area.y.saturating_add(1)
            && *row < workspace_area.bottom().saturating_sub(1)
    }) {
        put_text(
            buffer,
            body.x,
            row,
            body.width,
            &"─".repeat(body.width as usize),
            Style::default().fg(palette.accent),
        );
    }

    let footer_y = workspace_area.bottom().saturating_sub(1);
    if config.mouse_capture {
        if !new_in_header {
            hits.new_workspace = Rect::new(
                workspace_area.x,
                footer_y,
                5.min(workspace_area.width),
                u16::from(workspace_area.height > 0),
            );
            put_text(
                buffer,
                workspace_area.x,
                footer_y,
                workspace_area.width,
                " new",
                Style::default().fg(palette.overlay0),
            );
        }
        let attention = super::super::global_menu::global_menu_attention(snapshot);
        let launcher_width = if attention { 8 } else { 6 }.min(workspace_area.width);
        hits.global_launcher = Rect::new(
            workspace_area.right().saturating_sub(launcher_width),
            footer_y,
            launcher_width,
            1,
        );
        if attention {
            let start_x = workspace_area.right().saturating_sub(6);
            put_text(
                buffer,
                start_x,
                footer_y,
                2,
                "● ",
                Style::default()
                    .fg(palette.accent)
                    .add_modifier(Modifier::BOLD),
            );
            put_text(
                buffer,
                start_x.saturating_add(2),
                footer_y,
                4,
                "menu",
                Style::default().fg(palette.overlay0),
            );
        } else {
            put_right_text(
                buffer,
                workspace_area,
                footer_y,
                "menu",
                Style::default().fg(palette.overlay0),
            );
        }
    }

    super::render_agent_panel(
        buffer,
        reserve_collapse_button_row(detail_area),
        snapshot,
        config,
        state.agent_scroll,
        hits,
    );

    render_sidebar_collapse_button(buffer, area, palette, hits);
}

/// Leaves the last row of the agent section free for the collapse button.
pub(in crate::client::shell) fn reserve_collapse_button_row(detail_area: Rect) -> Rect {
    Rect::new(
        detail_area.x,
        detail_area.y,
        detail_area.width,
        detail_area.height.saturating_sub(1),
    )
}

pub(crate) fn workspace_entries(
    snapshot: &ClientShellSnapshot,
    collapsed_groups: &HashSet<String>,
) -> Vec<WorkspaceEntry> {
    let mut members = HashMap::<&str, Vec<usize>>::new();
    for (index, workspace) in snapshot.workspaces.iter().enumerate() {
        if let Some(worktree) = &workspace.worktree {
            members.entry(&worktree.key).or_default().push(index);
        }
    }
    let grouped = members
        .iter()
        .filter(|(_, indices)| {
            indices.iter().any(|index| {
                snapshot.workspaces[*index]
                    .worktree
                    .as_ref()
                    .is_some_and(|worktree| worktree.is_linked_worktree)
            }) && indices.iter().any(|index| {
                snapshot.workspaces[*index]
                    .worktree
                    .as_ref()
                    .is_some_and(|worktree| !worktree.is_linked_worktree)
            })
        })
        .map(|(key, _)| *key)
        .collect::<HashSet<_>>();
    let mut emitted = HashSet::<&str>::new();
    let mut entries = Vec::new();
    for (index, workspace) in snapshot.workspaces.iter().enumerate() {
        let Some(worktree) = workspace
            .worktree
            .as_ref()
            .filter(|worktree| grouped.contains(worktree.key.as_str()))
        else {
            entries.push(WorkspaceEntry {
                index,
                indented: false,
                last_child: false,
            });
            continue;
        };
        if !emitted.insert(&worktree.key) {
            continue;
        }
        let Some(group_members) = members.get(worktree.key.as_str()) else {
            continue;
        };
        for parent in group_members.iter().copied().filter(|member| {
            snapshot.workspaces[*member]
                .worktree
                .as_ref()
                .is_some_and(|worktree| !worktree.is_linked_worktree)
        }) {
            entries.push(WorkspaceEntry {
                index: parent,
                indented: false,
                last_child: false,
            });
        }
        if collapsed_groups.contains(&worktree.key) {
            if let Some(active) = group_members.iter().copied().find(|member| {
                let workspace = &snapshot.workspaces[*member];
                workspace.focused
                    && workspace
                        .worktree
                        .as_ref()
                        .is_some_and(|worktree| worktree.is_linked_worktree)
            }) {
                entries.push(WorkspaceEntry {
                    index: active,
                    indented: true,
                    last_child: true,
                });
            }
            continue;
        }
        let children = group_members
            .iter()
            .copied()
            .filter(|member| {
                snapshot.workspaces[*member]
                    .worktree
                    .as_ref()
                    .is_some_and(|worktree| worktree.is_linked_worktree)
            })
            .collect::<Vec<_>>();
        for (child_index, child) in children.iter().enumerate() {
            entries.push(WorkspaceEntry {
                index: *child,
                indented: true,
                last_child: child_index + 1 == children.len(),
            });
        }
    }
    sort_entries_by_activity(snapshot, entries)
}

/// Moves busy spaces to the top. A worktree group (parent plus its indented
/// children) moves as one block ranked by its busiest member; ties keep their
/// existing order.
fn sort_entries_by_activity(
    snapshot: &ClientShellSnapshot,
    entries: Vec<WorkspaceEntry>,
) -> Vec<WorkspaceEntry> {
    let rank = |entry: &WorkspaceEntry| {
        let workspace = &snapshot.workspaces[entry.index];
        activity_key(snapshot, workspace.agent_status, &workspace.workspace_id)
    };
    let mut blocks: Vec<Vec<WorkspaceEntry>> = Vec::new();
    for entry in entries {
        match blocks.last_mut() {
            Some(block) if entry.indented => block.push(entry),
            _ => blocks.push(vec![entry]),
        }
    }
    for block in &mut blocks {
        block[1..].sort_by_key(rank);
        let last = block.len() - 1;
        for (position, entry) in block.iter_mut().enumerate().skip(1) {
            entry.last_child = position == last;
        }
    }
    blocks.sort_by_key(|block| block.iter().map(rank).min());
    blocks.into_iter().flatten().collect()
}

fn parent_group_key(snapshot: &ClientShellSnapshot, index: usize) -> Option<String> {
    let workspace = snapshot.workspaces.get(index)?;
    let worktree = workspace.worktree.as_ref()?;
    if worktree.is_linked_worktree {
        return None;
    }
    snapshot
        .workspaces
        .iter()
        .any(|candidate| {
            candidate.worktree.as_ref().is_some_and(|candidate| {
                candidate.key == worktree.key && candidate.is_linked_worktree
            })
        })
        .then(|| worktree.key.clone())
}

pub(in crate::client::shell) fn workspace_close_is_group(
    snapshot: &ClientShellSnapshot,
    workspace: &ClientShellWorkspace,
) -> bool {
    let Some(worktree) = workspace
        .worktree
        .as_ref()
        .filter(|worktree| !worktree.is_linked_worktree)
    else {
        return false;
    };
    let mut has_child = false;
    for member in &snapshot.workspaces {
        if member.workspace_id == workspace.workspace_id {
            continue;
        }
        if let Some(candidate) = member
            .worktree
            .as_ref()
            .filter(|candidate| candidate.key == worktree.key)
        {
            if !candidate.is_linked_worktree {
                return false;
            }
            has_child = true;
        }
    }
    has_child
}

pub(in crate::client::shell) fn render_parent_group_toggle(
    buffer: &mut Buffer,
    workspace_rect: Rect,
    snapshot: &ClientShellSnapshot,
    workspace_index: usize,
    collapsed_groups: &HashSet<String>,
    palette: &Palette,
) -> Option<(Rect, String)> {
    let key = parent_group_key(snapshot, workspace_index)?;
    let toggle = Rect::new(
        workspace_rect.right().saturating_sub(1),
        workspace_rect.y,
        1,
        1,
    );
    put_text(
        buffer,
        toggle.x,
        toggle.y,
        toggle.width,
        if collapsed_groups.contains(&key) {
            "▸"
        } else {
            "▾"
        },
        Style::default().fg(palette.accent),
    );
    Some((toggle, key))
}

pub(in crate::client::shell) fn displayed_workspace_status(
    snapshot: &ClientShellSnapshot,
    workspace: &ClientShellWorkspace,
    collapsed_groups: &HashSet<String>,
) -> crate::api::schema::AgentStatus {
    let Some(worktree) = workspace
        .worktree
        .as_ref()
        .filter(|worktree| !worktree.is_linked_worktree)
    else {
        return workspace.agent_status;
    };
    if !collapsed_groups.contains(&worktree.key) {
        return workspace.agent_status;
    }
    snapshot
        .workspaces
        .iter()
        .filter(|candidate| {
            candidate.worktree.as_ref().is_some_and(|member| {
                member.key == worktree.key
                    && (member.is_linked_worktree
                        || candidate.workspace_id == workspace.workspace_id)
            })
        })
        .map(|candidate| candidate.agent_status)
        .max_by_key(|status| status_priority(*status))
        .unwrap_or(workspace.agent_status)
}

pub(in crate::client::shell) fn workspace_rows(
    snapshot: &ClientShellSnapshot,
    workspace: &ClientShellWorkspace,
    status: crate::api::schema::AgentStatus,
    indented: bool,
    config: &SpacesSidebarConfig,
) -> Vec<Vec<crate::ui::ResolvedToken>> {
    let label = if indented && !workspace.custom_label {
        workspace
            .branch
            .as_deref()
            .and_then(|branch| branch.strip_prefix("worktree/").or(Some(branch)))
            .unwrap_or(&workspace.label)
    } else {
        &workspace.label
    };
    let token_values = workspace.tokens.iter().cloned().collect::<HashMap<_, _>>();
    let mut rows = crate::ui::sidebar_space_rows(
        config,
        crate::ui::SpaceTokenContext {
            workspace: label,
            branch: workspace.branch.as_deref(),
            state_text: status_text(status),
            ahead_behind: workspace.git_ahead_behind,
            tokens: &token_values,
            suppress_git_details: indented,
        },
    );
    add_active_tab_token(&mut rows, snapshot, workspace, config.rows.len());
    rows
}

/// "N/M" for the workspace's active tab, shown at the right edge of its first row.
pub(in crate::client::shell) fn workspace_tab_position(
    snapshot: &ClientShellSnapshot,
    workspace: &ClientShellWorkspace,
) -> Option<String> {
    let tabs = snapshot
        .tabs
        .iter()
        .filter(|tab| tab.workspace_id == workspace.workspace_id)
        .collect::<Vec<_>>();
    let active = tabs
        .iter()
        .position(|tab| tab.tab_id == workspace.active_tab_id)?;
    Some(format!("{}/{}", active + 1, tabs.len()))
}

/// Puts the active tab's bare name at the start of the second (detail) row.
fn add_active_tab_token(
    rows: &mut Vec<Vec<crate::ui::ResolvedToken>>,
    snapshot: &ClientShellSnapshot,
    workspace: &ClientShellWorkspace,
    configured_rows: usize,
) {
    if configured_rows < 2 {
        return;
    }
    let Some(tab) = snapshot
        .tabs
        .iter()
        .find(|tab| tab.tab_id == workspace.active_tab_id)
    else {
        return;
    };
    let token = crate::ui::ResolvedToken {
        kind: crate::ui::ResolvedTokenKind::Tab(tab.label.clone()),
        style: Default::default(),
    };
    match rows.get_mut(1) {
        Some(row) => row.insert(0, token),
        None => rows.push(vec![token]),
    }
}

/// How a workspace row is highlighted.
#[derive(Clone, Copy)]
pub(in crate::client::shell) struct WorkspaceRowState {
    pub focused: bool,
    pub selected: bool,
    pub navigating: bool,
    pub dragged: bool,
    /// Position in the list, so rows can alternate their shade.
    pub index: usize,
}

/// The background of a space row: the accent tint for the focused row (stronger for the
/// navigation cursor), otherwise alternating dark shades. Palettes without RGB colors
/// keep their themed highlight colors and are not striped.
pub(in crate::client::shell) fn workspace_row_background(
    palette: &Palette,
    row_state: WorkspaceRowState,
) -> Option<ratatui::style::Color> {
    if row_state.selected {
        Some(
            super::mobile::accent_tint(palette, true)
                .unwrap_or_else(|| workspace_selection_background(palette)),
        )
    } else if row_state.dragged {
        Some(palette.surface1)
    } else if row_state.focused {
        Some(
            super::mobile::accent_tint(palette, false)
                .unwrap_or_else(|| workspace_active_background(palette, row_state.navigating)),
        )
    } else {
        super::mobile::try_row_shade(palette, row_state.index)
    }
}

pub(in crate::client::shell) fn render_workspace_rows(
    buffer: &mut Buffer,
    area: Rect,
    status: crate::api::schema::AgentStatus,
    indicators: crate::config::StatusIndicatorStyle,
    entry: &WorkspaceEntry,
    rows: Vec<Vec<crate::ui::ResolvedToken>>,
    row_state: WorkspaceRowState,
    tab_position: Option<&str>,
    palette: &Palette,
) {
    let WorkspaceRowState {
        focused, dragged, ..
    } = row_state;
    for (row_index, row) in rows.iter().enumerate() {
        let y = area.y + row_index as u16;
        if y >= area.bottom() {
            break;
        }
        let mut x = area.x;
        if entry.indented {
            let prefix = if row_index == 0 {
                if entry.last_child {
                    "   └─ "
                } else {
                    "   ├─ "
                }
            } else if entry.last_child {
                "        "
            } else {
                "   │    "
            };
            x = put_segment(
                buffer,
                x,
                y,
                area.right(),
                prefix,
                Style::default().fg(palette.overlay0),
            );
        } else if row_index == 0 {
            x = x.saturating_add(1);
        } else {
            x = x.saturating_add(3);
        }
        let highlighted = focused || dragged;
        let _ = highlighted;
        let workspace_style = Style::default()
            .fg(palette.text)
            .add_modifier(Modifier::BOLD);
        let secondary_style = Style::default().fg(palette.overlay0);
        let right = area.right().saturating_sub(2);
        let render_spans = |width: u16| {
            crate::ui::resolved_token_spans(
                row,
                (
                    status_icon(status, indicators),
                    Style::default().fg(status_color(status, palette)),
                ),
                Style::default().fg(status_color(status, palette)),
                workspace_style,
                secondary_style,
                Style::default().fg(palette.overlay1),
                palette,
                usize::from(width),
            )
        };
        // The position only shows when the whole row still fits beside it.
        let full = right.saturating_sub(x);
        let position = tab_position.filter(|_| row_index == 0).filter(|text| {
            let needed = render_spans(full)
                .iter()
                .map(|span| display_width(span.content.as_ref()))
                .sum::<u16>();
            needed + display_width(text) < full
        });
        let text_right =
            position.map_or(right, |text| right.saturating_sub(display_width(text) + 1));
        Paragraph::new(Line::from(render_spans(text_right.saturating_sub(x))))
            .render(Rect::new(x, y, text_right.saturating_sub(x), 1), buffer);
        if let Some(text) = position {
            super::render::put_right_text(
                buffer,
                Rect::new(area.x, y, right.saturating_sub(area.x), 1),
                y,
                text,
                Style::default().fg(palette.overlay0),
            );
        }
    }

    // Rows alternate two dark shades; the focused row takes an accent tint (stronger for
    // the navigation cursor), the same as the mobile list.
    let background = workspace_row_background(palette, row_state);
    if let Some(background) = background {
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                buffer[(x, y)].set_bg(background);
            }
        }
    }
}
