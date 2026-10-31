use super::super::link_unwrap::tag_wrapped_urls;
use super::*;
use crate::protocol::FrameData;
use ratatui::{buffer::Buffer, layout::Rect};

const WIDTH: u16 = 48;

fn frame(rows: &[&str]) -> FrameData {
    let mut buffer = Buffer::empty(Rect::new(0, 0, WIDTH, rows.len() as u16));
    for (y, row) in rows.iter().enumerate() {
        buffer.set_string(0, y as u16, row, ratatui::style::Style::default());
    }
    FrameData::from_ratatui_buffer(&buffer, None)
}

fn tagged(frame: &FrameData, row: u16) -> Vec<(u16, &str)> {
    (0..frame.width)
        .filter_map(|col| {
            let cell = &frame.cells[usize::from(row) * usize::from(frame.width) + usize::from(col)];
            Some((
                col,
                frame.hyperlinks.get(cell.hyperlink? as usize)?.as_str(),
            ))
        })
        .collect()
}

fn tagged_text(frame: &FrameData, row: u16) -> String {
    tagged(frame, row)
        .iter()
        .map(|(col, _)| {
            frame.cells[usize::from(row) * usize::from(frame.width) + usize::from(*col)]
                .symbol
                .as_str()
        })
        .collect()
}

fn run(rows: &[&str]) -> FrameData {
    let mut frame = frame(rows);
    tag_wrapped_urls(&mut frame, Rect::new(0, 0, WIDTH, rows.len() as u16));
    frame
}

const PAGE: &str = "http://joelbox.pomfret-ray.ts.net:8888/nvidia-workshops/plans/aman-kv-rows-flash-attention.html";
const MP4: &str = "https://joelbox.pomfret-ray.ts.net:9451/files/Technical%20Videos/llama.cpp%20KV%20Cache/Inside%20llama.cpp%20KV%20rows%20-%20cells%2C%20bytes%2C%20and%20flash%20attention.mp4";

#[test]
fn hyphen_broken_indented_url_gets_one_target_on_every_row() {
    let frame = run(&[
        "  • Watch the page (http://joelbox.pomfret-",
        "    ray.ts.net:8888/nvidia-workshops/plans/",
        "    aman-kv-rows-flash-attention.html)",
    ]);
    for row in 0..3 {
        assert!(tagged(&frame, row).iter().all(|(_, uri)| *uri == PAGE));
    }
    assert_eq!(tagged_text(&frame, 0), "http://joelbox.pomfret-");
    assert_eq!(
        tagged_text(&frame, 1),
        "ray.ts.net:8888/nvidia-workshops/plans/"
    );
    assert_eq!(tagged_text(&frame, 2), "aman-kv-rows-flash-attention.html");
}

#[test]
fn short_rows_are_joined_when_the_next_token_would_not_have_fit() {
    let frame = run(&[
        "  • Download the MP4 (https://joelbox.pomfret-",
        "    ray.ts.net:9451/files/Technical%20Videos/",
        "    llama.cpp%20KV%20Cache/",
        "    Inside%20llama.cpp%20KV%20rows%20-%20cells%",
        "    2C%20bytes%2C%20and%20flash%20attention.mp4",
        "    )",
    ]);
    for row in 0..5 {
        assert!(tagged(&frame, row).iter().all(|(_, uri)| *uri == MP4));
        assert!(!tagged(&frame, row).is_empty());
    }
    assert!(tagged(&frame, 5).is_empty());
}

#[test]
fn terminal_soft_wrapped_url_is_joined_across_a_full_row() {
    let head = format!("https://example.com/{}", "a".repeat(28));
    let frame = run(&[&head, "bbbb/cc and then words"]);
    let uri = format!("{head}bbbb/cc");
    assert!(tagged(&frame, 0).iter().all(|(_, found)| *found == uri));
    assert_eq!(tagged_text(&frame, 1), "bbbb/cc");
}

#[test]
fn prose_after_a_short_url_row_is_left_alone() {
    let frame = run(&["see http://example.com/a/b", "    and some more words"]);
    assert!(frame.hyperlinks.is_empty());
}

#[test]
fn prose_words_after_a_url_that_fills_the_row_are_not_joined() {
    for next in ["    first, install the tools", "    today.", "    and more"] {
        let frame = run(&["  • Full instructions: https://example.com/setup", next]);
        assert!(frame.hyperlinks.is_empty(), "{next}");
    }
}

#[test]
fn paths_and_flags_after_a_url_that_ends_in_plain_text_are_not_joined() {
    for rows in [
        [
            "- PR: https://github.com/acme/herdr/pull/1234",
            "  src/client/shell.rs: fix crash on resize",
        ],
        [
            "Usage, see https://example.com/docs",
            "  --output-dir=PATH  where to write",
        ],
    ] {
        let frame = run(&rows);
        assert!(frame.hyperlinks.is_empty(), "{rows:?}");
    }
}

#[test]
fn urls_in_an_offset_area_are_tagged_at_frame_columns() {
    let mut frame = frame(&[
        "XX  • Watch the page (http://joelbox.pomfret-",
        "XX    ray.ts.net:8888/nvidia-workshops/plans/",
        "XX    aman-kv-rows-flash-attention.html)",
    ]);
    tag_wrapped_urls(&mut frame, Rect::new(2, 0, WIDTH - 2, 3));
    assert!(tagged(&frame, 0).iter().all(|(col, _)| *col >= 2));
    assert_eq!(
        tagged_text(&frame, 1),
        "ray.ts.net:8888/nvidia-workshops/plans/"
    );
}

#[test]
fn separate_urls_on_consecutive_rows_are_not_merged() {
    let frame = run(&[
        "  • http://example.com/first-link-that-is-long",
        "  • http://example.com/second",
    ]);
    assert!(frame.hyperlinks.is_empty());
}

#[test]
fn single_row_urls_are_untouched() {
    let frame = run(&["open https://example.com/x now"]);
    assert!(frame.hyperlinks.is_empty());
}

#[test]
fn existing_osc8_cells_keep_their_target() {
    let mut frame = frame(&[
        "  • Watch the page (http://joelbox.pomfret-",
        "    ray.ts.net:8888/nvidia-workshops/plans/",
        "    aman-kv-rows-flash-attention.html)",
    ]);
    frame.hyperlinks.push("https://native.example/".into());
    frame.cells[25].hyperlink = Some(0);
    tag_wrapped_urls(&mut frame, Rect::new(0, 0, WIDTH, 3));
    assert_eq!(frame.cells[25].hyperlink, Some(0));
    assert_eq!(frame.cells[26].hyperlink, Some(1));
    assert_eq!(frame.hyperlinks[1], PAGE);
}

#[test]
fn rows_outside_the_area_are_never_joined() {
    let mut frame = frame(&[
        "  • Watch the page (http://joelbox.pomfret-",
        "    ray.ts.net:8888/nvidia-workshops/plans/",
        "    aman-kv-rows-flash-attention.html)",
    ]);
    tag_wrapped_urls(&mut frame, Rect::new(0, 0, WIDTH, 2));
    assert!(tagged(&frame, 2).is_empty());
    assert!(tagged(&frame, 1)
        .iter()
        .all(|(_, uri)| !uri.ends_with(".html")));
}

fn composed_state(cols: u16, rows: u16, lines: &[&str]) -> ClientShellState {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    let mut surface = surface();
    let mut buffer = Buffer::empty(Rect::new(0, 0, cols, rows));
    for (y, line) in lines.iter().enumerate() {
        buffer.set_string(0, y as u16, line, ratatui::style::Style::default());
    }
    surface.frame = FrameData::from_ratatui_buffer_with_hyperlinks(&buffer, None, &[]);
    let rect = crate::protocol::SurfaceRect {
        x: 0,
        y: 0,
        width: cols,
        height: rows,
    };
    surface.panes[0].rect = rect;
    surface.panes[0].inner_rect = rect;
    state.set_pane_surface(surface);
    state
}

const WRAPPED: [&str; 3] = [
    "  • Watch the page (http://joelbox.pomfret-",
    "    ray.ts.net:8888/nvidia-workshops/plans/",
    "    aman-kv-rows-flash-attention.html)",
];

#[test]
fn mobile_layout_unwraps_urls_even_while_pane_mouse_capture_strips_pane_links() {
    let mut state = composed_state(WIDTH, 18, &WRAPPED);
    let composed = state.compose(WIDTH, 20).expect("mobile frame");
    let top = state.hits.panes[0].inner_rect.y;
    assert_eq!(composed.native_links_from, 0);
    let mut client = crate::client::state::ClientState::test_new();
    client.shell_mouse_capture_preference = true;
    client.mouse_capture_active = true;
    let mut output = Vec::new();
    assert!(client.try_present_frame_to(&mut output, composed));
    let mut host = crate::ghostty::Terminal::new(WIDTH, 20, 0).unwrap();
    host.write(&output);
    for (row, col) in [(0, 21), (1, 4), (1, 42), (2, 4), (2, 36)] {
        assert_eq!(
            host.viewport_hyperlink_uri(col, u32::from(top + row))
                .unwrap()
                .as_deref(),
            Some(PAGE),
            "row {row} col {col}"
        );
    }
    assert_eq!(
        host.viewport_hyperlink_uri(37, u32::from(top + 2)).unwrap(),
        None
    );
}

#[test]
fn desktop_layout_leaves_wrapped_urls_to_the_pane() {
    let mut state = composed_state(106, 30, &WRAPPED);
    let composed = state.compose(106, 30).expect("desktop frame");
    assert!(composed.hyperlinks.is_empty());
}

fn row_patch(state: &ClientShellState) -> crate::protocol::PaneSurfacePatch {
    let surface = state.pane_surface.as_ref().expect("surface");
    let mut pane = surface.panes[0].clone();
    pane.content_revision += 2;
    crate::protocol::PaneSurfacePatch {
        boot_id: surface.boot_id.clone(),
        projection_revision: surface.projection_revision,
        base_surface_revision: surface.surface_revision,
        surface_revision: surface.surface_revision + 1,
        rows: vec![crate::protocol::PaneSurfacePatchRow {
            x: 0,
            y: 0,
            cells: vec![surface.frame.cells[0].clone()],
        }],
        panes: vec![pane],
        cursor: None,
    }
}

#[test]
fn mobile_row_patches_recompose_so_streamed_urls_get_their_link() {
    let mut mobile = composed_state(WIDTH, 18, &WRAPPED);
    mobile.compose(WIDTH, 20).expect("mobile frame");
    let patch = row_patch(&mobile);
    assert!(matches!(
        mobile.apply_pane_surface_patch(patch),
        ClientPaneSurfacePatchOutcome::Applied(None)
    ));

    let mut desktop = composed_state(106, 30, &WRAPPED);
    desktop.compose(106, 30).expect("desktop frame");
    let patch = row_patch(&desktop);
    assert!(matches!(
        desktop.apply_pane_surface_patch(patch),
        ClientPaneSurfacePatchOutcome::Applied(Some(_))
    ));
}
