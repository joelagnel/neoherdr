use super::*;

pub(super) const RECENT_LIMIT: usize = 15;
const RECENT_LOG_CAP: usize = 200;
const HOUR: u64 = 3600;
const DAY: u64 = 24 * HOUR;
const WEEK: u64 = 7 * DAY;

/// When the user last had a pane focused, newest first in `ClientShellState::recent_access`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ClientRecentAccess {
    pub(super) endpoint_id: ClientEndpointId,
    pub(super) pane_id: String,
    pub(super) at: u64,
}

/// One access as shown by the Recent view, with its age fixed when the view opened.
#[derive(Clone, Debug)]
pub(super) struct ClientRecentEntry {
    pub(super) endpoint_id: ClientEndpointId,
    pub(super) pane_id: String,
    pub(super) age_secs: u64,
}

pub(super) fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(super) fn format_age(secs: u64) -> String {
    match secs {
        0..HOUR => "<1h ago".to_owned(),
        HOUR..DAY => format!("{}h ago", secs / HOUR),
        DAY..WEEK => format!("{}d ago", secs / DAY),
        _ => format!("{}w ago", secs / WEEK),
    }
}

pub(super) fn restore_recent_access(
    saved: &[preferences::ClientRecentPreference],
) -> Vec<ClientRecentAccess> {
    let mut log = saved
        .iter()
        .filter_map(|entry| {
            let endpoint_id = match entry.endpoint.as_str() {
                "local" => ClientEndpointId::Local,
                key => ClientEndpointId::Ssh(
                    crate::client::endpoint::ProfileId::parse(key.strip_prefix("ssh:")?).ok()?,
                ),
            };
            Some(ClientRecentAccess {
                endpoint_id,
                pane_id: entry.pane_id.clone(),
                at: entry.at,
            })
        })
        .collect::<Vec<_>>();
    log.sort_by_key(|entry| std::cmp::Reverse(entry.at));
    log.truncate(RECENT_LOG_CAP);
    log
}

pub(super) fn recent_preferences(
    log: &[ClientRecentAccess],
) -> Vec<preferences::ClientRecentPreference> {
    log.iter()
        .map(|entry| preferences::ClientRecentPreference {
            endpoint: entry.endpoint_id.storage_key(),
            pane_id: entry.pane_id.clone(),
            at: entry.at,
        })
        .collect()
}

/// Keeps the newest `RECENT_LIMIT` panes that still exist, in access order, with an age column.
pub(super) fn recent_rows(
    rows: Vec<ClientNavigatorRow>,
    entries: &[ClientRecentEntry],
) -> Vec<ClientNavigatorRow> {
    let mut by_pane = rows
        .into_iter()
        .filter_map(|row| match &row.target {
            ClientNavigatorTarget::Pane {
                endpoint_id,
                pane_id,
            } => Some(((endpoint_id.clone(), pane_id.clone()), row)),
            _ => None,
        })
        .collect::<HashMap<_, _>>();
    entries
        .iter()
        .filter_map(|entry| {
            let mut row = by_pane.remove(&(entry.endpoint_id.clone(), entry.pane_id.clone()))?;
            row.depth = 0;
            row.age = Some(format_age(entry.age_secs));
            Some(row)
        })
        .take(RECENT_LIMIT)
        .collect()
}

impl ClientShellState {
    /// Records the focused pane of the active machine as just accessed; true when the log changed.
    pub(super) fn note_focus_access(&mut self, now: u64) -> bool {
        let Some(pane_id) = self
            .snapshot
            .as_deref()
            .and_then(|snapshot| snapshot.focused_pane_id.clone())
        else {
            return false;
        };
        let key = (self.active_endpoint_id.clone(), pane_id);
        if self.last_focus_access.as_ref() == Some(&key) {
            return false;
        }
        self.recent_access
            .retain(|entry| (&entry.endpoint_id, &entry.pane_id) != (&key.0, &key.1));
        self.recent_access.insert(
            0,
            ClientRecentAccess {
                endpoint_id: key.0.clone(),
                pane_id: key.1.clone(),
                at: now,
            },
        );
        self.recent_access.truncate(RECENT_LOG_CAP);
        self.last_focus_access = Some(key);
        true
    }

    pub(super) fn open_recent_overlay(&mut self) {
        self.open_recent_overlay_at(unix_now());
    }

    pub(super) fn open_recent_overlay_at(&mut self, now: u64) {
        let entries = self
            .recent_access
            .iter()
            .map(|entry| ClientRecentEntry {
                endpoint_id: entry.endpoint_id.clone(),
                pane_id: entry.pane_id.clone(),
                age_secs: now.saturating_sub(entry.at),
            })
            .collect();
        self.open_navigator_overlay_scoped(None, Some(entries));
    }
}
