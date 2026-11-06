use super::*;
use crate::api::schema::{Method, PaneZoomMode};
use crossterm::event::{MouseButton, MouseEventKind};

fn split_state(zoomed: bool) -> ClientShellState {
    let mut projected = snapshot();
    let mut second = projected.panes[0].clone();
    second.pane_id = "pane_2".into();
    second.focused = false;
    projected.panes.push(second);
    projected.tabs[0].zoomed = zoomed;
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    state
}

fn cell_text(buffer: &ratatui::buffer::Buffer, rect: Rect) -> Vec<String> {
    (rect.y..rect.bottom())
        .map(|y| {
            (rect.x..rect.right())
                .map(|x| buffer[(x, y)].symbol())
                .collect()
        })
        .collect()
}

fn click(state: &mut ClientShellState, rect: Rect) -> ClientShellInput {
    state.handle_raw_events(vec![crate::raw_input::RawInputEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: rect.x,
        row: rect.y,
        modifiers: KeyModifiers::empty(),
    })])
}

fn zoom_requests(outcome: &ClientShellInput) -> usize {
    outcome
        .actions
        .iter()
        .filter(|action| {
            matches!(action, ClientShellAction::Endpoint { request, .. }
                if matches!(&request.method, Method::PaneZoom(params)
                    if params.mode == PaneZoomMode::Toggle))
        })
        .count()
}

#[test]
fn split_tab_overlays_a_two_by_two_box_in_the_pane_corner() {
    let mut state = split_state(false);
    let frame = state.compose(106, 24).unwrap();
    let surface = state.layout(106, 24).pane_surface;
    let button = state.hits.pane_zoom_button;
    assert_eq!(button, Rect::new(surface.right() - 2, surface.y, 2, 2));
    let buffer = frame.to_ratatui_buffer().unwrap();
    assert_eq!(cell_text(&buffer, button), ["┌┐", "└┘"]);
}

#[test]
fn zoomed_tab_keeps_the_button_with_a_filled_box() {
    let mut state = split_state(true);
    let frame = state.compose(106, 24).unwrap();
    let button = state.hits.pane_zoom_button;
    assert!(!button.is_empty());
    let buffer = frame.to_ratatui_buffer().unwrap();
    assert_eq!(cell_text(&buffer, button), ["▛▜", "▙▟"]);
}

#[test]
fn single_pane_and_mobile_layouts_have_no_button() {
    let mut single = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    single.set_snapshot(Box::new(snapshot()));
    single.set_pane_surface(surface());
    single.compose(106, 24).unwrap();
    assert!(single.hits.pane_zoom_button.is_empty());

    let mut mobile = split_state(false);
    mobile.compose(44, 30).unwrap();
    assert!(mobile.hits.pane_zoom_button.is_empty());
}

#[test]
fn clicking_the_button_toggles_zoom_like_the_keybinding() {
    let mut state = split_state(false);
    state.compose(106, 24).unwrap();
    let button = state.hits.pane_zoom_button;
    for corner in [
        Rect::new(button.x, button.y, 1, 1),
        Rect::new(button.right() - 1, button.bottom() - 1, 1, 1),
    ] {
        assert_eq!(zoom_requests(&click(&mut state, corner)), 1);
    }
    let outside = Rect::new(button.x - 1, button.y, 1, 1);
    assert_eq!(zoom_requests(&click(&mut state, outside)), 0);
}
