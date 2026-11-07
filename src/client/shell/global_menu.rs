use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ClientGlobalMenuAction {
    Binding(crate::input::KeybindAction),
    WhatsNew,
    Switch,
    Find,
    Tabs,
    Recent,
    GotoDone,
    NextWorking,
}

pub(super) fn global_menu_attention(snapshot: &ClientShellSnapshot) -> bool {
    snapshot.update_available.is_some() || snapshot.integration_updates_available
}

pub(super) fn global_menu_item_has_badge(
    snapshot: &ClientShellSnapshot,
    action: ClientGlobalMenuAction,
) -> bool {
    (action == ClientGlobalMenuAction::WhatsNew && snapshot.update_available.is_some())
        || (action == ClientGlobalMenuAction::Binding(crate::input::KeybindAction::Settings)
            && snapshot.integration_updates_available)
}

/// The menu entries; `with_navigation` adds the header menu's navigation buttons in front, which
/// the desktop Menu button offers too.
pub(super) fn global_menu_items(
    snapshot: &ClientShellSnapshot,
    with_navigation: bool,
) -> Vec<(&'static str, ClientGlobalMenuAction)> {
    let mut items = if with_navigation {
        vec![
            ("switch", ClientGlobalMenuAction::Switch),
            ("Find", ClientGlobalMenuAction::Find),
            ("tabs", ClientGlobalMenuAction::Tabs),
            ("Recent", ClientGlobalMenuAction::Recent),
            ("Goto done", ClientGlobalMenuAction::GotoDone),
            ("Next working", ClientGlobalMenuAction::NextWorking),
        ]
    } else {
        Vec::new()
    };
    items.extend([
        (
            "settings",
            ClientGlobalMenuAction::Binding(crate::input::KeybindAction::Settings),
        ),
        (
            "keybinds",
            ClientGlobalMenuAction::Binding(crate::input::KeybindAction::Help),
        ),
        (
            "reload config",
            ClientGlobalMenuAction::Binding(crate::input::KeybindAction::ReloadConfig),
        ),
    ]);
    if snapshot.update_available.is_some() || snapshot.latest_release_notes_available {
        items.push((
            if snapshot.update_available.is_some() {
                "update ready"
            } else {
                "what's new"
            },
            ClientGlobalMenuAction::WhatsNew,
        ));
    }
    items.push((
        "detach",
        ClientGlobalMenuAction::Binding(crate::input::KeybindAction::Detach),
    ));
    items
}

impl ClientShellState {
    pub(super) fn toggle_global_menu(&mut self) {
        if matches!(self.overlay, Some(ClientShellOverlay::GlobalMenu(_))) {
            self.overlay = None;
        } else {
            self.overlay = Some(ClientShellOverlay::GlobalMenu(ClientGlobalMenuOverlay {
                highlighted: 0,
            }));
        }
    }

    pub(super) fn move_global_menu_selection(&mut self, delta: isize) {
        let item_count = self
            .snapshot
            .as_deref()
            .map(|snapshot| global_menu_items(snapshot, true))
            .map_or(0, |items| items.len());
        let Some(ClientShellOverlay::GlobalMenu(menu)) = self.overlay.as_mut() else {
            return;
        };
        menu.highlighted = (menu.highlighted as isize + delta)
            .clamp(0, item_count.saturating_sub(1) as isize) as usize;
    }

    pub(super) fn activate_global_menu_item(
        &mut self,
        index: usize,
        outcome: &mut ClientShellInput,
    ) {
        let Some(action) = self.snapshot.as_deref().and_then(|snapshot| {
            global_menu_items(snapshot, true)
                .get(index)
                .map(|(_, action)| *action)
        }) else {
            return;
        };
        self.activate_global_menu_action(action, outcome);
    }

    pub(super) fn activate_global_menu_action(
        &mut self,
        action: ClientGlobalMenuAction,
        outcome: &mut ClientShellInput,
    ) {
        if action == ClientGlobalMenuAction::WhatsNew
            && self
                .snapshot
                .as_deref()
                .and_then(|snapshot| snapshot.release_notes.as_ref())
                .is_none()
        {
            return;
        }
        self.overlay = None;
        match action {
            ClientGlobalMenuAction::Binding(binding) => {
                self.record_binding(crate::input::KeybindMatch::Action(binding), outcome)
            }
            ClientGlobalMenuAction::WhatsNew => self.open_release_notes(),
            ClientGlobalMenuAction::Switch => self.open_mobile_switcher(),
            ClientGlobalMenuAction::Find => self.open_search_overlay(),
            ClientGlobalMenuAction::Tabs => self.open_tabs_overlay(),
            ClientGlobalMenuAction::Recent => self.open_recent_overlay(),
            ClientGlobalMenuAction::GotoDone => self.goto_next_done(outcome),
            ClientGlobalMenuAction::NextWorking => self.goto_next_working(outcome),
        }
        outcome.repaint = true;
    }
}
