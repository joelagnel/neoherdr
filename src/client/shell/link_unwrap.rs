use crate::ghostty::{
    has_url_scheme, plausible_url_continuation, HARD_WRAP_MAX_INDENT, HARD_WRAP_MAX_ROWS,
};
use crate::protocol::FrameData;
use ratatui::layout::Rect;

/// A wrapper breaks a URL after one of these, or at the right edge when the token is too long.
const URL_BREAK: &str = "/-%&?=._#";

/// Characters a sentence word does not carry, so a hanging-indent row containing one is a URL tail.
const URL_ONLY: &str = "/?#=&%_-:~+@";

fn looks_like_url_tail(token: &str) -> bool {
    token
        .trim_end_matches(|c: char| ".,;:!?)]}'\"".contains(c))
        .chars()
        .any(|c| URL_ONLY.contains(c))
}

struct Row<'a> {
    cells: &'a [crate::protocol::CellData],
}

impl Row<'_> {
    fn blank(&self, col: usize) -> bool {
        let symbol = &self.cells[col].symbol;
        !symbol.is_empty() && symbol.chars().all(char::is_whitespace)
    }

    fn token_from(&self, from: usize) -> Option<(usize, usize)> {
        let start = (from..self.cells.len()).find(|&col| !self.blank(col))?;
        let mut end = start;
        while end + 1 < self.cells.len() && !self.blank(end + 1) {
            end += 1;
        }
        Some((start, end))
    }

    fn text(&self, start: usize, end: usize) -> String {
        self.cells[start..=end]
            .iter()
            .map(|cell| cell.symbol.as_str())
            .collect()
    }

    fn content_after(&self, end: usize) -> bool {
        (end + 1..self.cells.len()).any(|col| !self.blank(col))
    }
}

struct Segment {
    row: u16,
    start: usize,
    end: usize,
}

/// Gives a URL that an application or the pane wrapped over several rows one OSC 8 target, so a
/// host terminal that only sees separate rows still opens the whole address. Only `area` is read,
/// and cells that already carry a hyperlink keep it.
pub(super) fn tag_wrapped_urls(frame: &mut FrameData, area: Rect) {
    let area = area.intersection(Rect::new(0, 0, frame.width, frame.height));
    if area.width < 2 || area.height < 2 {
        return;
    }
    let width = usize::from(area.width);
    let first_new = frame.hyperlinks.len();
    let row_at = |row: u16| -> &[crate::protocol::CellData] {
        let start = usize::from(row) * usize::from(frame.width) + usize::from(area.x);
        &frame.cells[start..start + width]
    };
    let mut tags: Vec<(String, Vec<(u16, usize)>)> = Vec::new();
    for row in area.y..area.bottom() {
        let line = Row { cells: row_at(row) };
        let mut from = 0;
        while let Some((start, end)) = line.token_from(from) {
            from = end + 1;
            let text = line.text(start, end);
            let Some(scheme) = ["https://", "http://"]
                .iter()
                .filter_map(|scheme| text.find(scheme))
                .min()
            else {
                continue;
            };
            if line.content_after(end) {
                continue;
            }
            let mut head = start;
            let mut offset = 0;
            while head < end && offset < scheme {
                offset += line.cells[head].symbol.len();
                head += 1;
            }
            let mut joined = text[scheme..].to_owned();
            let head_len = joined.len();
            let mut segments = vec![Segment {
                row,
                start: head,
                end,
            }];
            while segments.len() < HARD_WRAP_MAX_ROWS {
                let last = segments.last().expect("a chain has a head segment");
                let next_row = last.row + 1;
                if next_row >= area.bottom() {
                    break;
                }
                let next = Row {
                    cells: row_at(next_row),
                };
                let Some((first, last_col)) = next.token_from(0) else {
                    break;
                };
                let token = next.text(first, last_col);
                if has_url_scheme(&token) || !plausible_url_continuation(&token) {
                    break;
                }
                if first == 0 {
                    // A soft wrap of the pane's own terminal fills its row to the last column.
                    if last.end != width - 1 {
                        break;
                    }
                } else if first > HARD_WRAP_MAX_INDENT
                    || last.end + 1 + (last_col - first + 1) <= width
                    || !looks_like_url_tail(&token)
                    || !(last.end + 2 >= width
                        || row_at(last.row)[last.end]
                            .symbol
                            .chars()
                            .last()
                            .is_some_and(|c| URL_BREAK.contains(c)))
                {
                    // An application only wraps a token that no longer fits on the row, and a
                    // plain word after a URL is prose.
                    break;
                }
                let shares_line = next.content_after(last_col);
                joined.push_str(&token);
                segments.push(Segment {
                    row: next_row,
                    start: first,
                    end: last_col,
                });
                if shares_line {
                    break;
                }
            }
            if segments.len() < 2 {
                continue;
            }
            let Some(range) = crate::app::actions::url_byte_range(&joined, 0) else {
                continue;
            };
            if range.end <= head_len {
                continue;
            }
            let mut covered = Vec::new();
            let mut offset = 0;
            for segment in &segments {
                for (col, cell) in row_at(segment.row)
                    .iter()
                    .enumerate()
                    .take(segment.end + 1)
                    .skip(segment.start)
                {
                    if offset >= range.end {
                        break;
                    }
                    covered.push((segment.row, col));
                    offset += cell.symbol.len();
                }
            }
            tags.push((joined[range].to_owned(), covered));
        }
    }
    for (uri, covered) in tags {
        let index = frame.hyperlinks[first_new..]
            .iter()
            .position(|existing| *existing == uri)
            .map_or_else(
                || {
                    frame.hyperlinks.push(uri);
                    frame.hyperlinks.len() - 1
                },
                |offset| first_new + offset,
            );
        for (row, col) in covered {
            let idx = usize::from(row) * usize::from(frame.width) + usize::from(area.x) + col;
            if frame.cells[idx].hyperlink.is_none() {
                frame.cells[idx].hyperlink = Some(index as u32);
            }
        }
    }
}
