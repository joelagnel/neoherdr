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
fn the_active_row_background_is_a_visible_dark_yellow() {
    let mut state = desktop_state(Some("b"));
    let palette = state.config.palette.clone();
    let rgb = |color| match color {
        ratatui::style::Color::Rgb(r, g, b) => (r, g, b),
        other => panic!("expected an rgb highlight, got {other:?}"),
    };
    let (r, g, b) = rgb(highlight_background(&palette, false, false));
    let (cr, cg, cb) = rgb(highlight_background(&palette, true, false));
    for (r, g, b) in [(r, g, b), (cr, cg, cb)] {
        assert!(r >= g && g > b, "yellow hue, not gray or blue: {r},{g},{b}");
        assert!(
            u16::from(g) * 100 >= u16::from(r) * 75,
            "yellow rather than orange: {r},{g},{b}"
        );
        assert!(b < 30, "saturated: {b}");
        assert!((60..=140).contains(&r), "dark enough for dim text: {r}");
    }
    assert!(
        cr > r,
        "the navigation cursor is stronger than the active row"
    );
    for shade in 0..2 {
        assert_ne!((r, g, b), rgb(row_shade(&palette, shade)));
    }
    let buffer = buffer_of(&mut state);
    let focused = state
        .hits
        .workspaces
        .iter()
        .find(|hit| hit.workspace_id == "b")
        .unwrap()
        .rect;
    assert_eq!(
        rgb(buffer[(focused.x + 3, focused.y)].bg),
        (r, g, b),
        "the focused desktop row uses it"
    );
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

fn click_space(state: &mut ClientShellState, workspace_id: &str) -> ClientShellInput {
    let rect = state
        .hits
        .workspaces
        .iter()
        .find(|hit| hit.workspace_id == workspace_id)
        .unwrap_or_else(|| panic!("no sidebar row for {workspace_id}"))
        .rect;
    let at = |kind| {
        RawInputEvent::Mouse(crossterm::event::MouseEvent {
            kind,
            column: rect.x + 3,
            row: rect.y,
            modifiers: KeyModifiers::NONE,
        })
    };
    state.handle_raw_events(vec![
        at(MouseEventKind::Down(MouseButton::Left)),
        at(MouseEventKind::Up(MouseButton::Left)),
    ])
}

fn focus_request(outcome: &ClientShellInput) -> Option<crate::api::schema::Method> {
    outcome.actions.iter().find_map(|action| match action {
        ClientShellAction::Endpoint { request, .. } => Some(request.method.clone()),
        _ => None,
    })
}

fn pane_focus(method: Option<crate::api::schema::Method>) -> Option<String> {
    match method {
        Some(crate::api::schema::Method::PaneFocus(target)) => Some(target.pane_id),
        _ => None,
    }
}

/// Space `a` has two tabs: its remembered active tab holds the idle agent, the other tab an
/// agent in `status`.
fn two_tab_state(status: AgentStatus) -> ClientShellState {
    let mut idle = agent_for("pane-idle", "a", AgentStatus::Idle, 1);
    idle.tab_id = "tab_a_first".into();
    let mut active = agent_for("pane-active", "a", status, 2);
    active.tab_id = "tab_a_second".into();
    let mut state = state_with(
        vec![
            workspace("a", "alpha", status, None),
            workspace("b", "bravo", AgentStatus::Idle, None),
        ],
        vec![idle, active, agent_for("pane-b", "b", AgentStatus::Idle, 1)],
    );
    state.compose(100, 40).expect("desktop shell");
    state
}

#[test]
fn clicking_a_space_with_a_filled_circle_focuses_the_tab_that_earned_it() {
    for status in [AgentStatus::Working, AgentStatus::Blocked] {
        let mut state = two_tab_state(status);
        let outcome = click_space(&mut state, "a");
        assert_eq!(
            pane_focus(focus_request(&outcome)).as_deref(),
            Some("pane-active"),
            "{status:?}: straight to the busy tab, not the remembered one"
        );
    }
}

#[test]
fn clicking_a_space_with_a_finished_unseen_agent_focuses_that_tab() {
    let mut state = two_tab_state(AgentStatus::Working);
    let mut next = state.snapshot.as_deref().unwrap().clone();
    next.revision += 1;
    for agent in &mut next.agents {
        agent.agent_status = AgentStatus::Idle;
        agent.state_change_seq += 10;
    }
    let mut presented = surface();
    presented.projection_revision = next.revision;
    state.set_endpoint_snapshot(&ClientEndpointId::Local, Box::new(next));
    state.set_pane_surface(presented);
    state.compose(100, 40).expect("desktop shell");
    let status = state.snapshot.as_deref().unwrap().workspaces[0].agent_status;
    assert_eq!(status, AgentStatus::Done, "finished and unseen");
    let outcome = click_space(&mut state, "a");
    assert_eq!(
        pane_focus(focus_request(&outcome)).as_deref(),
        Some("pane-active")
    );
}

#[test]
fn clicking_an_idle_space_still_focuses_the_space_itself() {
    let mut state = two_tab_state(AgentStatus::Working);
    let outcome = click_space(&mut state, "b");
    assert!(matches!(
        focus_request(&outcome),
        Some(crate::api::schema::Method::WorkspaceFocus(target)) if target.workspace_id == "b"
    ));
}

#[test]
fn the_most_urgent_agent_of_a_space_wins_and_ties_go_to_the_newest_change() {
    let mut state = state_with(
        vec![workspace("a", "alpha", AgentStatus::Blocked, None)],
        vec![
            agent_for("working-new", "a", AgentStatus::Working, 9),
            agent_for("blocked-old", "a", AgentStatus::Blocked, 1),
            agent_for("blocked-new", "a", AgentStatus::Blocked, 5),
        ],
    );
    state.compose(100, 40).expect("desktop shell");
    let outcome = click_space(&mut state, "a");
    assert_eq!(
        pane_focus(focus_request(&outcome)).as_deref(),
        Some("blocked-new")
    );
}

#[test]
fn only_sidebar_clicks_jump_to_the_busy_tab() {
    let mut state = two_tab_state(AgentStatus::Working);
    let target = state.navigation_target(&ClientEndpointId::Local, "a");
    state.mode = ClientShellMode::Navigate;
    state.navigate_workspace_id = target;
    let outcome = state.handle_input_bytes(b"\r");
    assert!(matches!(
        focus_request(&outcome),
        Some(crate::api::schema::Method::WorkspaceFocus(target)) if target.workspace_id == "a"
    ));
}
