use super::*;

impl ClientShellState {
    pub(super) fn open_tab_overflow_menu(&mut self) {
        let Some(snapshot) = self.snapshot.as_deref() else {
            return;
        };
        let workspace_id = snapshot.focused_workspace_id.as_deref();
        let tabs = snapshot
            .tabs
            .iter()
            .filter(|tab| Some(tab.workspace_id.as_str()) == workspace_id)
            .collect::<Vec<_>>();
        let visible = |tab: &ClientShellTab| self.hits.tabs.iter().any(|(_, id)| *id == tab.tab_id);
        let hidden = tabs.iter().filter(|tab| !visible(tab)).count();
        let items = tabs
            .iter()
            .filter(|tab| hidden == 0 || !visible(tab))
            .map(|tab| ClientTabOverflowItem {
                tab_id: tab.tab_id.clone(),
                label: tab.label.clone(),
                focused: tab.focused,
            })
            .collect::<Vec<_>>();
        if items.is_empty() {
            return;
        }
        self.overlay = Some(ClientShellOverlay::TabOverflow(ClientTabOverflowOverlay {
            items,
            anchor_x: self.hits.tab_overflow.x,
            anchor_y: self.hits.tab_overflow.y,
            highlighted: 0,
            scroll: 0,
        }));
    }

    fn tab_overflow_visible_rows(&self) -> usize {
        self.hits.tab_overflow_rows.len().max(1)
    }

    pub(super) fn move_tab_overflow_selection(&mut self, delta: isize) {
        let visible = self.tab_overflow_visible_rows();
        let Some(ClientShellOverlay::TabOverflow(menu)) = self.overlay.as_mut() else {
            return;
        };
        let last = menu.items.len().saturating_sub(1);
        menu.highlighted = (menu.highlighted as isize + delta).clamp(0, last as isize) as usize;
        if menu.highlighted < menu.scroll {
            menu.scroll = menu.highlighted;
        } else if menu.highlighted >= menu.scroll + visible {
            menu.scroll = menu.highlighted + 1 - visible;
        }
    }

    pub(super) fn scroll_tab_overflow(&mut self, delta: isize) {
        let visible = self.tab_overflow_visible_rows();
        let Some(ClientShellOverlay::TabOverflow(menu)) = self.overlay.as_mut() else {
            return;
        };
        let max_scroll = menu.items.len().saturating_sub(visible);
        menu.scroll = (menu.scroll as isize + delta).clamp(0, max_scroll as isize) as usize;
    }

    pub(super) fn activate_tab_overflow_item(
        &mut self,
        index: usize,
        outcome: &mut ClientShellInput,
    ) {
        let Some(ClientShellOverlay::TabOverflow(menu)) = self.overlay.take() else {
            return;
        };
        outcome.repaint = true;
        let Some(item) = menu.items.into_iter().nth(index) else {
            return;
        };
        self.push_endpoint_method(
            crate::api::schema::Method::TabFocus(crate::api::schema::TabTarget {
                tab_id: item.tab_id,
            }),
            outcome,
        );
    }
}
