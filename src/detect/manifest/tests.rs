use super::*;

// Codex is only a registry key here; behavior tests supply synthetic rules.
fn remote_manifest(version: &str, state: &str, contains: &str) -> String {
    format!(
        r#"
id = "codex"
version = "{version}"
min_engine_version = 1
updated_at = "2026-06-10T12:00:00Z"

[[rules]]
id = "test"
state = "{state}"
contains = ["{contains}"]
"#
    )
}

fn local_manifest(state: &str, contains: &str) -> String {
    format!(
        r#"
id = "codex"

[[rules]]
id = "test"
state = "{state}"
contains = ["{contains}"]
"#
    )
}

fn rules_manifest(rules: &str) -> String {
    format!(
        r#"
id = "codex"

{rules}
"#
    )
}

fn with_manifest_dirs<T>(name: &str, f: impl FnOnce() -> T) -> T {
    let _guard = crate::config::test_config_env_lock().lock().unwrap();
    let old_config = std::env::var_os("XDG_CONFIG_HOME");
    let old_state = std::env::var_os("XDG_STATE_HOME");
    let base = std::env::temp_dir().join(format!(
        "herdr-manifest-loader-{name}-{}",
        std::process::id()
    ));
    let config_dir = base.join("config");
    let state_dir = base.join("state");
    let _ = std::fs::remove_dir_all(&base);
    std::env::set_var("XDG_CONFIG_HOME", &config_dir);
    std::env::set_var("XDG_STATE_HOME", &state_dir);
    reload_manifests();
    let result = f();
    match old_config {
        Some(value) => std::env::set_var("XDG_CONFIG_HOME", value),
        None => std::env::remove_var("XDG_CONFIG_HOME"),
    }
    match old_state {
        Some(value) => std::env::set_var("XDG_STATE_HOME", value),
        None => std::env::remove_var("XDG_STATE_HOME"),
    }
    reload_manifests();
    let _ = std::fs::remove_dir_all(&base);
    result
}

fn write_remote_codex(content: &str) {
    let path = crate::detect::manifest_update::remote_manifest_path(Agent::Codex);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
    reload_manifests();
}

fn write_remote_codex_without_reload(content: &str) {
    let path = crate::detect::manifest_update::remote_manifest_path(Agent::Codex);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn write_local_codex(content: &str) {
    let path = override_path(Agent::Codex).unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
    reload_manifests();
}

#[test]
fn known_agent_no_match_defaults_to_idle_fallback() {
    with_manifest_dirs("no-match", || {
        write_local_codex(&local_manifest("working", "active-marker"));
        let explain = explain(Agent::Codex, "unmatched-marker");

        assert_eq!(explain.state, AgentState::Idle);
        assert!(!explain.visible_idle);
        assert_eq!(
            explain.fallback_reason.as_deref(),
            Some(DEFAULT_KNOWN_AGENT_IDLE_FALLBACK)
        );
    });
}

#[test]
fn rule_semantics_apply_gates_priority_and_line_regex() {
    with_manifest_dirs("rule-semantics", || {
        write_local_codex(&rules_manifest(
            r#"
[[rules]]
id = "low_contains"
state = "idle"
priority = 1
contains = ["match"]

[[rules]]
id = "high_nested_gates"
state = "working"
priority = 10
contains = ["match"]
all = [
  { any = [{ regex = ["w[io]n"] }, { contains = ["fallback"] }] },
]
not = [
  { contains = ["blocked"] },
]

[[rules]]
id = "line_regex"
state = "blocked"
priority = 20
line_regex = ["^exact line$"]
"#,
        ));

        let high = explain(Agent::Codex, "match win");
        assert_eq!(high.state, AgentState::Working);
        assert_eq!(
            high.matched_rule.as_ref().map(|rule| rule.id.as_str()),
            Some("high_nested_gates")
        );

        let not_gate = explain(Agent::Codex, "match win blocked");
        assert_eq!(not_gate.state, AgentState::Idle);
        assert_eq!(
            not_gate.matched_rule.as_ref().map(|rule| rule.id.as_str()),
            Some("low_contains")
        );

        let line = explain(Agent::Codex, "before\nexact line\nafter");
        assert_eq!(line.state, AgentState::Blocked);
        assert_eq!(
            line.matched_rule.as_ref().map(|rule| rule.id.as_str()),
            Some("line_regex")
        );
    });
}

// A synthetic terminal protocol: live controls may precede a named divider
// and an optional panel, but a newer prompt/output makes them historical.
#[test]
fn end_anchored_footer_gates_allow_only_auxiliary_tail() {
    with_manifest_dirs("footer-tail", || {
        write_local_codex(&rules_manifest(
            r#"
[[rules]]
id = "live_controls"
state = "blocked"
region = "whole_recent"
regex = ['(?i)(?:^|\n)controls:\s+(?:confirm-marker|select-marker\s+navigate-marker)\s+cancel-marker[ \t]*\n---[^\n]*(?:\npanel: \d+(?:\nrow: [^\n]+)+(?:\n… \+\d+ pending)?)?(?:\n[ \t]*)*\z']
"#,
        ));
        let footer = "controls: select-marker navigate-marker cancel-marker";
        for tail in [
            "--- name",
            "--- name\npanel: 5\nrow: active\nrow: pending\nrow: pending\nrow: pending\nrow: done\n",
            "--- name\npanel: 5\nrow: active\nrow: done\n… +3 pending",
        ] {
            let live = format!("old output\n{footer}\n{tail}");
            assert_eq!(explain(Agent::Codex, &live).state, AgentState::Blocked);
            for later in [
                "new output",
                "prompt:",
                "--- next\nprompt:",
                "prompt: continue…",
                "working: processing…",
                "…ordinary output",
                "finished…",
                "… +2 arbitrary words",
            ] {
                assert_eq!(
                    explain(Agent::Codex, &format!("{live}\n{later}")).state,
                    AgentState::Idle
                );
            }
        }
        for tail in [
            "--- name\n… +2 pending",
            "--- name\nrow: active",
            "--- name\npanel: 5",
            "--- name\npanel: 5\nrow: active\n  working: processing…",
            "--- name\npanel: 5\nrow: active\n  prompt: continue…",
        ] {
            assert_eq!(
                explain(Agent::Codex, &format!("{footer}\n{tail}")).state,
                AgentState::Idle
            );
        }
        for controls in [
            "controls: confirm-marker cancel-marker",
            "controls: select-marker\nnavigate-marker cancel-marker",
            "controls:\nselect-marker\nnavigate-marker\ncancel-marker",
        ] {
            assert_eq!(
                explain(Agent::Codex, &format!("{controls}\n--- name")).state,
                AgentState::Blocked
            );
        }
        for controls in [
            "controls: select-marker cancel-marker",
            "controls: navigate-marker cancel-marker",
            "controls: confirm-marker",
            "controls: select-marker unrelated navigate-marker cancel-marker",
            "controls: select-marker\nprompt: navigate-marker cancel-marker",
        ] {
            assert_eq!(
                explain(
                    Agent::Codex,
                    &format!("confirm-marker navigate-marker\n{controls}\n--- name")
                )
                .state,
                AgentState::Idle
            );
        }
    });
}

#[test]
fn paired_alternatives_require_complete_gates_at_bottom_buffer() {
    with_manifest_dirs("paired-alternatives", || {
        write_local_codex(&rules_manifest(
            r#"
[[rules]]
id = "paired"
state = "blocked"
region = "bottom_non_empty_lines(3)"
visible_blocker = true
any = [
  { all = [
    { contains = ["alpha heading"] },
    { regex = ['alpha\s+control\s*\z'] },
  ] },
  { all = [
    { contains = ["beta heading"] },
    { regex = ['beta\s+control\s*\z'] },
  ] },
]
"#,
        ));
        for text in [
            "alpha heading\nalpha control",
            "beta heading\nbeta\ncontrol",
        ] {
            let result = explain(Agent::Codex, text);
            assert_eq!(result.state, AgentState::Blocked);
            assert!(result.visible_blocker);
        }
        for text in [
            "alpha heading\nbeta control",
            "beta heading\nalpha control",
            "beta control",
            "beta heading",
            "beta heading\nbeta control\nnewer content",
            "beta heading\nfiller one\nfiller two\nbeta control",
        ] {
            let result = explain(Agent::Codex, text);
            assert!(result.matched_rule.is_none(), "{text}");
            assert!(!result.visible_blocker, "{text}");
        }
    });
}

#[test]
fn remote_manifest_loads_between_local_override_and_bundled() {
    with_manifest_dirs("remote-source", || {
        write_remote_codex(&remote_manifest("9999.01.01.1", "blocked", "remote-ready"));

        let explain = explain(Agent::Codex, "remote-ready");

        assert_eq!(explain.state, AgentState::Blocked);
        assert!(matches!(
            explain.source,
            Some(ManifestSource::Remote { .. })
        ));
        assert_eq!(explain.manifest_version.as_deref(), Some("9999.01.01.1"));
        assert_eq!(
            explain.cached_remote_version.as_deref(),
            Some("9999.01.01.1")
        );
    });
}

#[test]
fn fallback_explain_preserves_active_manifest_version() {
    with_manifest_dirs("fallback-version", || {
        write_remote_codex(&remote_manifest("9999.01.01.1", "blocked", "remote-ready"));

        let explain = explain(Agent::Codex, "ordinary prompt text");

        assert_eq!(explain.state, AgentState::Idle);
        assert_eq!(
            explain.fallback_reason.as_deref(),
            Some(DEFAULT_KNOWN_AGENT_IDLE_FALLBACK)
        );
        assert_eq!(explain.manifest_version.as_deref(), Some("9999.01.01.1"));
        assert!(matches!(
            explain.source,
            Some(ManifestSource::Remote { .. })
        ));
    });
}

#[test]
fn older_cached_remote_manifest_does_not_shadow_newer_bundled_manifest() {
    with_manifest_dirs("older-remote-bundled-fallback", || {
        write_remote_codex(&remote_manifest("2026.06.10.0", "blocked", "remote-ready"));

        let explain = explain(Agent::Codex, "remote-ready");

        assert!(matches!(explain.source, Some(ManifestSource::Bundled)));
        assert_eq!(
            explain.cached_remote_version.as_deref(),
            Some("2026.06.10.0")
        );
        assert!(explain
            .warning
            .as_deref()
            .is_some_and(|warning| warning.contains("older than bundled")));
    });
}

#[test]
fn local_override_shadows_cached_remote_manifest() {
    with_manifest_dirs("local-shadows-remote", || {
        write_remote_codex(&remote_manifest("9999.01.01.1", "blocked", "remote-ready"));
        write_local_codex(&local_manifest("idle", "local-ready"));

        let explain = explain(Agent::Codex, "local-ready");

        assert_eq!(explain.state, AgentState::Idle);
        assert!(matches!(explain.source, Some(ManifestSource::Override(_))));
        assert!(explain.local_override_shadowing_remote);
        assert_eq!(
            explain.cached_remote_version.as_deref(),
            Some("9999.01.01.1")
        );
    });
}

#[test]
fn invalid_local_override_falls_back_to_cached_remote_manifest() {
    with_manifest_dirs("invalid-local-remote-fallback", || {
        write_remote_codex(&remote_manifest("9999.01.01.1", "blocked", "remote-ready"));
        write_local_codex("id = ");

        let explain = explain(Agent::Codex, "remote-ready");

        assert_eq!(explain.state, AgentState::Blocked);
        assert!(matches!(
            explain.source,
            Some(ManifestSource::Remote { .. })
        ));
        assert!(explain.warning.is_some());
    });
}

#[test]
fn detection_uses_cached_manifest_until_explicit_reload() {
    with_manifest_dirs("cache-boundary", || {
        write_remote_codex(&remote_manifest("9999.01.01.1", "blocked", "cached-ready"));

        let cached = explain(Agent::Codex, "cached-ready");
        assert_eq!(cached.state, AgentState::Blocked);
        assert!(matches!(cached.source, Some(ManifestSource::Remote { .. })));
        assert_eq!(
            cached.matched_rule.as_ref().map(|rule| rule.id.as_str()),
            Some("test")
        );

        write_remote_codex_without_reload(&remote_manifest("9999.01.01.2", "working", "new-ready"));

        let unchanged = explain(Agent::Codex, "new-ready");
        assert_eq!(unchanged.state, AgentState::Idle);
        assert_eq!(
            unchanged.fallback_reason.as_deref(),
            Some(DEFAULT_KNOWN_AGENT_IDLE_FALLBACK)
        );
        assert_eq!(
            unchanged.cached_remote_version.as_deref(),
            Some("9999.01.01.1")
        );

        reload_manifests();

        let reloaded = explain(Agent::Codex, "new-ready");
        assert_eq!(reloaded.state, AgentState::Working);
        assert_eq!(
            reloaded.cached_remote_version.as_deref(),
            Some("9999.01.01.2")
        );
        assert_eq!(
            reloaded.matched_rule.as_ref().map(|rule| rule.id.as_str()),
            Some("test")
        );
    });
}

#[test]
fn compiled_rules_are_shared_until_manifest_reload() {
    with_manifest_dirs("shared-compiled-rules", || {
        write_remote_codex(&format!(
            "{}\nregex = ['^cached-[a-z]+$']\n",
            remote_manifest("9999.01.01.1", "blocked", "cached-ready")
        ));
        let first = load_manifest(Agent::Codex).unwrap();
        let second = load_manifest(Agent::Codex).unwrap();
        assert!(!first.compiled_rules.is_empty());
        assert_eq!(
            first.compiled_rules.as_ptr(),
            second.compiled_rules.as_ptr(),
            "cached loads must retain the same compiled rules and regex search caches"
        );

        write_remote_codex_without_reload(&format!(
            "{}\nregex = ['^new-[a-z]+$']\n",
            remote_manifest("9999.01.01.2", "working", "new-ready")
        ));
        let unchanged = load_manifest(Agent::Codex).unwrap();
        assert_eq!(
            first.compiled_rules.as_ptr(),
            unchanged.compiled_rules.as_ptr()
        );

        reload_manifests_for_agents(&[Agent::Codex]);
        let reloaded = load_manifest(Agent::Codex).unwrap();
        let shared_reload = load_manifest(Agent::Codex).unwrap();
        assert_ne!(
            first.compiled_rules.as_ptr(),
            reloaded.compiled_rules.as_ptr()
        );
        assert_eq!(
            reloaded.compiled_rules.as_ptr(),
            shared_reload.compiled_rules.as_ptr()
        );
        assert!(compiled_rule_matches(
            &first.compiled_rules[0],
            "cached-ready"
        ));
        assert!(!compiled_rule_matches(
            &first.compiled_rules[0],
            "new-ready"
        ));
        assert_eq!(
            explain(Agent::Codex, "new-ready").state,
            AgentState::Working
        );

        std::thread::scope(|scope| {
            for _ in 0..4 {
                let reloaded = &reloaded;
                scope.spawn(move || {
                    let loaded = load_manifest(Agent::Codex).unwrap();
                    assert_eq!(
                        loaded.compiled_rules.as_ptr(),
                        reloaded.compiled_rules.as_ptr()
                    );
                    for _ in 0..8 {
                        assert_eq!(detect(Agent::Codex, "new-ready").state, AgentState::Working);
                    }
                });
            }
        });
    });
}

#[test]
fn osc_regions_use_separate_inputs_and_share_rule_priority() {
    with_manifest_dirs("osc-regions", || {
        write_local_codex(&rules_manifest(
            r#"
[[rules]]
id = "screen"
state = "idle"
priority = 10
region = "whole_recent"
visible_idle = true
contains = ["screen-marker"]

[[rules]]
id = "title"
state = "working"
priority = 20
region = "osc_title"
visible_working = true
regex = ['^title-marker$']

[[rules]]
id = "progress"
state = "blocked"
priority = 30
region = "osc_progress"
visible_blocker = true
regex = ['^progress-marker$']
"#,
        ));
        for (screen, title, progress, state, rule) in [
            ("screen-marker", "", "", AgentState::Idle, "screen"),
            (
                "screen-marker",
                "title-marker",
                "",
                AgentState::Working,
                "title",
            ),
            (
                "screen-marker",
                "title-marker",
                "progress-marker",
                AgentState::Blocked,
                "progress",
            ),
            (
                "screen-marker title-marker progress-marker",
                "",
                "",
                AgentState::Idle,
                "screen",
            ),
        ] {
            let input = DetectionInput {
                screen,
                osc_title: title,
                osc_progress: progress,
            };
            let result = explain_with_input(Agent::Codex, input);
            assert_eq!(result.state, state);
            assert_eq!(
                result
                    .matched_rule
                    .as_ref()
                    .map(|matched| matched.id.as_str()),
                Some(rule)
            );
            let detection =
                crate::detect::detect_agent_with_osc(Some(Agent::Codex), screen, title, progress);
            assert_eq!(detection.state, state);
            assert_eq!(detection.visible_idle, state == AgentState::Idle);
            assert_eq!(detection.visible_working, state == AgentState::Working);
            assert_eq!(detection.visible_blocker, state == AgentState::Blocked);
        }
        let swapped = explain_with_input(
            Agent::Codex,
            DetectionInput {
                screen: "",
                osc_title: "progress-marker",
                osc_progress: "title-marker",
            },
        );
        assert!(swapped.matched_rule.is_none());
    });
}

#[test]
fn skip_rule_suppresses_state_update_without_visible_state_evidence() {
    with_manifest_dirs("skip-rule", || {
        write_local_codex(&rules_manifest(
            r#"
[[rules]]
id = "activity"
state = "working"
priority = 10
visible_working = true
contains = ["activity-marker"]

[[rules]]
id = "overlay"
state = "unknown"
priority = 20
skip_state_update = true
contains = ["overlay-marker"]
"#,
        ));
        let screen = "activity-marker overlay-marker";
        let result = explain(Agent::Codex, screen);
        assert_eq!(result.state, AgentState::Unknown);
        assert!(result.skip_state_update);
        assert_eq!(
            result.skipped_update_reason.as_deref(),
            Some("matched_rule:overlay")
        );
        assert!(!result.visible_idle);
        assert!(!result.visible_working);
        assert!(!result.visible_blocker);
        assert!(detect(Agent::Codex, screen).skip_state_update);
    });
}

#[test]
fn screen_regions_extract_structure_without_classifying_agent_state() {
    for (screen, spec, expected) in [
        ("old\n\nnew\n", "bottom_lines(2)", "\nnew\n"),
        (
            "before\n› input\nafter\n",
            "after_last_prompt_marker",
            "after\n",
        ),
        (
            "before\n› input\nafter\n",
            "before_current_prompt_marker",
            "before\n",
        ),
        (
            "before\n› input\nafter\n",
            "whole_recent_without_current_prompt_marker",
            "",
        ),
        (
            "no marker\n",
            "whole_recent_without_current_prompt_marker",
            "no marker\n",
        ),
        (
            "• old\n■ latest\n› input\n",
            "current_prompt_block_marker",
            "■ latest",
        ),
        (
            "• old\n■ latest\n› input\n",
            "after_current_prompt_block_marker",
            "■ latest\n› input\n",
        ),
        ("› old\n• new\n", "current_prompt_block_marker", ""),
        (
            "above\n\n───\nbody\n───\nfooter\n",
            "above_prompt_box",
            "above\n\n",
        ),
        (
            "above\n\n───\nbody\n───\nfooter\n",
            "last_non_empty_above_prompt_box",
            "above",
        ),
        (
            "above\n───\nbody\n───\nfooter\n",
            "prompt_box_body",
            "body\n",
        ),
        (
            "above\n───\nbody\n───\nfooter\n",
            "after_last_horizontal_rule",
            "footer\n",
        ),
    ] {
        assert_eq!(
            region(
                DetectionInput {
                    screen,
                    osc_title: "",
                    osc_progress: ""
                },
                spec
            ),
            expected,
            "region={spec}"
        );
    }
}

#[test]
fn all_bundled_manifests_parse_and_validate() {
    for agent in Agent::SCREEN_MANIFEST_AGENTS {
        assert!(
            bundled_manifest(agent).is_some(),
            "missing bundled manifest for {}",
            agent_label(agent)
        );
    }
}

#[test]
fn manifest_validation_rejects_unknown_fields_empty_rules_invalid_regions_and_regexes() {
    assert!(parse_manifest(
        r#"
id = "codex"

[[rules]]
id = "typo"
state = "working"
contain = ["Working"]
"#
    )
    .is_err());
    assert!(parse_manifest(
        r#"
id = "codex"

[[rules]]
id = "empty"
state = "working"
"#
    )
    .is_err());
    assert!(parse_manifest(
        r#"
id = "codex"

[[rules]]
id = "bad_region"
state = "working"
region = "after_last_promt_marker"
contains = ["Working"]
"#
    )
    .is_err());
    assert!(parse_manifest(
        r#"
id = "codex"

[[rules]]
id = "bad_regex"
state = "working"
regex = ["["]
"#
    )
    .is_err());
    assert!(parse_manifest(
        r#"
id = "codex"

[[rules]]
id = "bad_nested_regex"
state = "working"
any = [{ line_regex = ["["] }]
"#
    )
    .is_err());
}

#[test]
fn manifest_validation_keeps_skip_rules_neutral() {
    assert!(parse_manifest(
        r#"
id = "codex"

[[rules]]
id = "bad_skip_state"
state = "idle"
skip_state_update = true
contains = ["menu"]
"#
    )
    .is_err());
    assert!(parse_manifest(
        r#"
id = "codex"

[[rules]]
id = "bad_skip_visible"
state = "unknown"
skip_state_update = true
visible_blocker = true
contains = ["menu"]
"#
    )
    .is_err());
}

#[test]
fn manifest_validation_rejects_excessive_rule_count() {
    let mut manifest = String::from(
        r#"
id = "codex"
"#,
    );
    for index in 0..129 {
        manifest.push_str(&format!(
            r#"
[[rules]]
id = "rule_{index}"
state = "idle"
contains = ["ready"]
"#
        ));
    }
    assert!(parse_manifest(&manifest).is_err());
}

#[test]
fn manifest_validation_rejects_excessive_gate_depth() {
    let manifest = r#"
id = "codex"

[[rules]]
id = "deep"
state = "idle"
contains = ["ready"]
all = [
  { contains = ["1"], all = [
    { contains = ["2"], all = [
      { contains = ["3"], all = [
        { contains = ["4"], all = [
          { contains = ["5"], all = [
            { contains = ["6"], all = [
              { contains = ["7"], all = [
                { contains = ["8"], all = [
                  { contains = ["9"] },
                ] },
              ] },
            ] },
          ] },
        ] },
      ] },
    ] },
  ] },
]
"#;
    assert!(parse_manifest(manifest).is_err());
}

#[test]
fn manifest_validation_rejects_excessive_matchers() {
    let matchers = (0..33)
        .map(|index| format!(r#""m{index}""#))
        .collect::<Vec<_>>()
        .join(", ");
    let manifest = format!(
        r#"
id = "codex"

[[rules]]
id = "many"
state = "idle"
contains = [{matchers}]
"#
    );
    assert!(parse_manifest(&manifest).is_err());
}

#[test]
fn bottom_non_empty_lines_uses_bottom_occurrence_for_repeated_text() {
    let content = "marker\nold\n\nmiddle\nmarker\nnew\n";
    assert_eq!(
        region(
            DetectionInput {
                screen: content,
                osc_title: "",
                osc_progress: ""
            },
            "bottom_non_empty_lines(2)"
        ),
        "marker\nnew\n"
    );
}

#[test]
fn top_non_empty_lines_uses_top_occurrence_for_repeated_text() {
    let content = "\nmarker\nold\n\nmiddle\nmarker\nnew\n";
    assert_eq!(
        region(
            DetectionInput {
                screen: content,
                osc_title: "",
                osc_progress: ""
            },
            "top_non_empty_lines(2)"
        ),
        "\nmarker\nold\n"
    );
}

#[test]
fn top_non_empty_lines_requires_a_canonical_positive_bounded_count() {
    let name = "top_non_empty_lines";
    assert!(validate_region_name(&format!("{name}(1)")).is_ok());
    assert!(validate_region_name(&format!("{name}({})", u16::MAX)).is_ok());
    for count in ["0", "01", "+1", "65536", "999999999999999999999999"] {
        assert!(
            validate_region_name(&format!("{name}({count})")).is_err(),
            "{name} accepted invalid count {count}"
        );
    }
}

#[test]
fn top_non_empty_lines_requires_engine_three_when_declared() {
    let manifest = r#"
id = "codex"
version = "1"
min_engine_version = 2

[[rules]]
id = "background"
state = "working"
region = " top_non_empty_lines(1) "
contains = ["active"]
"#;
    assert!(parse_manifest(manifest).is_err());
}

// --- Claude background shells and monitors ---

fn claude_background_screen(footer: &str) -> String {
    format!(
        "✻ Sautéed for 10s\n\n\
         ────────────────────────────────────────────────────────────────\n\
         ❯\n\
         ────────────────────────────────────────────────────────────────\n\
         {footer}\n"
    )
}

fn assert_claude_background_working(footer: &str) {
    with_manifest_dirs("claude-background", || {
        let result = explain(Agent::Claude, &claude_background_screen(footer));
        assert_eq!(result.state, AgentState::Working, "footer: {footer}");
        assert_eq!(
            result.matched_rule.as_ref().map(|rule| rule.id.as_str()),
            Some("background_shell_working"),
            "footer: {footer}"
        );
        assert!(result.visible_working);
    });
}

#[test]
fn claude_idle_prompt_with_background_shell_is_working() {
    assert_claude_background_working("  ⏵⏵ auto mode on · 1 shell · ← for agents       /rc");
    assert_claude_background_working("  ⏵⏵ auto mode on · 3 shells");
}

#[test]
fn claude_idle_prompt_with_shells_and_monitors_is_working() {
    assert_claude_background_working("  ⏵⏵ auto mode on · 2 shells, 1 monitor · ← for agents");
    assert_claude_background_working("  ⏵⏵ auto mode on · 1 shell, 2 monitors");
}

#[test]
fn claude_idle_prompt_with_monitor_only_is_working() {
    assert_claude_background_working("  ⏵⏵ auto mode on · 1 monitor · ← for agents");
    assert_claude_background_working("  ⏸ plan mode on · 2 monitors");
}

#[test]
fn claude_shell_still_running_status_is_working() {
    with_manifest_dirs("claude-shell-still-running", || {
        let screen = concat!(
            "✻ Sautéed for 10s · 1 shell still running\n\n",
            "────────────────────────────────────────────────────────────────\n",
            "❯\n",
            "────────────────────────────────────────────────────────────────\n",
            "  ⏵⏵ auto mode on\n",
        );
        let result = explain(Agent::Claude, screen);
        assert_eq!(result.state, AgentState::Working);
        assert!(result.visible_working);
        assert_eq!(
            result.matched_rule.as_ref().map(|rule| rule.id.as_str()),
            Some("background_shell_working")
        );
    });
}

#[test]
fn claude_idle_prompt_with_monitors_and_agents_footer_is_working() {
    with_manifest_dirs("claude-monitors-agents-footer", || {
        // Footer captured from a live Claude Code pane with three monitors and two agents.
        let screen = concat!(
            "────────────────────────────────────────────────────────────────\n",
            "❯\n",
            "────────────────────────────────────────────────────────────────\n",
            "\n",
            "  ⏵⏵ bypass permissions on · 3 monitors · ← 2 agents\n",
        );
        let result = explain(Agent::Claude, screen);
        assert_eq!(result.state, AgentState::Working);
        assert_eq!(
            result.matched_rule.as_ref().map(|rule| rule.id.as_str()),
            Some("background_shell_working")
        );
    });
}

#[test]
fn claude_indented_prompt_text_cannot_fake_a_still_running_status() {
    with_manifest_dirs("claude-typed-still-running", || {
        let screen = concat!(
            "────────────────────────────────────────────────────────────────\n",
            "❯ note\n",
            "  * x · 1 shell still running\n",
            "────────────────────────────────────────────────────────────────\n",
            "  ⏵⏵ auto mode on · ← for agents\n",
        );
        let result = explain(Agent::Claude, screen);
        assert_eq!(result.state, AgentState::Idle);
        assert!(!result.visible_working);
    });
}

#[test]
fn claude_idle_prompt_without_background_work_stays_idle() {
    with_manifest_dirs("claude-no-background", || {
        let result = explain(
            Agent::Claude,
            &claude_background_screen("  ⏵⏵ auto mode on · ← for agents"),
        );
        assert_eq!(result.state, AgentState::Idle);
        assert!(result.visible_idle);
        assert!(!result.visible_working);
    });
}

#[test]
fn claude_blocker_with_background_shell_remains_blocked() {
    with_manifest_dirs("claude-blocker-background", || {
        let screen = concat!(
            "do you want to proceed?\n",
            "bash command: rm -rf /tmp/test\n",
            "❯ 1. Yes\n",
            "  2. No\n\n",
            "Esc to cancel · Tab to amend · ctrl+e to explain\n",
            "  ⏵⏵ auto mode on · 1 shell · ← for agents\n",
        );
        let result = explain(Agent::Claude, screen);
        assert_eq!(result.state, AgentState::Blocked);
        assert_eq!(
            result.matched_rule.as_ref().map(|rule| rule.id.as_str()),
            Some("bash_permission_prompt")
        );
        assert!(result.visible_blocker);
        assert!(!result.visible_working);
    });
}

fn codex_background_screen(footer: &str) -> String {
    // Shaped like a real Codex pane: the finished turn, the background-terminal footer,
    // then the empty prompt.
    format!("  Worked for 28s • 2:33 PM\n\n{footer}\n\n\n› Ask Codex to do anything\n\n  gpt-5 · ~/repo\n")
}

fn assert_codex_background_working(footer: &str) {
    with_manifest_dirs("codex-background", || {
        let screen = codex_background_screen(footer);
        let input = DetectionInput {
            screen: &screen,
            osc_title: "Verify warmup commits | repo",
            osc_progress: "",
        };
        let result = explain_with_input(Agent::Codex, input);
        assert_eq!(result.state, AgentState::Working, "footer: {footer}");
        assert_eq!(
            result.matched_rule.as_ref().map(|rule| rule.id.as_str()),
            Some("background_terminal_working"),
            "footer: {footer}"
        );
        assert!(result.visible_working);
    });
}

#[test]
fn codex_idle_prompt_with_background_terminals_is_working() {
    assert_codex_background_working(
        "  1 background terminal running · /ps to view · /stop to close",
    );
    assert_codex_background_working(
        "  3 background terminals running · /ps to view · /stop to close",
    );
}

#[test]
fn codex_prompt_without_background_terminals_stays_idle() {
    with_manifest_dirs("codex-no-background", || {
        for footer in [
            "  0 background terminals running",
            "  no background terminal running",
            "  background terminal finished",
        ] {
            let screen = codex_background_screen(footer);
            let input = DetectionInput {
                screen: &screen,
                osc_title: "Verify warmup commits | repo",
                osc_progress: "",
            };
            let result = explain_with_input(Agent::Codex, input);
            assert_eq!(result.state, AgentState::Idle, "footer: {footer}");
        }
    });
}

#[test]
fn codex_typed_background_terminal_text_cannot_fake_working() {
    with_manifest_dirs("codex-typed-background", || {
        let screen =
            "  Worked for 28s • 2:33 PM\n\n› 1 background terminal running\n\n  gpt-5 · ~/repo\n";
        let input = DetectionInput {
            screen,
            osc_title: "Verify warmup commits | repo",
            osc_progress: "",
        };
        assert_eq!(
            explain_with_input(Agent::Codex, input).state,
            AgentState::Idle
        );
    });
}

#[test]
fn codex_permission_prompt_still_wins_over_background_terminals() {
    with_manifest_dirs("codex-background-blocked", || {
        let screen = "  1 background terminal running · /ps to view\n\n›\n  press enter to confirm or esc to cancel\n";
        let input = DetectionInput {
            screen,
            osc_title: "Verify warmup commits | repo",
            osc_progress: "",
        };
        assert_eq!(
            explain_with_input(Agent::Codex, input).state,
            AgentState::Blocked
        );
    });
}

fn codex_title_case(screen: &str, title: &str) -> DetectionExplain {
    explain_with_input(
        Agent::Codex,
        DetectionInput {
            screen,
            osc_title: title,
            osc_progress: "",
        },
    )
}

// Captured from a real pane: Codex finished its turn but kept animating its window title
// because it still tracked dead child processes.
const CODEX_FINISHED_TURN: &str = "› r u done or still working\n\n\n• Done. All 28 runs are complete.\n  The 5090 is free.\n\n  Worked for 10s • 8:12 AM\n\n\n› p\n\n  GPT-6-Astra high · ~/repo… Goal achieved (3h 42m)\n  tab to queue message\n";

#[test]
fn codex_finished_turn_is_idle_even_if_the_title_still_spins() {
    with_manifest_dirs("codex-stale-title", || {
        let result = codex_title_case(CODEX_FINISHED_TURN, "⠙ Verify warmup commits | repo");
        assert_eq!(result.state, AgentState::Idle);
        assert_eq!(
            result.matched_rule.as_ref().map(|rule| rule.id.as_str()),
            Some("finished_turn_ignores_stale_title")
        );
        assert!(result.visible_idle);
    });
}

#[test]
fn codex_title_spinner_still_wins_while_a_turn_is_live() {
    with_manifest_dirs("codex-live-turn", || {
        for status in [
            "• Working (12s • esc to interrupt)",
            "• Working (1m 05s)",
            "◦ Thinking (3s)",
        ] {
            let screen = format!(
                "  Worked for 10s • 8:12 AM\n\n› next question\n\n{status}\n\n› \n  tab to queue message\n"
            );
            let result = codex_title_case(&screen, "⠙ Verify warmup commits | repo");
            assert_eq!(result.state, AgentState::Working, "status: {status}");
        }
    });
}

#[test]
fn codex_background_terminals_still_count_after_a_finished_turn() {
    with_manifest_dirs("codex-finished-with-background", || {
        let screen = "  Worked for 10s • 8:12 AM\n\n  1 background terminal running · /ps to view · /stop to close\n\n› \n";
        let result = codex_title_case(screen, "⠙ Verify warmup commits | repo");
        assert_eq!(result.state, AgentState::Working);
    });
}

#[test]
fn codex_finished_turn_rule_does_not_hide_permission_prompts() {
    with_manifest_dirs("codex-finished-blocked", || {
        let screen = "  Worked for 10s • 8:12 AM\n\n›\n  press enter to confirm or esc to cancel\n";
        let result = codex_title_case(screen, "Action Required | repo");
        assert_eq!(result.state, AgentState::Blocked);
    });
}

const CAPACITY_ERROR: &str = "■ Selected model is at capacity. Please try a different model.";

#[test]
fn codex_model_at_capacity_is_blocked_until_work_resumes() {
    with_manifest_dirs("codex-capacity", || {
        // Captured shape: the error is among the latest lines and the prompt is idle.
        let screen = format!(
            "• Completed `/root/pro5000_benchmark`\n\n{CAPACITY_ERROR}\n\n\n› Ask Codex to do anything\n\n  gpt-5 · ~/repo\n"
        );
        let result = codex_title_case(&screen, "Verify warmup commits | repo");
        assert_eq!(result.state, AgentState::Blocked);
        assert_eq!(
            result.matched_rule.as_ref().map(|rule| rule.id.as_str()),
            Some("capacity_error_blocked")
        );
        assert!(result.visible_blocker);

        // Even with a finished-turn line and a stale spinner above it.
        let stale = format!("  Worked for 10s • 8:12 AM\n\n{CAPACITY_ERROR}\n\n› \n");
        assert_eq!(
            codex_title_case(&stale, "⠙ Verify warmup commits | repo").state,
            AgentState::Blocked
        );
    });
}

#[test]
fn codex_capacity_error_does_not_hide_a_live_turn_or_old_errors() {
    with_manifest_dirs("codex-capacity-live", || {
        // The user continued: a live status line means working, not blocked.
        let live =
            format!("{CAPACITY_ERROR}\n\n› try again\n\n• Working (4s • esc to interrupt)\n\n› \n");
        assert_eq!(
            codex_title_case(&live, "⠙ Verify warmup commits | repo").state,
            AgentState::Working
        );
        // The error has scrolled out of the latest lines.
        let mut old = format!("{CAPACITY_ERROR}\n");
        for line in 0..12 {
            old.push_str(&format!("• later output {line}\n"));
        }
        old.push_str("\n› Ask Codex to do anything\n");
        assert_eq!(
            codex_title_case(&old, "Verify warmup commits | repo").state,
            AgentState::Idle
        );
        // Text typed at the prompt cannot fake it.
        let typed = format!("  Worked for 10s • 8:12 AM\n\n› {CAPACITY_ERROR}\n");
        assert_ne!(
            codex_title_case(&typed, "Verify warmup commits | repo").state,
            AgentState::Blocked
        );
    });
}
