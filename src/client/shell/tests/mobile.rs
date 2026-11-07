use super::*;

#[test]
fn navigate_update_status_uses_released_desktop_and_mobile_placement() {
    let mut config = ClientShellConfig::from_config(&Config::default());
    config.tab_bar_position = crate::config::TabBarPositionConfig::Bottom;
    config.hide_tab_bar_when_single_tab = false;
    let mut state = ClientShellState::new(config);
    let mut endpoint_snapshot = snapshot();
    endpoint_snapshot.update_available = Some("0.8.3".into());
    state.set_snapshot(Box::new(endpoint_snapshot));
    state.set_pane_surface(surface());
    state.mode = ClientShellMode::Navigate;

    let bottom = state.compose(106, 30).expect("bottom-tab update shell");
    let row_text = |frame: &FrameData, row: u16| {
        let width = usize::from(frame.width);
        let start = usize::from(row) * width;
        frame.cells[start..start + width]
            .iter()
            .map(|cell| cell.symbol.as_str())
            .collect::<String>()
    };
    assert!(row_text(&bottom, 29).contains("update ready"));
    assert!(!row_text(&bottom, 28).contains("update ready"));
    assert!(state.hits.tabs.is_empty());
    assert!(state.hits.new_tab.is_empty());
    assert!(state.hits.tab_scroll_left.is_empty());
    assert!(state.hits.tab_scroll_right.is_empty());

    state.config.tab_bar_position = crate::config::TabBarPositionConfig::Top;
    state.visible_notification = Some(ClientVisibleNotification {
        endpoint_id: ClientEndpointId::Local,
        event: SemanticNotification {
            kind: SemanticNotificationKind::Custom,
            title: "bottom notification".into(),
            body: None,
            sound: None,
            agent: None,
            workspace_id: None,
            tab_id: None,
            pane_id: None,
            position: Some(crate::config::ToastHerdrPosition::BottomRight),
        },
        deadline: std::time::Instant::now(),
    });
    let top = state.compose(106, 30).expect("top-tab update shell");
    assert!(row_text(&top, 29).contains("update ready"));

    let mobile = state.compose(44, 30).expect("mobile update shell");
    let mobile_text = mobile
        .cells
        .iter()
        .map(|cell| cell.symbol.as_str())
        .collect::<String>();
    assert!(mobile_text.contains("update ready"));
}

#[test]
fn mobile_layout_reserves_only_client_header() {
    let config = ClientShellConfig::from_config(&Config::default());
    let state = ClientShellState::new(config);
    let layout = state.layout(44, 20);
    assert_eq!(layout.mobile_header, Rect::new(0, 0, 44, 2));
    assert_eq!(layout.pane_surface, Rect::new(0, 2, 44, 18));
    assert_eq!(
        state.surface_size(44, 20),
        ClientSurfaceSize { cols: 44, rows: 18 }
    );
}

#[test]
fn mobile_switcher_can_activate_an_online_saved_machine() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    let profile = crate::client::endpoint::SavedSshEndpoint {
        id: crate::client::endpoint::ProfileId::parse("0123456789abcdef0123456789abcdef").unwrap(),
        label: "Build".into(),
        target: "build".into(),
        session: "agents".into(),
        enabled: true,
    };
    let endpoint_id = ClientEndpointId::Ssh(profile.id.clone());
    state.set_endpoint_catalog(&[profile]);
    state.set_endpoint_status(&endpoint_id, ClientEndpointStatus::Online);
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    let mut remote = snapshot();
    remote.boot_id = "remote-boot".into();
    state.set_endpoint_snapshot(&endpoint_id, Box::new(remote));
    state.mode = ClientShellMode::Navigate;

    let frame = state.compose(44, 30).expect("mobile machine switcher");
    let text = frame
        .cells
        .iter()
        .map(|cell| cell.symbol.as_str())
        .collect::<String>();
    assert!(text.contains("machines"));
    assert!(text.contains("Build"));
    let machine = state
        .hits
        .mobile_targets
        .iter()
        .find_map(|(rect, target)| {
            matches!(target, ClientMobileTarget::Machine(id) if id == &endpoint_id).then_some(*rect)
        })
        .expect("saved machine target");
    let outcome = state.handle_raw_events(vec![RawInputEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: machine.x,
        row: machine.y,
        modifiers: KeyModifiers::empty(),
    })]);
    assert!(matches!(
        outcome.actions.as_slice(),
        [ClientShellAction::ActivateEndpoint {
            endpoint_id: activated,
            target: None,
        }] if activated == &endpoint_id
    ));

    // Local is still projected until the remote handoff completes.
    state.mode = ClientShellMode::Navigate;
    state.compose(44, 30).unwrap();
    let local = state
        .hits
        .mobile_targets
        .iter()
        .find_map(|(rect, target)| {
            matches!(target, ClientMobileTarget::Machine(ClientEndpointId::Local)).then_some(*rect)
        })
        .unwrap();
    let outcome = state.handle_raw_events(vec![RawInputEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: local.x,
        row: local.y,
        modifiers: KeyModifiers::empty(),
    })]);
    assert!(matches!(
        outcome.actions.as_slice(),
        [ClientShellAction::ActivateEndpoint {
            endpoint_id: ClientEndpointId::Local,
            target: None,
        }]
    ));

    state.set_endpoint_status(&endpoint_id, ClientEndpointStatus::Online);
    assert!(state.activate_endpoint_projection(&endpoint_id));
    let mut remote_surface = surface();
    remote_surface.boot_id = "remote-boot".into();
    state.set_pane_surface(remote_surface);
    state.mark_endpoint_disconnected(&endpoint_id);
    state.receive_endpoint_unavailable("network lost".into());
    state.mode = ClientShellMode::Navigate;
    let stale = state.compose(44, 30).expect("stale mobile switcher");
    let text = stale
        .cells
        .iter()
        .map(|cell| cell.symbol.as_str())
        .collect::<String>();
    assert!(text.contains("reconnecting"), "frame: {text}");
    assert!(text.contains("network lost"), "frame: {text}");
}

#[test]
fn mobile_shell_controls_remain_clickable_when_pane_mouse_capture_is_disabled() {
    let mut config = ClientShellConfig::from_config(&Config::default());
    config.mouse_capture = false;
    let mut state = ClientShellState::new(config);
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    open_mobile_menu(&mut state, 44, 20);
    assert!(!state.hits.mobile_switch.is_empty());
    let switch = state.hits.mobile_switch;
    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: switch.x,
        row: switch.y,
        modifiers: KeyModifiers::empty(),
    })]);
    assert_eq!(state.mode, ClientShellMode::Navigate);
    state.compose(44, 20).expect("mobile switcher");
    assert!(!state.hits.mobile_close.is_empty());
    assert!(!state.hits.mobile_targets.is_empty());
}

#[test]
fn mobile_header_and_switcher_render_released_sections_and_stable_targets() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    let mut projected = snapshot();
    projected.agents.push(ClientShellAgent {
        pane_id: "pane_1".into(),
        workspace_id: "ws_1".into(),
        tab_id: "tab_1".into(),
        name: Some("pi".into()),
        display_agent: Some("pi".into()),
        agent: Some("pi".into()),
        title: None,
        terminal_title: None,
        terminal_title_stripped: None,
        agent_status: AgentStatus::Blocked,
        state_change_seq: 1,
        state_labels: vec![("blocked".into(), "waiting".into())],
        tokens: Vec::new(),
        focused: true,
    });
    projected.workspaces[0].agent_status = AgentStatus::Blocked;
    state.set_snapshot(Box::new(projected));
    let mut projected_surface = surface();
    for cell in &mut projected_surface.frame.cells {
        cell.symbol = "X".to_owned();
    }
    state.set_pane_surface(projected_surface);

    let header = state.compose(44, 20).expect("mobile header");
    let header_text = header
        .cells
        .iter()
        .map(|cell| cell.symbol.as_str())
        .collect::<String>();
    assert!(header_text.contains("client-shell"));
    assert!(header_text.contains("tab 1"));
    assert!(header_text.contains("blocked"));
    assert!(header_text.contains("menu"));
    assert_eq!(state.hits.mobile_menu, Rect::new(36, 0, 8, 2));

    let click = |rect: Rect| {
        RawInputEvent::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: rect.x,
            row: rect.y,
            modifiers: KeyModifiers::empty(),
        })
    };
    open_mobile_menu(&mut state, 44, 20);
    let opened = state.handle_raw_events(vec![click(state.hits.mobile_switch)]);
    assert!(opened.repaint);
    assert_eq!(state.mode, ClientShellMode::Navigate);
    let switcher = state.compose(44, 34).expect("mobile switcher");
    let switcher_text = switcher
        .cells
        .chunks(switcher.width as usize)
        .map(|row| {
            row.iter()
                .map(|cell| cell.symbol.as_str())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !switcher_text.contains('X'),
        "switcher must clear the pane surface"
    );
    for expected in [
        "switch",
        "close",
        "agents",
        "spaces",
        "+ new",
        "tabs",
        "+ new tab",
        "menu",
        "settings",
        "detach",
    ] {
        assert!(switcher_text.contains(expected), "missing {expected}");
    }
    let workspace_hit = state
        .hits
        .mobile_targets
        .iter()
        .find_map(|(rect, target)| {
            matches!(
                target,
                ClientMobileTarget::Workspace { workspace_id, .. } if workspace_id == "ws_1"
            )
            .then_some(*rect)
        })
        .expect("workspace hit");
    let focused = state.handle_raw_events(vec![click(workspace_hit)]);
    assert_eq!(state.mode, ClientShellMode::Terminal);
    assert!(state.navigate_workspace_id.is_none());
    assert!(focused.actions.iter().any(|action| matches!(
        action,
        ClientShellAction::Endpoint { request, .. }
            if matches!(
                &request.method,
                crate::api::schema::Method::WorkspaceFocus(params)
                    if params.workspace_id == "ws_1"
            )
    )));

    open_mobile_menu(&mut state, 44, 20);
    state.handle_raw_events(vec![click(state.hits.mobile_switch)]);
    state.compose(44, 20).expect("agent switcher");
    let agent_hit = state
        .hits
        .mobile_targets
        .iter()
        .find_map(|(rect, target)| {
            matches!(
                target,
                ClientMobileTarget::Agent { pane_id, .. } if pane_id == "pane_1"
            )
            .then_some(*rect)
        })
        .expect("agent hit");
    let focused = state.handle_raw_events(vec![click(agent_hit)]);
    assert!(focused.actions.iter().any(|action| matches!(
        action,
        ClientShellAction::Endpoint { request, .. }
            if matches!(
                &request.method,
                crate::api::schema::Method::PaneFocus(params)
                    if params.pane_id == "pane_1"
            )
    )));

    open_mobile_menu(&mut state, 44, 20);
    state.handle_raw_events(vec![click(state.hits.mobile_switch)]);
    state.compose(44, 20).expect("tab switcher");
    let tab_hit = state
        .hits
        .mobile_targets
        .iter()
        .find_map(|(rect, target)| {
            matches!(
                target,
                ClientMobileTarget::Tab {
                    endpoint_id,
                    tab_id,
                } if endpoint_id.is_local() && tab_id == "tab_1"
            )
            .then_some(*rect)
        })
        .expect("tab hit");
    let focused = state.handle_raw_events(vec![click(tab_hit)]);
    assert!(focused.actions.iter().any(|action| matches!(
        action,
        ClientShellAction::Endpoint { request, .. }
            if matches!(
                &request.method,
                crate::api::schema::Method::TabFocus(params)
                    if params.tab_id == "tab_1"
            )
    )));
}

#[test]
fn mobile_background_workspace_uses_its_own_active_tab_status() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    let mut projected = snapshot();
    projected.tabs.push(ClientShellTab {
        tab_id: "tab_7".into(),
        workspace_id: "ws_1".into(),
        number: 7,
        label: "logs".into(),
        custom_label: true,
        zoomed: false,
        focused: false,
        agent_status: AgentStatus::Idle,
    });
    projected.workspaces.push(ClientShellWorkspace {
        workspace_id: "ws_2".into(),
        active_tab_id: "tab_3".into(),
        new_workspace_cwd: "/feature".into(),
        number: 2,
        label: "background".into(),
        custom_label: true,
        branch: Some("feature".into()),
        git_ahead_behind: None,
        tokens: Vec::new(),
        worktree: None,
        focused: false,
        agent_status: AgentStatus::Idle,
    });
    for (number, tab_id, label) in [(1, "tab_2", "one"), (7, "tab_3", "two")] {
        projected.tabs.push(ClientShellTab {
            tab_id: tab_id.into(),
            workspace_id: "ws_2".into(),
            number,
            label: label.into(),
            custom_label: true,
            zoomed: false,
            focused: false,
            agent_status: AgentStatus::Idle,
        });
    }
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    state.mode = ClientShellMode::Navigate;
    state.navigate_workspace_id = state.navigation_target(&ClientEndpointId::Local, "ws_2");
    let frame = state.compose(44, 20).expect("mobile switcher");
    let text = frame
        .cells
        .chunks(frame.width as usize)
        .map(|row| {
            row.iter()
                .map(|cell| cell.symbol.as_str())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("two → feature"), "{text}");
    assert!(text.contains("2/2"), "{text}");
    assert!(text.contains("2 · logs"), "{text}");
    assert!(!text.contains("7 · logs"), "{text}");
}

#[test]
fn mobile_switcher_create_and_menu_rows_reuse_client_actions() {
    let mut config = ClientShellConfig::from_config(&Config::default());
    config.prompt_new_workspace_name = true;
    config.prompt_new_tab_name = true;
    let mut state = ClientShellState::new(config);
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    let click = |rect: Rect| {
        RawInputEvent::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: rect.x,
            row: rect.y,
            modifiers: KeyModifiers::empty(),
        })
    };

    state.mode = ClientShellMode::Navigate;
    state.compose(44, 20).expect("mobile create switcher");
    let new_tab = state
        .hits
        .mobile_targets
        .iter()
        .find_map(|(rect, target)| matches!(target, ClientMobileTarget::NewTab).then_some(*rect))
        .expect("new tab hit");
    state.handle_raw_events(vec![click(new_tab)]);
    assert!(matches!(
        state.overlay,
        Some(ClientShellOverlay::Rename(ClientRenameOverlay {
            target: ClientRenameTarget::NewTab { .. },
            ..
        }))
    ));
    state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::Esc,
        KeyModifiers::empty(),
    ))]);
    assert!(state.overlay.is_none());
    assert_eq!(state.mode, ClientShellMode::Terminal);

    state.mode = ClientShellMode::Navigate;
    state.compose(44, 20).expect("mobile workspace switcher");
    let new_workspace = state
        .hits
        .mobile_targets
        .iter()
        .find_map(|(rect, target)| {
            matches!(target, ClientMobileTarget::NewWorkspace).then_some(*rect)
        })
        .expect("new workspace hit");
    state.handle_raw_events(vec![click(new_workspace)]);
    assert!(matches!(
        state.overlay,
        Some(ClientShellOverlay::Rename(ClientRenameOverlay {
            target: ClientRenameTarget::NewWorkspace { .. },
            ..
        }))
    ));
    state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::Esc,
        KeyModifiers::empty(),
    ))]);
    assert!(state.overlay.is_none());
    assert_eq!(state.mode, ClientShellMode::Terminal);

    state.mode = ClientShellMode::Navigate;
    state.compose(44, 20).expect("mobile menu switcher");
    let settings = state
        .hits
        .mobile_targets
        .iter()
        .find_map(|(rect, target)| matches!(target, ClientMobileTarget::Menu(0)).then_some(*rect))
        .expect("settings hit");
    state.handle_raw_events(vec![click(settings)]);
    assert!(matches!(
        state.overlay,
        Some(ClientShellOverlay::Settings(_))
    ));
    state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::Esc,
        KeyModifiers::empty(),
    ))]);
    assert!(state.overlay.is_none());
    assert_eq!(state.mode, ClientShellMode::Terminal);
}

#[test]
fn mobile_menu_keeps_inert_notes_open_and_cancel_without_workspace_in_navigate() {
    let mut source_config = Config::default();
    source_config.ui.prompt_new_workspace_name = true;
    let config = ClientShellConfig::from_config(&source_config);
    let mut projected = snapshot();
    projected.latest_release_notes_available = true;
    projected.release_notes = None;
    let mut state = ClientShellState::new(config);
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    state.mode = ClientShellMode::Navigate;
    state.compose(44, 20).expect("mobile switcher");
    let inert_notes = state
        .hits
        .mobile_targets
        .iter()
        .find_map(|(rect, target)| matches!(target, ClientMobileTarget::Menu(3)).then_some(*rect))
        .expect("what's new row");
    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: inert_notes.x,
        row: inert_notes.y,
        modifiers: KeyModifiers::empty(),
    })]);
    assert_eq!(state.mode, ClientShellMode::Navigate);
    assert!(state.overlay.is_none());
    assert!(!state.mobile_switcher_suspended);

    let mut empty = snapshot();
    empty.focused_workspace_id = None;
    empty.focused_tab_id = None;
    empty.focused_pane_id = None;
    empty.workspaces.clear();
    empty.tabs.clear();
    empty.panes.clear();
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&source_config));
    state.set_snapshot(Box::new(empty));
    state.set_pane_surface(surface());
    state.mode = ClientShellMode::Navigate;
    state.compose(44, 20).expect("empty mobile switcher");
    let new_workspace = state
        .hits
        .mobile_targets
        .iter()
        .find_map(|(rect, target)| {
            matches!(target, ClientMobileTarget::NewWorkspace).then_some(*rect)
        })
        .expect("new workspace row");
    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: new_workspace.x,
        row: new_workspace.y,
        modifiers: KeyModifiers::empty(),
    })]);
    assert!(matches!(state.overlay, Some(ClientShellOverlay::Rename(_))));
    state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::Esc,
        KeyModifiers::empty(),
    ))]);
    assert_eq!(state.mode, ClientShellMode::Navigate);
}

#[test]
fn mobile_previous_workspace_action_wraps_across_expanded_entries() {
    let mut projected = snapshot();
    for index in 2..=3 {
        projected.workspaces.push(ClientShellWorkspace {
            workspace_id: format!("ws_{index}"),
            active_tab_id: format!("tab_{index}"),
            new_workspace_cwd: "/tmp".into(),
            number: index,
            label: format!("workspace-{index}"),
            custom_label: true,
            branch: None,
            git_ahead_behind: None,
            tokens: Vec::new(),
            worktree: None,
            focused: false,
            agent_status: AgentStatus::Idle,
        });
    }
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    state.compose(44, 20).expect("mobile layout");
    let mut outcome = ClientShellInput::default();
    state.record_binding(
        crate::input::KeybindMatch::Action(crate::input::KeybindAction::PreviousWorkspace),
        &mut outcome,
    );
    assert!(outcome.actions.iter().any(|action| matches!(
        action,
        ClientShellAction::Endpoint { request, .. }
            if matches!(
                &request.method,
                crate::api::schema::Method::WorkspaceFocus(target)
                    if target.workspace_id == "ws_3"
            )
    )));
}

#[test]
fn mobile_switcher_scroll_close_and_width_transition_clear_mobile_hits() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    let mut projected = snapshot();
    for index in 2..=8 {
        projected.workspaces.push(ClientShellWorkspace {
            workspace_id: format!("ws_{index}"),
            active_tab_id: format!("tab_{index}"),
            new_workspace_cwd: "/tmp".into(),
            number: index,
            label: format!("workspace-{index}"),
            custom_label: true,
            branch: None,
            git_ahead_behind: None,
            tokens: Vec::new(),
            worktree: None,
            focused: false,
            agent_status: AgentStatus::Idle,
        });
    }
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    open_mobile_menu(&mut state, 44, 10);
    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: state.hits.mobile_switch.x,
        row: state.hits.mobile_switch.y,
        modifiers: KeyModifiers::empty(),
    })]);
    state.compose(44, 10).expect("mobile switcher");
    let wheel = state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 20,
        row: 8,
        modifiers: KeyModifiers::empty(),
    })]);
    assert!(wheel.repaint);
    assert_eq!(state.mobile_switcher_scroll, 2);
    state.compose(44, 10).expect("wheel position stays stable");
    assert_eq!(state.mobile_switcher_scroll, 2);
    for _ in 0..7 {
        state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
            KeyCode::Down,
            KeyModifiers::empty(),
        ))]);
    }
    state.compose(44, 10).expect("revealed mobile selection");
    assert_eq!(
        state.navigate_workspace_id,
        state.navigation_target(&ClientEndpointId::Local, "ws_8")
    );
    assert!(state.mobile_switcher_scroll > 2);
    assert!(state.hits.mobile_targets.iter().any(|(_, target)| {
        matches!(
            target,
            ClientMobileTarget::Workspace { workspace_id, .. } if workspace_id == "ws_8"
        )
    }));
    let close = state.hits.mobile_close;
    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: close.x,
        row: close.y,
        modifiers: KeyModifiers::empty(),
    })]);
    assert_eq!(state.mode, ClientShellMode::Terminal);
    assert!(state.navigate_workspace_id.is_none());

    state.compose(80, 20).expect("desktop transition");
    assert!(state.hits.mobile_switch.is_empty() && state.hits.mobile_menu.is_empty());
    assert!(state.hits.mobile_close.is_empty());
    assert!(state.hits.mobile_targets.is_empty());

    state.mode = ClientShellMode::Navigate;
    let short = state.compose(44, 2).expect("short mobile switcher");
    assert_eq!(short.cells[0].symbol, "─");
    assert!(state.hits.mobile_close.is_empty());
    assert!(state.hits.mobile_targets.is_empty());
}

fn mobile_row_text(frame: &FrameData, row: u16) -> String {
    let width = usize::from(frame.width);
    let start = usize::from(row) * width;
    frame.cells[start..start + width]
        .iter()
        .map(|cell| cell.symbol.as_str())
        .collect()
}

fn type_text(state: &mut ClientShellState, text: &str) {
    let events = text
        .chars()
        .map(|ch| {
            RawInputEvent::Key(crate::input::TerminalKey::new(
                KeyCode::Char(ch),
                KeyModifiers::empty(),
            ))
        })
        .collect();
    state.handle_raw_events(events);
}

fn mobile_click(rect: Rect) -> RawInputEvent {
    RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: rect.x,
        row: rect.y,
        modifiers: KeyModifiers::empty(),
    })
}

#[test]
fn mobile_switcher_lists_spaces_before_agents() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    let mut projected = snapshot();
    projected.agents.push(ClientShellAgent {
        pane_id: "pane_1".into(),
        workspace_id: "ws_1".into(),
        tab_id: "tab_1".into(),
        name: Some("pi".into()),
        display_agent: Some("pi".into()),
        agent: Some("pi".into()),
        title: None,
        terminal_title: None,
        terminal_title_stripped: None,
        agent_status: AgentStatus::Idle,
        state_change_seq: 1,
        state_labels: Vec::new(),
        tokens: Vec::new(),
        focused: true,
    });
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    state.mode = ClientShellMode::Navigate;
    let frame = state.compose(44, 30).expect("mobile switcher");
    let rows = (0..frame.height)
        .map(|row| mobile_row_text(&frame, row))
        .collect::<Vec<_>>();
    let spaces = rows.iter().position(|row| row.contains("spaces")).unwrap();
    let agents = rows.iter().position(|row| row.contains("agents")).unwrap();
    let tabs = rows.iter().position(|row| row.contains("tabs")).unwrap();
    assert!(spaces < agents, "spaces must come before agents");
    assert!(agents < tabs, "agents stay ahead of tabs");
    let hit_y = |wanted: fn(&ClientMobileTarget) -> bool| {
        state
            .hits
            .mobile_targets
            .iter()
            .find(|(_, target)| wanted(target))
            .map(|(rect, _)| rect.y)
            .unwrap()
    };
    assert!(
        hit_y(|target| matches!(target, ClientMobileTarget::Workspace { .. }))
            < hit_y(|target| matches!(target, ClientMobileTarget::Agent { .. }))
    );
}

#[test]
fn mobile_header_has_no_collapse_button() {
    for sidebar_collapsed in [false, true] {
        let mut state = mobile_state();
        state.sidebar_collapsed = sidebar_collapsed;
        let frame = state.compose(60, 20).expect("mobile header");
        let header = format!(
            "{}{}",
            mobile_row_text(&frame, 0),
            mobile_row_text(&frame, 1)
        );
        for label in ["Collapse", "Expand"] {
            assert!(
                !header.contains(label),
                "{label} does nothing visible on mobile"
            );
        }
    }
}

#[test]
fn mobile_switcher_header_has_a_find_button_next_to_close() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    open_mobile_menu(&mut state, 60, 24);
    state.handle_raw_events(vec![mobile_click(state.hits.mobile_switch)]);
    assert_eq!(state.mode, ClientShellMode::Navigate);
    let frame = state.compose(60, 24).expect("mobile switcher");
    let header = format!(
        "{}{}",
        mobile_row_text(&frame, 0),
        mobile_row_text(&frame, 1)
    );
    assert!(
        header.contains("Find"),
        "Find in the switcher header: {header:?}"
    );
    assert!(header.contains("close"));
    let (find, close) = (state.hits.mobile_find, state.hits.mobile_close);
    assert!(!find.is_empty() && !close.is_empty());
    assert_eq!(find.right(), close.x, "Find sits right next to close");

    let outcome = state.handle_raw_events(vec![mobile_click(find)]);
    assert!(outcome.repaint);
    let Some(ClientShellOverlay::Navigator(navigator)) = state.overlay.as_ref() else {
        panic!("Find must open the navigator search");
    };
    assert!(navigator.search_focused);
}

#[test]
fn mobile_find_button_opens_live_filtering_search() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    open_mobile_menu(&mut state, 60, 24);
    let outcome = state.handle_raw_events(vec![mobile_click(state.hits.mobile_find)]);
    assert!(outcome.repaint);
    let Some(ClientShellOverlay::Navigator(navigator)) = state.overlay.as_ref() else {
        panic!("search must open the navigator overlay");
    };
    assert!(navigator.search_focused);
    type_text(&mut state, "client");
    let frame = state.compose(60, 24).expect("search frame");
    // Rows 0-1 are the mobile header, which always shows the workspace name.
    let text = (2..frame.height)
        .map(|row| mobile_row_text(&frame, row))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("/ client"), "typed query must be visible");
    assert!(
        text.contains("client-shell"),
        "matching space must be listed as you type"
    );
    type_text(&mut state, "zzzz");
    let frame = state.compose(60, 24).expect("search frame");
    let text = (0..frame.height)
        .map(|row| mobile_row_text(&frame, row))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !text.contains("client-shell ·"),
        "non-matching rows disappear"
    );
}

#[test]
fn unfocused_tab_names_use_the_readable_gray_in_the_tab_bar() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    let mut projected = snapshot();
    let mut second = projected.tabs[0].clone();
    second.tab_id = "tab_2".into();
    second.number = 2;
    second.label = "logs".into();
    second.custom_label = true;
    second.focused = false;
    projected.tabs.push(second);
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    let frame = state.compose(100, 24).expect("desktop shell");
    let buffer = frame.to_ratatui_buffer().expect("buffer");
    let (rect, _) = state
        .hits
        .tabs
        .iter()
        .find(|(_, tab_id)| tab_id == "tab_2")
        .expect("unfocused tab hit");
    let label_x = (rect.x..rect.right())
        .find(|x| buffer[(*x, rect.y)].symbol() == "l")
        .expect("tab label");
    assert_eq!(
        buffer[(label_x, rect.y)].fg,
        state.config.palette.subtext0,
        "tab names use the brighter gray, not the dim overlay colors"
    );
}

#[test]
fn mobile_switcher_rows_follow_the_reference_layout() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    let mut projected = snapshot();
    projected.workspaces[0].branch =
        Some("a-rather-long-feature-branch-name-that-keeps-going-and-going-until-it-cannot".into());
    let mut second = projected.tabs[0].clone();
    second.tab_id = "tab_2".into();
    second.number = 2;
    second.label = "logs".into();
    second.custom_label = true;
    second.focused = false;
    projected.tabs.push(second);
    projected.agents.push(ClientShellAgent {
        pane_id: "pane_1".into(),
        workspace_id: "ws_1".into(),
        tab_id: "tab_2".into(),
        name: Some("pi".into()),
        display_agent: Some("pi".into()),
        agent: Some("pi".into()),
        title: None,
        terminal_title: None,
        terminal_title_stripped: None,
        agent_status: AgentStatus::Idle,
        state_change_seq: 1,
        state_labels: Vec::new(),
        tokens: Vec::new(),
        focused: true,
    });
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    let gray = state.config.palette.subtext0;

    let header = state.compose(60, 40).expect("mobile header");
    let row0 = mobile_row_text(&header, 0);
    assert!(
        row0.contains("tab 1 · 1/2"),
        "top bar keeps the word tab: {row0:?}"
    );

    state.mode = ClientShellMode::Navigate;
    let frame = state.compose(60, 40).expect("mobile switcher");
    let buffer = frame.to_ratatui_buffer().expect("buffer");
    let rows = (0..frame.height)
        .map(|row| mobile_row_text(&frame, row))
        .collect::<Vec<_>>();
    let find = |needle: &str| {
        rows.iter()
            .position(|row| row.contains(needle))
            .unwrap_or_else(|| panic!("{needle:?} missing in {rows:#?}"))
    };
    for title in ["spaces · Local", "agents · Local"] {
        let at = find(title);
        assert!(
            rows[at + 1].trim().chars().all(|c| c == '─') && rows[at + 1].contains("────"),
            "rule under {title}: {:?}",
            rows[at + 1]
        );
    }
    assert!(find("spaces · Local") < find("agents · Local"));
    assert!(
        rows.iter().all(|row| !row.contains("Local · ")),
        "machine prefix dropped"
    );
    // Rules are readable, titles are not underlined, and "+ new" rides the spaces title.
    let overlay = state.config.palette.overlay0;
    // A rule also sits above the agents title so it is not smooshed against the last space.
    let rule_rows = [
        2,
        find("spaces · Local") + 1,
        find("agents · Local") - 1,
        find("agents · Local") + 1,
    ];
    for row in rule_rows {
        let at = (0..frame.width)
            .find(|x| buffer[(*x, row as u16)].symbol() == "─")
            .unwrap_or_else(|| panic!("rule glyph on row {row}"));
        assert_eq!(
            buffer[(at, row as u16)].fg,
            overlay,
            "rule color on row {row}"
        );
    }
    for title in ["spaces · Local", "agents · Local"] {
        let row = find(title) as u16;
        let x = rows[usize::from(row)]
            .chars()
            .position(|c| c == 's' || c == 'a')
            .unwrap() as u16;
        assert!(
            !buffer[(x, row)]
                .modifier
                .contains(ratatui::style::Modifier::UNDERLINED),
            "{title} title is not underlined"
        );
    }
    assert!(!rows.iter().any(|row| row.contains("+ new workspace")));
    let title_row = find("spaces · Local");
    assert!(
        rows[title_row].trim_end().ends_with("+ new"),
        "+ new on the right of the spaces title: {:?}",
        rows[title_row]
    );
    let new_hit = state
        .hits
        .mobile_targets
        .iter()
        .find_map(|(rect, target)| {
            matches!(target, ClientMobileTarget::NewWorkspace).then_some(*rect)
        })
        .expect("new workspace hit");
    assert_eq!(new_hit.y as usize, title_row);
    assert_eq!(new_hit.height, 1);
    assert!(
        new_hit.width < 12,
        "only the button is clickable, not the whole title"
    );

    let hit = |wanted: fn(&ClientMobileTarget) -> bool| {
        state
            .hits
            .mobile_targets
            .iter()
            .find(|(_, target)| wanted(target))
            .map(|(rect, _)| *rect)
            .expect("row hit")
    };
    let space = hit(|t| matches!(t, ClientMobileTarget::Workspace { .. }));
    assert_eq!(space.height, 2, "two lines, no blank or bar row");
    let line1 = &rows[usize::from(space.y)];
    let line2 = &rows[usize::from(space.y) + 1];
    assert!(
        !line1.contains('▸'),
        "the shade marks the current space, no marker: {line1:?}"
    );
    assert!(line1.contains("client-shell"), "{line1:?}");
    assert!(
        line1.trim_end().ends_with("1/2"),
        "tab position on the right: {line1:?}"
    );
    assert!(
        line2.trim_start().starts_with("1 → a-rather"),
        "tab name first, then the branch: {line2:?}"
    );
    assert!(line2.contains('…'), "long branch truncated: {line2:?}");
    assert!(
        !line2.contains("tab"),
        "no 'tab' word in the list: {line2:?}"
    );
    let x = line2.chars().position(|c| c == '1').unwrap() as u16;
    assert_eq!(buffer[(x, space.y + 1)].fg, gray, "tab name gray");

    let agent = hit(|t| matches!(t, ClientMobileTarget::Agent { .. }));
    assert_eq!(agent.height, 2);
    let line1 = &rows[usize::from(agent.y)];
    let line2 = &rows[usize::from(agent.y) + 1];
    assert!(
        line1.trim_end().ends_with("idle"),
        "state on the right: {line1:?}"
    );
    assert!(line2.trim_start().starts_with("logs · pi"), "{line2:?}");

    let tabs = find("tabs");
    for row in &rows[tabs..] {
        assert!(
            !row.contains("tab 1") || row.trim().starts_with("1"),
            "{row:?}"
        );
    }
}

#[test]
fn mobile_rows_alternate_their_background_instead_of_using_separators() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    let mut projected = snapshot();
    projected.focused_workspace_id = None;
    projected.workspaces[0].focused = false;
    let template = projected.workspaces[0].clone();
    for index in 2..=4 {
        let mut next = template.clone();
        next.workspace_id = format!("ws_{index}");
        next.label = format!("space {index}");
        projected.workspaces.push(next);
    }
    for index in 1..=4 {
        projected.agents.push(ClientShellAgent {
            pane_id: format!("pane_{index}"),
            workspace_id: "ws_1".into(),
            tab_id: "tab_1".into(),
            name: Some(format!("agent {index}")),
            display_agent: Some("pi".into()),
            agent: Some("pi".into()),
            title: None,
            terminal_title: None,
            terminal_title_stripped: None,
            agent_status: AgentStatus::Idle,
            state_change_seq: 1,
            state_labels: Vec::new(),
            tokens: Vec::new(),
            focused: false,
        });
    }
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    state.mode = ClientShellMode::Navigate;
    let frame = state.compose(60, 60).expect("mobile switcher");
    let buffer = frame.to_ratatui_buffer().expect("buffer");
    for kind in ["workspace", "agent"] {
        let mut rects = state
            .hits
            .mobile_targets
            .iter()
            .filter(|(_, target)| match kind {
                "workspace" => matches!(target, ClientMobileTarget::Workspace { .. }),
                _ => matches!(target, ClientMobileTarget::Agent { .. }),
            })
            .map(|(rect, _)| *rect)
            .collect::<Vec<_>>();
        rects.sort_by_key(|rect| rect.y);
        assert!(rects.len() >= 4, "{kind} rows: {}", rects.len());
        assert!(
            rects.iter().all(|rect| rect.height == 2),
            "{kind} rows are 2 lines"
        );
        for rect in &rects {
            for y in rect.y..rect.bottom() {
                for x in rect.x..rect.right() {
                    assert!(
                        !buffer[(x, y)]
                            .modifier
                            .contains(ratatui::style::Modifier::UNDERLINED),
                        "{kind}: no underline bars ({x},{y})"
                    );
                }
            }
        }
        let bg = |rect: &Rect| buffer[(rect.x + 3, rect.y)].bg;
        assert_ne!(bg(&rects[0]), bg(&rects[1]), "{kind}: neighbours differ");
        assert_eq!(
            bg(&rects[0]),
            bg(&rects[2]),
            "{kind}: every other row matches"
        );
        assert_eq!(
            bg(&rects[1]),
            bg(&rects[3]),
            "{kind}: every other row matches"
        );
        // The whole row, both lines and the far edge, carries its background.
        assert_eq!(
            bg(&rects[1]),
            buffer[(rects[1].right() - 1, rects[1].y + 1)].bg
        );
    }
}

/// Opens the header menu so that its buttons have hit rects.
fn open_mobile_menu(state: &mut ClientShellState, cols: u16, rows: u16) {
    state.compose(cols, rows).expect("mobile header");
    state.handle_raw_events(vec![mobile_click(state.hits.mobile_menu)]);
    state.compose(cols, rows).expect("mobile menu");
}

fn mobile_state() -> ClientShellState {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    state
}

#[test]
fn mobile_header_has_one_menu_button_instead_of_find_and_switch() {
    for cols in 30..=64u16 {
        let mut state = mobile_state();
        let frame = state.compose(cols, 20).expect("mobile header");
        let header = format!(
            "{}{}",
            mobile_row_text(&frame, 0),
            mobile_row_text(&frame, 1)
        );
        assert!(
            header.contains("menu"),
            "menu missing at {cols}: {header:?}"
        );
        for label in ["Find", "switch"] {
            assert!(
                !header.contains(label),
                "{label} lives in the menu: {header:?}"
            );
        }
        let menu = state.hits.mobile_menu;
        assert!(!menu.is_empty(), "menu hit missing at {cols}");
        assert_eq!(menu.right(), cols, "top right corner");
        assert_eq!(menu.y, 0);
        assert!(menu.width < 10, "narrower than the old switch button");
        assert!(state.hits.mobile_find.is_empty() && state.hits.mobile_switch.is_empty());
        assert!(state.hits.mobile_menu_panel.is_empty(), "closed by default");
    }
}

#[test]
fn mobile_menu_opens_a_wide_right_aligned_drop_down_of_stacked_buttons() {
    let mut state = mobile_state();
    open_mobile_menu(&mut state, 60, 30);
    assert!(state.mobile_menu_open);
    let buttons = [
        state.hits.mobile_switch,
        state.hits.mobile_find,
        state.hits.mobile_tabs,
        state.hits.mobile_recent,
        state.hits.mobile_goto_done,
        state.hits.mobile_next_working,
        state.hits.mobile_close_pane,
    ];
    for button in buttons {
        assert!(!button.is_empty());
        assert_eq!(button.width, buttons[0].width, "equal widths");
        assert_eq!(button.x, buttons[0].x, "one below the other");
        assert_eq!(button.right(), 60, "right aligned under the menu button");
    }
    for pair in buttons.windows(2) {
        assert_eq!(pair[1].y, pair[0].y + 2, "two rows each, in order");
        assert_eq!(pair[0].height, 2);
    }
    let (panel, menu) = (state.hits.mobile_menu_panel, state.hits.mobile_menu);
    assert_eq!(panel.y, 2, "directly below the header");
    assert_eq!(panel.right(), menu.right());
    assert!(
        panel.width >= menu.width + 6,
        "much wider than the menu button: {panel:?} vs {menu:?}"
    );
    assert_eq!(panel.height, 15, "seven buttons plus a blank last row");
    let last = buttons[6];
    assert_eq!(
        last.bottom(),
        panel.bottom(),
        "the blank row taps the last button"
    );
    assert_eq!(last.height, 3);
    let frame = state.compose(60, 30).expect("mobile menu");
    let rows = (panel.y..panel.bottom())
        .map(|row| mobile_row_text(&frame, row))
        .collect::<Vec<_>>();
    let text = rows.concat();
    let positions = ["switch", "Find", "tabs", "Goto done", "Next working"].map(|label| {
        text.find(label)
            .unwrap_or_else(|| panic!("{label} in {text:?}"))
    });
    assert!(positions.windows(2).all(|w| w[0] < w[1]), "order: {text:?}");
    let label_part = rows
        .last()
        .unwrap()
        .chars()
        .skip(usize::from(panel.x) + 1)
        .collect::<String>();
    assert!(label_part.trim().is_empty(), "the last row is blank");
    let cell = |col: u16, row: u16| {
        &frame.cells[usize::from(row) * usize::from(frame.width) + usize::from(col)]
    };
    let blank_row = panel.bottom() - 1;
    let button_bg = cell(panel.x + 3, last.y).bg;
    assert_eq!(
        cell(panel.x, blank_row).symbol,
        "│",
        "same divider as the buttons"
    );
    for col in panel.x..panel.right() {
        assert_eq!(
            cell(col, blank_row).bg,
            button_bg,
            "blank row at column {col}"
        );
    }
}

#[test]
fn mobile_menu_blank_last_row_taps_the_last_button() {
    let mut state = mobile_state();
    open_mobile_menu(&mut state, 60, 24);
    let panel = state.hits.mobile_menu_panel;
    let blank = Rect::new(panel.x + 2, panel.bottom() - 1, 1, 1);
    let outcome = state.handle_raw_events(vec![mobile_click(blank)]);
    assert!(outcome.repaint);
    assert!(!state.mobile_menu_open, "the tap ran the last item");
    assert_eq!(state.mode, ClientShellMode::Terminal);
}

#[test]
fn mobile_menu_leaves_out_buttons_that_do_not_fit_the_height() {
    let mut state = mobile_state();
    open_mobile_menu(&mut state, 60, 5);
    assert!(!state.hits.mobile_switch.is_empty());
    assert!(state.hits.mobile_find.is_empty() && state.hits.mobile_goto_done.is_empty());
    assert_eq!(
        state.hits.mobile_menu_panel.height, 3,
        "one button plus the blank row"
    );
}

#[test]
fn mobile_menu_switch_opens_the_switcher_and_closes_the_menu() {
    let mut state = mobile_state();
    open_mobile_menu(&mut state, 60, 24);
    let outcome = state.handle_raw_events(vec![mobile_click(state.hits.mobile_switch)]);
    assert!(outcome.repaint);
    assert_eq!(state.mode, ClientShellMode::Navigate);
    assert!(!state.mobile_menu_open);
    state.compose(60, 24).expect("mobile switcher");
    assert!(state.hits.mobile_menu_panel.is_empty());
}

#[test]
fn mobile_menu_find_opens_search_and_closes_the_menu() {
    let mut state = mobile_state();
    open_mobile_menu(&mut state, 60, 24);
    let outcome = state.handle_raw_events(vec![mobile_click(state.hits.mobile_find)]);
    assert!(outcome.repaint);
    assert!(!state.mobile_menu_open);
    let Some(ClientShellOverlay::Navigator(navigator)) = state.overlay.as_ref() else {
        panic!("Find must open the navigator search");
    };
    assert!(navigator.search_focused);
}

#[test]
fn mobile_menu_closes_on_the_menu_button_another_tap_or_typing() {
    for closer in ["menu", "elsewhere", "typing"] {
        let mut state = mobile_state();
        open_mobile_menu(&mut state, 60, 24);
        assert!(state.mobile_menu_open, "{closer}");
        let events = match closer {
            "menu" => vec![mobile_click(state.hits.mobile_menu)],
            "elsewhere" => vec![mobile_click(Rect::new(5, 15, 1, 1))],
            _ => vec![RawInputEvent::Key(crate::input::TerminalKey::new(
                KeyCode::Char('x'),
                KeyModifiers::empty(),
            ))],
        };
        let outcome = state.handle_raw_events(events);
        assert!(outcome.repaint, "{closer}");
        assert!(!state.mobile_menu_open, "{closer} closes the menu");
        assert_eq!(state.mode, ClientShellMode::Terminal, "{closer}");
        assert!(state.overlay.is_none(), "{closer}");
        state.compose(60, 24).expect("closed menu");
        assert!(state.hits.mobile_menu_panel.is_empty(), "{closer}");
    }
}

#[test]
fn mobile_menu_does_not_exist_on_desktop() {
    let mut state = mobile_state();
    state.compose(106, 30).expect("desktop");
    assert!(state.hits.mobile_menu.is_empty());
    state.mobile_menu_open = true;
    state.compose(106, 30).expect("desktop");
    assert!(!state.mobile_menu_open, "a stale open flag is dropped");
    assert!(state.hits.mobile_menu_panel.is_empty());
}

use super::status_order::{agent_for, state_with, workspace};

fn done_state(finished: &[&str]) -> ClientShellState {
    let working = |pane: &str| {
        if finished.contains(&pane) {
            AgentStatus::Working
        } else {
            AgentStatus::Idle
        }
    };
    let mut state = state_with(
        vec![
            workspace("a", "alpha", AgentStatus::Idle, None),
            workspace("b", "bravo", AgentStatus::Idle, None),
            workspace("c", "charlie", AgentStatus::Idle, None),
        ],
        vec![
            agent_for("pane-a", "a", working("pane-a"), 1),
            agent_for("pane-b", "b", working("pane-b"), 2),
            agent_for("pane-c", "c", working("pane-c"), 3),
        ],
    );
    // The client only calls an agent done once it has watched it finish.
    let mut next = state.snapshot.as_deref().unwrap().clone();
    next.revision = 2;
    for agent in &mut next.agents {
        agent.agent_status = AgentStatus::Idle;
        agent.state_change_seq += 10;
    }
    present_snapshot(&mut state, next);
    state
}

fn present_snapshot(state: &mut ClientShellState, next: ClientShellSnapshot) {
    let mut presented = surface();
    presented.projection_revision = next.revision;
    state.set_endpoint_snapshot(&ClientEndpointId::Local, Box::new(next));
    state.set_pane_surface(presented);
    state.compose(60, 24).expect("mobile header");
}

fn focus_pane(state: &mut ClientShellState, pane: &str) {
    let mut next = state.snapshot.as_deref().unwrap().clone();
    next.revision += 1;
    next.focused_pane_id = Some(pane.into());
    present_snapshot(state, next);
}

fn goto_done_pane(state: &mut ClientShellState) -> (Option<String>, bool) {
    open_mobile_menu(state, 60, 24);
    let outcome = state.handle_raw_events(vec![mobile_click(state.hits.mobile_goto_done)]);
    assert!(!state.mobile_menu_open, "the menu closes");
    assert_eq!(state.mode, ClientShellMode::Terminal);
    let pane = outcome.actions.iter().find_map(|action| match action {
        ClientShellAction::Endpoint { request, .. } => match &request.method {
            crate::api::schema::Method::PaneFocus(target) => Some(target.pane_id.clone()),
            _ => None,
        },
        _ => None,
    });
    (pane, outcome.repaint)
}

#[test]
fn mobile_goto_done_walks_the_done_agents_and_wraps_around() {
    let mut state = done_state(&["pane-b", "pane-c"]);
    let statuses = state
        .snapshot
        .as_deref()
        .unwrap()
        .agents
        .iter()
        .map(|agent| agent.agent_status)
        .collect::<Vec<_>>();
    assert_eq!(
        statuses,
        [AgentStatus::Idle, AgentStatus::Done, AgentStatus::Done]
    );
    let (pane, repaint) = goto_done_pane(&mut state);
    assert_eq!(pane.as_deref(), Some("pane-b"), "first done one");
    assert!(repaint);
    focus_pane(&mut state, "pane-b");
    assert_eq!(goto_done_pane(&mut state).0.as_deref(), Some("pane-c"));
    focus_pane(&mut state, "pane-c");
    assert_eq!(
        goto_done_pane(&mut state).0.as_deref(),
        Some("pane-b"),
        "wraps"
    );
}

#[test]
fn mobile_goto_done_does_nothing_without_a_done_agent() {
    let mut state = done_state(&[]);
    let (pane, _) = goto_done_pane(&mut state);
    assert_eq!(pane, None);
}

#[test]
fn next_done_walks_the_done_entries_and_wraps_around() {
    use super::super::mobile::next_flagged_index;
    let done = [false, true, false, true, false];
    assert_eq!(next_flagged_index(&done, None), Some(1));
    assert_eq!(next_flagged_index(&done, Some(0)), Some(1));
    assert_eq!(next_flagged_index(&done, Some(1)), Some(3));
    assert_eq!(next_flagged_index(&done, Some(3)), Some(1), "wraps around");
    assert_eq!(next_flagged_index(&done, Some(4)), Some(1));
    assert_eq!(next_flagged_index(&[false, false], Some(0)), None);
    assert_eq!(next_flagged_index(&[], None), None);
    assert_eq!(
        next_flagged_index(&[true], Some(0)),
        None,
        "only the current one"
    );
    assert_eq!(next_flagged_index(&[true], None), Some(0));
}

#[test]
fn mobile_header_has_a_side_button_left_of_menu_on_the_lower_line() {
    for cols in 30..=64u16 {
        let mut state = mobile_state();
        let frame = state.compose(cols, 20).expect("mobile header");
        let (side, menu) = (state.hits.mobile_side, state.hits.mobile_menu);
        assert!(!side.is_empty(), "side missing at {cols} columns");
        assert_eq!(side.right(), menu.x, "directly left of menu at {cols}");
        assert_eq!(
            (side.y, side.height),
            (menu.y, menu.height),
            "same two lines"
        );
        assert_eq!(side.width, menu.width, "same size as the menu button");
        let (upper, lower) = (mobile_row_text(&frame, 0), mobile_row_text(&frame, 1));
        assert!(
            lower.contains("side") && lower.contains("menu"),
            "both labels on the lower line at {cols}: {lower:?}"
        );
        assert!(
            !upper.contains("side") && !upper.contains("menu"),
            "nothing on the upper line at {cols}: {upper:?}"
        );
    }
}

#[test]
fn mobile_side_button_opens_the_switcher_like_switch() {
    let mut state = mobile_state();
    state.compose(60, 24).expect("mobile header");
    let outcome = state.handle_raw_events(vec![mobile_click(state.hits.mobile_side)]);
    assert!(outcome.repaint);
    assert_eq!(state.mode, ClientShellMode::Navigate);
    assert!(!state.mobile_menu_open);
    let frame = state.compose(60, 24).expect("mobile switcher");
    let header = format!(
        "{}{}",
        mobile_row_text(&frame, 0),
        mobile_row_text(&frame, 1)
    );
    assert!(header.contains("close"), "the switcher is open: {header:?}");
}

#[test]
fn mobile_side_button_still_opens_the_switcher_while_the_menu_is_open() {
    let mut state = mobile_state();
    open_mobile_menu(&mut state, 60, 24);
    state.handle_raw_events(vec![mobile_click(state.hits.mobile_side)]);
    assert_eq!(state.mode, ClientShellMode::Navigate);
    assert!(!state.mobile_menu_open);
}

#[test]
fn mobile_switcher_find_label_sits_on_the_lower_line_left_of_close() {
    let mut state = mobile_state();
    state.compose(48, 12).expect("mobile header");
    state.handle_raw_events(vec![mobile_click(state.hits.mobile_side)]);
    let frame = state.compose(48, 12).expect("mobile switcher");
    let (upper, lower) = (mobile_row_text(&frame, 0), mobile_row_text(&frame, 1));
    assert!(lower.contains("Find"), "Find on the lower line: {lower:?}");
    assert!(!upper.contains("Find"), "not on the upper line: {upper:?}");
    let (find, close) = (state.hits.mobile_find, state.hits.mobile_close);
    assert_eq!((find.y, find.height), (0, 2), "a two line button");
    assert_eq!(find.right(), close.x, "directly left of close");
}

/// Space `ws_1` (focused, tab_1 shown) has three tabs; space `ws_2` one. `working` lists the
/// panes whose agent works.
fn tabs_state(working: &[&str]) -> ClientShellState {
    let mut projected = snapshot();
    let mut second = projected.workspaces[0].clone();
    second.workspace_id = "ws_2".into();
    second.label = "other".into();
    second.number = 2;
    second.focused = false;
    second.active_tab_id = "tab_4".into();
    projected.workspaces.push(second);
    for (tab, pane, workspace, label) in [
        ("tab_2", "pane_2", "ws_1", "build"),
        ("tab_3", "pane_3", "ws_1", "notes"),
        ("tab_4", "pane_4", "ws_2", "remote"),
    ] {
        let mut new_tab = projected.tabs[0].clone();
        new_tab.tab_id = tab.into();
        new_tab.workspace_id = workspace.into();
        new_tab.label = label.into();
        new_tab.custom_label = true;
        new_tab.focused = false;
        projected.tabs.push(new_tab);
        let mut new_pane = projected.panes[0].clone();
        new_pane.pane_id = pane.into();
        new_pane.workspace_id = workspace.into();
        new_pane.tab_id = tab.into();
        new_pane.focused = false;
        projected.panes.push(new_pane);
    }
    projected.tabs[0].label = "main".into();
    projected.tabs[0].custom_label = true;
    for (pane, tab, workspace) in [
        ("pane_1", "tab_1", "ws_1"),
        ("pane_2", "tab_2", "ws_1"),
        ("pane_3", "tab_3", "ws_1"),
        ("pane_4", "tab_4", "ws_2"),
    ] {
        let status = if working.contains(&pane) {
            AgentStatus::Working
        } else {
            AgentStatus::Idle
        };
        let mut agent = agent_for(pane, workspace, status, 1);
        agent.tab_id = tab.into();
        projected.agents.push(agent);
    }
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    state.compose(60, 30).expect("mobile header");
    state
}

fn focus_tab(state: &mut ClientShellState, tab: &str, pane: &str) {
    let mut next = state.snapshot.as_deref().unwrap().clone();
    next.revision += 1;
    next.focused_tab_id = Some(tab.into());
    next.focused_pane_id = Some(pane.into());
    present_snapshot(state, next);
}

fn menu_click(state: &mut ClientShellState, button: fn(&ShellHitMap) -> Rect) -> ClientShellInput {
    open_mobile_menu(state, 60, 30);
    let rect = button(&state.hits);
    state.handle_raw_events(vec![mobile_click(rect)])
}

fn pane_focus_of(outcome: &ClientShellInput) -> Option<String> {
    outcome.actions.iter().find_map(|action| match action {
        ClientShellAction::Endpoint { request, .. } => match &request.method {
            crate::api::schema::Method::PaneFocus(target) => Some(target.pane_id.clone()),
            _ => None,
        },
        _ => None,
    })
}

#[test]
fn mobile_menu_tabs_lists_only_the_sibling_tabs_of_the_current_space() {
    let mut state = tabs_state(&[]);
    let outcome = menu_click(&mut state, |hits| hits.mobile_tabs);
    assert!(outcome.repaint);
    assert!(!state.mobile_menu_open);
    let Some(ClientShellOverlay::Navigator(navigator)) = state.overlay.as_ref() else {
        panic!("tabs must open a Find-style list");
    };
    let rows =
        render::client_navigator_rows(&state.endpoints, &state.active_endpoint_id, navigator);
    let panes = rows
        .iter()
        .map(|row| match &row.target {
            ClientNavigatorTarget::Pane { pane_id, .. } => pane_id.clone(),
            other => panic!("only tab rows, got {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        panes,
        ["pane_1", "pane_2", "pane_3"],
        "no other space, no headers"
    );
    assert!(
        rows[0].current && !rows[1].current,
        "the current tab is marked"
    );
    let frame = state.compose(60, 30).expect("tabs list");
    let text = (0..frame.height)
        .map(|row| mobile_row_text(&frame, row))
        .collect::<Vec<_>>()
        .join("\n");
    for label in ["Tabs", "main", "build", "notes", "3 tabs"] {
        assert!(text.contains(label), "{label} in:\n{text}");
    }
    for label in ["remote", "other", "Go to"] {
        assert!(!text.contains(label), "{label} must not appear:\n{text}");
    }
}

#[test]
fn choosing_a_row_in_the_tabs_list_switches_to_that_tab() {
    let mut state = tabs_state(&[]);
    menu_click(&mut state, |hits| hits.mobile_tabs);
    state.compose(60, 30).expect("tabs list");
    let (rect, _) = state
        .hits
        .navigator_rows
        .iter()
        .find(|(_, target)| matches!(target, ClientNavigatorTarget::Pane { pane_id, .. } if pane_id == "pane_2"))
        .cloned()
        .expect("a row for the build tab");
    let outcome = state.handle_raw_events(vec![mobile_click(rect)]);
    assert_eq!(pane_focus_of(&outcome).as_deref(), Some("pane_2"));
}

#[test]
fn find_still_lists_every_space() {
    let mut state = tabs_state(&[]);
    menu_click(&mut state, |hits| hits.mobile_find);
    let Some(ClientShellOverlay::Navigator(navigator)) = state.overlay.as_ref() else {
        panic!("Find opens the navigator");
    };
    let rows =
        render::client_navigator_rows(&state.endpoints, &state.active_endpoint_id, navigator);
    assert!(rows
        .iter()
        .any(|row| matches!(&row.target, ClientNavigatorTarget::Workspace { workspace_id, .. } if workspace_id == "ws_2")));
}

#[test]
fn next_working_walks_the_working_tabs_across_spaces_and_wraps() {
    let mut state = tabs_state(&["pane_2", "pane_4"]);
    let go = |state: &mut ClientShellState| {
        let outcome = menu_click(state, |hits| hits.mobile_next_working);
        assert!(!state.mobile_menu_open);
        pane_focus_of(&outcome)
    };
    assert_eq!(go(&mut state).as_deref(), Some("pane_2"), "from tab_1");
    focus_tab(&mut state, "tab_2", "pane_2");
    assert_eq!(
        go(&mut state).as_deref(),
        Some("pane_4"),
        "into the other space"
    );
    focus_tab(&mut state, "tab_4", "pane_4");
    assert_eq!(go(&mut state).as_deref(), Some("pane_2"), "wraps around");
}

#[test]
fn next_working_does_nothing_without_another_working_tab() {
    let mut state = tabs_state(&[]);
    assert_eq!(
        pane_focus_of(&menu_click(&mut state, |hits| hits.mobile_next_working)),
        None
    );
    let mut state = tabs_state(&["pane_1"]);
    assert_eq!(
        pane_focus_of(&menu_click(&mut state, |hits| hits.mobile_next_working)),
        None,
        "only the current tab works"
    );
}
