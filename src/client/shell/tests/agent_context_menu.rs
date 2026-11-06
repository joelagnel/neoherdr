use super::status_order::{agent_for, state_with, workspace};
use super::*;
use crate::api::schema::Method;
use crossterm::event::{MouseButton, MouseEventKind};

fn agent_state() -> ClientShellState {
    let mut state = state_with(
        vec![workspace("a", "alpha", AgentStatus::Idle, None)],
        vec![agent_for("pane-a", "a", AgentStatus::Idle, 1)],
    );
    state.compose(100, 40).expect("desktop shell");
    state
}

fn mouse(state: &mut ClientShellState, button: MouseButton, rect: Rect) -> ClientShellInput {
    state.handle_raw_events(vec![crate::raw_input::RawInputEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(button),
        column: rect.x + 1,
        row: rect.y,
        modifiers: KeyModifiers::empty(),
    })])
}

fn open_menu(state: &mut ClientShellState) -> Vec<&'static str> {
    let rect = state.hits.agents[0].0;
    mouse(state, MouseButton::Right, rect);
    match state.overlay.as_ref() {
        Some(ClientShellOverlay::ContextMenu(menu)) => {
            menu.items().iter().map(|item| item.label).collect()
        }
        _ => panic!("agent context menu"),
    }
}

#[test]
fn right_clicking_an_agent_row_offers_rename_close_and_mark_as_done() {
    let mut state = agent_state();
    assert_eq!(open_menu(&mut state), ["Rename", "Close", "Mark as done"]);
}

#[test]
fn mark_as_done_turns_the_agent_done() {
    let mut state = agent_state();
    open_menu(&mut state);
    state.compose(100, 40).unwrap();
    let row = state.hits.context_menu_rows[2].0;
    mouse(&mut state, MouseButton::Left, row);
    let agent = &state.snapshot.as_deref().unwrap().agents[0];
    assert_eq!(agent.agent_status, AgentStatus::Done);
    assert!(state.overlay.is_none());
}

#[test]
fn close_from_the_agent_menu_closes_that_pane() {
    let mut state = agent_state();
    open_menu(&mut state);
    state.compose(100, 40).unwrap();
    let row = state.hits.context_menu_rows[1].0;
    let outcome = mouse(&mut state, MouseButton::Left, row);
    let closes = outcome.actions.iter().any(|action| {
        matches!(action, ClientShellAction::Endpoint { request, .. }
            if matches!(&request.method, Method::PaneClose(target) if target.pane_id == "pane-a"))
    }) || matches!(state.overlay, Some(ClientShellOverlay::ConfirmClose(_)));
    assert!(closes);
}
