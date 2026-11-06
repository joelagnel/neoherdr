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
    let mut two = surface();
    let mut second = two.panes[0].clone();
    second.pane_id = "pane_2".into();
    second.focused = false;
    for (pane, x) in two.panes.iter_mut().chain([&mut second]).zip([0, 10]) {
        pane.rect = SurfaceRect {
            x,
            y: 0,
            width: 8,
            height: 4,
        };
        pane.inner_rect = pane.rect;
    }
    two.panes.push(second);
    state.set_pane_surface(two);
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
fn every_pane_of_a_split_tab_overlays_a_two_by_two_box_in_its_corner() {
    let mut state = split_state(false);
    let frame = state.compose(106, 24).unwrap();
    let buffer = frame.to_ratatui_buffer().unwrap();
    let buttons = state.hits.pane_zoom_buttons.clone();
    assert_eq!(buttons.len(), 2);
    for (button, pane_id) in &buttons {
        let pane = state
            .hits
            .panes
            .iter()
            .find(|hit| &hit.pane_id == pane_id)
            .unwrap();
        assert_eq!(*button, Rect::new(pane.rect.right() - 2, pane.rect.y, 2, 2));
        assert_eq!(cell_text(&buffer, *button), ["┌┐", "└┘"]);
    }
}

#[test]
fn zoomed_pane_keeps_the_button_while_other_splits_are_hidden() {
    let mut state = split_state(true);
    state.set_pane_surface(surface());
    let frame = state.compose(106, 24).unwrap();
    assert_eq!(state.hits.panes.len(), 1);
    let (button, _) = state.hits.pane_zoom_buttons[0].clone();
    let buffer = frame.to_ratatui_buffer().unwrap();
    assert_eq!(cell_text(&buffer, button), ["▛▜", "▙▟"]);
}

#[test]
fn single_pane_tabs_have_no_button() {
    let mut single = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    single.set_snapshot(Box::new(snapshot()));
    single.set_pane_surface(surface());
    single.compose(106, 24).unwrap();
    assert!(single.hits.pane_zoom_buttons.is_empty());
}

#[test]
fn mobile_layout_shows_the_button_and_zooms_on_tap() {
    let mut mobile = split_state(false);
    mobile.compose(44, 30).unwrap();
    assert!(!mobile.layout(44, 30).mobile_header.is_empty());
    let (button, pane_id) = mobile.hits.pane_zoom_buttons[0].clone();
    let outcome = click(&mut mobile, button);
    let zoomed = outcome.actions.iter().any(|action| {
        matches!(action, ClientShellAction::Endpoint { request, .. }
            if matches!(&request.method, Method::PaneZoom(params)
                if params.pane_id.as_deref() == Some(pane_id.as_str())))
    });
    assert!(zoomed);
}

#[test]
fn clicking_a_pane_button_toggles_zoom_for_that_pane() {
    let mut state = split_state(false);
    state.compose(106, 24).unwrap();
    for (button, pane_id) in state.hits.pane_zoom_buttons.clone() {
        for corner in [
            Rect::new(button.x, button.y, 1, 1),
            Rect::new(button.right() - 1, button.bottom() - 1, 1, 1),
        ] {
            let outcome = click(&mut state, corner);
            let zoomed = outcome.actions.iter().any(|action| {
                matches!(action, ClientShellAction::Endpoint { request, .. }
                    if matches!(&request.method, Method::PaneZoom(params)
                        if params.mode == PaneZoomMode::Toggle
                            && params.pane_id.as_deref() == Some(pane_id.as_str())))
            });
            assert!(zoomed, "{pane_id}");
        }
    }
    let (button, _) = state.hits.pane_zoom_buttons[0].clone();
    let outside = Rect::new(button.x - 1, button.y, 1, 1);
    assert_eq!(zoom_requests(&click(&mut state, outside)), 0);
}
