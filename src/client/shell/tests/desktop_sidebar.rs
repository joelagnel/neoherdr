use super::status_order::{agent_for, state_with, workspace};
use super::*;
use crate::client::shell::mobile::{highlight_background, row_shade};

fn desktop_state(focused: Option<&str>) -> ClientShellState {
    let mut workspaces = vec![
        workspace("a", "alpha", AgentStatus::Idle, None),
        workspace("b", "bravo", AgentStatus::Idle, None),
        workspace("c", "charlie", AgentStatus::Idle, None),
        workspace("d", "delta", AgentStatus::Idle, None),
    ];
    for workspace in &mut workspaces {
        workspace.focused = focused == Some(workspace.workspace_id.as_str());
        workspace.branch = Some("main".into());
    }
    let agents = vec![
        agent_for("pane-a", "a", AgentStatus::Idle, 1),
        agent_for("pane-b", "b", AgentStatus::Idle, 1),
        agent_for("pane-c", "c", AgentStatus::Working, 1),
        agent_for("pane-d", "d", AgentStatus::Idle, 1),
    ];
    let mut state = state_with(workspaces, agents);
    state.compose(100, 40).expect("desktop shell");
    state
}

fn buffer_of(state: &mut ClientShellState) -> ratatui::buffer::Buffer {
    state
        .compose(100, 40)
        .expect("desktop shell")
        .to_ratatui_buffer()
        .expect("buffer")
}

fn text_row(buffer: &ratatui::buffer::Buffer, y: u16, x0: u16, x1: u16) -> String {
    (x0..x1).map(|x| buffer[(x, y)].symbol()).collect()
}

#[test]
fn desktop_space_rows_alternate_dark_shades_like_the_mobile_list() {
    let mut state = desktop_state(None);
    let buffer = buffer_of(&mut state);
    let palette = state.config.palette.clone();
    let rects = state
        .hits
        .workspaces
        .iter()
        .map(|hit| hit.rect)
        .collect::<Vec<_>>();
    assert!(rects.len() >= 4 && rects.iter().all(|rect| rect.height == 2));
    for (index, rect) in rects.iter().enumerate() {
        let expected = row_shade(&palette, index);
        for y in rect.y..rect.bottom() {
            assert_eq!(buffer[(rect.x + 3, y)].bg, expected, "row {index} line {y}");
            assert_eq!(
                buffer[(rect.right() - 1, y)].bg,
                expected,
                "row {index} edge"
            );
        }
    }
    assert_ne!(row_shade(&palette, 0), row_shade(&palette, 1));
}

#[test]
fn the_focused_space_gets_the_accent_tint_without_a_marker() {
    let mut state = desktop_state(Some("b"));
    let buffer = buffer_of(&mut state);
    let palette = state.config.palette.clone();
    let focused = state
        .hits
        .workspaces
        .iter()
        .find(|hit| hit.workspace_id == "b")
        .unwrap()
        .rect;
    let tint = highlight_background(&palette, false, false);
    assert_eq!(buffer[(focused.x + 3, focused.y)].bg, tint);
    assert_ne!(tint, row_shade(&palette, 0));
    assert_ne!(tint, row_shade(&palette, 1));
    assert_ne!(
        buffer[(focused.x, focused.y)].symbol(),
        "▸",
        "the tint marks the focused row, no marker"
    );
    let other = state
        .hits
        .workspaces
        .iter()
        .find(|hit| hit.workspace_id == "a")
        .unwrap()
        .rect;
    assert_ne!(buffer[(other.x, other.y)].symbol(), "▸");
}

#[test]
fn rules_sit_under_the_spaces_title_and_around_the_agents_title() {
    let mut state = desktop_state(None);
    let buffer = buffer_of(&mut state);
    let overlay = state.config.palette.overlay0;
    let body = state.hits.workspace_body;
    let width = body.width.saturating_sub(2);
    let under_spaces = text_row(&buffer, body.y - 1, body.x, body.x + width);
    assert!(
        under_spaces.chars().all(|c| c == '─'),
        "rule under spaces: {under_spaces:?}"
    );
    assert_eq!(buffer[(body.x + 2, body.y - 1)].fg, overlay);
    let agents = state.hits.agent_body;
    for y in [agents.y - 3, agents.y - 1] {
        let rule = text_row(&buffer, y, agents.x, agents.x + width);
        assert!(rule.chars().all(|c| c == '─'), "rule on row {y}: {rule:?}");
        assert_eq!(
            buffer[(agents.x + 2, y)].fg,
            overlay,
            "rule color on row {y}"
        );
    }
    let title = text_row(&buffer, agents.y - 2, agents.x, agents.x + width);
    assert!(title.contains("agents"), "{title:?}");
}

#[test]
fn desktop_agent_rows_show_the_state_on_the_right_and_tab_dot_agent_below() {
    let mut state = desktop_state(None);
    // Name the first workspace's tab so the tab shows on the agent's second line.
    let mut snapshot = state.snapshot.take().unwrap();
    snapshot.tabs[0].label = "logs".into();
    snapshot.tabs[0].custom_label = true;
    state.set_snapshot(snapshot);
    state.set_pane_surface(surface());
    let buffer = buffer_of(&mut state);
    let palette = state.config.palette.clone();
    let rect = state
        .hits
        .agents
        .iter()
        .find(|(_, pane)| pane == "pane-a")
        .map(|(rect, _)| *rect)
        .expect("agent row");
    assert_eq!(rect.height, 2);
    let line1 = text_row(&buffer, rect.y, rect.x, rect.right());
    let line2 = text_row(&buffer, rect.y + 1, rect.x, rect.right());
    assert!(line1.contains("alpha"), "{line1:?}");
    assert!(
        line1.trim_end().ends_with("idle"),
        "state at the right edge: {line1:?}"
    );
    assert!(line2.trim_start().starts_with("logs · pane-a"), "{line2:?}");
    // Rows alternate shades; the focused agent would use the tint instead.
    let rects = state
        .hits
        .agents
        .iter()
        .map(|(rect, _)| *rect)
        .collect::<Vec<_>>();
    assert_eq!(
        buffer[(rects[0].x + 3, rects[0].y)].bg,
        row_shade(&palette, 0)
    );
    assert_eq!(
        buffer[(rects[1].x + 3, rects[1].y)].bg,
        row_shade(&palette, 1)
    );
}
