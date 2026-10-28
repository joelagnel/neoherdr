use super::*;

fn crowded_state() -> ClientShellState {
    let mut projected = snapshot();
    let workspace = projected.workspaces[0].clone();
    let tab = projected.tabs[0].clone();
    for index in 2..=30 {
        let mut next = workspace.clone();
        next.workspace_id = format!("ws_{index}");
        next.number = index;
        next.label = format!("space {index}");
        next.focused = false;
        next.active_tab_id = format!("tab_{index}");
        projected.workspaces.push(next);
        let mut next_tab = tab.clone();
        next_tab.tab_id = format!("tab_{index}");
        next_tab.workspace_id = format!("ws_{index}");
        next_tab.focused = false;
        projected.tabs.push(next_tab);
    }
    projected.agents = (0..20)
        .map(|index| ClientShellAgent {
            pane_id: format!("pane_{}", index + 1),
            workspace_id: "ws_1".into(),
            tab_id: "tab_1".into(),
            name: Some(format!("agent {index}")),
            display_agent: None,
            agent: Some("pi".into()),
            title: None,
            terminal_title: None,
            terminal_title_stripped: None,
            agent_status: AgentStatus::Idle,
            state_change_seq: 1,
            state_labels: Vec::new(),
            tokens: Vec::new(),
            focused: index == 0,
        })
        .collect();
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    state.compose(100, 40).expect("desktop shell");
    state
}

fn buttons(state: &ClientShellState, wanted: &[SidebarButton]) -> Vec<Rect> {
    wanted
        .iter()
        .map(|kind| {
            state
                .hits
                .sidebar_buttons
                .iter()
                .find(|(_, button)| button == kind)
                .map(|(rect, _)| *rect)
                .unwrap_or_else(|| panic!("missing {kind:?}"))
        })
        .collect()
}

fn click(state: &mut ClientShellState, rect: Rect) -> ClientShellInput {
    state.handle_raw_events(vec![RawInputEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: rect.x,
        row: rect.y,
        modifiers: KeyModifiers::NONE,
    })])
}

#[test]
fn spaces_header_has_two_scroll_buttons_and_agents_header_has_four() {
    let mut state = crowded_state();
    let [ws_up, ws_down] = buttons(
        &state,
        &[SidebarButton::WorkspaceUp, SidebarButton::WorkspaceDown],
    )[..] else {
        unreachable!()
    };
    assert_eq!(ws_up.y, ws_down.y);
    assert!(ws_up.right() <= ws_down.x);
    let spaces_row = state.hits.workspace_body.y.saturating_sub(2);
    assert_eq!(ws_up.y, spaces_row, "buttons share the spaces title row");

    let agent_buttons = buttons(
        &state,
        &[
            SidebarButton::AgentUp,
            SidebarButton::AgentDown,
            SidebarButton::SplitUp,
            SidebarButton::SplitDown,
        ],
    );
    assert!(agent_buttons
        .iter()
        .all(|rect| rect.y == agent_buttons[0].y));
    assert!(agent_buttons
        .windows(2)
        .all(|pair| pair[0].right() <= pair[1].x));
    assert!(agent_buttons
        .iter()
        .all(|rect| rect.right() <= state.hits.agent_sort_toggle.x));

    let frame = state.compose(100, 40).expect("frame");
    let buffer = frame.to_ratatui_buffer().expect("buffer");
    let glyphs = |rect: Rect| buffer[(rect.x, rect.y)].symbol().to_owned();
    assert_eq!(glyphs(ws_up), "▲");
    assert_eq!(glyphs(ws_down), "▼");
    assert_eq!(glyphs(agent_buttons[0]), "▲");
    assert_eq!(glyphs(agent_buttons[1]), "▼");
}

#[test]
fn scroll_buttons_move_each_list_and_clamp() {
    let mut state = crowded_state();
    assert!(state.hits.workspace_max_scroll > 0 && state.hits.agent_max_scroll > 0);
    let [ws_down, ws_up, ag_down, ag_up] = buttons(
        &state,
        &[
            SidebarButton::WorkspaceDown,
            SidebarButton::WorkspaceUp,
            SidebarButton::AgentDown,
            SidebarButton::AgentUp,
        ],
    )[..] else {
        unreachable!()
    };
    let outcome = click(&mut state, ws_down);
    assert!(outcome.repaint);
    assert!(state.workspace_scroll > 0);
    assert_eq!(state.agent_scroll, 0, "lists scroll independently");
    click(&mut state, ws_up);
    assert_eq!(state.workspace_scroll, 0);
    click(&mut state, ws_up);
    assert_eq!(state.workspace_scroll, 0);

    for _ in 0..40 {
        click(&mut state, ag_down);
    }
    assert_eq!(state.agent_scroll, state.hits.agent_max_scroll);
    click(&mut state, ag_up);
    assert!(state.agent_scroll < state.hits.agent_max_scroll);
}

#[test]
fn split_buttons_adjust_the_section_split_and_persist_it() {
    let mut state = crowded_state();
    let [up, down] = buttons(&state, &[SidebarButton::SplitUp, SidebarButton::SplitDown])[..]
    else {
        unreachable!()
    };
    let before = state.sidebar_section_split;
    click(&mut state, up);
    assert!(state.sidebar_section_split < before);
    assert!(state.sidebar_section_split_manual);
    click(&mut state, down);
    click(&mut state, down);
    assert!(state.sidebar_section_split > before);
    for _ in 0..40 {
        click(&mut state, down);
    }
    assert!((state.sidebar_section_split - 0.9).abs() < 1e-4);
    for _ in 0..40 {
        click(&mut state, up);
    }
    assert!((state.sidebar_section_split - 0.1).abs() < 1e-4);
}

#[test]
fn new_workspace_rides_the_spaces_title_and_rules_are_readable() {
    let mut state = crowded_state();
    let [ws_up] = buttons(&state, &[SidebarButton::WorkspaceUp])[..] else {
        unreachable!()
    };
    let header_y = ws_up.y;
    let new = state.hits.new_workspace;
    assert_eq!(
        new.y, header_y,
        "new workspace sits on the spaces title row"
    );
    assert!(new.right() <= ws_up.x, "left of the scroll buttons");
    let frame = state.compose(100, 40).expect("frame");
    let buffer = frame.to_ratatui_buffer().expect("buffer");
    let text = (new.x..new.right())
        .map(|x| buffer[(x, new.y)].symbol())
        .collect::<String>();
    assert!(text.contains("+ new"), "got {text:?}");
    let footer = (0..25)
        .map(|x| buffer[(x, state.hits.workspace_body.bottom())].symbol())
        .collect::<String>();
    assert!(
        !footer.contains("new"),
        "footer no longer has new: {footer:?}"
    );
    let outcome = click(&mut state, new);
    assert!(
        matches!(
            outcome.actions.as_slice(),
            [ClientShellAction::Endpoint { .. }]
        ),
        "clicking + new creates a workspace through the endpoint API"
    );
    let divider = state.hits.sidebar_section_divider;
    assert_eq!(
        buffer[(divider.x + 2, divider.y)].fg,
        state.config.palette.overlay0,
        "section rule is the lighter gray"
    );
}

#[test]
fn desktop_find_button_sits_next_to_collapse_and_opens_search() {
    let mut state = crowded_state();
    let [find] = buttons(&state, &[SidebarButton::Find])[..] else {
        unreachable!()
    };
    let collapse = state.hits.sidebar_toggle;
    assert_eq!(find.y, collapse.y);
    assert!(collapse.right() <= find.x);
    let frame = state.compose(100, 40).expect("frame");
    let buffer = frame.to_ratatui_buffer().expect("buffer");
    let text = (find.x..find.right())
        .map(|x| buffer[(x, find.y)].symbol())
        .collect::<String>();
    assert!(text.contains("Find"), "got {text:?}");
    click(&mut state, find);
    let Some(ClientShellOverlay::Navigator(navigator)) = state.overlay.as_ref() else {
        panic!("Find must open the navigator");
    };
    assert!(navigator.search_focused);
}

#[test]
fn space_rows_show_the_active_tab_name_in_readable_gray() {
    let mut projected = snapshot();
    projected.tabs[0].label = "logs".into();
    projected.tabs[0].custom_label = true;
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    let frame = state.compose(100, 30).expect("desktop shell");
    let buffer = frame.to_ratatui_buffer().expect("buffer");
    let row = state.hits.workspaces[0].rect.y + 1;
    let text = (0..26)
        .map(|x| buffer[(x, row)].symbol())
        .collect::<String>();
    assert!(
        text.contains("logs"),
        "tab name on the branch row: {text:?}"
    );
    assert!(text.contains("main"), "branch still shown: {text:?}");
    let x = (0..26)
        .find(|x| buffer[(*x, row)].symbol() == "l")
        .expect("tab label cell");
    assert_eq!(buffer[(x, row)].fg, state.config.palette.subtext0);
}

#[test]
fn space_rows_lead_with_the_bare_tab_name_then_the_branch() {
    let mut projected = snapshot();
    projected.workspaces[0].branch = Some("a-rather-long-feature-branch-name".into());
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    let frame = state.compose(100, 30).expect("desktop shell");
    let buffer = frame.to_ratatui_buffer().expect("buffer");
    let row = state.hits.workspaces[0].rect.y + 1;
    let text = (0..25)
        .map(|x| buffer[(x, row)].symbol())
        .collect::<String>();
    assert!(!text.contains("tab"), "no 'tab' word: {text:?}");
    assert!(
        text.trim_start().starts_with("1 → a-rather"),
        "got {text:?}"
    );
    assert!(text.contains('…'), "long branch truncated: {text:?}");
}

#[test]
fn space_rows_show_the_tab_position_at_the_right_edge() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    let mut projected = snapshot();
    let mut second = projected.tabs[0].clone();
    second.tab_id = "tab_2".into();
    second.number = 2;
    second.focused = false;
    projected.tabs.push(second);
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    let frame = state.compose(100, 30).expect("desktop shell");
    let buffer = frame.to_ratatui_buffer().expect("buffer");
    let row = state.hits.workspaces[0].rect.y;
    let text = (0..25)
        .map(|x| buffer[(x, row)].symbol())
        .collect::<String>();
    assert!(text.contains("client-shell"), "{text:?}");
    assert!(
        text.trim_end().ends_with("1/2"),
        "tab position on the right: {text:?}"
    );
}
