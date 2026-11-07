use super::*;

fn many_tabs(count: usize) -> ClientShellSnapshot {
    let mut projected = snapshot();
    projected
        .tabs
        .extend((2..=count).map(|number| ClientShellTab {
            tab_id: format!("tab_{number}"),
            workspace_id: "ws_1".into(),
            number,
            label: format!("tab-{number}"),
            custom_label: true,
            zoomed: false,
            focused: false,
            agent_status: AgentStatus::Idle,
        }));
    projected
}

fn click(state: &mut ClientShellState, column: u16, row: u16) -> ClientShellInput {
    state.handle_raw_events(vec![RawInputEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::empty(),
    })])
}

fn key(state: &mut ClientShellState, code: KeyCode) -> ClientShellInput {
    state.handle_raw_events(vec![RawInputEvent::Key(
        crossterm::event::KeyEvent::new(code, KeyModifiers::empty()).into(),
    )])
}

fn open_menu(count: usize, width: u16, height: u16) -> ClientShellState {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(many_tabs(count)));
    state.set_pane_surface(surface());
    state.compose(width, height).expect("overflowing tab bar");
    let button = state.hits.tab_overflow;
    assert!(button.width > 0, "overflow shows a [..] button");
    click(&mut state, button.x + 1, button.y);
    state.compose(width, height).expect("tab overflow menu");
    state
}

#[test]
fn overflowing_tab_bar_ends_in_a_dots_button_before_the_new_tab_button() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(many_tabs(8)));
    state.set_pane_surface(surface());
    let frame = state.compose(80, 20).expect("overflow tab bar");
    let button = state.hits.tab_overflow;
    assert_eq!(button.width, 4);
    assert_eq!(button.right(), state.hits.new_tab.x);
    assert!(button.x >= state.hits.tab_scroll_right.right());
    let text = (button.x..button.right())
        .map(|x| {
            frame.cells[(button.y * frame.width + x) as usize]
                .symbol
                .as_str()
        })
        .collect::<String>();
    assert_eq!(text, "[..]");
    for (rect, _) in &state.hits.tabs {
        assert!(rect.right() <= state.hits.tab_scroll_right.x);
    }

    state.compose(300, 20).expect("no overflow");
    assert!(state.hits.tab_overflow.is_empty());
}

#[test]
fn dots_button_lists_only_hidden_tabs_and_focuses_the_chosen_one() {
    let mut state = open_menu(8, 80, 20);
    let visible = state
        .hits
        .tabs
        .iter()
        .map(|(_, id)| id.clone())
        .collect::<Vec<_>>();
    let Some(ClientShellOverlay::TabOverflow(menu)) = state.overlay.as_ref() else {
        panic!("dots button opens the tab overflow menu");
    };
    assert!(!menu.items.is_empty());
    for item in &menu.items {
        assert!(
            !visible.contains(&item.tab_id),
            "{} is visible",
            item.tab_id
        );
    }
    assert_eq!(menu.items.len() + visible.len(), 8);
    let expected = menu.items[1].tab_id.clone();
    let row = state.hits.tab_overflow_rows[1].0;
    let outcome = click(&mut state, row.x + 1, row.y);
    assert!(state.overlay.is_none());
    assert!(matches!(
        &outcome.actions[..],
        [ClientShellAction::Endpoint { request, .. }]
            if matches!(&request.method, crate::api::schema::Method::TabFocus(t) if t.tab_id == expected)
    ));
}

#[test]
fn long_tab_menu_scrolls_with_wheel_and_arrow_keys_and_closes_on_escape_or_outside_click() {
    let mut state = open_menu(60, 80, 14);
    let total = match state.overlay.as_ref() {
        Some(ClientShellOverlay::TabOverflow(menu)) => menu.items.len(),
        _ => panic!("menu"),
    };
    let visible = state.hits.tab_overflow_rows.len();
    assert!(visible > 0 && visible < total, "{visible} of {total}");

    state.handle_raw_events(vec![RawInputEvent::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: state.hits.tab_overflow_rows[0].0.x,
        row: state.hits.tab_overflow_rows[0].0.y,
        modifiers: KeyModifiers::empty(),
    })]);
    let Some(ClientShellOverlay::TabOverflow(menu)) = state.overlay.as_ref() else {
        panic!("menu stays open on wheel");
    };
    assert_eq!(menu.scroll, 1);

    for _ in 0..total {
        key(&mut state, KeyCode::Down);
    }
    state.compose(80, 14).expect("scrolled menu");
    let Some(ClientShellOverlay::TabOverflow(menu)) = state.overlay.as_ref() else {
        panic!("menu");
    };
    assert_eq!(menu.highlighted, total - 1);
    assert_eq!(menu.scroll, total - visible);
    assert!(state
        .hits
        .tab_overflow_rows
        .iter()
        .any(|(_, index)| *index == total - 1));

    key(&mut state, KeyCode::Esc);
    assert!(state.overlay.is_none());

    let mut state = open_menu(60, 80, 14);
    click(&mut state, 0, 0);
    assert!(state.overlay.is_none());

    let mut state = open_menu(60, 80, 14);
    let outcome = key(&mut state, KeyCode::Enter);
    assert!(state.overlay.is_none());
    assert!(matches!(
        &outcome.actions[..],
        [ClientShellAction::Endpoint { .. }]
    ));
}
