use super::*;
use crate::client::shell::global_menu::{global_menu_items, ClientGlobalMenuAction};
use crate::client::shell::recent::format_age;

const HOUR: u64 = 3600;
const DAY: u64 = 24 * HOUR;

fn recent_state(pane_count: usize) -> ClientShellState {
    let mut projected = snapshot();
    for number in 2..=pane_count {
        let mut pane = projected.panes[0].clone();
        pane.pane_id = format!("pane_{number}");
        pane.focused = false;
        projected.panes.push(pane);
    }
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    state
}

fn focus(state: &mut ClientShellState, pane_id: &str, at: u64) {
    let mut projected = state.snapshot.as_deref().unwrap().clone();
    projected.focused_pane_id = Some(pane_id.into());
    projected.revision += 1;
    let mut presented = surface();
    presented.projection_revision = projected.revision;
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(presented);
    state.note_focus_access(at);
}

fn recent_rows(state: &ClientShellState) -> Vec<ClientNavigatorRow> {
    match state.overlay.as_ref() {
        Some(ClientShellOverlay::Navigator(navigator)) => {
            crate::client::shell::aggregate_navigation::navigator_rows(
                &state.endpoints,
                &state.active_endpoint_id,
                navigator,
            )
        }
        _ => panic!("recent overlay"),
    }
}

fn pane_ids(rows: &[ClientNavigatorRow]) -> Vec<String> {
    rows.iter()
        .map(|row| match &row.target {
            ClientNavigatorTarget::Pane { pane_id, .. } => pane_id.clone(),
            other => panic!("unexpected {other:?}"),
        })
        .collect()
}

#[test]
fn ages_are_shown_in_minutes_hours_days_and_weeks() {
    assert_eq!(format_age(30), "<1m ago");
    assert_eq!(format_age(60), "1m ago");
    assert_eq!(format_age(5 * 60), "5m ago");
    assert_eq!(format_age(59 * 60 + 59), "59m ago");
    assert_eq!(format_age(HOUR), "1h ago");
    assert_eq!(format_age(23 * HOUR + 59 * 60), "23h ago");
    assert_eq!(format_age(DAY), "1d ago");
    assert_eq!(format_age(6 * DAY), "6d ago");
    assert_eq!(format_age(7 * DAY), "1w ago");
    assert_eq!(format_age(30 * DAY), "4w ago");
}

#[test]
fn recent_lists_accessed_panes_newest_first_with_their_age() {
    let mut state = recent_state(3);
    focus(&mut state, "pane_1", 1_000);
    focus(&mut state, "pane_2", 1_000 + 5 * HOUR);
    focus(&mut state, "pane_3", 1_000 + 3 * DAY);
    focus(&mut state, "pane_1", 1_000 + 4 * DAY);
    state.open_recent_overlay_at(1_000 + 4 * DAY + 2 * HOUR);
    let rows = recent_rows(&state);
    assert_eq!(pane_ids(&rows), ["pane_1", "pane_3", "pane_2"]);
    let ages = rows
        .iter()
        .map(|row| row.age.clone().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(ages, ["<1m ago", "2h ago", "1d ago"]);
}

#[test]
fn recent_keeps_only_the_last_fifteen_and_skips_closed_panes() {
    let mut state = recent_state(20);
    for number in 1..=20u64 {
        focus(&mut state, &format!("pane_{number}"), 1_000 + number * 60);
    }
    state.open_recent_overlay_at(10_000);
    let rows = recent_rows(&state);
    assert_eq!(rows.len(), 15);
    assert_eq!(rows[0].target, pane_target("pane_20"));
    assert_eq!(rows[14].target, pane_target("pane_6"));

    let mut closed = recent_state(2);
    focus(&mut closed, "pane_1", 1_000);
    focus(&mut closed, "pane_2", 2_000);
    let mut projected = closed.snapshot.as_deref().unwrap().clone();
    projected.panes.retain(|pane| pane.pane_id != "pane_2");
    projected.focused_pane_id = Some("pane_1".into());
    projected.revision += 1;
    closed.set_snapshot(Box::new(projected));
    closed.open_recent_overlay_at(3_000);
    assert_eq!(pane_ids(&recent_rows(&closed)), ["pane_1"]);
}

fn pane_target(pane_id: &str) -> ClientNavigatorTarget {
    ClientNavigatorTarget::Pane {
        endpoint_id: ClientEndpointId::Local,
        pane_id: pane_id.into(),
    }
}

#[test]
fn recent_view_renders_the_age_column_first() {
    let mut state = recent_state(2);
    focus(&mut state, "pane_1", 1_000);
    focus(&mut state, "pane_2", 1_000 + 2 * DAY);
    state.open_recent_overlay_at(1_000 + 2 * DAY + 3 * HOUR);
    let frame = state.compose(106, 30).unwrap();
    let text = frame_rows(&frame).join("\n");
    assert!(text.contains("Recent"), "{text}");
    let row = frame_rows(&frame)
        .into_iter()
        .find(|row| row.contains("3h ago"))
        .expect("newest row");
    let age = row.find("3h ago").unwrap();
    let dot = row
        .find(|c| ['●', '○', '◐', '◉'].contains(&c))
        .unwrap_or(usize::MAX);
    assert!(age < dot, "age column comes before the status dot: {row}");
    assert!(text.contains("<1m ago"), "{text}");
}

#[test]
fn the_access_log_survives_a_restart() {
    let path = std::env::temp_dir().join(format!("herdr-recent-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let mut first = recent_state(2);
    first.config.preferences_path = Some(path.clone());
    focus(&mut first, "pane_1", 1_000);
    focus(&mut first, "pane_2", 2_000);
    first.persist_chrome_preferences(&mut ClientShellInput::default());

    let config =
        ClientShellConfig::from_config(&Config::default()).with_preferences_path(path.clone());
    let mut second = ClientShellState::new(config);
    second.set_snapshot(Box::new(first.snapshot.as_deref().unwrap().clone()));
    second.open_recent_overlay_at(3_000);
    assert_eq!(pane_ids(&recent_rows(&second)), ["pane_2", "pane_1"]);
    std::fs::remove_file(path).expect("remove preferences");
}

#[test]
fn mobile_menu_puts_recent_right_below_tabs_and_opens_it() {
    let mut state = recent_state(2);
    focus(&mut state, "pane_1", 1_000);
    state.compose(44, 30).unwrap();
    state.mobile_menu_open = true;
    state.compose(44, 30).unwrap();
    let tabs = state.hits.mobile_tabs;
    let recent = state.hits.mobile_recent;
    assert!(!recent.is_empty());
    assert_eq!(recent.y, tabs.bottom());
    assert_eq!(recent.x, tabs.x);
    state.handle_raw_events(vec![crate::raw_input::RawInputEvent::Mouse(MouseEvent {
        kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        column: recent.x + 1,
        row: recent.y,
        modifiers: KeyModifiers::empty(),
    })]);
    match state.overlay.as_ref() {
        Some(ClientShellOverlay::Navigator(navigator)) => assert!(navigator.recent.is_some()),
        _ => panic!("recent view opens"),
    }
}

#[test]
fn desktop_menu_dropdown_offers_every_mobile_menu_item_and_recent_follows_tabs() {
    let state = recent_state(1);
    let items = global_menu_items(state.snapshot.as_deref().unwrap(), true);
    let labels = items.iter().map(|(label, _)| *label).collect::<Vec<_>>();
    let tabs = labels.iter().position(|label| *label == "tabs").unwrap();
    assert_eq!(labels[tabs + 1], "Recent");
    let working = labels
        .iter()
        .position(|label| *label == "Next working")
        .unwrap();
    assert_eq!(labels[working + 1], "Close pane");
    for label in [
        "switch",
        "Find",
        "Goto done",
        "Next working",
        "settings",
        "detach",
    ] {
        assert!(labels.contains(&label), "{label} in {labels:?}");
    }
    assert!(items
        .iter()
        .any(|(_, action)| *action == ClientGlobalMenuAction::Recent));
    let mobile = global_menu_items(state.snapshot.as_deref().unwrap(), false);
    assert!(mobile.iter().all(|(label, _)| *label != "Recent"));
}

#[test]
fn desktop_menu_button_recent_item_opens_the_recent_view() {
    let mut state = recent_state(2);
    focus(&mut state, "pane_1", 1_000);
    state.compose(106, 30).unwrap();
    let launcher = state.hits.global_launcher;
    assert!(!launcher.is_empty(), "menu button on desktop");
    state.toggle_global_menu();
    state.compose(106, 30).unwrap();
    let index = global_menu_items(state.snapshot.as_deref().unwrap(), true)
        .iter()
        .position(|(_, action)| *action == ClientGlobalMenuAction::Recent)
        .unwrap();
    let (row, _) = *state
        .hits
        .global_menu_rows
        .iter()
        .find(|(_, row_index)| *row_index == index)
        .expect("recent row");
    state.handle_raw_events(vec![crate::raw_input::RawInputEvent::Mouse(MouseEvent {
        kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        column: row.x + 1,
        row: row.y,
        modifiers: KeyModifiers::empty(),
    })]);
    match state.overlay.as_ref() {
        Some(ClientShellOverlay::Navigator(navigator)) => assert!(navigator.recent.is_some()),
        _ => panic!("recent view opens"),
    }
}

#[test]
fn entries_from_an_earlier_server_run_are_not_shown() {
    let mut state = recent_state(2);
    focus(&mut state, "pane_1", 1_000);
    focus(&mut state, "pane_2", 2_000);
    let mut restarted = state.snapshot.as_deref().unwrap().clone();
    restarted.boot_id = "boot-2".into();
    restarted.revision = 1;
    state.set_snapshot(Box::new(restarted));
    state.open_recent_overlay_at(3_000);
    assert!(recent_rows(&state).is_empty());
}

#[test]
fn recent_starts_on_the_previous_pane_not_the_focused_one() {
    let mut state = recent_state(2);
    focus(&mut state, "pane_1", 1_000);
    focus(&mut state, "pane_2", 2_000);
    state.open_recent_overlay_at(3_000);
    match state.overlay.as_ref() {
        Some(ClientShellOverlay::Navigator(navigator)) => {
            assert_eq!(navigator.selected, Some(pane_target("pane_1")));
        }
        _ => panic!("recent overlay"),
    }
}

#[test]
fn menu_close_pane_closes_the_focused_pane_from_both_menus() {
    use crate::api::schema::Method;
    let closed = |outcome: &ClientShellInput| {
        outcome.actions.iter().any(|action| {
            matches!(action, ClientShellAction::Endpoint { request, .. }
                if matches!(&request.method, Method::PaneClose(target) if target.pane_id == "pane_1"))
        })
    };
    let mut state = recent_state(2);
    let mut outcome = ClientShellInput::default();
    state.activate_global_menu_action(
        ClientGlobalMenuAction::Binding(crate::input::KeybindAction::ClosePane),
        &mut outcome,
    );
    assert!(closed(&outcome));

    let mut mobile = recent_state(2);
    mobile.compose(44, 40).unwrap();
    mobile.mobile_menu_open = true;
    mobile.compose(44, 40).unwrap();
    let button = mobile.hits.mobile_close_pane;
    assert_eq!(button.y, mobile.hits.mobile_next_working.bottom());
    let outcome =
        mobile.handle_raw_events(vec![crate::raw_input::RawInputEvent::Mouse(MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: button.x + 1,
            row: button.y,
            modifiers: KeyModifiers::empty(),
        })]);
    assert!(closed(&outcome));
}

#[test]
fn the_pane_you_sit_in_stays_newest_however_long_you_stay() {
    let mut state = recent_state(2);
    focus(&mut state, "pane_1", 1_000);
    focus(&mut state, "pane_2", 1_000 + 3 * HOUR);
    focus(&mut state, "pane_1", 1_000 + 3 * HOUR + 600);
    state.open_recent_overlay_at(1_000 + 6 * HOUR);
    let rows = recent_rows(&state);
    assert_eq!(pane_ids(&rows), ["pane_1", "pane_2"]);
    assert_eq!(rows[0].age.as_deref(), Some("<1m ago"));
    assert_eq!(rows[1].age.as_deref(), Some("2h ago"));
}
