use super::*;

fn workspace(
    id: &str,
    label: &str,
    status: AgentStatus,
    worktree: Option<(&str, bool)>,
) -> ClientShellWorkspace {
    let mut workspace = snapshot().workspaces[0].clone();
    workspace.workspace_id = id.into();
    workspace.label = label.into();
    workspace.agent_status = status;
    workspace.focused = false;
    workspace.active_tab_id = format!("tab_{id}");
    workspace.worktree = worktree.map(|(key, linked)| crate::protocol::ClientShellWorktree {
        key: key.into(),
        label: label.into(),
        is_linked_worktree: linked,
    });
    workspace
}

fn agent_for(id: &str, workspace_id: &str, status: AgentStatus, seq: u64) -> ClientShellAgent {
    ClientShellAgent {
        pane_id: id.into(),
        workspace_id: workspace_id.into(),
        tab_id: format!("tab_{workspace_id}"),
        name: Some(id.into()),
        display_agent: Some(id.into()),
        agent: Some("pi".into()),
        title: None,
        terminal_title: None,
        terminal_title_stripped: None,
        agent_status: status,
        state_change_seq: seq,
        state_labels: Vec::new(),
        tokens: Vec::new(),
        focused: false,
    }
}

fn state_with(
    workspaces: Vec<ClientShellWorkspace>,
    agents: Vec<ClientShellAgent>,
) -> ClientShellState {
    let mut projected = snapshot();
    let tab = projected.tabs[0].clone();
    projected.tabs = workspaces
        .iter()
        .map(|workspace| {
            let mut tab = tab.clone();
            tab.tab_id = workspace.active_tab_id.clone();
            tab.workspace_id = workspace.workspace_id.clone();
            tab.focused = false;
            tab
        })
        .collect();
    projected.workspaces = workspaces;
    projected.agents = agents;
    projected.focused_workspace_id = None;
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    state
}

fn labels(snapshot: &ClientShellSnapshot, entries: &[WorkspaceEntry]) -> Vec<String> {
    entries
        .iter()
        .map(|entry| snapshot.workspaces[entry.index].label.clone())
        .collect()
}

#[test]
fn spaces_are_ordered_working_blocked_done_then_idle_keeping_ties_stable() {
    let state = state_with(
        vec![
            workspace("a", "idle-first", AgentStatus::Idle, None),
            workspace("b", "done", AgentStatus::Done, None),
            workspace("c", "working", AgentStatus::Working, None),
            workspace("d", "idle-second", AgentStatus::Idle, None),
            workspace("e", "blocked", AgentStatus::Blocked, None),
            workspace("f", "working-two", AgentStatus::Working, None),
        ],
        Vec::new(),
    );
    let snapshot = state.snapshot.as_deref().unwrap();
    let entries = super::super::render::workspace_entries(snapshot, &HashSet::new());
    assert_eq!(
        labels(snapshot, &entries),
        [
            "working",
            "working-two",
            "blocked",
            "done",
            "idle-first",
            "idle-second"
        ]
    );
}

#[test]
fn a_worktree_group_moves_as_one_block_with_its_working_child_first() {
    let state = state_with(
        vec![
            workspace("a", "idle", AgentStatus::Idle, None),
            workspace("p", "parent", AgentStatus::Idle, Some(("repo", false))),
            workspace("c1", "child-idle", AgentStatus::Idle, Some(("repo", true))),
            workspace(
                "c2",
                "child-working",
                AgentStatus::Working,
                Some(("repo", true)),
            ),
        ],
        Vec::new(),
    );
    let snapshot = state.snapshot.as_deref().unwrap();
    let entries = super::super::render::workspace_entries(snapshot, &HashSet::new());
    assert_eq!(
        labels(snapshot, &entries),
        ["parent", "child-working", "child-idle", "idle"]
    );
    assert!(entries[1].indented && !entries[1].last_child);
    assert!(entries[2].indented && entries[2].last_child);
}

#[test]
fn agents_are_ordered_working_blocked_then_idle_in_both_sort_modes() {
    use crate::config::AgentPanelSortConfig;
    let state = state_with(
        vec![workspace("w", "ws", AgentStatus::Idle, None)],
        vec![
            agent_for("idle-a", "w", AgentStatus::Idle, 5),
            agent_for("working", "w", AgentStatus::Working, 1),
            agent_for("blocked", "w", AgentStatus::Blocked, 3),
            agent_for("idle-b", "w", AgentStatus::Idle, 2),
        ],
    );
    let snapshot = state.snapshot.as_deref().unwrap();
    for sort in [AgentPanelSortConfig::Spaces, AgentPanelSortConfig::Priority] {
        let order = super::super::agent_sidebar::ordered_agent_pane_ids(snapshot, sort);
        assert_eq!(&order[..2], ["working", "blocked"], "{sort:?}: {order:?}");
        assert!(order[2..].contains(&"idle-a".to_owned()), "{order:?}");
    }
}

#[test]
fn mobile_switcher_and_desktop_sidebar_list_working_spaces_and_agents_first() {
    let workspaces = vec![
        workspace("a", "idle-space", AgentStatus::Idle, None),
        workspace("b", "working-space", AgentStatus::Working, None),
    ];
    let agents = vec![
        agent_for("idle-agent", "a", AgentStatus::Idle, 2),
        agent_for("working-agent", "b", AgentStatus::Working, 1),
    ];

    let mut state = state_with(workspaces.clone(), agents.clone());
    state.mode = ClientShellMode::Navigate;
    let frame = state.compose(60, 40).expect("mobile switcher");
    let rows = frame_rows(&frame);
    let at = |needle: &str| {
        rows.iter()
            .position(|row| row.contains(needle))
            .unwrap_or_else(|| panic!("{needle:?} in {rows:#?}"))
    };
    assert!(at("working-space") < at("idle-space"), "mobile spaces");
    let agents_title = at("agents · Local");
    let working_agent = rows
        .iter()
        .enumerate()
        .skip(agents_title)
        .find(|(_, row)| row.contains("working-space"))
        .map(|(index, _)| index)
        .expect("working agent row");
    let idle_agent = rows
        .iter()
        .enumerate()
        .skip(agents_title)
        .find(|(_, row)| row.contains("idle-space"))
        .map(|(index, _)| index)
        .expect("idle agent row");
    assert!(working_agent < idle_agent, "mobile agents");

    let mut state = state_with(workspaces, agents);
    state.compose(100, 40).expect("desktop shell");
    let y = |id: &str| {
        state
            .hits
            .workspaces
            .iter()
            .find(|hit| hit.workspace_id == id)
            .map(|hit| hit.rect.y)
            .expect("workspace hit")
    };
    assert!(y("b") < y("a"), "desktop spaces");
    let agent_y = |id: &str| {
        state
            .hits
            .agents
            .iter()
            .find(|(_, pane)| pane == id)
            .map(|(rect, _)| rect.y)
            .expect("agent hit")
    };
    assert!(
        agent_y("working-agent") < agent_y("idle-agent"),
        "desktop agents"
    );
}

fn with_seen_order(state: &mut ClientShellState, seen: &[(&str, u64)]) -> Box<ClientShellSnapshot> {
    let mut snapshot = state.snapshot.take().expect("snapshot");
    snapshot.seen_order = seen
        .iter()
        .map(|(id, order)| ((*id).to_owned(), *order))
        .collect();
    snapshot
}

#[test]
fn a_just_seen_space_becomes_the_first_idle_one_below_the_colored_ones() {
    let mut state = state_with(
        vec![
            workspace("a", "idle-old", AgentStatus::Idle, None),
            workspace("b", "seen-first", AgentStatus::Idle, None),
            workspace("c", "working", AgentStatus::Working, None),
            workspace("d", "seen-last", AgentStatus::Idle, None),
            workspace("e", "done", AgentStatus::Done, None),
            workspace("f", "idle-newer", AgentStatus::Idle, None),
        ],
        Vec::new(),
    );
    let snapshot = with_seen_order(&mut state, &[("b", 1), ("d", 2)]);
    let entries = super::super::render::workspace_entries(&snapshot, &HashSet::new());
    assert_eq!(
        labels(&snapshot, &entries),
        [
            "working",
            "done",
            "seen-last",
            "seen-first",
            "idle-old",
            "idle-newer"
        ]
    );
}

#[test]
fn a_worktree_group_with_a_just_seen_member_leads_the_idle_spaces() {
    let mut state = state_with(
        vec![
            workspace("a", "idle", AgentStatus::Idle, None),
            workspace("p", "parent", AgentStatus::Idle, Some(("repo", false))),
            workspace("c1", "child-old", AgentStatus::Idle, Some(("repo", true))),
            workspace("c2", "child-seen", AgentStatus::Idle, Some(("repo", true))),
            workspace("w", "working", AgentStatus::Working, None),
        ],
        Vec::new(),
    );
    let snapshot = with_seen_order(&mut state, &[("c2", 4)]);
    let entries = super::super::render::workspace_entries(&snapshot, &HashSet::new());
    assert_eq!(
        labels(&snapshot, &entries),
        ["working", "parent", "child-seen", "child-old", "idle"]
    );
}

#[test]
fn a_just_seen_agent_becomes_the_first_idle_one_in_both_sort_modes() {
    use crate::config::AgentPanelSortConfig;
    let mut state = state_with(
        vec![workspace("w", "ws", AgentStatus::Idle, None)],
        vec![
            agent_for("idle-old", "w", AgentStatus::Idle, 9),
            agent_for("seen-first", "w", AgentStatus::Idle, 3),
            agent_for("working", "w", AgentStatus::Working, 1),
            agent_for("seen-last", "w", AgentStatus::Idle, 2),
        ],
    );
    let snapshot = with_seen_order(&mut state, &[("seen-first", 7), ("seen-last", 8)]);
    for sort in [AgentPanelSortConfig::Spaces, AgentPanelSortConfig::Priority] {
        let order = super::super::agent_sidebar::ordered_agent_pane_ids(&snapshot, sort);
        assert_eq!(
            order,
            ["working", "seen-last", "seen-first", "idle-old"],
            "{sort:?}"
        );
    }
}

fn flow_snapshot(revision: u64, herdr: AgentStatus, herdr_seq: u64) -> ClientShellSnapshot {
    let mut projected = snapshot();
    projected.boot_id = "flow-boot".into();
    projected.revision = revision;
    let mut workspaces = vec![
        workspace("a", "idle-one", AgentStatus::Idle, None),
        workspace("h", "herdr", AgentStatus::Idle, None),
        workspace("c", "idle-two", AgentStatus::Idle, None),
    ];
    workspaces[1].focused = true;
    let tab = projected.tabs[0].clone();
    projected.tabs = workspaces
        .iter()
        .map(|workspace| {
            let mut tab = tab.clone();
            tab.tab_id = workspace.active_tab_id.clone();
            tab.workspace_id = workspace.workspace_id.clone();
            tab
        })
        .collect();
    projected.workspaces = workspaces;
    projected.agents = vec![
        agent_for("pane-a", "a", AgentStatus::Idle, 1),
        agent_for("pane-h", "h", herdr, herdr_seq),
        agent_for("pane-c", "c", AgentStatus::Idle, 1),
    ];
    projected.focused_workspace_id = Some("h".into());
    projected
}

fn flow_order(state: &ClientShellState) -> Vec<String> {
    let snapshot = state.snapshot.as_deref().expect("snapshot");
    let entries = super::super::render::workspace_entries(snapshot, &HashSet::new());
    labels(snapshot, &entries)
}

/// Working -> finished (unseen) -> clicked: the clicked space must stay at the top of the
/// idle ones instead of falling back to its original position.
#[test]
fn a_clicked_finished_space_stays_below_the_colored_ones() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    let local = ClientEndpointId::Local;
    state.set_snapshot(Box::new(flow_snapshot(1, AgentStatus::Working, 1)));
    state.set_pane_surface(surface());
    assert_eq!(
        flow_order(&state),
        ["herdr", "idle-one", "idle-two"],
        "working leads"
    );

    state.set_endpoint_snapshot(&local, Box::new(flow_snapshot(2, AgentStatus::Idle, 2)));
    assert_eq!(
        flow_order(&state),
        ["herdr", "idle-one", "idle-two"],
        "finished and unseen is still colored"
    );
    let status = state.snapshot.as_deref().unwrap().workspaces[1].agent_status;
    assert_eq!(status, AgentStatus::Done);

    let mut presented = surface();
    presented.boot_id = "flow-boot".into();
    presented.projection_revision = 2;
    presented.panes[0].pane_id = "pane-h".into();
    assert!(
        state.acknowledge_active_surface_agents(&presented),
        "click acknowledges"
    );
    let status = state.snapshot.as_deref().unwrap().workspaces[1].agent_status;
    assert_eq!(status, AgentStatus::Idle, "color cleared");
    assert_eq!(
        flow_order(&state),
        ["herdr", "idle-one", "idle-two"],
        "stays the first idle entry instead of dropping below idle-one"
    );

    // The same holds for the agent list, in both sort modes and on the mobile path.
    {
        use crate::config::AgentPanelSortConfig;
        let snapshot = state.snapshot.as_deref().unwrap();
        for sort in [AgentPanelSortConfig::Spaces, AgentPanelSortConfig::Priority] {
            let order = super::super::agent_sidebar::ordered_agent_pane_ids(snapshot, sort);
            assert_eq!(order[0], "pane-h", "sidebar agents, {sort:?}: {order:?}");
            let rows = super::super::aggregate_navigation::aggregate_agent_rows(
                &state.endpoints,
                &state.active_endpoint_id,
                sort,
            );
            assert_eq!(rows[0].agent.pane_id, "pane-h", "switcher agents, {sort:?}");
        }
    }

    // A later server snapshot (new revision) must not undo it.
    state.set_endpoint_snapshot(&local, Box::new(flow_snapshot(3, AgentStatus::Idle, 2)));
    assert_eq!(flow_order(&state), ["herdr", "idle-one", "idle-two"]);
}
