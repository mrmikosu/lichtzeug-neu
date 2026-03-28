use crate::core::automation::{clamp_automation_value, sort_lane_points};
use crate::core::fixtures::fixture_mode_channel_count;
use crate::core::project::next_venture_name;
use crate::core::state::{
    ChasePhase, ClipEditorPhase, CuePhase, CueVisualState, DmxBackendKind, DmxInterfaceKind,
    EngineLinkPhase, EnginePhase, FixturePhase, HoverTarget, MIN_CLIP_DURATION, MidiControlHint,
    MidiLearnPhase, SelectionState, SnapPhase, StateLifecycle, StudioState, TimelinePhase,
    TimelineViewport,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationIssueKind {
    TypeConsistency,
    StateConsistency,
    Determinism,
    ReferenceIntegrity,
    TimingConsistency,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub kind: ValidationIssueKind,
    pub code: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReport {
    pub valid: bool,
    pub issues: Vec<ValidationIssue>,
    pub corrections: Vec<String>,
}

pub fn validate_state(state: &StudioState) -> ValidationReport {
    let mut issues = Vec::new();
    validate_ids(state, &mut issues);
    validate_selection_and_hover(state, &mut issues);
    validate_timeline_bounds(state, &mut issues);
    validate_show_references(state, &mut issues);
    validate_settings_hardware(state, &mut issues);
    validate_engine_link(state, &mut issues);
    validate_authoring_extensions(state, &mut issues);
    validate_event_queue(state, &mut issues);
    validate_phase_consistency(state, &mut issues);

    ValidationReport {
        valid: issues.is_empty(),
        issues,
        corrections: Vec::new(),
    }
}

pub fn recover_state(state: &mut StudioState, report: &ValidationReport) -> ValidationReport {
    let mut corrections = Vec::new();

    for issue in &report.issues {
        match issue.code.as_str() {
            "selection.clip.missing" | "selection.track.missing" | "selection.clip_set.invalid" => {
                state.timeline.selection = SelectionState::None;
                state.timeline.selected_clips.clear();
                corrections.push("selection reset".to_owned());
            }
            "selection.cue.missing" => {
                state.cue_system.selected = None;
                corrections.push("cue selection reset".to_owned());
            }
            "selection.chase.missing" => {
                state.chase_system.selected = None;
                state.chase_system.selected_step = None;
                corrections.push("chase selection reset".to_owned());
            }
            "selection.chase.step.invalid" => {
                state.chase_system.selected_step =
                    state.chase_system.selected.and_then(|selected| {
                        state
                            .chase(selected)
                            .and_then(|chase| (!chase.steps.is_empty()).then_some(0))
                    });
                corrections.push("chase step selection reset".to_owned());
            }
            "selection.fx.missing" => {
                state.fx_system.selected = None;
                corrections.push("fx selection reset".to_owned());
            }
            "selection.fixture.missing" => {
                state.fixture_system.selected = None;
                corrections.push("fixture selection reset".to_owned());
            }
            "fixture.profile.selected.missing" => {
                state.fixture_system.library.selected_profile = None;
                corrections.push("fixture profile selection reset".to_owned());
            }
            "fixture.patch.selected.missing" => {
                state.fixture_system.library.selected_patch = None;
                corrections.push("fixture patch selection reset".to_owned());
            }
            "clip_editor.clip.missing" | "clip_editor.selection.mismatch" => {
                state.clip_editor.phase = ClipEditorPhase::Closed;
                state.clip_editor.clip_id = None;
                state.clip_editor.selected_automation_point = None;
                corrections.push("clip editor closed".to_owned());
            }
            "clip_editor.automation_point.out_of_bounds" => {
                state.clip_editor.selected_automation_point = None;
                corrections.push("clip editor automation selection reset".to_owned());
            }
            "hover.clip.missing" => {
                state.timeline.hover = None;
                corrections.push("hover reset".to_owned());
            }
            "context_menu.target.missing" => {
                state.context_menu.open = false;
                state.context_menu.target = None;
                corrections.push("context menu closed".to_owned());
            }
            "clipboard.track.missing" => {
                state.clipboard = Default::default();
                corrections.push("clipboard cleared".to_owned());
            }
            "replay_log.capacity.exceeded" => {
                if state.replay_log.events.len() > state.replay_log.capacity {
                    let overflow = state.replay_log.events.len() - state.replay_log.capacity;
                    state.replay_log.events.drain(0..overflow);
                }
                corrections.push("replay log trimmed".to_owned());
            }
            "venture.selected.missing" => {
                state.venture.selected = None;
                if state.venture.draft_name.trim().is_empty() {
                    state.venture.draft_name = next_venture_name(&state.venture.ventures);
                }
                corrections.push("venture selection reset".to_owned());
            }
            "venture.recovery.selected.missing" => {
                state.venture.selected_recovery = None;
                corrections.push("recovery selection reset".to_owned());
            }
            "settings.dmx.interface.selected.missing" => {
                state.settings.dmx.selected_interface = None;
                corrections.push("dmx interface selection reset".to_owned());
            }
            "settings.midi.input.selected.missing" => {
                state.settings.midi.selected_input = None;
                state.settings.midi.detected_controller = None;
                state.settings.midi.learn.phase = MidiLearnPhase::Idle;
                state.settings.midi.learn.target_binding = None;
                state.settings.midi.learn.capture_queue.clear();
                state.settings.midi.learn.expected_hint = MidiControlHint::Any;
                corrections.push("midi input selection reset".to_owned());
            }
            "settings.midi.output.selected.missing" => {
                state.settings.midi.selected_output = None;
                corrections.push("midi output selection reset".to_owned());
            }
            "settings.engine.device.selected.missing" => {
                state.settings.engine_link.selected_device = None;
                state.settings.engine_link.telemetry = None;
                state.settings.engine_link.phase = if state.settings.engine_link.enabled {
                    EngineLinkPhase::Idle
                } else {
                    EngineLinkPhase::Disabled
                };
                corrections.push("engine-link selection reset".to_owned());
            }
            "settings.engine.discovery_port.out_of_bounds" => {
                state.settings.engine_link.discovery_port = state
                    .settings
                    .engine_link
                    .discovery_port
                    .clamp(1_024, 65_535);
                corrections.push("engine-link discovery port clamped".to_owned());
            }
            "settings.engine.telemetry.device.mismatch" => {
                state.settings.engine_link.telemetry = None;
                state.settings.engine_link.phase =
                    if state.settings.engine_link.selected_device.is_some() {
                        EngineLinkPhase::DeviceSelected
                    } else if state.settings.engine_link.enabled {
                        EngineLinkPhase::Idle
                    } else {
                        EngineLinkPhase::Disabled
                    };
                corrections.push("engine-link telemetry reset".to_owned());
            }
            "settings.dmx.refresh_rate.out_of_bounds" => {
                state.settings.dmx.refresh_rate_hz =
                    state.settings.dmx.refresh_rate_hz.clamp(1, 44);
                corrections.push("dmx refresh rate clamped".to_owned());
            }
            "settings.dmx.enttec.break.out_of_bounds" => {
                state.settings.dmx.enttec_break_us =
                    state.settings.dmx.enttec_break_us.clamp(88, 1000);
                corrections.push("enttec break clamped".to_owned());
            }
            "settings.dmx.enttec.mab.out_of_bounds" => {
                state.settings.dmx.enttec_mark_after_break_us =
                    state.settings.dmx.enttec_mark_after_break_us.clamp(8, 1000);
                corrections.push("enttec mark-after-break clamped".to_owned());
            }
            "settings.dmx.output.without_interface" => {
                state.settings.dmx.output_enabled = false;
                corrections.push("dmx output disabled".to_owned());
            }
            "settings.midi.detected_controller.missing_input" => {
                state.settings.midi.detected_controller = None;
                corrections.push("detected controller cleared".to_owned());
            }
            "settings.midi.learn.target.missing"
            | "settings.midi.learn.queue.invalid"
            | "settings.midi.learn.without_input"
            | "settings.midi.learn.idle_state.invalid" => {
                state.settings.midi.learn.phase = MidiLearnPhase::Idle;
                state.settings.midi.learn.target_binding = None;
                state.settings.midi.learn.capture_queue.clear();
                state.settings.midi.learn.expected_hint = MidiControlHint::Any;
                corrections.push("midi learn reset".to_owned());
            }
            "settings.midi.binding.duplicate" => {
                let mut seen = Vec::new();
                for binding in &mut state.settings.midi.bindings {
                    if let Some(message) = binding.message.clone() {
                        if seen.contains(&message) {
                            binding.message = None;
                            binding.learned = false;
                        } else {
                            seen.push(message);
                        }
                    }
                }
                corrections.push("duplicate midi bindings cleared".to_owned());
            }
            "snap.guide.out_of_bounds" => {
                state.timeline.snap.guide = None;
                state.timeline.snap.phase = SnapPhase::Free;
                corrections.push("snap guide cleared".to_owned());
            }
            "timeline.scroll.out_of_bounds" => {
                state.timeline.viewport = TimelineViewport {
                    zoom: state.timeline.viewport.zoom,
                    scroll: state
                        .timeline
                        .viewport
                        .scroll
                        .min(state.engine.transport.song_length),
                };
                corrections.push("timeline scroll clamped".to_owned());
            }
            "clip.duration.too_short" => {
                for track in &mut state.timeline.tracks {
                    for clip in &mut track.clips {
                        if clip.duration < MIN_CLIP_DURATION {
                            clip.duration = MIN_CLIP_DURATION;
                        }
                    }
                }
                corrections.push("clip durations clamped".to_owned());
            }
            "clip.end.out_of_bounds" => {
                for track in &mut state.timeline.tracks {
                    for clip in &mut track.clips {
                        let clip_end = clip.start.saturating_add(clip.duration);
                        if clip_end > state.engine.transport.song_length {
                            clip.start = clip.start.min(
                                state
                                    .engine
                                    .transport
                                    .song_length
                                    .saturating_sub(clip.duration),
                            );
                        }
                    }
                }
                corrections.push("clip positions clamped".to_owned());
            }
            "clip.linked_cue.missing" => {
                let valid_cues = state
                    .cue_system
                    .cues
                    .iter()
                    .map(|cue| cue.id.0)
                    .collect::<HashSet<_>>();
                for track in &mut state.timeline.tracks {
                    for clip in &mut track.clips {
                        let cue_exists = clip
                            .linked_cue
                            .map(|cue_id| valid_cues.contains(&cue_id.0))
                            .unwrap_or(true);
                        if !cue_exists {
                            clip.linked_cue = None;
                            clip.cue_state = CueVisualState::Inactive;
                        }
                    }
                }
                corrections.push("clip cue links cleared".to_owned());
            }
            "clip.automation.target.duplicate"
            | "clip.automation.point.out_of_bounds"
            | "clip.automation.value.out_of_bounds" => {
                for track in &mut state.timeline.tracks {
                    for clip in &mut track.clips {
                        let mut seen = HashSet::new();
                        clip.automation.retain(|lane| seen.insert(lane.target));
                        for lane in &mut clip.automation {
                            for point in &mut lane.points {
                                point.offset = point.offset.min(clip.duration);
                                point.value = clamp_automation_value(lane.target, point.value);
                            }
                            sort_lane_points(lane);
                        }
                    }
                }
                corrections.push("automation lanes clamped".to_owned());
            }
            "cue.linked_clip.missing" => {
                let valid_clips = state
                    .timeline
                    .tracks
                    .iter()
                    .flat_map(|track| track.clips.iter())
                    .map(|clip| clip.id.0)
                    .collect::<HashSet<_>>();
                for cue in &mut state.cue_system.cues {
                    let clip_exists = cue
                        .linked_clip
                        .map(|clip_id| valid_clips.contains(&clip_id.0))
                        .unwrap_or(true);
                    if !clip_exists {
                        cue.linked_clip = None;
                    }
                }
                corrections.push("cue clip links cleared".to_owned());
            }
            "chase.linked_clip.missing" => {
                let valid_clips = state
                    .timeline
                    .tracks
                    .iter()
                    .flat_map(|track| track.clips.iter())
                    .map(|clip| clip.id.0)
                    .collect::<HashSet<_>>();
                for chase in &mut state.chase_system.chases {
                    let clip_exists = chase
                        .linked_clip
                        .map(|clip_id| valid_clips.contains(&clip_id.0))
                        .unwrap_or(true);
                    if !clip_exists {
                        chase.linked_clip = None;
                    }
                }
                corrections.push("chase clip links cleared".to_owned());
            }
            "chase.step.cue.missing" => {
                let valid_cues = state
                    .cue_system
                    .cues
                    .iter()
                    .map(|cue| cue.id.0)
                    .collect::<HashSet<_>>();
                for chase in &mut state.chase_system.chases {
                    for step in &mut chase.steps {
                        let cue_exists = step
                            .cue_id
                            .map(|cue_id| valid_cues.contains(&cue_id.0))
                            .unwrap_or(true);
                        if !cue_exists {
                            step.cue_id = None;
                        }
                    }
                }
                corrections.push("chase cue links cleared".to_owned());
            }
            "cue.active.invalid" => {
                state.cue_system.active = state
                    .cue_system
                    .cues
                    .iter()
                    .find(|cue| matches!(cue.phase, CuePhase::Triggered | CuePhase::Active))
                    .map(|cue| cue.id);
                corrections.push("active cue recomputed".to_owned());
            }
            "chase.step.out_of_bounds" => {
                for chase in &mut state.chase_system.chases {
                    if chase.steps.is_empty() {
                        chase.current_step = 0;
                        chase.phase = ChasePhase::Stopped;
                    } else {
                        chase.current_step = chase.current_step.min(chase.steps.len() - 1);
                    }
                }
                corrections.push("chase step indices clamped".to_owned());
            }
            "chase.step.duration.zero" => {
                for chase in &mut state.chase_system.chases {
                    for step in &mut chase.steps {
                        step.duration = step.duration.max(MIN_CLIP_DURATION);
                    }
                }
                corrections.push("chase step durations clamped".to_owned());
            }
            "fx.linked_clip.missing" => {
                let valid_clips = state
                    .timeline
                    .tracks
                    .iter()
                    .flat_map(|track| track.clips.iter())
                    .map(|clip| clip.id.0)
                    .collect::<HashSet<_>>();
                for layer in &mut state.fx_system.layers {
                    let clip_exists = layer
                        .linked_clip
                        .map(|clip_id| valid_clips.contains(&clip_id.0))
                        .unwrap_or(true);
                    if !clip_exists {
                        layer.linked_clip = None;
                    }
                }
                corrections.push("fx clip links cleared".to_owned());
            }
            "fx.depth.out_of_bounds" => {
                for layer in &mut state.fx_system.layers {
                    layer.depth_permille = layer.depth_permille.min(1000);
                }
                corrections.push("fx depth clamped".to_owned());
            }
            "fx.spread.out_of_bounds" => {
                for layer in &mut state.fx_system.layers {
                    layer.spread_permille = layer.spread_permille.min(1000);
                }
                corrections.push("fx spread clamped".to_owned());
            }
            "fx.phase_offset.out_of_bounds" => {
                for layer in &mut state.fx_system.layers {
                    layer.phase_offset_permille = layer.phase_offset_permille.min(1000);
                }
                corrections.push("fx phase offset clamped".to_owned());
            }
            "fx.output.out_of_bounds" => {
                for layer in &mut state.fx_system.layers {
                    layer.output_level = layer.output_level.min(1000);
                }
                corrections.push("fx output clamped".to_owned());
            }
            "fixture.online.exceeds_count" => {
                for group in &mut state.fixture_system.groups {
                    group.online = group.online.min(group.fixture_count);
                }
                corrections.push("fixture online counts clamped".to_owned());
            }
            "fixture.linked_cue.missing" => {
                let valid_cues = state
                    .cue_system
                    .cues
                    .iter()
                    .map(|cue| cue.id.0)
                    .collect::<HashSet<_>>();
                for group in &mut state.fixture_system.groups {
                    let cue_exists = group
                        .linked_cue
                        .map(|cue_id| valid_cues.contains(&cue_id.0))
                        .unwrap_or(true);
                    if !cue_exists {
                        group.linked_cue = None;
                    }
                }
                corrections.push("fixture cue links cleared".to_owned());
            }
            "fixture.linked_fx.missing" => {
                let valid_fx = state
                    .fx_system
                    .layers
                    .iter()
                    .map(|layer| layer.id.0)
                    .collect::<HashSet<_>>();
                for group in &mut state.fixture_system.groups {
                    let fx_exists = group
                        .linked_fx
                        .map(|fx_id| valid_fx.contains(&fx_id.0))
                        .unwrap_or(true);
                    if !fx_exists {
                        group.linked_fx = None;
                    }
                }
                corrections.push("fixture fx links cleared".to_owned());
            }
            "fixture.output.out_of_bounds" => {
                for group in &mut state.fixture_system.groups {
                    group.output_level = group.output_level.min(1000);
                }
                corrections.push("fixture output clamped".to_owned());
            }
            "fixture.preview_node.out_of_bounds" => {
                for group in &mut state.fixture_system.groups {
                    for node in &mut group.preview_nodes {
                        node.x_permille = node.x_permille.min(1000);
                        node.y_permille = node.y_permille.min(1000);
                        node.z_permille = node.z_permille.min(1000);
                    }
                }
                corrections.push("fixture preview nodes clamped".to_owned());
            }
            "fixture.patch.profile.missing" => {
                let valid_profiles = state
                    .fixture_system
                    .library
                    .profiles
                    .iter()
                    .map(|profile| profile.id.clone())
                    .collect::<HashSet<_>>();
                state
                    .fixture_system
                    .library
                    .patches
                    .retain(|patch| valid_profiles.contains(&patch.profile_id));
                corrections.push("fixture patches without profile removed".to_owned());
            }
            "fixture.patch.mode.missing" => {
                let profiles = state.fixture_system.library.profiles.clone();
                for patch in &mut state.fixture_system.library.patches {
                    if let Some(profile) = profiles
                        .iter()
                        .find(|profile| profile.id == patch.profile_id)
                        && !profile
                            .modes
                            .iter()
                            .any(|mode| mode.name == patch.mode_name)
                    {
                        if let Some(mode) = profile.modes.first() {
                            patch.mode_name = mode.name.clone();
                        }
                    }
                }
                corrections.push("fixture patch modes corrected".to_owned());
            }
            "fixture.patch.group.missing" => {
                let valid_groups = state
                    .fixture_system
                    .groups
                    .iter()
                    .map(|group| group.id.0)
                    .collect::<HashSet<_>>();
                for patch in &mut state.fixture_system.library.patches {
                    if patch
                        .group_id
                        .map(|group_id| !valid_groups.contains(&group_id.0))
                        .unwrap_or(false)
                    {
                        patch.group_id = None;
                    }
                }
                corrections.push("fixture patch groups cleared".to_owned());
            }
            "fixture.patch.address.out_of_bounds" => {
                for patch in &mut state.fixture_system.library.patches {
                    patch.address = patch.address.clamp(1, 512);
                }
                corrections.push("fixture patch addresses clamped".to_owned());
            }
            "fixture.patch.footprint.zero" => {
                let profiles = state.fixture_system.library.profiles.clone();
                state.fixture_system.library.patches.retain(|patch| {
                    profiles
                        .iter()
                        .find(|profile| profile.id == patch.profile_id)
                        .map(|profile| fixture_mode_channel_count(profile, &patch.mode_name) > 0)
                        .unwrap_or(false)
                });
                corrections.push("fixture patches without footprint removed".to_owned());
            }
            "fixture.patch.range.out_of_bounds" => {
                let profiles = state.fixture_system.library.profiles.clone();
                for patch in &mut state.fixture_system.library.patches {
                    let Some(profile) = profiles
                        .iter()
                        .find(|profile| profile.id == patch.profile_id)
                    else {
                        continue;
                    };
                    let footprint = fixture_mode_channel_count(profile, &patch.mode_name);
                    if footprint == 0 {
                        continue;
                    }

                    let footprint = u16::try_from(footprint).unwrap_or(512);
                    let max_start = 513u16.saturating_sub(footprint.max(1));
                    patch.address = patch.address.clamp(1, max_start.max(1));
                }
                corrections.push("fixture patch ranges clamped".to_owned());
            }
            "fixture.patch.universe.out_of_bounds" => {
                for patch in &mut state.fixture_system.library.patches {
                    patch.universe = patch.universe.clamp(1, 64);
                }
                corrections.push("fixture patch universes clamped".to_owned());
            }
            "fixture.uninitialized.with_online" => {
                for group in &mut state.fixture_system.groups {
                    if group.phase == FixturePhase::Uninitialized && group.online > 0 {
                        group.phase = FixturePhase::Mapped;
                    }
                }
                corrections.push("fixture phase corrected".to_owned());
            }
            _ => {}
        }
    }

    let mut post = validate_state(state);
    post.corrections = corrections;
    post
}

fn validate_authoring_extensions(state: &StudioState, issues: &mut Vec<ValidationIssue>) {
    for track in &state.timeline.tracks {
        for clip in &track.clips {
            let mut targets = HashSet::new();

            for lane in &clip.automation {
                if !targets.insert(lane.target) {
                    issues.push(issue(
                        ValidationIssueKind::ReferenceIntegrity,
                        "clip.automation.target.duplicate",
                        format!("Clip {} enthält doppelte Automation-Lanes.", clip.id.0),
                    ));
                }

                for point in &lane.points {
                    if point.offset > clip.duration {
                        issues.push(issue(
                            ValidationIssueKind::StateConsistency,
                            "clip.automation.point.out_of_bounds",
                            format!(
                                "Automation-Punkt in Clip {} liegt ausserhalb der Clip-Dauer.",
                                clip.id.0
                            ),
                        ));
                    }

                    if point.value != clamp_automation_value(lane.target, point.value) {
                        issues.push(issue(
                            ValidationIssueKind::TypeConsistency,
                            "clip.automation.value.out_of_bounds",
                            format!(
                                "Automation-Wert in Clip {} verletzt die Range von {:?}.",
                                clip.id.0, lane.target
                            ),
                        ));
                    }
                }
            }
        }
    }

    if let Some(target) = state.context_menu.target {
        let valid = match target {
            crate::core::ContextMenuTarget::Clip(clip_id) => state.clip(clip_id).is_some(),
            crate::core::ContextMenuTarget::Track(track_id) => state.track(track_id).is_some(),
            crate::core::ContextMenuTarget::Timeline => true,
        };

        if !valid {
            issues.push(issue(
                ValidationIssueKind::ReferenceIntegrity,
                "context_menu.target.missing",
                "Das Kontextmenü referenziert ein nicht existentes Ziel.".to_owned(),
            ));
        }
    }

    if state
        .clipboard
        .clips
        .iter()
        .any(|entry| state.track(entry.track_id).is_none())
    {
        issues.push(issue(
            ValidationIssueKind::ReferenceIntegrity,
            "clipboard.track.missing",
            "Das Clipboard referenziert einen nicht existenten Track.".to_owned(),
        ));
    }

    if let Some(index) = state.clip_editor.selected_automation_point
        && let Some(clip) = state.editor_clip()
    {
        let lane = clip
            .automation
            .iter()
            .find(|lane| lane.target == state.clip_editor.automation_target);
        let valid = lane.map(|lane| index < lane.points.len()).unwrap_or(false);
        if !valid {
            issues.push(issue(
                ValidationIssueKind::StateConsistency,
                "clip_editor.automation_point.out_of_bounds",
                "Der Clip-Editor selektiert einen nicht existenten Automation-Punkt.".to_owned(),
            ));
        }
    }

    if state.replay_log.events.len() > state.replay_log.capacity {
        issues.push(issue(
            ValidationIssueKind::Determinism,
            "replay_log.capacity.exceeded",
            "Der Replay-Log überschreitet seine feste Kapazität.".to_owned(),
        ));
    }

    if let Some(selected) = state.venture.selected.as_deref()
        && state
            .venture
            .ventures
            .iter()
            .all(|venture| venture.id != selected)
    {
        issues.push(issue(
            ValidationIssueKind::ReferenceIntegrity,
            "venture.selected.missing",
            format!(
                "Das selektierte Venture {} fehlt im Registry-State.",
                selected
            ),
        ));
    }

    if let Some(selected) = state.venture.selected_recovery.as_deref()
        && state
            .venture
            .recovery_slots
            .iter()
            .all(|slot| slot.id != selected)
    {
        issues.push(issue(
            ValidationIssueKind::ReferenceIntegrity,
            "venture.recovery.selected.missing",
            format!(
                "Der selektierte Recovery-Slot {} fehlt im Recovery-Registry-State.",
                selected
            ),
        ));
    }
}

fn validate_settings_hardware(state: &StudioState, issues: &mut Vec<ValidationIssue>) {
    if let Some(selected) = state.settings.dmx.selected_interface.as_deref()
        && state
            .settings
            .dmx
            .interfaces
            .iter()
            .all(|interface| interface.id != selected)
    {
        issues.push(issue(
            ValidationIssueKind::ReferenceIntegrity,
            "settings.dmx.interface.selected.missing",
            format!("Selektiertes DMX-Interface {} existiert nicht.", selected),
        ));
    }

    if let Some(selected) = state.settings.midi.selected_input.as_deref()
        && state
            .settings
            .midi
            .inputs
            .iter()
            .all(|port| port.id != selected)
    {
        issues.push(issue(
            ValidationIssueKind::ReferenceIntegrity,
            "settings.midi.input.selected.missing",
            format!("Selektierter MIDI-Input {} existiert nicht.", selected),
        ));
    }

    if let Some(selected) = state.settings.midi.selected_output.as_deref()
        && state
            .settings
            .midi
            .outputs
            .iter()
            .all(|port| port.id != selected)
    {
        issues.push(issue(
            ValidationIssueKind::ReferenceIntegrity,
            "settings.midi.output.selected.missing",
            format!("Selektierter MIDI-Output {} existiert nicht.", selected),
        ));
    }

    if !(1..=44).contains(&state.settings.dmx.refresh_rate_hz) {
        issues.push(issue(
            ValidationIssueKind::TypeConsistency,
            "settings.dmx.refresh_rate.out_of_bounds",
            format!(
                "DMX-Refresh-Rate {} Hz liegt außerhalb 1..=44.",
                state.settings.dmx.refresh_rate_hz
            ),
        ));
    }

    if !(88..=1000).contains(&state.settings.dmx.enttec_break_us) {
        issues.push(issue(
            ValidationIssueKind::TimingConsistency,
            "settings.dmx.enttec.break.out_of_bounds",
            format!(
                "ENTTEC Break {} µs liegt außerhalb 88..=1000.",
                state.settings.dmx.enttec_break_us
            ),
        ));
    }

    if !(8..=1000).contains(&state.settings.dmx.enttec_mark_after_break_us) {
        issues.push(issue(
            ValidationIssueKind::TimingConsistency,
            "settings.dmx.enttec.mab.out_of_bounds",
            format!(
                "ENTTEC Mark After Break {} µs liegt außerhalb 8..=1000.",
                state.settings.dmx.enttec_mark_after_break_us
            ),
        ));
    }

    if state.settings.dmx.backend == DmxBackendKind::EnttecOpenDmx {
        match state.selected_dmx_interface() {
            Some(interface)
                if !matches!(
                    interface.kind,
                    DmxInterfaceKind::EnttecOpenDmxCompatible | DmxInterfaceKind::UsbSerial
                ) =>
            {
                issues.push(issue(
                    ValidationIssueKind::ReferenceIntegrity,
                    "settings.dmx.enttec.interface.incompatible",
                    format!(
                        "Das selektierte DMX-Interface {} ist nicht ENTTEC/Open-DMX-kompatibel.",
                        interface.name
                    ),
                ));
            }
            None if state.settings.dmx.output_enabled => {
                issues.push(issue(
                    ValidationIssueKind::StateConsistency,
                    "settings.dmx.output.without_interface",
                    "DMX-Output ist aktiviert, aber kein Interface ist selektiert.".to_owned(),
                ));
            }
            _ => {}
        }
    }

    if state.settings.midi.detected_controller.is_some() && state.selected_midi_input().is_none() {
        issues.push(issue(
            ValidationIssueKind::StateConsistency,
            "settings.midi.detected_controller.missing_input",
            "Ein erkannter Controller ohne selektierten MIDI-Input ist nicht konsistent."
                .to_owned(),
        ));
    }

    if state.settings.midi.learn.phase != MidiLearnPhase::Idle
        && state.selected_midi_input().is_none()
    {
        issues.push(issue(
            ValidationIssueKind::StateConsistency,
            "settings.midi.learn.without_input",
            "MIDI Learn ist aktiv, aber kein MIDI-Input ist selektiert.".to_owned(),
        ));
    }

    if state.settings.midi.learn.phase == MidiLearnPhase::Idle
        && (state.settings.midi.learn.target_binding.is_some()
            || !state.settings.midi.learn.capture_queue.is_empty())
    {
        issues.push(issue(
            ValidationIssueKind::StateConsistency,
            "settings.midi.learn.idle_state.invalid",
            "Der MIDI-Learn-State ist idle, hält aber noch Targets oder Queue-Einträge.".to_owned(),
        ));
    }

    if let Some(target_binding) = state.settings.midi.learn.target_binding
        && state.midi_binding(target_binding).is_none()
    {
        issues.push(issue(
            ValidationIssueKind::ReferenceIntegrity,
            "settings.midi.learn.target.missing",
            format!(
                "MIDI Learn referenziert unbekanntes Binding {}.",
                target_binding
            ),
        ));
    }

    let valid_binding_ids = state
        .settings
        .midi
        .bindings
        .iter()
        .map(|binding| binding.id)
        .collect::<HashSet<_>>();
    if state
        .settings
        .midi
        .learn
        .capture_queue
        .iter()
        .any(|binding_id| !valid_binding_ids.contains(binding_id))
    {
        issues.push(issue(
            ValidationIssueKind::ReferenceIntegrity,
            "settings.midi.learn.queue.invalid",
            "Die MIDI-Learn-Queue enthält unbekannte Binding-Ids.".to_owned(),
        ));
    }

    let mut seen_messages = Vec::new();
    for binding in &state.settings.midi.bindings {
        if let Some(message) = &binding.message {
            if seen_messages.contains(message) {
                issues.push(issue(
                    ValidationIssueKind::Determinism,
                    "settings.midi.binding.duplicate",
                    format!("MIDI-Binding {} dupliziert {}.", binding.id, binding.label),
                ));
            } else {
                seen_messages.push(message.clone());
            }
        }
    }
}

fn validate_engine_link(state: &StudioState, issues: &mut Vec<ValidationIssue>) {
    if !(1_024..=65_535).contains(&state.settings.engine_link.discovery_port) {
        issues.push(issue(
            ValidationIssueKind::TypeConsistency,
            "settings.engine.discovery_port.out_of_bounds",
            format!(
                "Engine-Link Discovery-Port {} liegt außerhalb 1024..=65535.",
                state.settings.engine_link.discovery_port
            ),
        ));
    }

    if let Some(selected) = state.settings.engine_link.selected_device.as_deref()
        && state
            .settings
            .engine_link
            .devices
            .iter()
            .all(|device| device.id != selected)
    {
        issues.push(issue(
            ValidationIssueKind::ReferenceIntegrity,
            "settings.engine.device.selected.missing",
            format!(
                "Selektiertes Engine-Link-Device {} existiert nicht.",
                selected
            ),
        ));
    }

    if let Some(telemetry) = state.settings.engine_link.telemetry.as_ref()
        && let Some(selected) = state.settings.engine_link.selected_device.as_deref()
        && selected != telemetry.device_id
    {
        issues.push(issue(
            ValidationIssueKind::StateConsistency,
            "settings.engine.telemetry.device.mismatch",
            format!(
                "Engine-Link-Telemetrie {} passt nicht zur Selektions-Id {}.",
                telemetry.device_id, selected
            ),
        ));
    }
}

fn validate_ids(state: &StudioState, issues: &mut Vec<ValidationIssue>) {
    let mut track_ids = HashSet::new();
    let mut clip_ids = HashSet::new();
    let mut cue_ids = HashSet::new();
    let mut chase_ids = HashSet::new();
    let mut fx_ids = HashSet::new();
    let mut fixture_ids = HashSet::new();
    let mut fixture_profile_ids = HashSet::new();
    let mut fixture_patch_ids = HashSet::new();

    for track in &state.timeline.tracks {
        if !track_ids.insert(track.id.0) {
            issues.push(issue(
                ValidationIssueKind::ReferenceIntegrity,
                "track.id.duplicate",
                format!("TrackId {} ist nicht eindeutig.", track.id.0),
            ));
        }

        for clip in &track.clips {
            if !clip_ids.insert(clip.id.0) {
                issues.push(issue(
                    ValidationIssueKind::ReferenceIntegrity,
                    "clip.id.duplicate",
                    format!("ClipId {} ist nicht eindeutig.", clip.id.0),
                ));
            }
        }
    }

    for cue in &state.cue_system.cues {
        if !cue_ids.insert(cue.id.0) {
            issues.push(issue(
                ValidationIssueKind::ReferenceIntegrity,
                "cue.id.duplicate",
                format!("CueId {} ist nicht eindeutig.", cue.id.0),
            ));
        }
    }

    for chase in &state.chase_system.chases {
        if !chase_ids.insert(chase.id.0) {
            issues.push(issue(
                ValidationIssueKind::ReferenceIntegrity,
                "chase.id.duplicate",
                format!("ChaseId {} ist nicht eindeutig.", chase.id.0),
            ));
        }
    }

    for layer in &state.fx_system.layers {
        if !fx_ids.insert(layer.id.0) {
            issues.push(issue(
                ValidationIssueKind::ReferenceIntegrity,
                "fx.id.duplicate",
                format!("FxId {} ist nicht eindeutig.", layer.id.0),
            ));
        }
    }

    for group in &state.fixture_system.groups {
        if !fixture_ids.insert(group.id.0) {
            issues.push(issue(
                ValidationIssueKind::ReferenceIntegrity,
                "fixture.id.duplicate",
                format!("FixtureGroupId {} ist nicht eindeutig.", group.id.0),
            ));
        }
    }

    for profile in &state.fixture_system.library.profiles {
        if !fixture_profile_ids.insert(profile.id.clone()) {
            issues.push(issue(
                ValidationIssueKind::ReferenceIntegrity,
                "fixture.profile.id.duplicate",
                format!("Fixture-Profil {} ist nicht eindeutig.", profile.id),
            ));
        }
    }

    for patch in &state.fixture_system.library.patches {
        if !fixture_patch_ids.insert(patch.id) {
            issues.push(issue(
                ValidationIssueKind::ReferenceIntegrity,
                "fixture.patch.id.duplicate",
                format!("Fixture-Patch {} ist nicht eindeutig.", patch.id),
            ));
        }
    }
}

fn validate_selection_and_hover(state: &StudioState, issues: &mut Vec<ValidationIssue>) {
    match state.timeline.selection {
        SelectionState::Clip(clip_id) => {
            if state.clip(clip_id).is_none() {
                issues.push(issue(
                    ValidationIssueKind::ReferenceIntegrity,
                    "selection.clip.missing",
                    format!("Selektierter Clip {} existiert nicht.", clip_id.0),
                ));
            }

            let mut ids = HashSet::new();
            let clips_valid = !state.timeline.selected_clips.is_empty()
                && state.timeline.selected_clips.contains(&clip_id)
                && state
                    .timeline
                    .selected_clips
                    .iter()
                    .all(|clip_id| ids.insert(clip_id.0) && state.clip(*clip_id).is_some());

            if !clips_valid {
                issues.push(issue(
                    ValidationIssueKind::StateConsistency,
                    "selection.clip_set.invalid",
                    "Clip-Selektion ist leer, dupliziert oder referenziert ungültige Clips."
                        .to_owned(),
                ));
            }
        }
        SelectionState::Track(track_id) if state.track(track_id).is_none() => issues.push(issue(
            ValidationIssueKind::ReferenceIntegrity,
            "selection.track.missing",
            format!("Selektierter Track {} existiert nicht.", track_id.0),
        )),
        SelectionState::Track(_) => {
            if !state.timeline.selected_clips.is_empty() {
                issues.push(issue(
                    ValidationIssueKind::StateConsistency,
                    "selection.clip_set.invalid",
                    "Track-Selektion darf keine Clip-Mengen führen.".to_owned(),
                ));
            }
        }
        SelectionState::None => {
            if !state.timeline.selected_clips.is_empty() {
                issues.push(issue(
                    ValidationIssueKind::StateConsistency,
                    "selection.clip_set.invalid",
                    "Leere Selektion darf keine Clip-Mengen führen.".to_owned(),
                ));
            }
        }
    }

    match state.timeline.hover {
        Some(HoverTarget::ClipBody(clip_id))
        | Some(HoverTarget::ClipStartHandle(clip_id))
        | Some(HoverTarget::ClipEndHandle(clip_id))
            if state.clip(clip_id).is_none() =>
        {
            issues.push(issue(
                ValidationIssueKind::ReferenceIntegrity,
                "hover.clip.missing",
                format!("Hover-Clip {} existiert nicht.", clip_id.0),
            ));
        }
        _ => {}
    }

    if let Some(cue_id) = state.cue_system.selected
        && state.cue(cue_id).is_none()
    {
        issues.push(issue(
            ValidationIssueKind::ReferenceIntegrity,
            "selection.cue.missing",
            format!("Selektierter Cue {} existiert nicht.", cue_id.0),
        ));
    }

    if let Some(chase_id) = state.chase_system.selected
        && state.chase(chase_id).is_none()
    {
        issues.push(issue(
            ValidationIssueKind::ReferenceIntegrity,
            "selection.chase.missing",
            format!("Selektierter Chase {} existiert nicht.", chase_id.0),
        ));
    }

    if let Some(selected_step) = state.chase_system.selected_step {
        let valid = state
            .selected_chase()
            .map(|chase| selected_step < chase.steps.len())
            .unwrap_or(false);
        if !valid {
            issues.push(issue(
                ValidationIssueKind::StateConsistency,
                "selection.chase.step.invalid",
                format!("Selektierter Chase-Step {} ist ungueltig.", selected_step),
            ));
        }
    }

    if let Some(fx_id) = state.fx_system.selected
        && state.fx_layer(fx_id).is_none()
    {
        issues.push(issue(
            ValidationIssueKind::ReferenceIntegrity,
            "selection.fx.missing",
            format!("Selektierter FX-Layer {} existiert nicht.", fx_id.0),
        ));
    }

    if let Some(group_id) = state.fixture_system.selected
        && state.fixture_group(group_id).is_none()
    {
        issues.push(issue(
            ValidationIssueKind::ReferenceIntegrity,
            "selection.fixture.missing",
            format!("Selektierte Fixture-Gruppe {} existiert nicht.", group_id.0),
        ));
    }

    if let Some(profile_id) = state.fixture_system.library.selected_profile.as_deref()
        && state.fixture_profile(profile_id).is_none()
    {
        issues.push(issue(
            ValidationIssueKind::ReferenceIntegrity,
            "fixture.profile.selected.missing",
            format!(
                "Selektiertes Fixture-Profil {} existiert nicht.",
                profile_id
            ),
        ));
    }

    if let Some(patch_id) = state.fixture_system.library.selected_patch
        && state.fixture_patch(patch_id).is_none()
    {
        issues.push(issue(
            ValidationIssueKind::ReferenceIntegrity,
            "fixture.patch.selected.missing",
            format!("Selektierter Fixture-Patch {} existiert nicht.", patch_id),
        ));
    }

    if state.clip_editor.phase != ClipEditorPhase::Closed {
        match state.clip_editor.clip_id {
            Some(clip_id) if state.clip(clip_id).is_none() => issues.push(issue(
                ValidationIssueKind::ReferenceIntegrity,
                "clip_editor.clip.missing",
                format!("Clip-Editor referenziert unbekannten Clip {}.", clip_id.0),
            )),
            Some(clip_id)
                if state.timeline.selection != SelectionState::Clip(clip_id)
                    || state.timeline.selected_clips != vec![clip_id] =>
            {
                issues.push(issue(
                    ValidationIssueKind::StateConsistency,
                    "clip_editor.selection.mismatch",
                    format!(
                        "Clip-Editor für Clip {} ist offen, Selection zeigt auf etwas anderes.",
                        clip_id.0
                    ),
                ));
            }
            Some(_) => {}
            None => issues.push(issue(
                ValidationIssueKind::StateConsistency,
                "clip_editor.clip.missing",
                "Clip-Editor ist offen, aber ohne ClipId.".to_owned(),
            )),
        }
    }
}

fn validate_timeline_bounds(state: &StudioState, issues: &mut Vec<ValidationIssue>) {
    if state.timeline.viewport.scroll > state.engine.transport.song_length {
        issues.push(issue(
            ValidationIssueKind::StateConsistency,
            "timeline.scroll.out_of_bounds",
            "Timeline-Scroll liegt hinter der Songlänge.".to_owned(),
        ));
    }

    if let Some(guide) = &state.timeline.snap.guide
        && guide.beat > state.engine.transport.song_length
    {
        issues.push(issue(
            ValidationIssueKind::StateConsistency,
            "snap.guide.out_of_bounds",
            "Snap-Guide liegt außerhalb des Songs.".to_owned(),
        ));
    }

    for track in &state.timeline.tracks {
        for clip in &track.clips {
            if clip.duration < MIN_CLIP_DURATION {
                issues.push(issue(
                    ValidationIssueKind::TypeConsistency,
                    "clip.duration.too_short",
                    format!("Clip {} ist kürzer als das Minimum.", clip.id.0),
                ));
            }

            if clip.start.saturating_add(clip.duration) > state.engine.transport.song_length {
                issues.push(issue(
                    ValidationIssueKind::StateConsistency,
                    "clip.end.out_of_bounds",
                    format!("Clip {} endet hinter der Songlänge.", clip.id.0),
                ));
            }
        }
    }
}

fn validate_show_references(state: &StudioState, issues: &mut Vec<ValidationIssue>) {
    for track in &state.timeline.tracks {
        for clip in &track.clips {
            if let Some(cue_id) = clip.linked_cue
                && state.cue(cue_id).is_none()
            {
                issues.push(issue(
                    ValidationIssueKind::ReferenceIntegrity,
                    "clip.linked_cue.missing",
                    format!(
                        "Clip {} referenziert unbekannten Cue {}.",
                        clip.id.0, cue_id.0
                    ),
                ));
            }
        }
    }

    for cue in &state.cue_system.cues {
        if let Some(clip_id) = cue.linked_clip
            && state.clip(clip_id).is_none()
        {
            issues.push(issue(
                ValidationIssueKind::ReferenceIntegrity,
                "cue.linked_clip.missing",
                format!(
                    "Cue {} referenziert unbekannten Clip {}.",
                    cue.id.0, clip_id.0
                ),
            ));
        }
    }

    if let Some(active_cue_id) = state.cue_system.active {
        match state.cue(active_cue_id) {
            Some(cue)
                if !matches!(
                    cue.phase,
                    CuePhase::Triggered | CuePhase::Fading | CuePhase::Active
                ) =>
            {
                issues.push(issue(
                    ValidationIssueKind::StateConsistency,
                    "cue.active.invalid",
                    format!("Aktiver Cue {} ist nicht aktiv genug.", active_cue_id.0),
                ));
            }
            None => issues.push(issue(
                ValidationIssueKind::ReferenceIntegrity,
                "cue.active.invalid",
                format!("Aktiver Cue {} existiert nicht.", active_cue_id.0),
            )),
            Some(_) => {}
        }
    }

    for chase in &state.chase_system.chases {
        if chase.steps.is_empty() || chase.current_step >= chase.steps.len() {
            issues.push(issue(
                ValidationIssueKind::StateConsistency,
                "chase.step.out_of_bounds",
                format!("Chase {} hat einen ungültigen Step-Index.", chase.id.0),
            ));
        }

        if let Some(clip_id) = chase.linked_clip
            && state.clip(clip_id).is_none()
        {
            issues.push(issue(
                ValidationIssueKind::ReferenceIntegrity,
                "chase.linked_clip.missing",
                format!(
                    "Chase {} referenziert unbekannten Clip {}.",
                    chase.id.0, clip_id.0
                ),
            ));
        }

        for step in &chase.steps {
            if let Some(cue_id) = step.cue_id
                && state.cue(cue_id).is_none()
            {
                issues.push(issue(
                    ValidationIssueKind::ReferenceIntegrity,
                    "chase.step.cue.missing",
                    format!(
                        "Chase {} referenziert unbekannten Cue {}.",
                        chase.id.0, cue_id.0
                    ),
                ));
            }

            if step.duration < MIN_CLIP_DURATION {
                issues.push(issue(
                    ValidationIssueKind::TimingConsistency,
                    "chase.step.duration.zero",
                    format!(
                        "Chase {} enthaelt einen Step mit zu kurzer Dauer ({} Ticks).",
                        chase.id.0,
                        step.duration.ticks()
                    ),
                ));
            }
        }
    }

    for layer in &state.fx_system.layers {
        if let Some(clip_id) = layer.linked_clip
            && state.clip(clip_id).is_none()
        {
            issues.push(issue(
                ValidationIssueKind::ReferenceIntegrity,
                "fx.linked_clip.missing",
                format!(
                    "FX {} referenziert unbekannten Clip {}.",
                    layer.id.0, clip_id.0
                ),
            ));
        }

        if layer.depth_permille > 1000 {
            issues.push(issue(
                ValidationIssueKind::TypeConsistency,
                "fx.depth.out_of_bounds",
                format!(
                    "FX {} hat Depth {} > 1000.",
                    layer.id.0, layer.depth_permille
                ),
            ));
        }

        if layer.spread_permille > 1000 {
            issues.push(issue(
                ValidationIssueKind::TypeConsistency,
                "fx.spread.out_of_bounds",
                format!(
                    "FX {} hat Spread {} > 1000.",
                    layer.id.0, layer.spread_permille
                ),
            ));
        }

        if layer.phase_offset_permille > 1000 {
            issues.push(issue(
                ValidationIssueKind::TypeConsistency,
                "fx.phase_offset.out_of_bounds",
                format!(
                    "FX {} hat Phase Offset {} > 1000.",
                    layer.id.0, layer.phase_offset_permille
                ),
            ));
        }

        if layer.output_level > 1000 {
            issues.push(issue(
                ValidationIssueKind::StateConsistency,
                "fx.output.out_of_bounds",
                format!(
                    "FX {} hat Output {} > 1000.",
                    layer.id.0, layer.output_level
                ),
            ));
        }
    }

    for group in &state.fixture_system.groups {
        if let Some(cue_id) = group.linked_cue
            && state.cue(cue_id).is_none()
        {
            issues.push(issue(
                ValidationIssueKind::ReferenceIntegrity,
                "fixture.linked_cue.missing",
                format!(
                    "Fixture-Gruppe {} referenziert unbekannten Cue {}.",
                    group.id.0, cue_id.0
                ),
            ));
        }

        if let Some(fx_id) = group.linked_fx
            && state.fx_layer(fx_id).is_none()
        {
            issues.push(issue(
                ValidationIssueKind::ReferenceIntegrity,
                "fixture.linked_fx.missing",
                format!(
                    "Fixture-Gruppe {} referenziert unbekannten FX {}.",
                    group.id.0, fx_id.0
                ),
            ));
        }

        if group.online > group.fixture_count {
            issues.push(issue(
                ValidationIssueKind::StateConsistency,
                "fixture.online.exceeds_count",
                format!(
                    "Fixture-Gruppe {} meldet {} online bei {} Geräten.",
                    group.id.0, group.online, group.fixture_count
                ),
            ));
        }

        if group.output_level > 1000 {
            issues.push(issue(
                ValidationIssueKind::StateConsistency,
                "fixture.output.out_of_bounds",
                format!(
                    "Fixture-Gruppe {} hat Output {} > 1000.",
                    group.id.0, group.output_level
                ),
            ));
        }

        for node in &group.preview_nodes {
            if node.x_permille > 1000 || node.y_permille > 1000 || node.z_permille > 1000 {
                issues.push(issue(
                    ValidationIssueKind::TypeConsistency,
                    "fixture.preview_node.out_of_bounds",
                    format!(
                        "Fixture-Gruppe {} enthält Preview-Node {} außerhalb 0..=1000.",
                        group.id.0, node.label
                    ),
                ));
            }
        }
    }

    for patch in &state.fixture_system.library.patches {
        let Some(profile) = state.fixture_profile(&patch.profile_id) else {
            issues.push(issue(
                ValidationIssueKind::ReferenceIntegrity,
                "fixture.patch.profile.missing",
                format!(
                    "Fixture-Patch {} referenziert unbekanntes Profil {}.",
                    patch.id, patch.profile_id
                ),
            ));
            continue;
        };

        if !profile
            .modes
            .iter()
            .any(|mode| mode.name == patch.mode_name)
        {
            issues.push(issue(
                ValidationIssueKind::ReferenceIntegrity,
                "fixture.patch.mode.missing",
                format!(
                    "Fixture-Patch {} referenziert unbekannten Mode {}.",
                    patch.id, patch.mode_name
                ),
            ));
        }

        if !(1..=64).contains(&patch.universe) {
            issues.push(issue(
                ValidationIssueKind::TypeConsistency,
                "fixture.patch.universe.out_of_bounds",
                format!(
                    "Fixture-Patch {} liegt auf ungueltigem Universe {}.",
                    patch.id, patch.universe
                ),
            ));
        }

        if !(1..=512).contains(&patch.address) {
            issues.push(issue(
                ValidationIssueKind::TypeConsistency,
                "fixture.patch.address.out_of_bounds",
                format!(
                    "Fixture-Patch {} liegt auf ungueltiger Adresse {}.",
                    patch.id, patch.address
                ),
            ));
        }

        if let Some(group_id) = patch.group_id
            && state.fixture_group(group_id).is_none()
        {
            issues.push(issue(
                ValidationIssueKind::ReferenceIntegrity,
                "fixture.patch.group.missing",
                format!(
                    "Fixture-Patch {} referenziert unbekannte Gruppe {}.",
                    patch.id, group_id.0
                ),
            ));
        }

        let footprint = state.fixture_patch_channel_count(patch).unwrap_or(0);
        if footprint == 0 {
            issues.push(issue(
                ValidationIssueKind::TypeConsistency,
                "fixture.patch.footprint.zero",
                format!(
                    "Fixture-Patch {} besitzt keinen gueltigen DMX-Footprint.",
                    patch.id
                ),
            ));
            continue;
        }

        let end_address = patch.address.saturating_add(footprint.saturating_sub(1));
        if end_address > 512 {
            issues.push(issue(
                ValidationIssueKind::TimingConsistency,
                "fixture.patch.range.out_of_bounds",
                format!(
                    "Fixture-Patch {} belegt U{}.{}-{} und endet hinter Kanal 512.",
                    patch.id, patch.universe, patch.address, end_address
                ),
            ));
        }
    }

    for summary in state.fixture_universe_summaries() {
        if !summary.conflicting_patch_ids.is_empty() {
            issues.push(issue(
                ValidationIssueKind::StateConsistency,
                "fixture.patch.address.conflict",
                format!(
                    "Universe {} enthaelt ueberlappende Fixture-Patches {:?}.",
                    summary.universe, summary.conflicting_patch_ids
                ),
            ));
        }
    }
}

fn validate_event_queue(state: &StudioState, issues: &mut Vec<ValidationIssue>) {
    let mut last = 0u64;
    for queued in &state.event_queue.queue {
        if queued.sequence <= last {
            issues.push(issue(
                ValidationIssueKind::Determinism,
                "event.sequence.order",
                "Die Event-Reihenfolge ist nicht streng monoton.".to_owned(),
            ));
        }
        last = queued.sequence;
    }
}

fn validate_phase_consistency(state: &StudioState, issues: &mut Vec<ValidationIssue>) {
    if state.engine.clock.frame_interval_ns == 0 {
        issues.push(issue(
            ValidationIssueKind::TimingConsistency,
            "clock.interval.zero",
            "Die monotone Clock hat kein gültiges Frame-Intervall.".to_owned(),
        ));
    }

    if state.lifecycle == StateLifecycle::Invalid && state.engine.phase != EnginePhase::Error {
        issues.push(issue(
            ValidationIssueKind::StateConsistency,
            "lifecycle.invalid.without_engine_error",
            "Invalid-State ohne Engine-Fehlerzustand.".to_owned(),
        ));
    }

    if state.timeline.phase == TimelinePhase::Snapping
        && state.timeline.snap.phase == SnapPhase::Free
    {
        issues.push(issue(
            ValidationIssueKind::StateConsistency,
            "timeline.snapping.without_snap_phase",
            "Timeline ist in Snapping, Snap-FSM aber nicht.".to_owned(),
        ));
    }

    for group in &state.fixture_system.groups {
        if group.phase == FixturePhase::Uninitialized && group.online > 0 {
            issues.push(issue(
                ValidationIssueKind::StateConsistency,
                "fixture.uninitialized.with_online",
                format!(
                    "Fixture-Gruppe {} ist uninitialisiert, meldet aber {} Online-Geräte.",
                    group.id.0, group.online
                ),
            ));
        }
    }
}

fn issue(kind: ValidationIssueKind, code: &str, detail: String) -> ValidationIssue {
    ValidationIssue {
        kind,
        code: code.to_owned(),
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        BeatTime, ClipId, CueId, EngineDeckFollowMode, EngineDeckPhase, EnginePrimeDevice,
        EngineTelemetryFrame, FixtureGroupId, FixtureMode, FixturePatch, FixtureProfile,
        FixtureSourceInfo, FixtureSourceKind, IntensityLevel, MidiBinding, MidiBindingMessage,
        MidiControlHint, MidiLearnPhase, MidiMessageKind, SelectionState, StudioState, TempoBpm,
    };

    #[test]
    fn validation_detects_missing_selected_clip() {
        let mut state = StudioState::default();
        state.timeline.selection = SelectionState::Clip(ClipId(9999));
        state.timeline.selected_clips = vec![ClipId(9999)];

        let report = validate_state(&state);
        assert!(!report.valid);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "selection.clip.missing")
        );
    }

    #[test]
    fn recovery_clears_invalid_selection() {
        let mut state = StudioState::default();
        state.timeline.selection = SelectionState::Clip(ClipId(9999));
        state.timeline.selected_clips = vec![ClipId(9999)];

        let report = validate_state(&state);
        let recovered = recover_state(&mut state, &report);

        assert!(recovered.valid);
        assert_eq!(state.timeline.selection, SelectionState::None);
        assert!(state.timeline.selected_clips.is_empty());
    }

    #[test]
    fn recovery_clamps_fx_and_fixture_ranges() {
        let mut state = StudioState::default();
        state.fx_system.layers[0].depth_permille = 1400;
        state.fx_system.layers[0].output_level = 1600;
        state.fixture_system.groups[0].online = 32;
        state.fixture_system.groups[0].output_level = 1500;

        let report = validate_state(&state);
        let recovered = recover_state(&mut state, &report);

        assert!(recovered.valid);
        assert_eq!(state.fx_system.layers[0].depth_permille, 1000);
        assert_eq!(state.fx_system.layers[0].output_level, 1000);
        assert_eq!(
            state
                .fixture_group(FixtureGroupId(1))
                .expect("fixture exists")
                .online,
            16
        );
        assert_eq!(
            state
                .fixture_group(FixtureGroupId(1))
                .expect("fixture exists")
                .output_level,
            1000
        );
    }

    #[test]
    fn recovery_clamps_fx_preview_and_fixture_preview_ranges() {
        let mut state = StudioState::default();
        state.fx_system.layers[0].spread_permille = 1400;
        state.fx_system.layers[0].phase_offset_permille = 1700;
        state.fixture_system.groups[0].preview_nodes[0].x_permille = 1300;
        state.fixture_system.groups[0].preview_nodes[0].y_permille = 1200;
        state.fixture_system.groups[0].preview_nodes[0].z_permille = 1900;

        let report = validate_state(&state);
        let recovered = recover_state(&mut state, &report);

        assert!(recovered.valid);
        assert_eq!(state.fx_system.layers[0].spread_permille, 1000);
        assert_eq!(state.fx_system.layers[0].phase_offset_permille, 1000);
        assert_eq!(
            state.fixture_system.groups[0].preview_nodes[0].x_permille,
            1000
        );
        assert_eq!(
            state.fixture_system.groups[0].preview_nodes[0].y_permille,
            1000
        );
        assert_eq!(
            state.fixture_system.groups[0].preview_nodes[0].z_permille,
            1000
        );
    }

    #[test]
    fn validation_detects_missing_clip_cue_reference() {
        let mut state = StudioState::default();
        state.timeline.tracks[0].clips[0].linked_cue = Some(CueId(9999));

        let report = validate_state(&state);

        assert!(!report.valid);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "clip.linked_cue.missing")
        );
    }

    #[test]
    fn recovery_clamps_automation_lane_ranges() {
        let mut state = StudioState::default();
        state.timeline.tracks[0].clips[0].automation[0].points[0].value = 1400;
        state.timeline.tracks[0].clips[0].automation[1].points[0].value = 40;
        state.timeline.tracks[0].clips[0].automation[0].points[0].offset = BeatTime::from_beats(32);

        let report = validate_state(&state);
        let recovered = recover_state(&mut state, &report);

        assert!(recovered.valid);
        assert!(
            state.timeline.tracks[0].clips[0].automation[0]
                .points
                .iter()
                .any(|point| point.value == 1000)
        );
        assert!(
            state.timeline.tracks[0].clips[0].automation[1]
                .points
                .iter()
                .any(|point| point.value == 200)
        );
        assert!(
            state.timeline.tracks[0].clips[0].automation[0].points[0].offset
                <= state.timeline.tracks[0].clips[0].duration
        );
    }

    #[test]
    fn recovery_resets_invalid_selected_chase_step() {
        let mut state = StudioState::default();
        state.chase_system.selected = Some(crate::core::ChaseId(1));
        state.chase_system.selected_step = Some(99);

        let report = validate_state(&state);
        let recovered = recover_state(&mut state, &report);

        assert!(recovered.valid);
        assert_eq!(state.chase_system.selected_step, Some(0));
    }

    #[test]
    fn recovery_clamps_zero_chase_step_duration() {
        let mut state = StudioState::default();
        state.chase_system.chases[0].steps[0].duration = BeatTime::ZERO;

        let report = validate_state(&state);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "chase.step.duration.zero")
        );

        let recovered = recover_state(&mut state, &report);

        assert!(recovered.valid);
        assert_eq!(
            state.chase_system.chases[0].steps[0].duration,
            MIN_CLIP_DURATION
        );
    }

    #[test]
    fn recovery_resets_missing_selected_venture() {
        let mut state = StudioState::default();
        state.venture.selected = Some("ghost-venture".to_owned());

        let report = validate_state(&state);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "venture.selected.missing")
        );

        let recovered = recover_state(&mut state, &report);

        assert!(recovered.valid);
        assert!(state.venture.selected.is_none());
    }

    #[test]
    fn recovery_resets_missing_selected_recovery_slot() {
        let mut state = StudioState::default();
        state.venture.selected_recovery = Some("ghost-recovery".to_owned());

        let report = validate_state(&state);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "venture.recovery.selected.missing")
        );

        let recovered = recover_state(&mut state, &report);

        assert!(recovered.valid);
        assert!(state.venture.selected_recovery.is_none());
    }

    #[test]
    fn recovery_resets_missing_selected_fixture_profile() {
        let mut state = StudioState::default();
        state.fixture_system.library.selected_profile = Some("ghost-profile".to_owned());

        let report = validate_state(&state);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "fixture.profile.selected.missing")
        );

        let recovered = recover_state(&mut state, &report);

        assert!(recovered.valid);
        assert!(state.fixture_system.library.selected_profile.is_none());
    }

    #[test]
    fn recovery_corrects_invalid_fixture_patch_references() {
        let mut state = StudioState::default();
        let profile = crate::core::import_ofl_fixture(
            r#"{
              "$schema":"https://raw.githubusercontent.com/OpenLightingProject/open-fixture-library/master/schemas/fixture.json",
              "name":"Demo Bar 8",
              "categories":["Pixel Bar"],
              "meta":{"authors":["Tester"],"createDate":"2024-01-01","lastModifyDate":"2024-01-02"},
              "availableChannels":{
                "Dimmer":{"capability":{"type":"Intensity"}},
                "Red":{"capability":{"type":"ColorIntensity","color":"Red"}}
              },
              "modes":[{"name":"2ch","channels":["Dimmer","Red"]}]
            }"#,
            Some("demo"),
            Some("demo-bar-8"),
        )
        .expect("fixture profile");
        state.fixture_system.library.profiles.push(profile.clone());
        state
            .fixture_system
            .library
            .patches
            .push(crate::core::FixturePatch {
                id: 1,
                profile_id: profile.id,
                name: "Broken Patch".to_owned(),
                mode_name: "ghost-mode".to_owned(),
                universe: 0,
                address: 0,
                group_id: Some(FixtureGroupId(999)),
                enabled: true,
            });
        state.fixture_system.library.selected_patch = Some(1);

        let report = validate_state(&state);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "fixture.patch.mode.missing")
        );
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "fixture.patch.address.out_of_bounds")
        );

        let recovered = recover_state(&mut state, &report);

        assert!(recovered.valid);
        let patch = state
            .fixture_system
            .library
            .patches
            .first()
            .expect("patch survives");
        assert_eq!(patch.mode_name, "2ch");
        assert_eq!(patch.universe, 1);
        assert_eq!(patch.address, 1);
        assert_eq!(patch.group_id, None);
    }

    #[test]
    fn validation_detects_fixture_patch_universe_overlap() {
        let mut state = StudioState::default();
        state.fixture_system.library.profiles = vec![FixtureProfile {
            id: "fixture.overlap".to_owned(),
            manufacturer: "Test".to_owned(),
            model: "Overlap".to_owned(),
            short_name: "Overlap".to_owned(),
            categories: vec!["Spot".to_owned()],
            physical: None,
            channels: Vec::new(),
            modes: vec![FixtureMode {
                name: "4ch".to_owned(),
                short_name: None,
                channels: vec![
                    "Dimmer".to_owned(),
                    "Red".to_owned(),
                    "Green".to_owned(),
                    "Blue".to_owned(),
                ],
            }],
            source: FixtureSourceInfo {
                kind: FixtureSourceKind::Demo,
                manufacturer_key: None,
                fixture_key: None,
                source_path: None,
                ofl_url: None,
                creator_name: None,
                creator_version: None,
            },
        }];
        state.fixture_system.library.patches = vec![
            FixturePatch {
                id: 1,
                profile_id: "fixture.overlap".to_owned(),
                name: "A".to_owned(),
                mode_name: "4ch".to_owned(),
                universe: 1,
                address: 1,
                group_id: Some(FixtureGroupId(1)),
                enabled: true,
            },
            FixturePatch {
                id: 2,
                profile_id: "fixture.overlap".to_owned(),
                name: "B".to_owned(),
                mode_name: "4ch".to_owned(),
                universe: 1,
                address: 4,
                group_id: Some(FixtureGroupId(1)),
                enabled: true,
            },
        ];

        let report = validate_state(&state);

        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "fixture.patch.address.conflict")
        );
    }

    #[test]
    fn recovery_resets_missing_selected_engine_device() {
        let mut state = StudioState::default();
        state.settings.engine_link.enabled = true;
        state.settings.engine_link.selected_device = Some("ghost-prime".to_owned());
        state.settings.engine_link.adopt_transport = true;
        state.settings.engine_link.follow_mode = EngineDeckFollowMode::MasterDeck;
        state.settings.engine_link.telemetry = Some(EngineTelemetryFrame {
            device_id: "ghost-prime".to_owned(),
            decks: vec![crate::core::EngineDeckTelemetry {
                deck_index: 1,
                track_name: "Track".to_owned(),
                artist_name: "Artist".to_owned(),
                bpm: TempoBpm::from_whole_bpm(128),
                beat: BeatTime::from_beats(8),
                phase: EngineDeckPhase::Playing,
                is_master: true,
                is_synced: true,
            }],
            mixer: crate::core::EngineMixerTelemetry {
                crossfader: IntensityLevel::from_permille(500),
                channel_faders: vec![IntensityLevel::from_permille(1000)],
            },
            summary: "Ghost engine frame".to_owned(),
        });

        let report = validate_state(&state);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "settings.engine.device.selected.missing")
        );

        let recovered = recover_state(&mut state, &report);

        assert!(recovered.valid);
        assert!(state.settings.engine_link.selected_device.is_none());
        assert!(state.settings.engine_link.telemetry.is_none());
    }

    #[test]
    fn validation_detects_engine_telemetry_device_mismatch() {
        let mut state = StudioState::default();
        state.settings.engine_link.enabled = true;
        state.settings.engine_link.devices = vec![EnginePrimeDevice {
            id: "prime".to_owned(),
            name: "Denon Prime 2".to_owned(),
            address: "192.168.1.50".to_owned(),
            software_name: "Engine DJ".to_owned(),
            software_version: "4.1.0".to_owned(),
            announce_port: 51_337,
            service_port: Some(50_010),
            token_hint: None,
            services: Vec::new(),
            detail: "Prime".to_owned(),
            last_seen_frame: 0,
        }];
        state.settings.engine_link.selected_device = Some("prime".to_owned());
        state.settings.engine_link.telemetry = Some(EngineTelemetryFrame {
            device_id: "other".to_owned(),
            decks: vec![crate::core::EngineDeckTelemetry {
                deck_index: 1,
                track_name: "Track".to_owned(),
                artist_name: "Artist".to_owned(),
                bpm: TempoBpm::from_whole_bpm(128),
                beat: BeatTime::from_beats(8),
                phase: EngineDeckPhase::Playing,
                is_master: true,
                is_synced: true,
            }],
            mixer: crate::core::EngineMixerTelemetry {
                crossfader: IntensityLevel::from_permille(500),
                channel_faders: vec![IntensityLevel::from_permille(1000)],
            },
            summary: "Mismatch".to_owned(),
        });

        let report = validate_state(&state);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "settings.engine.telemetry.device.mismatch")
        );
    }

    #[test]
    fn recovery_resets_missing_selected_midi_input_and_learn_state() {
        let mut state = StudioState::default();
        state.settings.midi.selected_input = Some("ghost-midi".to_owned());
        state.settings.midi.detected_controller =
            Some(crate::core::ControllerProfileKind::Apc40Mk2);
        state.settings.midi.learn.phase = MidiLearnPhase::Listening;
        state.settings.midi.learn.target_binding = Some(42);

        let report = validate_state(&state);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "settings.midi.input.selected.missing")
        );

        let recovered = recover_state(&mut state, &report);

        assert!(recovered.valid);
        assert!(state.settings.midi.selected_input.is_none());
        assert!(state.settings.midi.detected_controller.is_none());
        assert_eq!(state.settings.midi.learn.phase, MidiLearnPhase::Idle);
        assert!(state.settings.midi.learn.target_binding.is_none());
    }

    #[test]
    fn recovery_deduplicates_midi_bindings() {
        let mut state = StudioState::default();
        let duplicate_message = MidiBindingMessage {
            kind: MidiMessageKind::ControlChange,
            channel: 1,
            key: 21,
        };
        state.settings.midi.bindings = vec![
            MidiBinding {
                id: 1,
                action: crate::core::MidiAction::MasterIntensity,
                label: "Master".to_owned(),
                message: Some(duplicate_message.clone()),
                hint: MidiControlHint::Continuous,
                learned: true,
                controller_profile: None,
            },
            MidiBinding {
                id: 2,
                action: crate::core::MidiAction::MasterSpeed,
                label: "Speed".to_owned(),
                message: Some(duplicate_message),
                hint: MidiControlHint::Continuous,
                learned: true,
                controller_profile: None,
            },
        ];

        let report = validate_state(&state);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "settings.midi.binding.duplicate")
        );

        let recovered = recover_state(&mut state, &report);

        assert!(recovered.valid);
        assert!(state.settings.midi.bindings[0].message.is_some());
        assert!(state.settings.midi.bindings[1].message.is_none());
        assert!(!state.settings.midi.bindings[1].learned);
    }
}
