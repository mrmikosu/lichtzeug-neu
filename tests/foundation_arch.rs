use luma_switch::core::{
    AppEvent, BeatTime, ChaseId, ClipId, CueId, EnginePhase, FixtureGroupId, FxId, FxWaveform, PPQ,
    SelectionState, StudioState, TimelineCursor, TimelineEvent, TimelineHit, TimelineZone, TrackId,
    dispatch, export_project_json, foundation_spec, foundation_spec_json, import_project_json,
    list_ventures, load_recovery_registry, load_venture, replay_events, save_venture,
    validate_state,
};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn integration_project_structure_exports_core_modules() {
    let spec = foundation_spec();
    assert!(
        spec.project_structure
            .iter()
            .any(|path| path == "src/core/state.rs")
    );
    assert!(
        spec.project_structure
            .iter()
            .any(|path| path == "src/core/reducer.rs")
    );
    assert!(
        spec.project_structure
            .iter()
            .any(|path| path == "src/core/automation.rs")
    );
    assert!(
        spec.project_structure
            .iter()
            .any(|path| path == "src/core/project.rs")
    );
    assert!(
        spec.project_structure
            .iter()
            .any(|path| path == "src/ui/fixture_view.rs")
    );
}

#[test]
fn integration_queue_drives_reducer_without_reordering() {
    let script = vec![
        AppEvent::Tick,
        AppEvent::SetMasterIntensity(900),
        AppEvent::Tick,
    ];
    let state = replay_events(&script);

    assert!(state.event_queue.completed.len() >= 3);
    assert!(
        state
            .event_queue
            .completed
            .windows(2)
            .all(|pair| pair[0].sequence < pair[1].sequence)
    );
}

#[test]
fn simulation_engine_tick_updates_clip_phases() {
    let mut state = luma_switch::core::StudioState::default();
    state.engine.phase = EnginePhase::Running;
    state.engine.transport.playhead = BeatTime::from_ticks((PPQ * 8) - 5);

    luma_switch::core::dispatch(&mut state, AppEvent::Tick);

    let clip = state.clip(ClipId(102)).expect("clip exists");
    assert!(matches!(
        clip.phase,
        luma_switch::core::ClipPhase::Triggered | luma_switch::core::ClipPhase::Active
    ));
}

#[test]
fn scenario_drag_zoom_scrub_keeps_state_valid() {
    let script = vec![
        AppEvent::Timeline(TimelineEvent::PointerPressed(TimelineCursor {
            beat: BeatTime::from_beats(8),
            track: Some(TrackId(1)),
            zone: TimelineZone::Track,
            target: Some(TimelineHit::ClipBody(ClipId(102))),
            x_px: 180,
            y_px: 82,
        })),
        AppEvent::Timeline(TimelineEvent::PointerMoved(TimelineCursor {
            beat: BeatTime::from_fraction(37, 4),
            track: Some(TrackId(1)),
            zone: TimelineZone::Track,
            target: Some(TimelineHit::ClipBody(ClipId(102))),
            x_px: 220,
            y_px: 82,
        })),
        AppEvent::SetTimelineZoom(1325),
        AppEvent::Timeline(TimelineEvent::PointerPressed(TimelineCursor {
            beat: BeatTime::from_beats(12),
            track: None,
            zone: TimelineZone::Header,
            target: Some(TimelineHit::Playhead),
            x_px: 260,
            y_px: 12,
        })),
        AppEvent::Timeline(TimelineEvent::PointerReleased(TimelineCursor {
            beat: BeatTime::from_beats(12),
            track: None,
            zone: TimelineZone::Header,
            target: Some(TimelineHit::Playhead),
            x_px: 260,
            y_px: 12,
        })),
    ];

    let state = replay_events(&script);
    let report = validate_state(&state);

    assert!(report.valid);
    assert!(matches!(
        state.timeline.selection,
        SelectionState::Clip(ClipId(102))
    ));
}

#[test]
fn replay_produces_identical_state_snapshot() {
    let script = vec![
        AppEvent::Tick,
        AppEvent::SetMasterIntensity(910),
        AppEvent::SetMasterSpeed(720),
        AppEvent::Timeline(TimelineEvent::Scrolled {
            delta_lines: -2,
            anchor_x_px: 200,
            anchor_beat: BeatTime::from_beats(8),
        }),
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);

    assert_eq!(
        serde_json::to_string(&left).expect("serialize left"),
        serde_json::to_string(&right).expect("serialize right")
    );
}

#[test]
fn integration_small_pointer_jitter_does_not_move_clip() {
    let script = vec![
        AppEvent::Timeline(TimelineEvent::PointerPressed(TimelineCursor {
            beat: BeatTime::from_beats(8),
            track: Some(TrackId(1)),
            zone: TimelineZone::Track,
            target: Some(TimelineHit::ClipBody(ClipId(102))),
            x_px: 180,
            y_px: 82,
        })),
        AppEvent::Timeline(TimelineEvent::PointerMoved(TimelineCursor {
            beat: BeatTime::from_beats_f32(8.03),
            track: Some(TrackId(1)),
            zone: TimelineZone::Track,
            target: Some(TimelineHit::ClipBody(ClipId(102))),
            x_px: 183,
            y_px: 84,
        })),
        AppEvent::Timeline(TimelineEvent::PointerReleased(TimelineCursor {
            beat: BeatTime::from_beats_f32(8.03),
            track: Some(TrackId(1)),
            zone: TimelineZone::Track,
            target: Some(TimelineHit::ClipBody(ClipId(102))),
            x_px: 183,
            y_px: 84,
        })),
    ];

    let state = replay_events(&script);

    assert_eq!(
        state.clip(ClipId(102)).expect("clip exists").start,
        BeatTime::from_beats(8)
    );
    assert!(validate_state(&state).valid);
}

#[test]
fn integration_zoom_anchor_replays_deterministically() {
    let script = vec![AppEvent::Timeline(TimelineEvent::Scrolled {
        delta_lines: -4,
        anchor_x_px: 200,
        anchor_beat: BeatTime::from_beats(8),
    })];

    let left = replay_events(&script);
    let right = replay_events(&script);
    let left_anchor = BeatTime::from_ticks(
        (left.timeline.viewport.scroll.ticks() as f32
            + ((200.0 / (40.0 * left.timeline.viewport.zoom.as_f32())) * PPQ as f32))
            .round() as u32,
    );
    let right_anchor = BeatTime::from_ticks(
        (right.timeline.viewport.scroll.ticks() as f32
            + ((200.0 / (40.0 * right.timeline.viewport.zoom.as_f32())) * PPQ as f32))
            .round() as u32,
    );

    assert_eq!(left_anchor, BeatTime::from_beats(8));
    assert_eq!(left_anchor, right_anchor);
    assert_eq!(
        serde_json::to_string(&left.timeline).expect("serialize left timeline"),
        serde_json::to_string(&right.timeline).expect("serialize right timeline")
    );
}

#[test]
fn integration_box_selection_replays_deterministically() {
    let script = vec![
        AppEvent::Timeline(TimelineEvent::PointerPressed(TimelineCursor {
            beat: BeatTime::from_fraction(1, 5),
            track: Some(TrackId(1)),
            zone: TimelineZone::Track,
            target: None,
            x_px: 10,
            y_px: 44,
        })),
        AppEvent::Timeline(TimelineEvent::PointerMoved(TimelineCursor {
            beat: BeatTime::from_beats(9),
            track: Some(TrackId(1)),
            zone: TimelineZone::Track,
            target: Some(TimelineHit::ClipBody(ClipId(102))),
            x_px: 420,
            y_px: 114,
        })),
        AppEvent::Timeline(TimelineEvent::PointerReleased(TimelineCursor {
            beat: BeatTime::from_beats(9),
            track: Some(TrackId(1)),
            zone: TimelineZone::Track,
            target: Some(TimelineHit::ClipBody(ClipId(102))),
            x_px: 420,
            y_px: 114,
        })),
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);

    assert_eq!(left.timeline.selection, SelectionState::Clip(ClipId(101)));
    assert_eq!(left.timeline.selected_clips, vec![ClipId(101), ClipId(102)]);
    assert_eq!(
        serde_json::to_string(&left.timeline).expect("serialize left timeline"),
        serde_json::to_string(&right.timeline).expect("serialize right timeline")
    );
}

#[test]
fn integration_cue_trigger_updates_fixture_and_clip_views() {
    let state = replay_events(&[AppEvent::TriggerCue(CueId(3))]);

    assert!(matches!(
        state.clip(ClipId(201)).expect("clip exists").cue_state,
        luma_switch::core::CueVisualState::Active
    ));
    assert!(matches!(
        state
            .fixture_group(FixtureGroupId(3))
            .expect("fixture exists")
            .phase,
        luma_switch::core::FixturePhase::Active
    ));
}

#[test]
fn integration_timeline_hotspots_dispatch_show_actions() {
    let script = vec![
        AppEvent::Timeline(TimelineEvent::PointerPressed(TimelineCursor {
            beat: BeatTime::from_beats(8),
            track: Some(TrackId(1)),
            zone: TimelineZone::Track,
            target: Some(TimelineHit::ClipCueHotspot(ClipId(102), CueId(1))),
            x_px: 188,
            y_px: 92,
        })),
        AppEvent::Timeline(TimelineEvent::PointerPressed(TimelineCursor {
            beat: BeatTime::from_beats(8),
            track: Some(TrackId(1)),
            zone: TimelineZone::Track,
            target: Some(TimelineHit::ClipFxHotspot(ClipId(102), FxId(1))),
            x_px: 248,
            y_px: 92,
        })),
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);

    assert_eq!(left.timeline.selection, SelectionState::Clip(ClipId(102)));
    assert_eq!(left.fx_system.selected, Some(FxId(1)));
    assert!(matches!(
        left.cue(CueId(1)).expect("cue exists").phase,
        luma_switch::core::CuePhase::Triggered | luma_switch::core::CuePhase::Active
    ));
    assert_eq!(
        serde_json::to_string(&left.cue_system).expect("serialize left cue system"),
        serde_json::to_string(&right.cue_system).expect("serialize right cue system")
    );
}

#[test]
fn integration_inline_parameter_drag_replays_deterministically() {
    let script = vec![
        AppEvent::Timeline(TimelineEvent::PointerPressed(TimelineCursor {
            beat: BeatTime::from_beats(8),
            track: Some(TrackId(1)),
            zone: TimelineZone::Track,
            target: Some(TimelineHit::ClipParamHandle(
                ClipId(102),
                luma_switch::core::ClipInlineParameterKind::Intensity,
            )),
            x_px: 284,
            y_px: 58,
        })),
        AppEvent::Timeline(TimelineEvent::PointerMoved(TimelineCursor {
            beat: BeatTime::from_beats(8),
            track: Some(TrackId(1)),
            zone: TimelineZone::Track,
            target: Some(TimelineHit::ClipParamHandle(
                ClipId(102),
                luma_switch::core::ClipInlineParameterKind::Intensity,
            )),
            x_px: 284,
            y_px: 100,
        })),
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);

    assert!(
        left.clip(ClipId(102))
            .expect("clip exists")
            .params
            .intensity
            .permille()
            < 300
    );
    assert_eq!(
        serde_json::to_string(&left.timeline).expect("serialize left timeline"),
        serde_json::to_string(&right.timeline).expect("serialize right timeline")
    );
    assert_eq!(
        serde_json::to_string(&left.fx_system).expect("serialize left fx system"),
        serde_json::to_string(&right.fx_system).expect("serialize right fx system")
    );
}

#[test]
fn simulation_chase_step_progress_is_deterministic() {
    let script = vec![
        AppEvent::ToggleChase(ChaseId(2)),
        AppEvent::ReverseChase(ChaseId(2)),
        AppEvent::Tick,
        AppEvent::Tick,
        AppEvent::Tick,
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);

    assert_eq!(
        left.chase(ChaseId(2)).expect("left chase").current_step,
        right.chase(ChaseId(2)).expect("right chase").current_step
    );
    assert_eq!(
        serde_json::to_string(&left.chase_system).expect("serialize left chase system"),
        serde_json::to_string(&right.chase_system).expect("serialize right chase system")
    );
}

#[test]
fn integration_fx_depth_event_is_clamped_and_replayed() {
    let state = replay_events(&[
        AppEvent::SelectFx(FxId(1)),
        AppEvent::SetFxDepth(FxId(1), 1400),
        AppEvent::Tick,
    ]);

    assert_eq!(
        state.fx_layer(FxId(1)).expect("fx exists").depth_permille,
        1000
    );
    assert!(validate_state(&state).valid);
}

#[test]
fn integration_fx_waveform_and_fixture_preview_replay_is_deterministic() {
    let script = vec![
        AppEvent::SelectFx(FxId(1)),
        AppEvent::SetFxRate(FxId(1), 1340),
        AppEvent::SetFxSpread(FxId(1), 910),
        AppEvent::SetFxPhaseOffset(FxId(1), 620),
        AppEvent::SetFxWaveform(FxId(1), FxWaveform::Saw),
        AppEvent::TriggerCue(CueId(1)),
        AppEvent::Tick,
        AppEvent::Tick,
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);

    assert!(validate_state(&left).valid);
    assert_eq!(
        serde_json::to_string(&left.fx_system).expect("serialize left fx system"),
        serde_json::to_string(&right.fx_system).expect("serialize right fx system")
    );
    assert_eq!(
        serde_json::to_string(&left.fixture_system).expect("serialize left fixture system"),
        serde_json::to_string(&right.fixture_system).expect("serialize right fixture system")
    );
}

#[test]
fn scenario_fixture_selection_stays_valid_after_show_updates() {
    let script = vec![
        AppEvent::SelectFixtureGroup(FixtureGroupId(2)),
        AppEvent::TriggerCue(CueId(2)),
        AppEvent::ToggleFx(FxId(2)),
        AppEvent::Tick,
        AppEvent::Tick,
    ];

    let state = replay_events(&script);
    let report = validate_state(&state);

    assert!(report.valid);
    assert_eq!(state.fixture_system.selected, Some(FixtureGroupId(2)));
}

#[test]
fn replay_produces_identical_show_state_snapshot() {
    let script = vec![
        AppEvent::TriggerCue(CueId(2)),
        AppEvent::ToggleChase(ChaseId(1)),
        AppEvent::SetFxDepth(FxId(2), 735),
        AppEvent::SelectFixtureGroup(FixtureGroupId(1)),
        AppEvent::Tick,
        AppEvent::Tick,
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);

    assert_eq!(
        serde_json::to_string(&left.cue_system).expect("serialize left cue system"),
        serde_json::to_string(&right.cue_system).expect("serialize right cue system")
    );
    assert_eq!(
        serde_json::to_string(&left.chase_system).expect("serialize left chase system"),
        serde_json::to_string(&right.chase_system).expect("serialize right chase system")
    );
    assert_eq!(
        serde_json::to_string(&left.fx_system).expect("serialize left fx system"),
        serde_json::to_string(&right.fx_system).expect("serialize right fx system")
    );
    assert_eq!(
        serde_json::to_string(&left.fixture_system).expect("serialize left fixture system"),
        serde_json::to_string(&right.fixture_system).expect("serialize right fixture system")
    );
}

#[test]
fn clip_editor_overlay_and_replay_are_deterministic() {
    let script = vec![
        AppEvent::OpenClipEditor(ClipId(102)),
        AppEvent::SetClipEditorIntensity(930),
        AppEvent::SetClipEditorSpeed(1080),
        AppEvent::SetClipEditorFxDepth(760),
        AppEvent::SetClipEditorCue(Some(CueId(2))),
        AppEvent::SetClipEditorChase(Some(ChaseId(2))),
        AppEvent::Tick,
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);

    assert_eq!(
        serde_json::to_string(&left.clip_editor).expect("serialize left clip editor"),
        serde_json::to_string(&right.clip_editor).expect("serialize right clip editor")
    );
    assert_eq!(
        serde_json::to_string(&left.timeline).expect("serialize left timeline"),
        serde_json::to_string(&right.timeline).expect("serialize right timeline")
    );
    assert_eq!(
        serde_json::to_string(&left.chase_system).expect("serialize left chase system"),
        serde_json::to_string(&right.chase_system).expect("serialize right chase system")
    );
}

#[test]
fn cue_and_chase_authoring_replay_is_deterministic() {
    let script = vec![
        AppEvent::SelectCue(CueId(2)),
        AppEvent::SetSelectedCueName("Build Cue".to_owned()),
        AppEvent::SetSelectedCueFadeDuration(BeatTime::from_fraction(3, 4)),
        AppEvent::CreateChase,
        AppEvent::SetSelectedChaseName("Build Chase".to_owned()),
        AppEvent::AddSelectedChaseStep,
        AppEvent::SelectChaseStep(Some(1)),
        AppEvent::SetSelectedChaseStepCue(Some(CueId(2))),
        AppEvent::SetSelectedChaseStepDuration(BeatTime::from_beats(1)),
        AppEvent::MoveSelectedChaseStepLeft,
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);

    assert_eq!(left.selected_cue().expect("selected cue").name, "Build Cue");
    assert_eq!(
        left.selected_chase().expect("selected chase").name,
        "Build Chase"
    );
    assert_eq!(
        serde_json::to_string(&left.cue_system).expect("serialize left cue system"),
        serde_json::to_string(&right.cue_system).expect("serialize right cue system")
    );
    assert_eq!(
        serde_json::to_string(&left.chase_system).expect("serialize left chase system"),
        serde_json::to_string(&right.chase_system).expect("serialize right chase system")
    );
}

#[test]
fn integration_undo_redo_replays_deterministically() {
    let script = vec![
        AppEvent::Timeline(TimelineEvent::PointerPressed(TimelineCursor {
            beat: BeatTime::from_beats(8),
            track: Some(TrackId(1)),
            zone: TimelineZone::Track,
            target: Some(TimelineHit::ClipBody(ClipId(102))),
            x_px: 180,
            y_px: 82,
        })),
        AppEvent::Timeline(TimelineEvent::PointerMoved(TimelineCursor {
            beat: BeatTime::from_fraction(37, 4),
            track: Some(TrackId(1)),
            zone: TimelineZone::Track,
            target: Some(TimelineHit::ClipBody(ClipId(102))),
            x_px: 222,
            y_px: 82,
        })),
        AppEvent::Timeline(TimelineEvent::PointerReleased(TimelineCursor {
            beat: BeatTime::from_fraction(37, 4),
            track: Some(TrackId(1)),
            zone: TimelineZone::Track,
            target: Some(TimelineHit::ClipBody(ClipId(102))),
            x_px: 222,
            y_px: 82,
        })),
        AppEvent::Undo,
        AppEvent::Redo,
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);

    assert_eq!(
        left.clip(ClipId(102)).expect("left clip exists").start,
        BeatTime::from_fraction(37, 4)
    );
    assert_eq!(left.history.undo_stack.len(), 1);
    assert!(left.history.redo_stack.is_empty());
    assert_eq!(
        serde_json::to_string(&left).expect("serialize left state"),
        serde_json::to_string(&right).expect("serialize right state")
    );
}

#[test]
fn integration_duplicate_selected_clips_replays_deterministically() {
    let script = vec![AppEvent::DuplicateSelectedClips];

    let left = replay_events(&script);
    let right = replay_events(&script);

    let duplicate = left.timeline.tracks[0]
        .clips
        .iter()
        .find(|clip| clip.title == "Drop Sweep Copy")
        .expect("duplicated clip");

    assert_eq!(duplicate.start, BeatTime::from_beats(16));
    assert_eq!(left.timeline.selected_clips, vec![duplicate.id]);
    assert_eq!(
        serde_json::to_string(&left.timeline).expect("serialize left timeline"),
        serde_json::to_string(&right.timeline).expect("serialize right timeline")
    );
}

#[test]
fn integration_split_selected_clips_replays_deterministically() {
    let script = vec![
        AppEvent::Timeline(TimelineEvent::PointerPressed(TimelineCursor {
            beat: BeatTime::from_beats(12),
            track: None,
            zone: TimelineZone::Header,
            target: Some(TimelineHit::Playhead),
            x_px: 260,
            y_px: 12,
        })),
        AppEvent::Timeline(TimelineEvent::PointerReleased(TimelineCursor {
            beat: BeatTime::from_beats(12),
            track: None,
            zone: TimelineZone::Header,
            target: Some(TimelineHit::Playhead),
            x_px: 260,
            y_px: 12,
        })),
        AppEvent::SplitSelectedClipsAtPlayhead,
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);

    let right_segment = left.timeline.tracks[0]
        .clips
        .iter()
        .find(|clip| clip.title == "Drop Sweep B")
        .expect("split clip");

    assert_eq!(
        left.clip(ClipId(102)).expect("left segment").duration,
        BeatTime::from_beats(4)
    );
    assert_eq!(right_segment.start, BeatTime::from_beats(12));
    assert_eq!(
        serde_json::to_string(&left.timeline).expect("serialize left timeline"),
        serde_json::to_string(&right.timeline).expect("serialize right timeline")
    );
}

#[test]
fn integration_delete_selected_clips_replays_deterministically() {
    let script = vec![AppEvent::DeleteSelectedClips];

    let left = replay_events(&script);
    let right = replay_events(&script);

    assert!(left.clip(ClipId(102)).is_none());
    assert_eq!(left.cue(CueId(1)).expect("cue exists").linked_clip, None);
    assert_eq!(left.fx_layer(FxId(1)).expect("fx exists").linked_clip, None);
    assert_eq!(
        serde_json::to_string(&left).expect("serialize left state"),
        serde_json::to_string(&right).expect("serialize right state")
    );
}

#[test]
fn integration_clipboard_paste_replays_deterministically() {
    let script = vec![
        AppEvent::CopySelectedClips,
        AppEvent::Timeline(TimelineEvent::PointerPressed(TimelineCursor {
            beat: BeatTime::from_beats(20),
            track: None,
            zone: TimelineZone::Header,
            target: Some(TimelineHit::Playhead),
            x_px: 420,
            y_px: 12,
        })),
        AppEvent::Timeline(TimelineEvent::PointerReleased(TimelineCursor {
            beat: BeatTime::from_beats(20),
            track: None,
            zone: TimelineZone::Header,
            target: Some(TimelineHit::Playhead),
            x_px: 420,
            y_px: 12,
        })),
        AppEvent::PasteClipboardAtPlayhead,
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);
    let pasted_id = *left
        .timeline
        .selected_clips
        .first()
        .expect("pasted clip selected");
    let pasted = left.clip(pasted_id).expect("pasted clip exists");

    assert_eq!(pasted.start, BeatTime::from_beats(20));
    assert_eq!(
        serde_json::to_string(&left.timeline).expect("serialize left timeline"),
        serde_json::to_string(&right.timeline).expect("serialize right timeline")
    );
}

#[test]
fn integration_context_menu_nudge_replays_deterministically() {
    let script = vec![
        AppEvent::Timeline(TimelineEvent::SecondaryPressed(TimelineCursor {
            beat: BeatTime::from_beats(8),
            track: Some(TrackId(1)),
            zone: TimelineZone::Track,
            target: Some(TimelineHit::ClipBody(ClipId(102))),
            x_px: 188,
            y_px: 92,
        })),
        AppEvent::ApplyContextMenuAction(luma_switch::core::ContextMenuAction::NudgeRight),
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);

    assert_eq!(
        left.clip(ClipId(102)).expect("clip exists").start,
        BeatTime::from_fraction(33, 4)
    );
    assert_eq!(
        serde_json::to_string(&left.timeline).expect("serialize left timeline"),
        serde_json::to_string(&right.timeline).expect("serialize right timeline")
    );
}

#[test]
fn integration_project_export_import_roundtrip_is_deterministic() {
    let state = replay_events(&[
        AppEvent::CopySelectedClips,
        AppEvent::PasteClipboardAtPlayhead,
        AppEvent::OpenClipEditor(ClipId(102)),
        AppEvent::SetClipEditorAutomationTarget(luma_switch::core::AutomationTarget::Intensity),
    ]);
    let json = export_project_json(&state);
    let restored = import_project_json(&json).expect("import project");

    assert_eq!(
        serde_json::to_string(&state.timeline.tracks).expect("serialize left tracks"),
        serde_json::to_string(&restored.timeline.tracks).expect("serialize restored tracks")
    );
    assert_eq!(state.timeline.selection, restored.timeline.selection);
    assert_eq!(
        state.timeline.selected_clips,
        restored.timeline.selected_clips
    );
    assert_eq!(state.clip_editor, restored.clip_editor);
}

#[test]
fn integration_venture_save_load_roundtrip_is_deterministic() {
    let directory = temp_venture_dir("integration");
    let state = replay_events(&[
        AppEvent::DuplicateSelectedClips,
        AppEvent::SetMasterSpeed(740),
        AppEvent::TriggerCue(CueId(2)),
    ]);
    let saved = save_venture(&state, &directory, None, "Festival Rig").expect("save venture");
    let listed = list_ventures(&directory).expect("list ventures");
    let (loaded, descriptor) = load_venture(&directory, &saved.id).expect("load venture");

    assert_eq!(listed.len(), 1);
    assert_eq!(descriptor.name, "Festival Rig");
    assert_eq!(
        serde_json::to_string(&state.timeline.tracks).expect("serialize source tracks"),
        serde_json::to_string(&loaded.timeline.tracks).expect("serialize loaded tracks")
    );
    assert_eq!(state.master.speed, loaded.master.speed);

    let _ = fs::remove_dir_all(directory);
}

#[test]
fn integration_autosave_recovery_restore_is_deterministic() {
    let directory = temp_venture_dir("autosave-restore");
    let mut state = StudioState::default();
    state.venture.directory = directory.to_string_lossy().into_owned();

    dispatch(
        &mut state,
        AppEvent::SetVentureDraftName("Autosave Venture".to_owned()),
    );
    dispatch(&mut state, AppEvent::SaveCurrentVenture);
    dispatch(&mut state, AppEvent::SetMasterIntensity(930));

    let recovery_id = state
        .venture
        .selected_recovery
        .clone()
        .expect("selected recovery after autosave");
    let registry = load_recovery_registry(&directory).expect("load recovery registry");
    let intensity_after_edit = state.master.intensity;

    assert_eq!(registry.slots.len(), 1);
    assert!(state.venture.dirty);

    dispatch(&mut state, AppEvent::RestoreSelectedRecoverySlot);

    assert_eq!(state.master.intensity, intensity_after_edit);
    assert_eq!(
        state.venture.selected_recovery.as_deref(),
        Some(recovery_id.as_str())
    );
    assert!(!state.venture.dirty);

    let _ = fs::remove_dir_all(directory);
}

#[test]
fn machine_readable_spec_roundtrip() {
    let json = foundation_spec_json();
    let parsed: luma_switch::core::MachineReadableSection =
        serde_json::from_str(&json).expect("json roundtrip");

    assert_eq!(parsed.modules.len(), 16);
}

fn temp_venture_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "luma-switch-integration-venture-{}-{}",
        label,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("unix time")
            .as_nanos()
    ))
}
