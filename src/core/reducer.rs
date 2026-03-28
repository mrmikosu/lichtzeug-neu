use crate::core::editor::{
    add_clip_editor_automation_point, close_clip_editor, delete_clip_editor_automation_point,
    nudge_clip_editor_automation_point, open_clip_editor, select_clip_editor_automation_point,
    set_clip_editor_automation_mode, set_clip_editor_automation_target,
    set_clip_editor_automation_value, set_clip_editor_chase, set_clip_editor_cue,
    set_clip_editor_fx_depth, set_clip_editor_grid, set_clip_editor_intensity,
    set_clip_editor_speed, toggle_clip_editor_automation_lane,
};
use crate::core::engine::{
    advance_engine_frame, enter_sync_phase, resume_after_sync, toggle_transport,
};
use crate::core::event::{AppEvent, StateDiff, TimelineEvent, TimelineHit, TimelineZone};
use crate::core::history::{
    apply_redo, apply_undo, begin_history_transaction, capture_history_snapshot,
    clear_pending_history, commit_history_transaction, record_history_entry,
};
use crate::core::ids::{ClipId, TrackId};
use crate::core::project::{
    delete_venture, load_recovery_registry, load_venture, load_venture_registry, next_venture_name,
    rename_venture, restore_recovery_slot, save_recovery_slot, save_venture, save_venture_as,
};
use crate::core::queue::{
    complete_current_event, enqueue_event, mark_event_dispatched, start_next_event,
};
use crate::core::show::{
    add_selected_chase_step, advance_show_frame, arm_cue, create_chase, create_cue,
    delete_selected_chase, delete_selected_chase_step, delete_selected_cue,
    move_selected_chase_step, reverse_chase, select_chase, select_chase_step, select_cue,
    select_fixture_group, select_fx, set_fx_depth, set_fx_phase_offset, set_fx_rate, set_fx_spread,
    set_fx_waveform, set_selected_chase_direction, set_selected_chase_loop,
    set_selected_chase_name, set_selected_chase_step_color, set_selected_chase_step_cue,
    set_selected_chase_step_duration, set_selected_chase_step_label, set_selected_cue_color,
    set_selected_cue_fade_duration, set_selected_cue_name, toggle_chase, toggle_fx, trigger_cue,
};
use crate::core::state::{
    ClipInlineParameterKind, ClipboardClip, ContextMenuAction, ContextMenuTarget, CpuLoad,
    EngineErrorState, EnginePhase, HoverTarget, MIN_CLIP_DURATION, SelectionState, SnapGuide,
    SnapPhase, StateLifecycle, StudioState, TIMELINE_CLIP_HEIGHT_PX, TIMELINE_CLIP_TOP_INSET_PX,
    TIMELINE_HEADER_HEIGHT_PX, TIMELINE_TRACK_GAP_PX, TIMELINE_TRACK_HEIGHT_PX,
    TimelineInteraction, TimelinePhase, VenturePhase,
};
use crate::core::time::{BeatTime, IntensityLevel, PPQ, SpeedRatio, ZoomFactor};
use crate::core::validation::{recover_state, validate_state};

const DRAG_HYSTERESIS_PX: i32 = 6;
const SNAP_ACQUIRE_DISTANCE_PX: f32 = 10.0;
const SNAP_RELEASE_DISTANCE_PX: f32 = 18.0;

enum HistoryPostAction {
    None,
    RecordImmediate { label: String },
    CommitPending,
    ClearPending,
}

pub fn dispatch(state: &mut StudioState, event: AppEvent) {
    enqueue_event(&mut state.event_queue, event);
    drain_event_queue(state);
}

pub fn replay_events(events: &[AppEvent]) -> StudioState {
    let mut state = StudioState::default();

    for event in events {
        dispatch(&mut state, event.clone());
    }

    state
}

fn drain_event_queue(state: &mut StudioState) {
    while let Some(queued) = start_next_event(&mut state.event_queue) {
        state.lifecycle = StateLifecycle::Updating;
        let history_action = prepare_history_action(state, &queued.event);
        let history_before = match history_action {
            HistoryPostAction::RecordImmediate { .. } => Some(capture_history_snapshot(state)),
            HistoryPostAction::None
            | HistoryPostAction::CommitPending
            | HistoryPostAction::ClearPending => None,
        };
        let mut diffs = vec![StateDiff::StateLifecycle, StateDiff::EventQueue];
        diffs.extend(reduce_one(state, queued.event.clone()));

        mark_event_dispatched(&mut state.event_queue);

        let report = validate_state(state);
        if report.valid {
            if state.lifecycle == StateLifecycle::Recovered {
                state.lifecycle = StateLifecycle::Valid;
            } else {
                state.lifecycle = StateLifecycle::Valid;
            }
        } else {
            state.lifecycle = StateLifecycle::Invalid;
            diffs.push(StateDiff::Validation);
            let recovered = recover_state(state, &report);

            if recovered.valid {
                state.lifecycle = StateLifecycle::Recovered;
                state.status.hint = "State recovery applied".to_owned();
                state.lifecycle = StateLifecycle::Valid;
            } else {
                state.engine.phase = EnginePhase::Error;
                state.engine.error = Some(EngineErrorState {
                    code: "validation".to_owned(),
                    detail: report
                        .issues
                        .first()
                        .map(|issue| issue.detail.clone())
                        .unwrap_or_else(|| "Unspecified validation failure".to_owned()),
                });
            }
        }

        let mut history_recorded = false;
        match history_action {
            HistoryPostAction::None => {}
            HistoryPostAction::RecordImmediate { label } => {
                if let Some(before) = history_before {
                    let after = capture_history_snapshot(state);
                    if record_history_entry(
                        &mut state.history,
                        label,
                        queued.event.clone(),
                        before,
                        after,
                    ) {
                        history_recorded = true;
                        diffs.push(StateDiff::History);
                    }
                }
            }
            HistoryPostAction::CommitPending => {
                let after = capture_history_snapshot(state);
                if commit_history_transaction(&mut state.history, queued.event.clone(), after) {
                    history_recorded = true;
                    diffs.push(StateDiff::History);
                } else {
                    clear_pending_history(&mut state.history);
                }
            }
            HistoryPostAction::ClearPending => clear_pending_history(&mut state.history),
        }

        sync_venture_dirty_state(state, &mut diffs);
        maybe_enqueue_recovery_autosave(state, &queued.event, history_recorded);
        state.status.last_diffs = diffs.clone();
        apply_revisions(&mut state.revisions, &diffs);
        if should_record_replay_event(&queued.event) {
            append_replay_event(
                &mut state.replay_log.events,
                state.replay_log.capacity,
                &queued.event,
            );
        }
        complete_current_event(&mut state.event_queue);
    }
}

fn reduce_one(state: &mut StudioState, event: AppEvent) -> Vec<StateDiff> {
    match event {
        AppEvent::Tick => {
            let mut diffs = advance_engine_frame(state);
            diffs.extend(advance_snap_feedback(state));
            if state.timeline.phase == TimelinePhase::Rendering
                && state.timeline.interaction == TimelineInteraction::Idle
            {
                state.timeline.phase = TimelinePhase::Idle;
                diffs.push(StateDiff::TimelinePhase);
            }
            state.status.hint = "Engine frame advanced".to_owned();
            diffs
        }
        AppEvent::Undo => {
            let label = state.undo_label().unwrap_or("Aenderung").to_owned();
            clear_pending_history(&mut state.history);
            let diffs = apply_undo(state);
            state.status.hint = if diffs.is_empty() {
                "Undo nicht verfuegbar".to_owned()
            } else {
                format!("Undo {}", label)
            };
            diffs
        }
        AppEvent::Redo => {
            let label = state.redo_label().unwrap_or("Aenderung").to_owned();
            clear_pending_history(&mut state.history);
            let diffs = apply_redo(state);
            state.status.hint = if diffs.is_empty() {
                "Redo nicht verfuegbar".to_owned()
            } else {
                format!("Redo {}", label)
            };
            diffs
        }
        AppEvent::RefreshVentures => refresh_venture_registry(state),
        AppEvent::SelectVenture(venture_id) => select_venture(state, &venture_id),
        AppEvent::SelectRecoverySlot(slot_id) => select_recovery_slot(state, &slot_id),
        AppEvent::SetVentureDraftName(name) => {
            let draft_name = name.trim_start().to_owned();
            let changed = state.venture.draft_name != draft_name;
            state.venture.draft_name = draft_name;
            state.venture.last_error = None;
            if changed {
                state.status.hint = format!("Venture-Name {}", state.venture.draft_name);
                vec![StateDiff::Venture]
            } else {
                Vec::new()
            }
        }
        AppEvent::SaveCurrentVenture => save_current_venture_to_disk(state),
        AppEvent::SaveCurrentVentureAs => save_current_venture_as_to_disk(state),
        AppEvent::RenameSelectedVenture => rename_selected_venture_on_disk(state),
        AppEvent::LoadSelectedVenture => load_selected_venture_from_disk(state),
        AppEvent::DeleteSelectedVenture => delete_selected_venture_from_disk(state),
        AppEvent::RestoreSelectedRecoverySlot => restore_selected_recovery_slot_from_disk(state),
        AppEvent::AutosaveRecoverySlot(label) => autosave_recovery_slot_to_disk(state, &label),
        AppEvent::CreateNewVenture => create_new_venture(state),
        AppEvent::DuplicateSelectedClips => {
            let diffs = duplicate_selected_clips(state);
            state.status.hint = if diffs.is_empty() {
                "Duplicate nicht moeglich".to_owned()
            } else {
                format!("{} Clip(s) dupliziert", state.selected_clip_count())
            };
            diffs
        }
        AppEvent::SplitSelectedClipsAtPlayhead => {
            let diffs = split_selected_clips_at_playhead(state);
            state.status.hint = if diffs.is_empty() {
                "Split am Playhead nicht moeglich".to_owned()
            } else {
                format!("Split bei {}", state.engine.transport.position_label())
            };
            diffs
        }
        AppEvent::DeleteSelectedClips => {
            let deleted_count = state.selected_clip_count();
            let diffs = delete_selected_clips(state);
            state.status.hint = if diffs.is_empty() {
                "Keine Clips geloescht".to_owned()
            } else {
                format!("{} Clip(s) geloescht", deleted_count)
            };
            diffs
        }
        AppEvent::CopySelectedClips => {
            let diffs = copy_selected_clips(state, false);
            state.status.hint = if diffs.is_empty() {
                "Keine Clips kopiert".to_owned()
            } else {
                format!("{} Clip(s) kopiert", state.clipboard.clips.len())
            };
            diffs
        }
        AppEvent::CutSelectedClips => {
            let count = state.selected_clip_count();
            let diffs = cut_selected_clips(state);
            state.status.hint = if diffs.is_empty() {
                "Cut nicht moeglich".to_owned()
            } else {
                format!("{} Clip(s) ausgeschnitten", count)
            };
            diffs
        }
        AppEvent::PasteClipboardAtPlayhead => {
            let diffs = paste_clipboard_at_playhead(state);
            state.status.hint = if diffs.is_empty() {
                "Paste nicht moeglich".to_owned()
            } else {
                format!(
                    "{} Clip(s) bei {} eingefuegt",
                    state.selected_clip_count(),
                    state.engine.transport.position_label()
                )
            };
            diffs
        }
        AppEvent::NudgeSelectedClipsLeft => {
            let diffs = nudge_selected_clips(state, -1);
            state.status.hint = if diffs.is_empty() {
                "Nudge links nicht moeglich".to_owned()
            } else {
                "Clips nach links verschoben".to_owned()
            };
            diffs
        }
        AppEvent::NudgeSelectedClipsRight => {
            let diffs = nudge_selected_clips(state, 1);
            state.status.hint = if diffs.is_empty() {
                "Nudge rechts nicht moeglich".to_owned()
            } else {
                "Clips nach rechts verschoben".to_owned()
            };
            diffs
        }
        AppEvent::ToggleTransport => {
            let diff = toggle_transport(&mut state.engine);
            state.status.hint = if state.engine.is_running() {
                "Transport gestartet".to_owned()
            } else {
                "Transport pausiert".to_owned()
            };
            vec![diff]
        }
        AppEvent::SetMasterIntensity(value) => {
            state.master.intensity = IntensityLevel::from_permille(value);
            state.status.hint = format!(
                "Master Intensity {:.0}%",
                state.master.intensity.as_f32() * 100.0
            );
            vec![StateDiff::Master]
        }
        AppEvent::SetMasterSpeed(value) => {
            state.master.speed = SpeedRatio::from_permille(value);
            state.status.hint = format!("Master Speed {:.0}%", state.master.speed.as_f32() * 100.0);
            vec![StateDiff::Master]
        }
        AppEvent::SetTimelineZoom(value) => {
            state.timeline.viewport.zoom = ZoomFactor::from_permille(value);
            state.timeline.phase = TimelinePhase::Zooming;
            state.status.hint = format!(
                "Timeline Zoom {:.0}%",
                state.timeline.viewport.zoom.as_f32() * 100.0
            );
            vec![StateDiff::TimelineViewport, StateDiff::TimelinePhase]
        }
        AppEvent::SetInputModifiers(modifiers) => {
            let changed = state.input_modifiers != modifiers;
            state.input_modifiers = modifiers;
            if changed {
                vec![StateDiff::Input]
            } else {
                Vec::new()
            }
        }
        AppEvent::CloseContextMenu => {
            state.status.hint = "Kontextmenue geschlossen".to_owned();
            close_context_menu_state(state)
        }
        AppEvent::ApplyContextMenuAction(action) => {
            state.status.hint = format!("Kontextaktion {}", context_action_label(action));
            let mut diffs = apply_context_menu_action(state, action);
            diffs.extend(close_context_menu_state(state));
            diffs
        }
        AppEvent::ToggleTrackMute(track_id) => toggle_track_mute(state, track_id),
        AppEvent::ToggleTrackSolo(track_id) => toggle_track_solo(state, track_id),
        AppEvent::SelectCue(cue_id) => {
            state.status.hint = format!("Cue {} selektiert", cue_id.0);
            select_cue(state, cue_id)
        }
        AppEvent::CreateCue => {
            state.status.hint = "Cue erstellt".to_owned();
            create_cue(state)
        }
        AppEvent::DeleteSelectedCue => {
            state.status.hint = "Selektierten Cue geloescht".to_owned();
            delete_selected_cue(state)
        }
        AppEvent::SetSelectedCueName(name) => {
            state.status.hint = format!("Cue Name {}", name);
            set_selected_cue_name(state, name)
        }
        AppEvent::SetSelectedCueColor(color) => {
            state.status.hint = format!("Cue Farbe rgb({}, {}, {})", color.r, color.g, color.b);
            set_selected_cue_color(state, color)
        }
        AppEvent::SetSelectedCueFadeDuration(duration) => {
            state.status.hint = format!("Cue Fade {:.2} Beats", duration.as_beats_f32());
            set_selected_cue_fade_duration(state, duration)
        }
        AppEvent::ArmCue(cue_id) => {
            state.status.hint = format!("Cue {} armed", cue_id.0);
            arm_cue(state, cue_id)
        }
        AppEvent::TriggerCue(cue_id) => {
            state.status.hint = format!("Cue {} triggered", cue_id.0);
            trigger_cue(state, cue_id)
        }
        AppEvent::SelectChase(chase_id) => {
            state.status.hint = format!("Chase {} selektiert", chase_id.0);
            select_chase(state, chase_id)
        }
        AppEvent::CreateChase => {
            state.status.hint = "Chase erstellt".to_owned();
            create_chase(state)
        }
        AppEvent::DeleteSelectedChase => {
            state.status.hint = "Selektierten Chase geloescht".to_owned();
            delete_selected_chase(state)
        }
        AppEvent::SetSelectedChaseName(name) => {
            state.status.hint = format!("Chase Name {}", name);
            set_selected_chase_name(state, name)
        }
        AppEvent::SetSelectedChaseDirection(direction) => {
            state.status.hint = format!("Chase Richtung {:?}", direction);
            set_selected_chase_direction(state, direction)
        }
        AppEvent::SetSelectedChaseLoop(loop_enabled) => {
            state.status.hint = if loop_enabled {
                "Chase Loop aktiviert".to_owned()
            } else {
                "Chase Loop deaktiviert".to_owned()
            };
            set_selected_chase_loop(state, loop_enabled)
        }
        AppEvent::SelectChaseStep(index) => {
            state.status.hint = index
                .map(|index| format!("Chase Step {} selektiert", index + 1))
                .unwrap_or_else(|| "Chase Step Auswahl geloescht".to_owned());
            select_chase_step(state, index)
        }
        AppEvent::AddSelectedChaseStep => {
            state.status.hint = "Chase Step hinzugefuegt".to_owned();
            add_selected_chase_step(state)
        }
        AppEvent::DeleteSelectedChaseStep => {
            state.status.hint = "Chase Step geloescht".to_owned();
            delete_selected_chase_step(state)
        }
        AppEvent::MoveSelectedChaseStepLeft => {
            state.status.hint = "Chase Step nach links".to_owned();
            move_selected_chase_step(state, -1)
        }
        AppEvent::MoveSelectedChaseStepRight => {
            state.status.hint = "Chase Step nach rechts".to_owned();
            move_selected_chase_step(state, 1)
        }
        AppEvent::SetSelectedChaseStepLabel(label) => {
            state.status.hint = format!("Chase Step Label {}", label);
            set_selected_chase_step_label(state, label)
        }
        AppEvent::SetSelectedChaseStepCue(cue_id) => {
            state.status.hint = cue_id
                .map(|cue_id| format!("Chase Step Cue {}", cue_id.0))
                .unwrap_or_else(|| "Chase Step Cue entfernt".to_owned());
            set_selected_chase_step_cue(state, cue_id)
        }
        AppEvent::SetSelectedChaseStepDuration(duration) => {
            state.status.hint = format!("Chase Step {:.2} Beats", duration.as_beats_f32());
            set_selected_chase_step_duration(state, duration)
        }
        AppEvent::SetSelectedChaseStepColor(color) => {
            state.status.hint = format!(
                "Chase Step Farbe rgb({}, {}, {})",
                color.r, color.g, color.b
            );
            set_selected_chase_step_color(state, color)
        }
        AppEvent::ToggleChase(chase_id) => {
            state.status.hint = format!("Chase {} toggled", chase_id.0);
            toggle_chase(state, chase_id)
        }
        AppEvent::ReverseChase(chase_id) => {
            state.status.hint = format!("Chase {} reversed", chase_id.0);
            reverse_chase(state, chase_id)
        }
        AppEvent::SelectFx(fx_id) => {
            state.status.hint = format!("FX {} selektiert", fx_id.0);
            select_fx(state, fx_id)
        }
        AppEvent::ToggleFx(fx_id) => {
            state.status.hint = format!("FX {} umgeschaltet", fx_id.0);
            toggle_fx(state, fx_id)
        }
        AppEvent::SetFxDepth(fx_id, depth) => {
            state.status.hint = format!("FX {} depth {}%", fx_id.0, depth / 10);
            set_fx_depth(state, fx_id, depth)
        }
        AppEvent::SetFxRate(fx_id, rate) => {
            state.status.hint = format!("FX {} rate {}%", fx_id.0, rate / 10);
            set_fx_rate(state, fx_id, rate)
        }
        AppEvent::SetFxSpread(fx_id, spread) => {
            state.status.hint = format!("FX {} spread {}%", fx_id.0, spread / 10);
            set_fx_spread(state, fx_id, spread)
        }
        AppEvent::SetFxPhaseOffset(fx_id, phase_offset) => {
            state.status.hint = format!("FX {} phase {}%", fx_id.0, phase_offset / 10);
            set_fx_phase_offset(state, fx_id, phase_offset)
        }
        AppEvent::SetFxWaveform(fx_id, waveform) => {
            state.status.hint = format!("FX {} waveform {}", fx_id.0, waveform);
            set_fx_waveform(state, fx_id, waveform)
        }
        AppEvent::SelectFixtureGroup(group_id) => {
            state.status.hint = format!("Fixture {} selektiert", group_id.0);
            select_fixture_group(state, group_id)
        }
        AppEvent::OpenClipEditor(clip_id) => {
            state.status.hint = format!("Clip {} editor geöffnet", clip_id.0);
            open_clip_editor(state, clip_id)
        }
        AppEvent::CloseClipEditor => {
            state.status.hint = "Clip editor geschlossen".to_owned();
            close_clip_editor(state)
        }
        AppEvent::SetClipEditorIntensity(value) => {
            state.status.hint = format!("Clip intensity {:.0}%", value as f32 / 10.0);
            let mut diffs = set_clip_editor_intensity(state, value);
            diffs.extend(advance_show_frame(state, BeatTime::ZERO));
            diffs
        }
        AppEvent::SetClipEditorSpeed(value) => {
            state.status.hint = format!("Clip speed {:.0}%", value as f32 / 10.0);
            let mut diffs = set_clip_editor_speed(state, value);
            diffs.extend(advance_show_frame(state, BeatTime::ZERO));
            diffs
        }
        AppEvent::SetClipEditorFxDepth(value) => {
            state.status.hint = format!("Clip FX depth {:.0}%", value as f32 / 10.0);
            let mut diffs = set_clip_editor_fx_depth(state, value);
            diffs.extend(advance_show_frame(state, BeatTime::ZERO));
            diffs
        }
        AppEvent::SetClipEditorCue(cue_id) => {
            state.status.hint = cue_id
                .map(|cue_id| format!("Clip Cue {} gesetzt", cue_id.0))
                .unwrap_or_else(|| "Clip Cue entfernt".to_owned());
            set_clip_editor_cue(state, cue_id)
        }
        AppEvent::SetClipEditorChase(chase_id) => {
            state.status.hint = chase_id
                .map(|chase_id| format!("Clip Chase {} gesetzt", chase_id.0))
                .unwrap_or_else(|| "Clip Chase entfernt".to_owned());
            set_clip_editor_chase(state, chase_id)
        }
        AppEvent::SetClipEditorGrid(grid) => {
            state.status.hint = format!("Clip grid {:?}", grid);
            set_clip_editor_grid(state, grid)
        }
        AppEvent::SetClipEditorAutomationTarget(target) => {
            state.status.hint = format!("Automation Lane {}", target);
            set_clip_editor_automation_target(state, target)
        }
        AppEvent::SetClipEditorAutomationMode(mode) => {
            state.status.hint = format!("Automation Mode {}", mode);
            let mut diffs = set_clip_editor_automation_mode(state, mode);
            diffs.extend(advance_show_frame(state, BeatTime::ZERO));
            diffs
        }
        AppEvent::ToggleClipEditorAutomationLane => {
            let mut diffs = toggle_clip_editor_automation_lane(state);
            state.status.hint = if let Some(clip) = state.editor_clip() {
                let enabled = clip
                    .automation
                    .iter()
                    .find(|lane| lane.target == state.clip_editor.automation_target)
                    .map(|lane| lane.enabled)
                    .unwrap_or(false);
                if enabled {
                    "Automation aktiviert".to_owned()
                } else {
                    "Automation deaktiviert".to_owned()
                }
            } else {
                "Automation umgeschaltet".to_owned()
            };
            diffs.extend(advance_show_frame(state, BeatTime::ZERO));
            diffs
        }
        AppEvent::AddClipEditorAutomationPointAtPlayhead => {
            state.status.hint = "Automation-Punkt hinzugefuegt".to_owned();
            let mut diffs = add_clip_editor_automation_point(state);
            diffs.extend(advance_show_frame(state, BeatTime::ZERO));
            diffs
        }
        AppEvent::SelectClipEditorAutomationPoint(index) => {
            state.status.hint = index
                .map(|point| format!("Automation-Punkt {} selektiert", point + 1))
                .unwrap_or_else(|| "Automation-Punkt abgewählt".to_owned());
            select_clip_editor_automation_point(state, index)
        }
        AppEvent::SetClipEditorAutomationPointValue(value) => {
            state.status.hint = format!("Automation-Wert {}%", value / 10);
            let mut diffs = set_clip_editor_automation_value(state, value);
            diffs.extend(advance_show_frame(state, BeatTime::ZERO));
            diffs
        }
        AppEvent::NudgeClipEditorAutomationPointLeft => {
            state.status.hint = "Automation-Punkt nach links verschoben".to_owned();
            let mut diffs = nudge_clip_editor_automation_point(state, -1);
            diffs.extend(advance_show_frame(state, BeatTime::ZERO));
            diffs
        }
        AppEvent::NudgeClipEditorAutomationPointRight => {
            state.status.hint = "Automation-Punkt nach rechts verschoben".to_owned();
            let mut diffs = nudge_clip_editor_automation_point(state, 1);
            diffs.extend(advance_show_frame(state, BeatTime::ZERO));
            diffs
        }
        AppEvent::DeleteClipEditorAutomationPoint => {
            state.status.hint = "Automation-Punkt geloescht".to_owned();
            let mut diffs = delete_clip_editor_automation_point(state);
            diffs.extend(advance_show_frame(state, BeatTime::ZERO));
            diffs
        }
        AppEvent::Timeline(event) => handle_timeline_event(state, event),
    }
}

fn advance_snap_feedback(state: &mut StudioState) -> Vec<StateDiff> {
    let mut diffs = Vec::new();

    if let Some(guide) = &mut state.timeline.snap.guide {
        guide.strength_permille = guide.strength_permille.saturating_sub(28);
        if guide.strength_permille == 0 {
            state.timeline.snap.guide = None;
            state.timeline.snap.phase = SnapPhase::Free;
        }
        diffs.push(StateDiff::SnapGuide);
    }

    state.performance.cpu_load = CpuLoad(
        (12 + (state.event_queue.queue.len() as u16 * 2) + state.performance.frame_budget_ms / 4)
            .min(100),
    );

    diffs
}

fn refresh_venture_registry(state: &mut StudioState) -> Vec<StateDiff> {
    state.venture.phase = VenturePhase::Loading;

    match load_venture_registry(&state.venture.directory) {
        Ok(registry) => {
            apply_venture_registry(state, registry);
            apply_recovery_registry(
                state,
                load_recovery_registry(&state.venture.directory).unwrap_or_default(),
            );
            state.venture.phase = VenturePhase::Idle;
            state.venture.last_error = None;
            state.status.hint = if state.venture.registry_issues.is_empty() {
                format!("{} Venture(s) verfuegbar", state.venture.ventures.len())
            } else {
                format!(
                    "{} Venture(s), {} Warnung(en)",
                    state.venture.ventures.len(),
                    state.venture.registry_issues.len()
                )
            };
            vec![StateDiff::Venture]
        }
        Err(error) => {
            state.venture.phase = VenturePhase::Error;
            state.venture.last_error = Some(error.clone());
            state.venture.registry_issues.clear();
            state.venture.recovery_issues.clear();
            state.status.hint = "Venture-Liste konnte nicht geladen werden".to_owned();
            vec![StateDiff::Venture]
        }
    }
}

fn select_venture(state: &mut StudioState, venture_id: &str) -> Vec<StateDiff> {
    let Some(venture) = state
        .venture
        .ventures
        .iter()
        .find(|venture| venture.id == venture_id)
        .cloned()
    else {
        state.venture.phase = VenturePhase::Error;
        state.venture.last_error = Some(format!("Venture {} existiert nicht.", venture_id));
        state.status.hint = "Venture-Auswahl fehlgeschlagen".to_owned();
        return vec![StateDiff::Venture];
    };

    state.venture.selected = Some(venture.id.clone());
    state.venture.draft_name = venture.name.clone();
    state.venture.last_error = None;
    state.venture.phase = VenturePhase::Idle;
    state.status.hint = format!("Venture {} ausgewählt", venture.name);
    vec![StateDiff::Venture]
}

fn select_recovery_slot(state: &mut StudioState, slot_id: &str) -> Vec<StateDiff> {
    let Some(slot) = state
        .venture
        .recovery_slots
        .iter()
        .find(|slot| slot.id == slot_id)
        .cloned()
    else {
        state.venture.phase = VenturePhase::Error;
        state.venture.last_error = Some(format!("Recovery-Slot {} existiert nicht.", slot_id));
        state.status.hint = "Recovery-Auswahl fehlgeschlagen".to_owned();
        return vec![StateDiff::Venture];
    };

    state.venture.selected_recovery = Some(slot.id.clone());
    state.venture.last_error = None;
    state.venture.phase = VenturePhase::Idle;
    state.status.hint = format!("Recovery {} ausgewählt", slot.label);
    vec![StateDiff::Venture]
}

fn save_current_venture_to_disk(state: &mut StudioState) -> Vec<StateDiff> {
    if !state.can_save_venture() {
        state.venture.phase = VenturePhase::Error;
        state.venture.last_error = Some("Venture-Name fehlt.".to_owned());
        state.status.hint = "Venture konnte nicht gespeichert werden".to_owned();
        return vec![StateDiff::Venture];
    }

    state.venture.phase = VenturePhase::Saving;

    match save_venture(
        state,
        &state.venture.directory,
        state.venture.selected.as_deref(),
        &state.venture.draft_name,
    ) {
        Ok(saved) => {
            if let Ok(registry) = load_venture_registry(&state.venture.directory) {
                apply_venture_registry(state, registry);
            } else {
                state.venture.ventures = vec![saved.clone()];
                state.venture.registry_issues.clear();
            }
            if let Ok(registry) = load_recovery_registry(&state.venture.directory) {
                apply_recovery_registry(state, registry);
            }
            state.venture.selected = Some(saved.id.clone());
            state.venture.last_saved = Some(saved.id.clone());
            state.venture.draft_name = saved.name.clone();
            state.venture.last_error = None;
            state.venture.phase = VenturePhase::Idle;
            mark_venture_saved_baseline(state);
            state.status.hint = format!("Venture {} gespeichert", saved.name);
            vec![StateDiff::Venture]
        }
        Err(error) => {
            state.venture.phase = VenturePhase::Error;
            state.venture.last_error = Some(error.clone());
            state.status.hint = "Venture konnte nicht gespeichert werden".to_owned();
            vec![StateDiff::Venture]
        }
    }
}

fn save_current_venture_as_to_disk(state: &mut StudioState) -> Vec<StateDiff> {
    if !state.can_save_venture_as() {
        state.venture.phase = VenturePhase::Error;
        state.venture.last_error = Some("Venture-Name fehlt.".to_owned());
        state.status.hint = "Venture Copy konnte nicht gespeichert werden".to_owned();
        return vec![StateDiff::Venture];
    }

    state.venture.phase = VenturePhase::Saving;

    match save_venture_as(state, &state.venture.directory, &state.venture.draft_name) {
        Ok(saved) => {
            if let Ok(registry) = load_venture_registry(&state.venture.directory) {
                apply_venture_registry(state, registry);
            } else {
                state.venture.ventures = vec![saved.clone()];
                state.venture.registry_issues.clear();
            }
            if let Ok(registry) = load_recovery_registry(&state.venture.directory) {
                apply_recovery_registry(state, registry);
            }
            state.venture.selected = Some(saved.id.clone());
            state.venture.last_saved = Some(saved.id.clone());
            state.venture.draft_name = saved.name.clone();
            state.venture.last_error = None;
            state.venture.phase = VenturePhase::Idle;
            mark_venture_saved_baseline(state);
            state.status.hint = format!("Venture {} als neue Kopie gespeichert", saved.name);
            vec![StateDiff::Venture]
        }
        Err(error) => {
            state.venture.phase = VenturePhase::Error;
            state.venture.last_error = Some(error.clone());
            state.status.hint = "Venture Copy konnte nicht gespeichert werden".to_owned();
            vec![StateDiff::Venture]
        }
    }
}

fn rename_selected_venture_on_disk(state: &mut StudioState) -> Vec<StateDiff> {
    let Some(selected_id) = state.venture.selected.clone() else {
        state.venture.phase = VenturePhase::Error;
        state.venture.last_error = Some("Kein Venture zum Umbenennen ausgewählt.".to_owned());
        state.status.hint = "Kein Venture zum Umbenennen ausgewählt".to_owned();
        return vec![StateDiff::Venture];
    };
    if !state.can_rename_selected_venture() {
        state.venture.phase = VenturePhase::Error;
        state.venture.last_error = Some("Venture-Name fehlt.".to_owned());
        state.status.hint = "Venture konnte nicht umbenannt werden".to_owned();
        return vec![StateDiff::Venture];
    }

    state.venture.phase = VenturePhase::Saving;

    match rename_venture(
        state,
        &state.venture.directory,
        &selected_id,
        &state.venture.draft_name,
    ) {
        Ok(saved) => {
            if let Ok(registry) = load_venture_registry(&state.venture.directory) {
                apply_venture_registry(state, registry);
            } else {
                state.venture.ventures = vec![saved.clone()];
                state.venture.registry_issues.clear();
            }
            if let Ok(registry) = load_recovery_registry(&state.venture.directory) {
                apply_recovery_registry(state, registry);
            }
            state.venture.selected = Some(saved.id.clone());
            state.venture.last_saved = Some(saved.id.clone());
            state.venture.draft_name = saved.name.clone();
            state.venture.last_error = None;
            state.venture.phase = VenturePhase::Idle;
            mark_venture_saved_baseline(state);
            state.status.hint = format!("Venture {} umbenannt", saved.name);
            vec![StateDiff::Venture]
        }
        Err(error) => {
            state.venture.phase = VenturePhase::Error;
            state.venture.last_error = Some(error.clone());
            state.status.hint = "Venture konnte nicht umbenannt werden".to_owned();
            vec![StateDiff::Venture]
        }
    }
}

fn load_selected_venture_from_disk(state: &mut StudioState) -> Vec<StateDiff> {
    let Some(selected_id) = state.venture.selected.clone() else {
        state.venture.phase = VenturePhase::Error;
        state.venture.last_error = Some("Kein Venture ausgewählt.".to_owned());
        state.status.hint = "Kein Venture zum Laden ausgewählt".to_owned();
        return vec![StateDiff::Venture];
    };

    state.venture.phase = VenturePhase::Loading;

    match load_venture(&state.venture.directory, &selected_id) {
        Ok((loaded_state, descriptor)) => {
            let registry = load_venture_registry(&state.venture.directory).ok();
            let recovery_registry = load_recovery_registry(&state.venture.directory).ok();
            replace_project_state(state, loaded_state);
            if let Some(registry) = registry {
                apply_venture_registry(state, registry);
            } else {
                state.venture.ventures = vec![descriptor.clone()];
                state.venture.registry_issues.clear();
            }
            if let Some(registry) = recovery_registry {
                apply_recovery_registry(state, registry);
            }
            state.venture.selected = Some(descriptor.id.clone());
            state.venture.last_saved = Some(descriptor.id.clone());
            state.venture.draft_name = descriptor.name.clone();
            state.venture.last_error = None;
            state.venture.phase = VenturePhase::Idle;
            mark_venture_saved_baseline(state);
            state.status.hint = format!("Venture {} geladen", descriptor.name);
            vec![StateDiff::History, StateDiff::ReplayLog, StateDiff::Venture]
        }
        Err(error) => {
            state.venture.phase = VenturePhase::Error;
            state.venture.last_error = Some(error.clone());
            state.status.hint = "Venture konnte nicht geladen werden".to_owned();
            vec![StateDiff::Venture]
        }
    }
}

fn delete_selected_venture_from_disk(state: &mut StudioState) -> Vec<StateDiff> {
    let Some(selected_id) = state.venture.selected.clone() else {
        state.venture.phase = VenturePhase::Error;
        state.venture.last_error = Some("Kein Venture zum Löschen ausgewählt.".to_owned());
        state.status.hint = "Kein Venture zum Löschen ausgewählt".to_owned();
        return vec![StateDiff::Venture];
    };

    state.venture.phase = VenturePhase::Loading;

    match delete_venture(&state.venture.directory, &selected_id) {
        Ok(()) => {
            if let Ok(registry) = load_venture_registry(&state.venture.directory) {
                apply_venture_registry(state, registry);
            } else {
                state.venture.ventures.clear();
                state.venture.registry_issues.clear();
            }
            if let Ok(registry) = load_recovery_registry(&state.venture.directory) {
                apply_recovery_registry(state, registry);
            } else {
                state.venture.recovery_slots.clear();
                state.venture.recovery_issues.clear();
            }
            state.venture.selected = None;
            state.venture.last_saved = None;
            if state.venture.draft_name.trim().is_empty() {
                state.venture.draft_name = next_venture_name(&state.venture.ventures);
            }
            state.venture.last_error = None;
            state.venture.phase = VenturePhase::Idle;
            state.status.hint =
                "Venture-Datei gelöscht, aktueller State bleibt als Draft offen".to_owned();
            vec![StateDiff::Venture]
        }
        Err(error) => {
            state.venture.phase = VenturePhase::Error;
            state.venture.last_error = Some(error.clone());
            state.status.hint = "Venture konnte nicht gelöscht werden".to_owned();
            vec![StateDiff::Venture]
        }
    }
}

fn restore_selected_recovery_slot_from_disk(state: &mut StudioState) -> Vec<StateDiff> {
    let Some(selected_id) = state.venture.selected_recovery.clone() else {
        state.venture.phase = VenturePhase::Error;
        state.venture.last_error = Some("Kein Recovery-Slot ausgewaehlt.".to_owned());
        state.status.hint = "Kein Recovery-Slot zum Laden ausgewaehlt".to_owned();
        return vec![StateDiff::Venture];
    };

    state.venture.phase = VenturePhase::Loading;

    match restore_recovery_slot(&state.venture.directory, &selected_id) {
        Ok((loaded_state, descriptor)) => {
            let venture_registry = load_venture_registry(&state.venture.directory).ok();
            let recovery_registry = load_recovery_registry(&state.venture.directory).ok();
            replace_project_state(state, loaded_state);
            if let Some(registry) = venture_registry {
                apply_venture_registry(state, registry);
            }
            if let Some(registry) = recovery_registry {
                apply_recovery_registry(state, registry);
            }
            state.venture.selected_recovery = Some(descriptor.id.clone());
            state.venture.last_autosave = Some(descriptor.id.clone());
            state.venture.last_error = None;
            state.venture.phase = VenturePhase::Idle;
            mark_venture_saved_baseline(state);
            state.status.hint = format!("Recovery {} wiederhergestellt", descriptor.label);
            vec![StateDiff::History, StateDiff::ReplayLog, StateDiff::Venture]
        }
        Err(error) => {
            state.venture.phase = VenturePhase::Error;
            state.venture.last_error = Some(error.clone());
            state.status.hint = "Recovery-Slot konnte nicht geladen werden".to_owned();
            vec![StateDiff::Venture]
        }
    }
}

fn autosave_recovery_slot_to_disk(state: &mut StudioState, label: &str) -> Vec<StateDiff> {
    if !state.venture.autosave_enabled {
        return Vec::new();
    }

    match save_recovery_slot(
        state,
        &state.venture.directory,
        label,
        state.venture.recovery_capacity,
    ) {
        Ok(saved) => {
            if let Ok(registry) = load_recovery_registry(&state.venture.directory) {
                apply_recovery_registry(state, registry);
            }
            state.venture.selected_recovery = Some(saved.id.clone());
            state.venture.last_autosave = Some(saved.id.clone());
            state.venture.last_error = None;
            state.venture.phase = VenturePhase::Idle;
            state.status.hint = format!("Autosave {}", saved.label);
            vec![StateDiff::Venture]
        }
        Err(error) => {
            state.venture.phase = VenturePhase::Error;
            state.venture.last_error = Some(error.clone());
            state.status.hint = "Autosave konnte nicht geschrieben werden".to_owned();
            vec![StateDiff::Venture]
        }
    }
}

fn create_new_venture(state: &mut StudioState) -> Vec<StateDiff> {
    let next_name = next_venture_name(&state.venture.ventures);
    let fresh = StudioState::default();
    let mut venture_state = state.venture.clone();
    venture_state.draft_name = next_name.clone();
    venture_state.selected = None;
    venture_state.selected_recovery = None;
    venture_state.last_saved = None;
    venture_state.last_autosave = None;
    venture_state.last_error = None;
    venture_state.phase = VenturePhase::Idle;
    replace_project_state(state, fresh);
    state.venture = venture_state;
    mark_venture_saved_baseline(state);
    state.status.hint = format!("Neues Venture {} bereit", next_name);
    vec![StateDiff::History, StateDiff::ReplayLog, StateDiff::Venture]
}

fn apply_venture_registry(state: &mut StudioState, registry: crate::core::VentureRegistry) {
    let previous_selection = state.venture.selected.clone();
    state.venture.ventures = registry.ventures;
    state.venture.registry_issues = registry.issues;
    state.venture.selected = previous_selection.filter(|selected| {
        state
            .venture
            .ventures
            .iter()
            .any(|venture| venture.id == *selected)
    });

    if let Some(selected) = state.selected_venture().cloned() {
        state.venture.last_saved = Some(selected.id.clone());
    } else if state.venture.selected.is_none() {
        state.venture.last_saved = None;
    }

    if state.venture.draft_name.trim().is_empty() {
        state.venture.draft_name = next_venture_name(&state.venture.ventures);
    }
}

fn apply_recovery_registry(state: &mut StudioState, registry: crate::core::RecoveryRegistry) {
    let previous_selection = state.venture.selected_recovery.clone();
    state.venture.recovery_slots = registry.slots;
    state.venture.recovery_issues = registry.issues;
    state.venture.selected_recovery = previous_selection.filter(|selected| {
        state
            .venture
            .recovery_slots
            .iter()
            .any(|slot| slot.id == *selected)
    });

    if state.venture.selected_recovery.is_none() {
        state.venture.selected_recovery = state
            .venture
            .recovery_slots
            .last()
            .map(|slot| slot.id.clone());
    }
    if let Some(selected) = state.selected_recovery_slot().cloned() {
        state.venture.last_autosave = Some(selected.id);
    } else if state.venture.recovery_slots.is_empty() {
        state.venture.last_autosave = None;
    }
}

fn mark_venture_saved_baseline(state: &mut StudioState) {
    state.venture.saved_fingerprint = state.authoring_fingerprint();
    state.venture.dirty = false;
}

fn sync_venture_dirty_state(state: &mut StudioState, diffs: &mut Vec<StateDiff>) {
    let fingerprint = state.authoring_fingerprint();
    let dirty = fingerprint != state.venture.saved_fingerprint;
    if state.venture.dirty != dirty {
        state.venture.dirty = dirty;
        diffs.push(StateDiff::Venture);
    }
}

fn maybe_enqueue_recovery_autosave(
    state: &mut StudioState,
    event: &AppEvent,
    history_recorded: bool,
) {
    if !state.venture.autosave_enabled || !state.venture.dirty || state.venture.selected.is_none() {
        return;
    }

    let should_autosave = history_recorded || matches!(event, AppEvent::Undo | AppEvent::Redo);
    if !should_autosave {
        return;
    }

    let label = state
        .history
        .undo_stack
        .last()
        .map(|entry| entry.label.clone())
        .unwrap_or_else(|| "Autosave".to_owned());

    enqueue_event(
        &mut state.event_queue,
        AppEvent::AutosaveRecoverySlot(label),
    );
}

fn replace_project_state(state: &mut StudioState, mut replacement: StudioState) {
    let event_queue = state.event_queue.clone();
    let performance = state.performance.clone();
    let input_modifiers = state.input_modifiers.clone();
    let revisions = state.revisions.clone();

    replacement.event_queue = event_queue;
    replacement.performance = performance;
    replacement.input_modifiers = input_modifiers;
    replacement.revisions = revisions;
    replacement.context_menu = Default::default();

    *state = replacement;
}

fn toggle_track_mute(state: &mut StudioState, track_id: TrackId) -> Vec<StateDiff> {
    if let Some(track) = state
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.id == track_id)
    {
        track.muted = !track.muted;
        state.status.hint = if track.muted {
            format!("{} stumm", track.name)
        } else {
            format!("{} aktiv", track.name)
        };
        return vec![StateDiff::TrackMix(track_id)];
    }

    Vec::new()
}

fn toggle_track_solo(state: &mut StudioState, track_id: TrackId) -> Vec<StateDiff> {
    if let Some(track) = state
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.id == track_id)
    {
        track.solo = !track.solo;
        state.status.hint = if track.solo {
            format!("{} solo", track.name)
        } else {
            format!("{} solo aus", track.name)
        };
        return vec![StateDiff::TrackMix(track_id)];
    }

    Vec::new()
}

fn handle_timeline_event(state: &mut StudioState, event: TimelineEvent) -> Vec<StateDiff> {
    match event {
        TimelineEvent::PointerMoved(cursor) => handle_pointer_moved(state, cursor),
        TimelineEvent::PointerPressed(cursor) => handle_pointer_pressed(state, cursor),
        TimelineEvent::SecondaryPressed(cursor) => handle_secondary_pressed(state, cursor),
        TimelineEvent::PointerReleased(cursor) => handle_pointer_released(state, cursor),
        TimelineEvent::PointerExited => {
            state.timeline.hover = None;
            if state.timeline.interaction == TimelineInteraction::Idle {
                state.timeline.phase = TimelinePhase::Idle;
                vec![StateDiff::Hover, StateDiff::TimelinePhase]
            } else {
                vec![StateDiff::Hover]
            }
        }
        TimelineEvent::Scrolled {
            delta_lines,
            anchor_x_px,
            anchor_beat,
        } => {
            apply_zoom_delta_around_anchor(state, delta_lines, anchor_x_px, anchor_beat);
            state.timeline.phase = TimelinePhase::Zooming;
            vec![StateDiff::TimelineViewport, StateDiff::TimelinePhase]
        }
    }
}

fn handle_pointer_pressed(
    state: &mut StudioState,
    cursor: crate::core::TimelineCursor,
) -> Vec<StateDiff> {
    let mut prefix_diffs = if matches!(cursor.target, Some(TimelineHit::ContextAction(_))) {
        Vec::new()
    } else {
        close_context_menu_state(state)
    };

    match cursor.target {
        Some(TimelineHit::ContextAction(action)) => {
            prefix_diffs.extend(apply_context_menu_action(state, action));
            prefix_diffs.extend(close_context_menu_state(state));
            prefix_diffs
        }
        Some(TimelineHit::ClipCueHotspot(clip_id, cue_id)) => {
            state.timeline.phase = TimelinePhase::Rendering;
            state.timeline.interaction = TimelineInteraction::Idle;
            state.status.hint = format!("Cue {} direkt aus Clip {} ausgelost", cue_id.0, clip_id.0);
            let mut diffs = prefix_diffs;
            diffs.extend(set_clip_selection(state, clip_id, vec![clip_id]));
            diffs.push(StateDiff::TimelinePhase);
            diffs.extend(trigger_cue(state, cue_id));
            diffs
        }
        Some(TimelineHit::ClipChaseHotspot(clip_id, chase_id)) => {
            state.timeline.phase = TimelinePhase::Rendering;
            state.timeline.interaction = TimelineInteraction::Idle;
            state.status.hint = format!(
                "Chase {} direkt aus Clip {} umgeschaltet",
                chase_id.0, clip_id.0
            );
            let mut diffs = prefix_diffs;
            diffs.extend(set_clip_selection(state, clip_id, vec![clip_id]));
            diffs.push(StateDiff::TimelinePhase);
            diffs.extend(toggle_chase(state, chase_id));
            diffs
        }
        Some(TimelineHit::ClipFxHotspot(clip_id, fx_id)) => {
            state.timeline.phase = TimelinePhase::Rendering;
            state.timeline.interaction = TimelineInteraction::Idle;
            state.status.hint = format!("FX {} direkt aus Clip {} fokussiert", fx_id.0, clip_id.0);
            let mut diffs = prefix_diffs;
            diffs.extend(set_clip_selection(state, clip_id, vec![clip_id]));
            diffs.push(StateDiff::TimelinePhase);
            diffs.extend(select_fx(state, fx_id));
            diffs
        }
        Some(TimelineHit::ClipParamHandle(clip_id, parameter)) => {
            state.timeline.phase = TimelinePhase::Dragging;
            state.timeline.interaction =
                TimelineInteraction::AdjustClipParameter { clip_id, parameter };
            let mut diffs = prefix_diffs;
            diffs.extend(set_clip_selection(state, clip_id, vec![clip_id]));
            begin_timeline_history_transaction(
                state,
                parameter_history_label(parameter).to_owned(),
            );
            diffs.push(StateDiff::TimelinePhase);
            diffs.extend(apply_inline_parameter_from_cursor(
                state,
                clip_id,
                parameter,
                cursor.y_px,
            ));
            diffs
        }
        Some(TimelineHit::ClipBody(clip_id)) => {
            if state.input_modifiers.shift {
                state.timeline.phase = TimelinePhase::Rendering;
                state.timeline.interaction = TimelineInteraction::Idle;
                state.status.hint = "Clip zur Auswahl hinzugefuegt".to_owned();
                let mut diffs = prefix_diffs;
                diffs.extend(toggle_clip_selection_membership(state, clip_id));
                diffs.push(StateDiff::TimelinePhase);
                return diffs;
            }

            state.timeline.phase = TimelinePhase::Rendering;

            if let Some((track_index, clip_index)) = state.clip_location(clip_id) {
                let clip = &state.timeline.tracks[track_index].clips[clip_index];
                state.timeline.interaction = TimelineInteraction::PendingClipDrag {
                    clip_id,
                    origin_track: state.timeline.tracks[track_index].id,
                    origin_start: clip.start,
                    pointer_origin: cursor.beat,
                    pointer_origin_x_px: cursor.x_px,
                    pointer_origin_y_px: cursor.y_px,
                };
            }

            state.status.hint = "Clip selektiert".to_owned();
            let mut diffs = prefix_diffs;
            diffs.extend(set_clip_selection(state, clip_id, vec![clip_id]));
            begin_timeline_history_transaction(state, "Clip Move".to_owned());
            diffs.push(StateDiff::TimelinePhase);
            diffs
        }
        Some(TimelineHit::ClipStartHandle(clip_id)) => {
            state.timeline.phase = TimelinePhase::Rendering;

            if let Some((track_index, clip_index)) = state.clip_location(clip_id) {
                let clip = &state.timeline.tracks[track_index].clips[clip_index];
                state.timeline.interaction = TimelineInteraction::PendingResizeClipStart {
                    clip_id,
                    origin_start: clip.start,
                    origin_duration: clip.duration,
                    pointer_origin: cursor.beat,
                    pointer_origin_x_px: cursor.x_px,
                    pointer_origin_y_px: cursor.y_px,
                };
            }

            state.status.hint = "Clip-Start bereit".to_owned();
            let mut diffs = prefix_diffs;
            diffs.extend(set_clip_selection(state, clip_id, vec![clip_id]));
            begin_timeline_history_transaction(state, "Clip Trim Start".to_owned());
            diffs.push(StateDiff::TimelinePhase);
            diffs
        }
        Some(TimelineHit::ClipEndHandle(clip_id)) => {
            state.timeline.phase = TimelinePhase::Rendering;

            if let Some((track_index, clip_index)) = state.clip_location(clip_id) {
                let clip = &state.timeline.tracks[track_index].clips[clip_index];
                state.timeline.interaction = TimelineInteraction::PendingResizeClipEnd {
                    clip_id,
                    origin_start: clip.start,
                    origin_duration: clip.duration,
                    pointer_origin: cursor.beat,
                    pointer_origin_x_px: cursor.x_px,
                    pointer_origin_y_px: cursor.y_px,
                };
            }

            state.status.hint = "Clip-Ende bereit".to_owned();
            let mut diffs = prefix_diffs;
            diffs.extend(set_clip_selection(state, clip_id, vec![clip_id]));
            begin_timeline_history_transaction(state, "Clip Trim End".to_owned());
            diffs.push(StateDiff::TimelinePhase);
            diffs
        }
        Some(TimelineHit::Playhead) | None if cursor.zone == TimelineZone::Header => {
            enter_sync_phase(&mut state.engine);
            state.timeline.interaction = TimelineInteraction::ScrubPlayhead;
            state.engine.transport.playhead = cursor
                .beat
                .clamp(BeatTime::ZERO, state.engine.transport.song_length);
            state.timeline.phase = TimelinePhase::Rendering;
            state.status.hint = "Playhead wird gescrubbt".to_owned();
            prefix_diffs.extend([
                StateDiff::Engine,
                StateDiff::Playhead,
                StateDiff::TimelinePhase,
            ]);
            prefix_diffs
        }
        None if cursor.zone == TimelineZone::Track => {
            state.timeline.phase = TimelinePhase::Rendering;
            state.timeline.interaction = TimelineInteraction::PendingBoxSelection {
                origin_track: cursor.track,
                origin_beat: cursor
                    .beat
                    .clamp(BeatTime::ZERO, state.engine.transport.song_length),
                origin_x_px: cursor.x_px,
                origin_y_px: cursor.y_px,
                current_x_px: cursor.x_px,
                current_y_px: cursor.y_px,
            };
            state.status.hint = "Bereichsauswahl bereit".to_owned();
            prefix_diffs.push(StateDiff::TimelinePhase);
            prefix_diffs
        }
        _ => prefix_diffs,
    }
}

fn handle_secondary_pressed(
    state: &mut StudioState,
    cursor: crate::core::TimelineCursor,
) -> Vec<StateDiff> {
    let mut diffs = close_context_menu_state(state);
    let target = match cursor.target {
        Some(
            TimelineHit::ClipBody(clip_id)
            | TimelineHit::ClipStartHandle(clip_id)
            | TimelineHit::ClipEndHandle(clip_id)
            | TimelineHit::ClipCueHotspot(clip_id, _)
            | TimelineHit::ClipChaseHotspot(clip_id, _)
            | TimelineHit::ClipFxHotspot(clip_id, _)
            | TimelineHit::ClipParamHandle(clip_id, _),
        ) => {
            diffs.extend(set_clip_selection(state, clip_id, vec![clip_id]));
            ContextMenuTarget::Clip(clip_id)
        }
        Some(TimelineHit::Playhead) | None if cursor.zone == TimelineZone::Header => {
            ContextMenuTarget::Timeline
        }
        None if cursor.zone == TimelineZone::Track => cursor
            .track
            .map(ContextMenuTarget::Track)
            .unwrap_or(ContextMenuTarget::Timeline),
        Some(TimelineHit::ContextAction(_)) => {
            return diffs;
        }
        _ => ContextMenuTarget::Timeline,
    };

    state.context_menu.open = true;
    state.context_menu.target = Some(target);
    state.context_menu.x_px = cursor.x_px;
    state.context_menu.y_px = cursor.y_px;
    state.timeline.phase = TimelinePhase::Rendering;
    state.timeline.interaction = TimelineInteraction::Idle;
    state.status.hint = "Kontextmenue geoeffnet".to_owned();

    diffs.push(StateDiff::ContextMenu);
    diffs.push(StateDiff::TimelinePhase);
    diffs
}

fn handle_pointer_moved(
    state: &mut StudioState,
    cursor: crate::core::TimelineCursor,
) -> Vec<StateDiff> {
    match state.timeline.interaction {
        TimelineInteraction::Idle => {
            state.timeline.hover = cursor.target.map(hit_to_hover);
            vec![StateDiff::Hover]
        }
        TimelineInteraction::PendingBoxSelection {
            origin_x_px,
            origin_y_px,
            current_x_px: _,
            current_y_px: _,
            ..
        } => {
            if !drag_hysteresis_reached(origin_x_px, origin_y_px, &cursor) {
                state.timeline.hover = None;
                return vec![StateDiff::Hover];
            }

            state.timeline.interaction = TimelineInteraction::BoxSelecting {
                origin_x_px,
                origin_y_px,
                current_x_px: cursor.x_px,
                current_y_px: cursor.y_px,
            };
            state.timeline.phase = TimelinePhase::Dragging;
            state.status.hint = "Bereichsauswahl aktiv".to_owned();

            let mut diffs =
                update_box_selection(state, origin_x_px, origin_y_px, cursor.x_px, cursor.y_px);
            diffs.push(StateDiff::TimelinePhase);
            diffs.push(StateDiff::Hover);
            diffs
        }
        TimelineInteraction::BoxSelecting {
            origin_x_px,
            origin_y_px,
            current_x_px: _,
            current_y_px: _,
        } => {
            state.timeline.interaction = TimelineInteraction::BoxSelecting {
                origin_x_px,
                origin_y_px,
                current_x_px: cursor.x_px,
                current_y_px: cursor.y_px,
            };
            state.timeline.phase = TimelinePhase::Dragging;
            state.timeline.hover = None;
            let mut diffs =
                update_box_selection(state, origin_x_px, origin_y_px, cursor.x_px, cursor.y_px);
            diffs.push(StateDiff::TimelinePhase);
            diffs.push(StateDiff::Hover);
            diffs
        }
        TimelineInteraction::PendingClipDrag {
            clip_id,
            origin_track,
            origin_start,
            pointer_origin,
            pointer_origin_x_px,
            pointer_origin_y_px,
        } => {
            if !drag_hysteresis_reached(pointer_origin_x_px, pointer_origin_y_px, &cursor) {
                state.timeline.hover = Some(crate::core::HoverTarget::ClipBody(clip_id));
                return vec![StateDiff::Hover];
            }

            state.timeline.interaction = TimelineInteraction::DragClip {
                clip_id,
                origin_track,
                origin_start,
                pointer_origin,
            };
            state.status.hint = "Clip wird bewegt".to_owned();
            handle_pointer_moved(state, cursor)
        }
        TimelineInteraction::PendingResizeClipStart {
            clip_id,
            origin_start,
            origin_duration,
            pointer_origin,
            pointer_origin_x_px,
            pointer_origin_y_px,
        } => {
            if !drag_hysteresis_reached(pointer_origin_x_px, pointer_origin_y_px, &cursor) {
                state.timeline.hover = Some(crate::core::HoverTarget::ClipStartHandle(clip_id));
                return vec![StateDiff::Hover];
            }

            state.timeline.interaction = TimelineInteraction::ResizeClipStart {
                clip_id,
                origin_start,
                origin_duration,
                pointer_origin,
            };
            state.status.hint = "Clip-Start wird angepasst".to_owned();
            handle_pointer_moved(state, cursor)
        }
        TimelineInteraction::PendingResizeClipEnd {
            clip_id,
            origin_start,
            origin_duration,
            pointer_origin,
            pointer_origin_x_px,
            pointer_origin_y_px,
        } => {
            if !drag_hysteresis_reached(pointer_origin_x_px, pointer_origin_y_px, &cursor) {
                state.timeline.hover = Some(crate::core::HoverTarget::ClipEndHandle(clip_id));
                return vec![StateDiff::Hover];
            }

            state.timeline.interaction = TimelineInteraction::ResizeClipEnd {
                clip_id,
                origin_start,
                origin_duration,
                pointer_origin,
            };
            state.status.hint = "Clip-Ende wird angepasst".to_owned();
            handle_pointer_moved(state, cursor)
        }
        TimelineInteraction::AdjustClipParameter { clip_id, parameter } => {
            state.timeline.phase = TimelinePhase::Dragging;
            let mut diffs =
                apply_inline_parameter_from_cursor(state, clip_id, parameter, cursor.y_px);
            diffs.extend(set_clip_selection(state, clip_id, vec![clip_id]));
            diffs.push(StateDiff::TimelinePhase);
            diffs
        }
        TimelineInteraction::DragClip {
            clip_id,
            origin_track,
            origin_start,
            pointer_origin,
        } => {
            let raw_start = translate(origin_start, pointer_origin, cursor.beat);
            let target_track = cursor.track.unwrap_or(origin_track);
            let duration = state
                .clip(clip_id)
                .map(|clip| clip.duration)
                .unwrap_or(MIN_CLIP_DURATION);
            let max_start = state.engine.transport.song_length.saturating_sub(duration);
            let (snapped_start, guide) = snap_value(state, raw_start.min(max_start));

            set_snap_guide(state, guide.as_ref(), target_track);
            move_clip_to_track(state, clip_id, target_track, snapped_start);
            let mut diffs = set_clip_selection(state, clip_id, vec![clip_id]);
            state.timeline.phase = if guide.is_some() {
                TimelinePhase::Snapping
            } else {
                TimelinePhase::Dragging
            };

            diffs.extend([
                StateDiff::ClipGeometry(clip_id),
                StateDiff::SnapGuide,
                StateDiff::TimelinePhase,
            ]);
            diffs
        }
        TimelineInteraction::ResizeClipStart {
            clip_id,
            origin_start,
            origin_duration,
            pointer_origin,
        } => {
            let clip_end = origin_start.saturating_add(origin_duration);
            let raw_start = translate(origin_start, pointer_origin, cursor.beat);
            let max_start = clip_end.saturating_sub(MIN_CLIP_DURATION);
            let (snapped_start, guide) =
                snap_value(state, raw_start.clamp(BeatTime::ZERO, max_start));

            set_snap_guide(state, guide.as_ref(), cursor.track.unwrap_or(TrackId(0)));

            if let Some(clip) = clip_mut(state, clip_id) {
                clip.start = snapped_start;
                clip.duration = clip_end
                    .saturating_sub(snapped_start)
                    .max(MIN_CLIP_DURATION);
            }

            state.timeline.phase = if guide.is_some() {
                TimelinePhase::Snapping
            } else {
                TimelinePhase::Dragging
            };

            vec![
                StateDiff::ClipGeometry(clip_id),
                StateDiff::SnapGuide,
                StateDiff::TimelinePhase,
            ]
        }
        TimelineInteraction::ResizeClipEnd {
            clip_id,
            origin_start,
            origin_duration,
            pointer_origin,
        } => {
            let raw_end = translate(
                origin_start.saturating_add(origin_duration),
                pointer_origin,
                cursor.beat,
            );
            let min_end = origin_start.saturating_add(MIN_CLIP_DURATION);
            let (snapped_end, guide) = snap_value(
                state,
                raw_end.clamp(min_end, state.engine.transport.song_length),
            );

            set_snap_guide(state, guide.as_ref(), cursor.track.unwrap_or(TrackId(0)));

            if let Some(clip) = clip_mut(state, clip_id) {
                clip.duration = snapped_end
                    .saturating_sub(clip.start)
                    .max(MIN_CLIP_DURATION);
            }

            state.timeline.phase = if guide.is_some() {
                TimelinePhase::Snapping
            } else {
                TimelinePhase::Dragging
            };

            vec![
                StateDiff::ClipGeometry(clip_id),
                StateDiff::SnapGuide,
                StateDiff::TimelinePhase,
            ]
        }
        TimelineInteraction::ScrubPlayhead => {
            state.engine.transport.playhead = cursor
                .beat
                .clamp(BeatTime::ZERO, state.engine.transport.song_length);
            state.timeline.phase = TimelinePhase::Rendering;
            vec![StateDiff::Playhead, StateDiff::TimelinePhase]
        }
    }
}

fn handle_pointer_released(
    state: &mut StudioState,
    cursor: crate::core::TimelineCursor,
) -> Vec<StateDiff> {
    let mut diffs = Vec::new();

    if let TimelineInteraction::PendingBoxSelection {
        origin_track,
        origin_beat,
        ..
    } = state.timeline.interaction
    {
        if let Some(track_id) = origin_track {
            diffs.extend(set_track_selection(state, track_id));
            state.engine.transport.playhead = origin_beat;
            state.timeline.interaction = TimelineInteraction::Idle;
            state.timeline.hover = cursor.target.map(hit_to_hover);
            state.timeline.phase = TimelinePhase::Rendering;
            diffs.push(StateDiff::Playhead);
            diffs.push(StateDiff::Hover);
            diffs.push(StateDiff::TimelinePhase);
            return diffs;
        }
    }

    if matches!(
        state.timeline.interaction,
        TimelineInteraction::BoxSelecting { .. }
    ) {
        state.timeline.interaction = TimelineInteraction::Idle;
        state.timeline.hover = cursor.target.map(hit_to_hover);
        state.timeline.phase = TimelinePhase::Rendering;
        diffs.push(StateDiff::Hover);
        diffs.push(StateDiff::TimelinePhase);
        return diffs;
    }

    let should_lock_snap = matches!(
        state.timeline.interaction,
        TimelineInteraction::DragClip { .. }
            | TimelineInteraction::ResizeClipStart { .. }
            | TimelineInteraction::ResizeClipEnd { .. }
    );

    if state.timeline.interaction == TimelineInteraction::ScrubPlayhead {
        state.engine.transport.playhead = cursor
            .beat
            .clamp(BeatTime::ZERO, state.engine.transport.song_length);
        resume_after_sync(&mut state.engine);
        diffs.push(StateDiff::Engine);
        diffs.push(StateDiff::Playhead);
    }

    if should_lock_snap && state.timeline.snap.guide.is_some() {
        state.timeline.snap.phase = SnapPhase::Locked;
        diffs.push(StateDiff::SnapGuide);
    }

    state.timeline.interaction = TimelineInteraction::Idle;
    state.timeline.hover = cursor.target.map(hit_to_hover);
    state.timeline.phase = TimelinePhase::Rendering;
    diffs.push(StateDiff::Hover);
    diffs.push(StateDiff::TimelinePhase);

    diffs
}

fn translate(origin: BeatTime, pointer_origin: BeatTime, current: BeatTime) -> BeatTime {
    if current >= pointer_origin {
        origin.saturating_add(current.saturating_sub(pointer_origin))
    } else {
        origin.saturating_sub(pointer_origin.saturating_sub(current))
    }
}

fn drag_hysteresis_reached(
    origin_x_px: i32,
    origin_y_px: i32,
    cursor: &crate::core::TimelineCursor,
) -> bool {
    let delta_x = cursor.x_px - origin_x_px;
    let delta_y = cursor.y_px - origin_y_px;
    let distance_squared = delta_x.pow(2) + delta_y.pow(2);
    distance_squared >= DRAG_HYSTERESIS_PX.pow(2)
}

fn update_box_selection(
    state: &mut StudioState,
    origin_x_px: i32,
    origin_y_px: i32,
    current_x_px: i32,
    current_y_px: i32,
) -> Vec<StateDiff> {
    let rect = PixelRect::from_points(origin_x_px, origin_y_px, current_x_px, current_y_px);
    let mut selected = Vec::new();

    for (track_index, track) in state.timeline.tracks.iter().enumerate() {
        for clip in &track.clips {
            if clip_pixel_rect(state, track_index, clip).intersects(rect) {
                selected.push(clip.id);
            }
        }
    }

    if let Some(primary) = selected.first().copied() {
        set_clip_selection(state, primary, selected)
    } else {
        clear_selection(state)
    }
}

fn set_clip_selection(
    state: &mut StudioState,
    primary: ClipId,
    selected_clips: Vec<ClipId>,
) -> Vec<StateDiff> {
    apply_selection_state(state, SelectionState::Clip(primary), selected_clips)
}

fn toggle_clip_selection_membership(state: &mut StudioState, clip_id: ClipId) -> Vec<StateDiff> {
    let mut selected = state.timeline.selected_clips.clone();

    if let Some(index) = selected.iter().position(|id| *id == clip_id) {
        selected.remove(index);
        if let Some(primary) = selected.first().copied() {
            apply_selection_state(state, SelectionState::Clip(primary), selected)
        } else {
            clear_selection(state)
        }
    } else {
        selected.push(clip_id);
        let primary = state.primary_selected_clip_id().unwrap_or(clip_id);
        apply_selection_state(state, SelectionState::Clip(primary), selected)
    }
}

fn set_track_selection(state: &mut StudioState, track_id: TrackId) -> Vec<StateDiff> {
    apply_selection_state(state, SelectionState::Track(track_id), Vec::new())
}

fn clear_selection(state: &mut StudioState) -> Vec<StateDiff> {
    apply_selection_state(state, SelectionState::None, Vec::new())
}

fn apply_selection_state(
    state: &mut StudioState,
    selection: SelectionState,
    mut selected_clips: Vec<ClipId>,
) -> Vec<StateDiff> {
    selected_clips.sort_by_key(|clip_id| clip_id.0);
    selected_clips.dedup_by_key(|clip_id| clip_id.0);

    match selection {
        SelectionState::Clip(primary) => {
            if !selected_clips.contains(&primary) {
                selected_clips.insert(0, primary);
            }
        }
        SelectionState::Track(_) | SelectionState::None => selected_clips.clear(),
    }

    let selection_changed =
        state.timeline.selection != selection || state.timeline.selected_clips != selected_clips;
    state.timeline.selection = selection;
    state.timeline.selected_clips = selected_clips;

    let mut diffs = if selection_changed {
        vec![StateDiff::Selection]
    } else {
        Vec::new()
    };

    let single_selected_clip = state.selected_clip().map(|clip| clip.id);
    if state.clip_editor.phase != crate::core::ClipEditorPhase::Closed
        && state.clip_editor.clip_id != single_selected_clip
    {
        state.clip_editor.phase = crate::core::ClipEditorPhase::Closed;
        state.clip_editor.clip_id = None;
        diffs.push(StateDiff::ClipEditor);
    }

    diffs
}

fn apply_zoom_delta_around_anchor(
    state: &mut StudioState,
    delta_lines: i16,
    anchor_x_px: i32,
    anchor_beat: BeatTime,
) {
    let zoom = state.timeline.viewport.zoom.permille() as i32 - (delta_lines as i32 * 45);
    state.timeline.viewport.zoom = ZoomFactor::from_permille(zoom.clamp(450, 2400) as u16);

    let anchor_ticks = anchor_beat.ticks() as f32;
    let scroll_ticks = anchor_ticks - ((anchor_x_px as f32 / pixels_per_beat(state)) * PPQ as f32);
    state.timeline.viewport.scroll = BeatTime::from_ticks(scroll_ticks.max(0.0).round() as u32)
        .clamp(BeatTime::ZERO, state.engine.transport.song_length);
}

fn hit_to_hover(hit: TimelineHit) -> crate::core::HoverTarget {
    match hit {
        TimelineHit::ContextAction(_) => crate::core::HoverTarget::Playhead,
        TimelineHit::Playhead => crate::core::HoverTarget::Playhead,
        TimelineHit::ClipBody(clip_id)
        | TimelineHit::ClipCueHotspot(clip_id, _)
        | TimelineHit::ClipChaseHotspot(clip_id, _)
        | TimelineHit::ClipFxHotspot(clip_id, _)
        | TimelineHit::ClipParamHandle(clip_id, _) => crate::core::HoverTarget::ClipBody(clip_id),
        TimelineHit::ClipStartHandle(clip_id) => crate::core::HoverTarget::ClipStartHandle(clip_id),
        TimelineHit::ClipEndHandle(clip_id) => crate::core::HoverTarget::ClipEndHandle(clip_id),
    }
}

fn apply_inline_parameter_from_cursor(
    state: &mut StudioState,
    clip_id: ClipId,
    parameter: ClipInlineParameterKind,
    y_px: i32,
) -> Vec<StateDiff> {
    let Some((track_index, _)) = state.clip_location(clip_id) else {
        return Vec::new();
    };

    let clip_top = TIMELINE_HEADER_HEIGHT_PX
        + (track_index as i32 * (TIMELINE_TRACK_HEIGHT_PX + TIMELINE_TRACK_GAP_PX))
        + TIMELINE_CLIP_TOP_INSET_PX;
    let clip_bottom = clip_top + TIMELINE_CLIP_HEIGHT_PX;
    let clamped_y = y_px.clamp(clip_top, clip_bottom);
    let value_permille =
        (((clip_bottom - clamped_y) * 1000) / TIMELINE_CLIP_HEIGHT_PX).clamp(0, 1000) as u16;

    let hint = {
        let Some(clip) = clip_mut(state, clip_id) else {
            return Vec::new();
        };

        match parameter {
            ClipInlineParameterKind::Intensity => {
                clip.params.intensity = IntensityLevel::from_permille(value_permille);
                format!("Clip {} intensity {}%", clip_id.0, value_permille / 10)
            }
            ClipInlineParameterKind::Speed => {
                let speed_permille = 200 + ((value_permille as u32 * 1300) / 1000) as u16;
                clip.params.speed = SpeedRatio::from_permille(speed_permille);
                format!("Clip {} speed {}%", clip_id.0, speed_permille / 10)
            }
            ClipInlineParameterKind::FxDepth => {
                clip.params.fx_depth = IntensityLevel::from_permille(value_permille);
                format!("Clip {} FX depth {}%", clip_id.0, value_permille / 10)
            }
        }
    };
    state.status.hint = hint;

    let mut diffs = vec![StateDiff::ClipGeometry(clip_id)];
    if state.clip_editor.clip_id == Some(clip_id)
        && state.clip_editor.phase != crate::core::ClipEditorPhase::Closed
    {
        diffs.push(StateDiff::ClipEditor);
    }
    diffs.extend(advance_show_frame(state, BeatTime::ZERO));
    diffs
}

fn set_snap_guide(state: &mut StudioState, guide: Option<&SnapGuide>, track: TrackId) {
    match guide {
        Some(guide) => {
            state.timeline.snap.phase = SnapPhase::Snapping;
            state.timeline.snap.guide = Some(SnapGuide {
                beat: guide.beat,
                track: Some(track),
                strength_permille: guide.strength_permille,
            });
        }
        None => {
            state.timeline.snap.phase = SnapPhase::Free;
            state.timeline.snap.guide = None;
        }
    }
}

fn snap_value(state: &StudioState, value: BeatTime) -> (BeatTime, Option<SnapGuide>) {
    if !state.timeline.snap.enabled || state.input_modifiers.alt {
        return (value, None);
    }

    let step = state.timeline.snap.resolution.step();
    let acquire_threshold =
        threshold_ticks_for_snap(state, step, SNAP_ACQUIRE_DISTANCE_PX, step.ticks() / 2);
    let release_threshold = threshold_ticks_for_snap(
        state,
        step,
        SNAP_RELEASE_DISTANCE_PX,
        (step.ticks() * 7) / 10,
    );

    if let Some(guide) = state
        .timeline
        .snap
        .guide
        .as_ref()
        .filter(|_| {
            matches!(
                state.timeline.snap.phase,
                SnapPhase::Snapping | SnapPhase::Locked
            )
        })
        .and_then(|guide| snap_guide_for_threshold(value, guide.beat, release_threshold))
    {
        return (guide.beat, Some(guide));
    }

    let snapped = value.quantize(step);

    snap_guide_for_threshold(value, snapped, acquire_threshold)
        .map(|guide| (snapped, Some(guide)))
        .unwrap_or((value, None))
}

fn snap_guide_for_threshold(
    value: BeatTime,
    snapped: BeatTime,
    threshold_ticks: u32,
) -> Option<SnapGuide> {
    let distance = beat_distance(value, snapped);

    if distance.ticks() > threshold_ticks {
        return None;
    }

    let strength = 1000 - ((distance.ticks() * 1000) / threshold_ticks.max(1));
    Some(SnapGuide {
        beat: snapped,
        track: None,
        strength_permille: strength as u16,
    })
}

fn beat_distance(left: BeatTime, right: BeatTime) -> BeatTime {
    if left >= right {
        left.saturating_sub(right)
    } else {
        right.saturating_sub(left)
    }
}

fn threshold_ticks_for_snap(
    state: &StudioState,
    step: BeatTime,
    distance_px: f32,
    max_step_ticks: u32,
) -> u32 {
    let pixel_ticks = ((distance_px / pixels_per_beat(state)) * PPQ as f32)
        .round()
        .max(1.0) as u32;
    pixel_ticks
        .min(max_step_ticks.max(1))
        .min(step.ticks().max(1))
}

fn pixels_per_beat(state: &StudioState) -> f32 {
    40.0 * state.timeline.viewport.zoom.as_f32()
}

fn clip_pixel_rect(state: &StudioState, track_index: usize, clip: &crate::core::Clip) -> PixelRect {
    PixelRect {
        x: beat_to_x_px(state, clip.start),
        y: TIMELINE_HEADER_HEIGHT_PX
            + (track_index as i32 * (TIMELINE_TRACK_HEIGHT_PX + TIMELINE_TRACK_GAP_PX))
            + TIMELINE_CLIP_TOP_INSET_PX,
        width: ((clip.duration.as_beats_f32() * pixels_per_beat(state)).max(36.0)).round() as i32,
        height: TIMELINE_CLIP_HEIGHT_PX,
    }
}

fn beat_to_x_px(state: &StudioState, beat: BeatTime) -> i32 {
    (((beat.ticks() as f32 - state.timeline.viewport.scroll.ticks() as f32) / PPQ as f32)
        * pixels_per_beat(state))
    .round() as i32
}

#[derive(Debug, Clone, Copy)]
struct PixelRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

impl PixelRect {
    fn from_points(x0: i32, y0: i32, x1: i32, y1: i32) -> Self {
        let min_x = x0.min(x1);
        let min_y = y0.min(y1);
        let max_x = x0.max(x1);
        let max_y = y0.max(y1);

        Self {
            x: min_x,
            y: min_y,
            width: (max_x - min_x).max(1),
            height: (max_y - min_y).max(1),
        }
    }

    fn intersects(self, other: Self) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }
}

fn clip_mut(state: &mut StudioState, clip_id: ClipId) -> Option<&mut crate::core::Clip> {
    let (track_index, clip_index) = state.clip_location(clip_id)?;
    state
        .timeline
        .tracks
        .get_mut(track_index)?
        .clips
        .get_mut(clip_index)
}

fn move_clip_to_track(
    state: &mut StudioState,
    clip_id: ClipId,
    target_track: TrackId,
    start: BeatTime,
) {
    let Some((source_track_index, clip_index)) = state.clip_location(clip_id) else {
        return;
    };
    let Some(target_track_index) = state.track_index(target_track) else {
        return;
    };

    if source_track_index == target_track_index {
        if let Some(clip) = state.timeline.tracks[source_track_index]
            .clips
            .get_mut(clip_index)
        {
            clip.start = start;
        }
        sort_track(state, source_track_index);
        return;
    }

    let mut clip = state.timeline.tracks[source_track_index]
        .clips
        .remove(clip_index);
    clip.start = start;
    state.timeline.tracks[target_track_index].clips.push(clip);
    sort_track(state, source_track_index);
    sort_track(state, target_track_index);
}

fn duplicate_selected_clips(state: &mut StudioState) -> Vec<StateDiff> {
    let selected_ids = state.timeline.selected_clips.clone();
    if selected_ids.is_empty() {
        return Vec::new();
    }

    let mut selected = Vec::new();
    let mut min_start = None;
    let mut max_end = BeatTime::ZERO;

    for clip_id in selected_ids {
        let Some((track_index, clip_index)) = state.clip_location(clip_id) else {
            continue;
        };
        let clip = state.timeline.tracks[track_index].clips[clip_index].clone();
        let clip_end = clip.start.saturating_add(clip.duration);
        min_start = Some(min_start.map_or(clip.start, |current: BeatTime| current.min(clip.start)));
        max_end = max_end.max(clip_end);
        selected.push((track_index, clip));
    }

    let Some(min_start) = min_start else {
        return Vec::new();
    };

    let selection_span = max_end.saturating_sub(min_start);
    if max_end.saturating_add(selection_span) > state.engine.transport.song_length {
        return Vec::new();
    }

    let mut next_id = state.next_clip_id().0;
    let mut duplicated_ids = Vec::new();
    let mut diffs = Vec::new();

    for (track_index, clip) in selected {
        let mut duplicate = clip.clone();
        duplicate.id = ClipId(next_id);
        next_id = next_id.saturating_add(1);
        duplicate.start = duplicate.start.saturating_add(selection_span);
        duplicate.title = format!("{} Copy", duplicate.title);
        state.timeline.tracks[track_index]
            .clips
            .push(duplicate.clone());
        sort_track(state, track_index);
        duplicated_ids.push(duplicate.id);
        diffs.push(StateDiff::ClipGeometry(duplicate.id));
    }

    if let Some(primary) = duplicated_ids.first().copied() {
        diffs.extend(set_clip_selection(state, primary, duplicated_ids));
    }

    state.timeline.phase = TimelinePhase::Rendering;
    diffs.push(StateDiff::TimelinePhase);
    diffs
}

fn split_selected_clips_at_playhead(state: &mut StudioState) -> Vec<StateDiff> {
    let playhead = state.engine.transport.playhead;
    let selected_ids = state.timeline.selected_clips.clone();
    if selected_ids.is_empty() {
        return Vec::new();
    }

    let mut next_id = state.next_clip_id().0;
    let mut selection_after = Vec::new();
    let mut diffs = Vec::new();
    let mut split_any = false;

    for clip_id in selected_ids {
        let Some((track_index, clip_index)) = state.clip_location(clip_id) else {
            continue;
        };

        let clip = state.timeline.tracks[track_index].clips[clip_index].clone();
        let clip_end = clip.start.saturating_add(clip.duration);
        if playhead <= clip.start || playhead >= clip_end {
            selection_after.push(clip_id);
            continue;
        }

        let left_duration = playhead.saturating_sub(clip.start);
        let right_duration = clip_end.saturating_sub(playhead);
        if left_duration < MIN_CLIP_DURATION || right_duration < MIN_CLIP_DURATION {
            selection_after.push(clip_id);
            continue;
        }

        split_any = true;
        let split_offset = playhead.saturating_sub(clip.start);

        let mut right_clip = clip.clone();
        right_clip.id = ClipId(next_id);
        next_id = next_id.saturating_add(1);
        right_clip.start = playhead;
        right_clip.duration = right_duration;
        right_clip.title = format!("{} B", clip.title);
        right_clip.markers = clip
            .markers
            .iter()
            .cloned()
            .filter_map(|mut marker| {
                if marker.offset < split_offset {
                    return None;
                }
                marker.offset = marker.offset.saturating_sub(split_offset);
                Some(marker)
            })
            .collect();

        if let Some(left_clip) = state.timeline.tracks[track_index].clips.get_mut(clip_index) {
            left_clip.duration = left_duration;
            left_clip
                .markers
                .retain(|marker| marker.offset < split_offset);
        }

        state.timeline.tracks[track_index]
            .clips
            .push(right_clip.clone());
        sort_track(state, track_index);

        selection_after.push(clip_id);
        selection_after.push(right_clip.id);
        diffs.push(StateDiff::ClipGeometry(clip_id));
        diffs.push(StateDiff::ClipGeometry(right_clip.id));
    }

    if !split_any {
        return Vec::new();
    }

    if let Some(primary) = selection_after.first().copied() {
        diffs.extend(set_clip_selection(state, primary, selection_after));
    }

    state.timeline.phase = TimelinePhase::Rendering;
    diffs.push(StateDiff::TimelinePhase);
    diffs
}

fn delete_selected_clips(state: &mut StudioState) -> Vec<StateDiff> {
    let selected_ids = state.timeline.selected_clips.clone();
    if selected_ids.is_empty() {
        return Vec::new();
    }

    let mut diffs = Vec::new();

    for track in &mut state.timeline.tracks {
        let mut retained = Vec::with_capacity(track.clips.len());
        for clip in track.clips.drain(..) {
            if selected_ids.contains(&clip.id) {
                diffs.push(StateDiff::ClipGeometry(clip.id));
            } else {
                retained.push(clip);
            }
        }
        track.clips = retained;
    }

    for cue in &mut state.cue_system.cues {
        if cue
            .linked_clip
            .is_some_and(|clip_id| selected_ids.contains(&clip_id))
        {
            cue.linked_clip = None;
            diffs.push(StateDiff::Cue(cue.id));
        }
    }

    for chase in &mut state.chase_system.chases {
        if chase
            .linked_clip
            .is_some_and(|clip_id| selected_ids.contains(&clip_id))
        {
            chase.linked_clip = None;
            diffs.push(StateDiff::Chase(chase.id));
        }
    }

    for layer in &mut state.fx_system.layers {
        if layer
            .linked_clip
            .is_some_and(|clip_id| selected_ids.contains(&clip_id))
        {
            layer.linked_clip = None;
            diffs.push(StateDiff::Fx(layer.id));
        }
    }

    if state
        .timeline
        .interaction
        .active_clip()
        .is_some_and(|clip_id| selected_ids.contains(&clip_id))
    {
        state.timeline.interaction = TimelineInteraction::Idle;
        state.timeline.snap.phase = SnapPhase::Free;
        state.timeline.snap.guide = None;
        diffs.push(StateDiff::SnapGuide);
    }

    if matches!(
        state.timeline.hover,
        Some(HoverTarget::ClipBody(clip_id)
            | HoverTarget::ClipStartHandle(clip_id)
            | HoverTarget::ClipEndHandle(clip_id)) if selected_ids.contains(&clip_id)
    ) {
        state.timeline.hover = None;
        diffs.push(StateDiff::Hover);
    }

    diffs.extend(clear_selection(state));
    state.timeline.phase = TimelinePhase::Rendering;
    diffs.push(StateDiff::TimelinePhase);
    diffs
}

fn copy_selected_clips(state: &mut StudioState, from_cut: bool) -> Vec<StateDiff> {
    let Some((min_start, max_end)) = state.selected_clip_time_span() else {
        return Vec::new();
    };

    let mut clips = state
        .timeline
        .selected_clips
        .iter()
        .filter_map(|clip_id| {
            let (track_index, clip_index) = state.clip_location(*clip_id)?;
            let track = state.timeline.tracks.get(track_index)?;
            let clip = track.clips.get(clip_index)?.clone();
            Some(ClipboardClip {
                track_id: track.id,
                relative_start: clip.start.saturating_sub(min_start),
                clip,
            })
        })
        .collect::<Vec<_>>();

    clips.sort_by_key(|entry| {
        (
            entry.track_id.0,
            entry.relative_start.ticks(),
            entry.clip.id.0,
        )
    });

    state.clipboard.clips = clips;
    state.clipboard.span = max_end.saturating_sub(min_start);
    state.clipboard.from_cut = from_cut;
    state.clipboard.version = state.clipboard.version.saturating_add(1);
    state.clipboard.last_paste_anchor = None;
    state.clipboard.next_paste_index = 0;

    vec![StateDiff::Clipboard]
}

fn cut_selected_clips(state: &mut StudioState) -> Vec<StateDiff> {
    let mut diffs = copy_selected_clips(state, true);
    if diffs.is_empty() {
        return diffs;
    }

    diffs.extend(delete_selected_clips(state));
    diffs
}

fn paste_clipboard_at_playhead(state: &mut StudioState) -> Vec<StateDiff> {
    if state.clipboard.clips.is_empty() {
        return Vec::new();
    }

    let anchor = state.engine.transport.playhead;
    let paste_index = if state.clipboard.last_paste_anchor == Some(anchor) {
        state.clipboard.next_paste_index
    } else {
        0
    };
    let offset_ticks = state
        .clipboard
        .span
        .ticks()
        .saturating_mul(paste_index as u32);
    let offset = BeatTime::from_ticks(offset_ticks);

    let placements = state
        .clipboard
        .clips
        .iter()
        .map(|entry| {
            let start = anchor
                .saturating_add(entry.relative_start)
                .saturating_add(offset);
            let end = start.saturating_add(entry.clip.duration);
            (entry.track_id, entry.clip.clone(), start, end)
        })
        .collect::<Vec<_>>();

    if placements
        .iter()
        .any(|(_, _, _, end)| *end > state.engine.transport.song_length)
    {
        return Vec::new();
    }

    let mut next_id = state.next_clip_id().0;
    let mut pasted_ids = Vec::new();
    let mut diffs = vec![StateDiff::Clipboard];

    for (track_id, mut clip, start, _) in placements {
        let Some(track_index) = state.track_index(track_id) else {
            continue;
        };

        clip.id = ClipId(next_id);
        next_id = next_id.saturating_add(1);
        clip.start = start;
        state.timeline.tracks[track_index].clips.push(clip.clone());
        sort_track(state, track_index);
        pasted_ids.push(clip.id);
        diffs.push(StateDiff::ClipGeometry(clip.id));
    }

    if pasted_ids.is_empty() {
        return Vec::new();
    }

    state.clipboard.last_paste_anchor = Some(anchor);
    state.clipboard.next_paste_index = paste_index.saturating_add(1);
    state.clipboard.from_cut = false;

    if let Some(primary) = pasted_ids.first().copied() {
        diffs.extend(set_clip_selection(state, primary, pasted_ids));
    }

    state.timeline.phase = TimelinePhase::Rendering;
    diffs.push(StateDiff::TimelinePhase);
    diffs
}

fn nudge_selected_clips(state: &mut StudioState, direction: i8) -> Vec<StateDiff> {
    let selected_ids = state.timeline.selected_clips.clone();
    if selected_ids.is_empty() {
        return Vec::new();
    }

    let step = nudge_step(state);
    let mut updates = Vec::new();

    for clip_id in &selected_ids {
        let Some(clip) = state.clip(*clip_id) else {
            return Vec::new();
        };
        let new_start = if direction < 0 {
            clip.start.saturating_sub(step)
        } else {
            clip.start.saturating_add(step)
        };
        if new_start.saturating_add(clip.duration) > state.engine.transport.song_length {
            return Vec::new();
        }
        updates.push((*clip_id, new_start));
    }

    let mut diffs = Vec::new();
    for (clip_id, start) in updates {
        if let Some(clip) = clip_mut(state, clip_id) {
            clip.start = start;
            diffs.push(StateDiff::ClipGeometry(clip_id));
        }
    }

    for index in 0..state.timeline.tracks.len() {
        sort_track(state, index);
    }

    state.timeline.phase = TimelinePhase::Rendering;
    diffs.push(StateDiff::TimelinePhase);
    diffs
}

fn nudge_step(state: &StudioState) -> BeatTime {
    let base = if state.timeline.snap.enabled {
        state.timeline.snap.resolution.step()
    } else {
        BeatTime::from_fraction(1, 4)
    };

    if state.input_modifiers.shift {
        BeatTime::from_ticks((base.ticks() / 2).max(1))
    } else {
        base
    }
}

fn select_all_on_context_track(state: &mut StudioState) -> Vec<StateDiff> {
    let Some(track_id) = state.context_track_id() else {
        return Vec::new();
    };
    let Some(track) = state.track(track_id) else {
        return Vec::new();
    };
    let selected = track.clips.iter().map(|clip| clip.id).collect::<Vec<_>>();
    let Some(primary) = selected.first().copied() else {
        return set_track_selection(state, track_id);
    };
    set_clip_selection(state, primary, selected)
}

fn trim_selected_clips_to_playhead(state: &mut StudioState) -> Vec<StateDiff> {
    let playhead = state.engine.transport.playhead;
    let selected_ids = state.timeline.selected_clips.clone();
    if selected_ids.is_empty() {
        return Vec::new();
    }

    let mut diffs = Vec::new();
    for clip_id in selected_ids {
        if let Some(clip) = clip_mut(state, clip_id) {
            if playhead > clip.start && playhead < clip.start.saturating_add(clip.duration) {
                let new_duration = playhead.saturating_sub(clip.start);
                if new_duration >= MIN_CLIP_DURATION {
                    clip.duration = new_duration;
                    diffs.push(StateDiff::ClipGeometry(clip_id));
                }
            }
        }
    }

    if diffs.is_empty() {
        return diffs;
    }

    state.timeline.phase = TimelinePhase::Rendering;
    diffs.push(StateDiff::TimelinePhase);
    diffs
}

fn apply_context_menu_action(state: &mut StudioState, action: ContextMenuAction) -> Vec<StateDiff> {
    match action {
        ContextMenuAction::Duplicate => duplicate_selected_clips(state),
        ContextMenuAction::Split => split_selected_clips_at_playhead(state),
        ContextMenuAction::Delete => delete_selected_clips(state),
        ContextMenuAction::Copy => copy_selected_clips(state, false),
        ContextMenuAction::Cut => cut_selected_clips(state),
        ContextMenuAction::Paste => paste_clipboard_at_playhead(state),
        ContextMenuAction::NudgeLeft => nudge_selected_clips(state, -1),
        ContextMenuAction::NudgeRight => nudge_selected_clips(state, 1),
        ContextMenuAction::SelectAllOnTrack => select_all_on_context_track(state),
        ContextMenuAction::TrimToPlayhead => trim_selected_clips_to_playhead(state),
        ContextMenuAction::Close => Vec::new(),
    }
}

fn context_action_label(action: ContextMenuAction) -> &'static str {
    match action {
        ContextMenuAction::Duplicate => "Duplicate",
        ContextMenuAction::Split => "Split",
        ContextMenuAction::Delete => "Delete",
        ContextMenuAction::Copy => "Copy",
        ContextMenuAction::Cut => "Cut",
        ContextMenuAction::Paste => "Paste",
        ContextMenuAction::NudgeLeft => "Nudge Left",
        ContextMenuAction::NudgeRight => "Nudge Right",
        ContextMenuAction::SelectAllOnTrack => "Select Track Clips",
        ContextMenuAction::TrimToPlayhead => "Trim To Playhead",
        ContextMenuAction::Close => "Close",
    }
}

fn close_context_menu_state(state: &mut StudioState) -> Vec<StateDiff> {
    if !state.context_menu.open && state.context_menu.target.is_none() {
        return Vec::new();
    }

    state.context_menu.open = false;
    state.context_menu.target = None;
    vec![StateDiff::ContextMenu]
}

fn append_replay_event(events: &mut Vec<AppEvent>, capacity: usize, event: &AppEvent) {
    events.push(event.clone());
    if events.len() > capacity {
        let overflow = events.len() - capacity;
        events.drain(0..overflow);
    }
}

fn should_record_replay_event(event: &AppEvent) -> bool {
    !matches!(
        event,
        AppEvent::RefreshVentures
            | AppEvent::SelectVenture(_)
            | AppEvent::SelectRecoverySlot(_)
            | AppEvent::SetVentureDraftName(_)
            | AppEvent::SaveCurrentVenture
            | AppEvent::SaveCurrentVentureAs
            | AppEvent::RenameSelectedVenture
            | AppEvent::LoadSelectedVenture
            | AppEvent::DeleteSelectedVenture
            | AppEvent::RestoreSelectedRecoverySlot
            | AppEvent::AutosaveRecoverySlot(_)
            | AppEvent::CreateNewVenture
    )
}

fn prepare_history_action(state: &StudioState, event: &AppEvent) -> HistoryPostAction {
    match event {
        AppEvent::DuplicateSelectedClips => HistoryPostAction::RecordImmediate {
            label: "Duplicate Clips".to_owned(),
        },
        AppEvent::SplitSelectedClipsAtPlayhead => HistoryPostAction::RecordImmediate {
            label: "Split Clips".to_owned(),
        },
        AppEvent::DeleteSelectedClips => HistoryPostAction::RecordImmediate {
            label: "Delete Clips".to_owned(),
        },
        AppEvent::CutSelectedClips => HistoryPostAction::RecordImmediate {
            label: "Cut Clips".to_owned(),
        },
        AppEvent::PasteClipboardAtPlayhead => HistoryPostAction::RecordImmediate {
            label: "Paste Clips".to_owned(),
        },
        AppEvent::NudgeSelectedClipsLeft => HistoryPostAction::RecordImmediate {
            label: "Nudge Clips Left".to_owned(),
        },
        AppEvent::NudgeSelectedClipsRight => HistoryPostAction::RecordImmediate {
            label: "Nudge Clips Right".to_owned(),
        },
        AppEvent::SetMasterIntensity(_) => HistoryPostAction::RecordImmediate {
            label: "Master Intensity".to_owned(),
        },
        AppEvent::SetMasterSpeed(_) => HistoryPostAction::RecordImmediate {
            label: "Master Speed".to_owned(),
        },
        AppEvent::ToggleTrackMute(_) => HistoryPostAction::RecordImmediate {
            label: "Track Mute".to_owned(),
        },
        AppEvent::ToggleTrackSolo(_) => HistoryPostAction::RecordImmediate {
            label: "Track Solo".to_owned(),
        },
        AppEvent::CreateCue => HistoryPostAction::RecordImmediate {
            label: "Create Cue".to_owned(),
        },
        AppEvent::DeleteSelectedCue => HistoryPostAction::RecordImmediate {
            label: "Delete Cue".to_owned(),
        },
        AppEvent::SetSelectedCueName(_) => HistoryPostAction::RecordImmediate {
            label: "Cue Name".to_owned(),
        },
        AppEvent::SetSelectedCueColor(_) => HistoryPostAction::RecordImmediate {
            label: "Cue Color".to_owned(),
        },
        AppEvent::SetSelectedCueFadeDuration(_) => HistoryPostAction::RecordImmediate {
            label: "Cue Fade".to_owned(),
        },
        AppEvent::CreateChase => HistoryPostAction::RecordImmediate {
            label: "Create Chase".to_owned(),
        },
        AppEvent::DeleteSelectedChase => HistoryPostAction::RecordImmediate {
            label: "Delete Chase".to_owned(),
        },
        AppEvent::SetSelectedChaseName(_) => HistoryPostAction::RecordImmediate {
            label: "Chase Name".to_owned(),
        },
        AppEvent::SetSelectedChaseDirection(_) => HistoryPostAction::RecordImmediate {
            label: "Chase Direction".to_owned(),
        },
        AppEvent::SetSelectedChaseLoop(_) => HistoryPostAction::RecordImmediate {
            label: "Chase Loop".to_owned(),
        },
        AppEvent::AddSelectedChaseStep => HistoryPostAction::RecordImmediate {
            label: "Add Chase Step".to_owned(),
        },
        AppEvent::DeleteSelectedChaseStep => HistoryPostAction::RecordImmediate {
            label: "Delete Chase Step".to_owned(),
        },
        AppEvent::MoveSelectedChaseStepLeft => HistoryPostAction::RecordImmediate {
            label: "Move Chase Step Left".to_owned(),
        },
        AppEvent::MoveSelectedChaseStepRight => HistoryPostAction::RecordImmediate {
            label: "Move Chase Step Right".to_owned(),
        },
        AppEvent::SetSelectedChaseStepLabel(_) => HistoryPostAction::RecordImmediate {
            label: "Chase Step Label".to_owned(),
        },
        AppEvent::SetSelectedChaseStepCue(_) => HistoryPostAction::RecordImmediate {
            label: "Chase Step Cue".to_owned(),
        },
        AppEvent::SetSelectedChaseStepDuration(_) => HistoryPostAction::RecordImmediate {
            label: "Chase Step Duration".to_owned(),
        },
        AppEvent::SetSelectedChaseStepColor(_) => HistoryPostAction::RecordImmediate {
            label: "Chase Step Color".to_owned(),
        },
        AppEvent::ToggleFx(_) => HistoryPostAction::RecordImmediate {
            label: "FX Toggle".to_owned(),
        },
        AppEvent::SetFxDepth(_, _) => HistoryPostAction::RecordImmediate {
            label: "FX Depth".to_owned(),
        },
        AppEvent::SetFxRate(_, _) => HistoryPostAction::RecordImmediate {
            label: "FX Rate".to_owned(),
        },
        AppEvent::SetFxSpread(_, _) => HistoryPostAction::RecordImmediate {
            label: "FX Spread".to_owned(),
        },
        AppEvent::SetFxPhaseOffset(_, _) => HistoryPostAction::RecordImmediate {
            label: "FX Phase Offset".to_owned(),
        },
        AppEvent::SetFxWaveform(_, _) => HistoryPostAction::RecordImmediate {
            label: "FX Waveform".to_owned(),
        },
        AppEvent::SetClipEditorIntensity(_) => HistoryPostAction::RecordImmediate {
            label: "Clip Intensity".to_owned(),
        },
        AppEvent::SetClipEditorSpeed(_) => HistoryPostAction::RecordImmediate {
            label: "Clip Speed".to_owned(),
        },
        AppEvent::SetClipEditorFxDepth(_) => HistoryPostAction::RecordImmediate {
            label: "Clip FX Depth".to_owned(),
        },
        AppEvent::SetClipEditorCue(_) => HistoryPostAction::RecordImmediate {
            label: "Clip Cue Link".to_owned(),
        },
        AppEvent::SetClipEditorChase(_) => HistoryPostAction::RecordImmediate {
            label: "Clip Chase Link".to_owned(),
        },
        AppEvent::SetClipEditorGrid(_) => HistoryPostAction::RecordImmediate {
            label: "Clip Grid".to_owned(),
        },
        AppEvent::SetClipEditorAutomationMode(_) => HistoryPostAction::RecordImmediate {
            label: "Automation Mode".to_owned(),
        },
        AppEvent::ToggleClipEditorAutomationLane => HistoryPostAction::RecordImmediate {
            label: "Automation Lane Toggle".to_owned(),
        },
        AppEvent::AddClipEditorAutomationPointAtPlayhead => HistoryPostAction::RecordImmediate {
            label: "Automation Point Add".to_owned(),
        },
        AppEvent::SetClipEditorAutomationPointValue(_) => HistoryPostAction::RecordImmediate {
            label: "Automation Point Value".to_owned(),
        },
        AppEvent::NudgeClipEditorAutomationPointLeft => HistoryPostAction::RecordImmediate {
            label: "Automation Point Nudge Left".to_owned(),
        },
        AppEvent::NudgeClipEditorAutomationPointRight => HistoryPostAction::RecordImmediate {
            label: "Automation Point Nudge Right".to_owned(),
        },
        AppEvent::DeleteClipEditorAutomationPoint => HistoryPostAction::RecordImmediate {
            label: "Automation Point Delete".to_owned(),
        },
        AppEvent::ApplyContextMenuAction(action) => match action {
            ContextMenuAction::Duplicate => HistoryPostAction::RecordImmediate {
                label: "Context Duplicate Clips".to_owned(),
            },
            ContextMenuAction::Split => HistoryPostAction::RecordImmediate {
                label: "Context Split Clips".to_owned(),
            },
            ContextMenuAction::Delete => HistoryPostAction::RecordImmediate {
                label: "Context Delete Clips".to_owned(),
            },
            ContextMenuAction::Cut => HistoryPostAction::RecordImmediate {
                label: "Context Cut Clips".to_owned(),
            },
            ContextMenuAction::Paste => HistoryPostAction::RecordImmediate {
                label: "Context Paste Clips".to_owned(),
            },
            ContextMenuAction::NudgeLeft => HistoryPostAction::RecordImmediate {
                label: "Context Nudge Left".to_owned(),
            },
            ContextMenuAction::NudgeRight => HistoryPostAction::RecordImmediate {
                label: "Context Nudge Right".to_owned(),
            },
            ContextMenuAction::TrimToPlayhead => HistoryPostAction::RecordImmediate {
                label: "Context Trim To Playhead".to_owned(),
            },
            ContextMenuAction::Copy
            | ContextMenuAction::SelectAllOnTrack
            | ContextMenuAction::Close => HistoryPostAction::None,
        },
        AppEvent::Timeline(TimelineEvent::PointerReleased(_)) => match state.timeline.interaction {
            TimelineInteraction::DragClip { .. }
            | TimelineInteraction::ResizeClipStart { .. }
            | TimelineInteraction::ResizeClipEnd { .. }
            | TimelineInteraction::AdjustClipParameter { .. } => HistoryPostAction::CommitPending,
            TimelineInteraction::PendingClipDrag { .. }
            | TimelineInteraction::PendingResizeClipStart { .. }
            | TimelineInteraction::PendingResizeClipEnd { .. } => HistoryPostAction::ClearPending,
            TimelineInteraction::Idle
            | TimelineInteraction::PendingBoxSelection { .. }
            | TimelineInteraction::BoxSelecting { .. }
            | TimelineInteraction::ScrubPlayhead => HistoryPostAction::None,
        },
        AppEvent::Tick
        | AppEvent::Undo
        | AppEvent::Redo
        | AppEvent::RefreshVentures
        | AppEvent::SelectVenture(_)
        | AppEvent::SelectRecoverySlot(_)
        | AppEvent::SetVentureDraftName(_)
        | AppEvent::SaveCurrentVenture
        | AppEvent::SaveCurrentVentureAs
        | AppEvent::RenameSelectedVenture
        | AppEvent::LoadSelectedVenture
        | AppEvent::DeleteSelectedVenture
        | AppEvent::RestoreSelectedRecoverySlot
        | AppEvent::AutosaveRecoverySlot(_)
        | AppEvent::CreateNewVenture
        | AppEvent::CopySelectedClips
        | AppEvent::ToggleTransport
        | AppEvent::SetTimelineZoom(_)
        | AppEvent::SetInputModifiers(_)
        | AppEvent::CloseContextMenu
        | AppEvent::SelectCue(_)
        | AppEvent::ArmCue(_)
        | AppEvent::TriggerCue(_)
        | AppEvent::SelectChase(_)
        | AppEvent::SelectChaseStep(_)
        | AppEvent::ToggleChase(_)
        | AppEvent::ReverseChase(_)
        | AppEvent::SelectFx(_)
        | AppEvent::SelectFixtureGroup(_)
        | AppEvent::OpenClipEditor(_)
        | AppEvent::CloseClipEditor
        | AppEvent::SetClipEditorAutomationTarget(_)
        | AppEvent::SelectClipEditorAutomationPoint(_)
        | AppEvent::Timeline(TimelineEvent::PointerMoved(_))
        | AppEvent::Timeline(TimelineEvent::PointerPressed(_))
        | AppEvent::Timeline(TimelineEvent::SecondaryPressed(_))
        | AppEvent::Timeline(TimelineEvent::PointerExited)
        | AppEvent::Timeline(TimelineEvent::Scrolled { .. }) => HistoryPostAction::None,
    }
}

fn begin_timeline_history_transaction(state: &mut StudioState, label: String) {
    clear_pending_history(&mut state.history);
    let before = capture_history_snapshot(state);
    begin_history_transaction(&mut state.history, label, before);
}

fn parameter_history_label(parameter: ClipInlineParameterKind) -> &'static str {
    match parameter {
        ClipInlineParameterKind::Intensity => "Clip Intensity Handle",
        ClipInlineParameterKind::Speed => "Clip Speed Handle",
        ClipInlineParameterKind::FxDepth => "Clip FX Depth Handle",
    }
}

fn sort_track(state: &mut StudioState, track_index: usize) {
    if let Some(track) = state.timeline.tracks.get_mut(track_index) {
        track.clips.sort_by_key(|clip| clip.start.ticks());
    }
}

fn apply_revisions(revisions: &mut crate::core::RenderRevisionState, diffs: &[StateDiff]) {
    for diff in diffs {
        match diff {
            StateDiff::History => {
                revisions.grid += 1;
                revisions.clips += 1;
                revisions.overlay += 1;
                revisions.chrome += 1;
            }
            StateDiff::TimelineViewport => {
                revisions.grid += 1;
                revisions.clips += 1;
                revisions.overlay += 1;
                revisions.chrome += 1;
            }
            StateDiff::Input | StateDiff::Clipboard | StateDiff::Venture => {
                revisions.chrome += 1;
            }
            StateDiff::Selection
            | StateDiff::Hover
            | StateDiff::ClipGeometry(_)
            | StateDiff::TrackMix(_)
            | StateDiff::TimelinePhase
            | StateDiff::ContextMenu
            | StateDiff::Cue(_)
            | StateDiff::Chase(_)
            | StateDiff::Fx(_)
            | StateDiff::Fixture(_) => {
                revisions.clips += 1;
            }
            StateDiff::SnapGuide | StateDiff::Playhead => revisions.overlay += 1,
            StateDiff::ClipEditor => {
                revisions.overlay += 1;
                revisions.chrome += 1;
                revisions.clips += 1;
            }
            StateDiff::StateLifecycle
            | StateDiff::Engine
            | StateDiff::Master
            | StateDiff::EventQueue
            | StateDiff::ReplayLog => {
                revisions.chrome += 1;
            }
            StateDiff::Performance | StateDiff::Validation => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        AppEvent, ChaseId, CueId, FxId, StudioState, TimelineCursor, TimelineEvent, TimelineHit,
        TimelineZone,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn clip_drag_snaps_to_quarter_grid() {
        let mut state = StudioState::default();

        dispatch(
            &mut state,
            AppEvent::Timeline(TimelineEvent::PointerPressed(TimelineCursor {
                beat: BeatTime::from_beats(8),
                track: Some(TrackId(1)),
                zone: TimelineZone::Track,
                target: Some(TimelineHit::ClipBody(ClipId(102))),
                x_px: 180,
                y_px: 82,
            })),
        );

        dispatch(
            &mut state,
            AppEvent::Timeline(TimelineEvent::PointerMoved(TimelineCursor {
                beat: BeatTime::from_beats_f32(9.13),
                track: Some(TrackId(1)),
                zone: TimelineZone::Track,
                target: Some(TimelineHit::ClipBody(ClipId(102))),
                x_px: 220,
                y_px: 82,
            })),
        );

        let clip = state.clip(ClipId(102)).expect("clip exists");
        assert_eq!(clip.start, BeatTime::from_fraction(37, 4));
    }

    #[test]
    fn resize_clip_end_respects_min_duration() {
        let mut state = StudioState::default();

        dispatch(
            &mut state,
            AppEvent::Timeline(TimelineEvent::PointerPressed(TimelineCursor {
                beat: BeatTime::from_fraction(31, 2),
                track: Some(TrackId(2)),
                zone: TimelineZone::Track,
                target: Some(TimelineHit::ClipEndHandle(ClipId(202))),
                x_px: 250,
                y_px: 178,
            })),
        );

        dispatch(
            &mut state,
            AppEvent::Timeline(TimelineEvent::PointerMoved(TimelineCursor {
                beat: BeatTime::from_beats_f32(12.01),
                track: Some(TrackId(2)),
                zone: TimelineZone::Track,
                target: Some(TimelineHit::ClipEndHandle(ClipId(202))),
                x_px: 242,
                y_px: 178,
            })),
        );

        let clip = state.clip(ClipId(202)).expect("clip exists");
        assert!(clip.duration >= MIN_CLIP_DURATION);
    }

    #[test]
    fn scenario_drag_zoom_scrub_keeps_state_valid() {
        let mut state = StudioState::default();
        let events = vec![
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
            AppEvent::SetTimelineZoom(1400),
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

        for event in events {
            dispatch(&mut state, event);
        }

        let report = validate_state(&state);
        assert!(report.valid);
    }

    #[test]
    fn replay_produces_identical_state_snapshot() {
        let script = vec![
            AppEvent::Tick,
            AppEvent::SetMasterIntensity(910),
            AppEvent::SetTimelineZoom(1300),
            AppEvent::Timeline(TimelineEvent::Scrolled {
                delta_lines: -3,
                anchor_x_px: 220,
                anchor_beat: BeatTime::from_beats(8),
            }),
        ];

        let left = replay_events(&script);
        let right = replay_events(&script);

        let left_json = serde_json::to_string(&left).expect("serialize left");
        let right_json = serde_json::to_string(&right).expect("serialize right");
        assert_eq!(left_json, right_json);
    }

    #[test]
    fn pending_clip_drag_requires_hysteresis_before_moving() {
        let mut state = StudioState::default();
        let before = state.clip(ClipId(102)).expect("clip exists").start;

        dispatch(
            &mut state,
            AppEvent::Timeline(TimelineEvent::PointerPressed(TimelineCursor {
                beat: BeatTime::from_beats(8),
                track: Some(TrackId(1)),
                zone: TimelineZone::Track,
                target: Some(TimelineHit::ClipBody(ClipId(102))),
                x_px: 180,
                y_px: 82,
            })),
        );

        dispatch(
            &mut state,
            AppEvent::Timeline(TimelineEvent::PointerMoved(TimelineCursor {
                beat: BeatTime::from_beats_f32(8.03),
                track: Some(TrackId(1)),
                zone: TimelineZone::Track,
                target: Some(TimelineHit::ClipBody(ClipId(102))),
                x_px: 183,
                y_px: 84,
            })),
        );

        assert_eq!(state.clip(ClipId(102)).expect("clip exists").start, before);
        assert_eq!(
            state.timeline.interaction,
            TimelineInteraction::PendingClipDrag {
                clip_id: ClipId(102),
                origin_track: TrackId(1),
                origin_start: before,
                pointer_origin: BeatTime::from_beats(8),
                pointer_origin_x_px: 180,
                pointer_origin_y_px: 82,
            }
        );
    }

    #[test]
    fn box_selection_selects_multiple_clips_deterministically() {
        let mut state = StudioState::default();

        dispatch(
            &mut state,
            AppEvent::Timeline(TimelineEvent::PointerPressed(TimelineCursor {
                beat: BeatTime::from_fraction(1, 5),
                track: Some(TrackId(1)),
                zone: TimelineZone::Track,
                target: None,
                x_px: 10,
                y_px: 44,
            })),
        );

        dispatch(
            &mut state,
            AppEvent::Timeline(TimelineEvent::PointerMoved(TimelineCursor {
                beat: BeatTime::from_beats(9),
                track: Some(TrackId(1)),
                zone: TimelineZone::Track,
                target: Some(TimelineHit::ClipBody(ClipId(102))),
                x_px: 420,
                y_px: 114,
            })),
        );

        assert_eq!(state.timeline.selection, SelectionState::Clip(ClipId(101)));
        assert_eq!(
            state.timeline.selected_clips,
            vec![ClipId(101), ClipId(102)]
        );
        assert!(matches!(
            state.timeline.interaction,
            TimelineInteraction::BoxSelecting { .. }
        ));
    }

    #[test]
    fn track_click_without_box_drag_preserves_track_selection_behavior() {
        let mut state = StudioState::default();

        dispatch(
            &mut state,
            AppEvent::Timeline(TimelineEvent::PointerPressed(TimelineCursor {
                beat: BeatTime::from_beats(10),
                track: Some(TrackId(2)),
                zone: TimelineZone::Track,
                target: None,
                x_px: 20,
                y_px: 144,
            })),
        );

        dispatch(
            &mut state,
            AppEvent::Timeline(TimelineEvent::PointerReleased(TimelineCursor {
                beat: BeatTime::from_beats(10),
                track: Some(TrackId(2)),
                zone: TimelineZone::Track,
                target: None,
                x_px: 22,
                y_px: 145,
            })),
        );

        assert_eq!(state.timeline.selection, SelectionState::Track(TrackId(2)));
        assert!(state.timeline.selected_clips.is_empty());
        assert_eq!(state.engine.transport.playhead, BeatTime::from_beats(10));
    }

    #[test]
    fn scroll_zoom_keeps_anchor_beat_stable() {
        let mut state = StudioState::default();

        dispatch(
            &mut state,
            AppEvent::Timeline(TimelineEvent::Scrolled {
                delta_lines: -4,
                anchor_x_px: 200,
                anchor_beat: BeatTime::from_beats(8),
            }),
        );

        let beat_at_anchor = BeatTime::from_ticks(
            (state.timeline.viewport.scroll.ticks() as f32
                + ((200.0 / pixels_per_beat(&state)) * PPQ as f32))
                .round() as u32,
        );

        assert_eq!(beat_at_anchor, BeatTime::from_beats(8));
    }

    #[test]
    fn locked_snap_guide_holds_until_release_threshold() {
        let mut state = StudioState::default();
        state.timeline.snap.phase = SnapPhase::Locked;
        state.timeline.snap.guide = Some(SnapGuide {
            beat: BeatTime::from_beats(10),
            track: Some(TrackId(1)),
            strength_permille: 1000,
        });

        let (snapped, guide) = snap_value(&state, BeatTime::from_ticks((PPQ * 10) + 154));

        assert_eq!(snapped, BeatTime::from_beats(10));
        assert_eq!(guide.expect("guide exists").beat, BeatTime::from_beats(10));
    }

    #[test]
    fn clip_cue_hotspot_triggers_linked_cue_without_dragging() {
        let mut state = StudioState::default();

        dispatch(
            &mut state,
            AppEvent::Timeline(TimelineEvent::PointerPressed(TimelineCursor {
                beat: BeatTime::from_beats(8),
                track: Some(TrackId(1)),
                zone: TimelineZone::Track,
                target: Some(TimelineHit::ClipCueHotspot(ClipId(102), CueId(1))),
                x_px: 188,
                y_px: 92,
            })),
        );

        assert_eq!(state.timeline.selection, SelectionState::Clip(ClipId(102)));
        assert_eq!(state.timeline.interaction, TimelineInteraction::Idle);
        assert!(matches!(
            state.cue(CueId(1)).expect("cue exists").phase,
            crate::core::CuePhase::Triggered | crate::core::CuePhase::Active
        ));
    }

    #[test]
    fn clip_chase_hotspot_toggles_linked_chase() {
        let mut state = StudioState::default();
        state.chase_system.chases[1].phase = crate::core::ChasePhase::Stopped;

        dispatch(
            &mut state,
            AppEvent::Timeline(TimelineEvent::PointerPressed(TimelineCursor {
                beat: BeatTime::from_beats(9),
                track: Some(TrackId(4)),
                zone: TimelineZone::Track,
                target: Some(TimelineHit::ClipChaseHotspot(ClipId(402), ChaseId(2))),
                x_px: 220,
                y_px: 370,
            })),
        );

        assert_eq!(state.timeline.selection, SelectionState::Clip(ClipId(402)));
        assert!(matches!(
            state.chase(ChaseId(2)).expect("chase exists").phase,
            crate::core::ChasePhase::Playing | crate::core::ChasePhase::Reversing
        ));
    }

    #[test]
    fn clip_fx_hotspot_focuses_fx_from_canvas() {
        let mut state = StudioState::default();

        dispatch(
            &mut state,
            AppEvent::Timeline(TimelineEvent::PointerPressed(TimelineCursor {
                beat: BeatTime::from_beats(8),
                track: Some(TrackId(1)),
                zone: TimelineZone::Track,
                target: Some(TimelineHit::ClipFxHotspot(ClipId(102), FxId(1))),
                x_px: 248,
                y_px: 92,
            })),
        );

        assert_eq!(state.timeline.selection, SelectionState::Clip(ClipId(102)));
        assert_eq!(state.fx_system.selected, Some(FxId(1)));
    }

    #[test]
    fn inline_parameter_drag_updates_clip_intensity_deterministically() {
        let mut state = StudioState::default();

        dispatch(
            &mut state,
            AppEvent::Timeline(TimelineEvent::PointerPressed(TimelineCursor {
                beat: BeatTime::from_beats(8),
                track: Some(TrackId(1)),
                zone: TimelineZone::Track,
                target: Some(TimelineHit::ClipParamHandle(
                    ClipId(102),
                    ClipInlineParameterKind::Intensity,
                )),
                x_px: 284,
                y_px: 58,
            })),
        );

        dispatch(
            &mut state,
            AppEvent::Timeline(TimelineEvent::PointerMoved(TimelineCursor {
                beat: BeatTime::from_beats(8),
                track: Some(TrackId(1)),
                zone: TimelineZone::Track,
                target: Some(TimelineHit::ClipParamHandle(
                    ClipId(102),
                    ClipInlineParameterKind::Intensity,
                )),
                x_px: 284,
                y_px: 100,
            })),
        );

        let clip = state.clip(ClipId(102)).expect("clip exists");
        assert!(clip.params.intensity.permille() < 300);
        assert_eq!(
            state.timeline.interaction,
            TimelineInteraction::AdjustClipParameter {
                clip_id: ClipId(102),
                parameter: ClipInlineParameterKind::Intensity,
            }
        );
    }

    #[test]
    fn undo_restores_clip_drag_as_single_history_step() {
        let mut state = StudioState::default();

        dispatch(
            &mut state,
            AppEvent::Timeline(TimelineEvent::PointerPressed(TimelineCursor {
                beat: BeatTime::from_beats(8),
                track: Some(TrackId(1)),
                zone: TimelineZone::Track,
                target: Some(TimelineHit::ClipBody(ClipId(102))),
                x_px: 180,
                y_px: 82,
            })),
        );
        dispatch(
            &mut state,
            AppEvent::Timeline(TimelineEvent::PointerMoved(TimelineCursor {
                beat: BeatTime::from_fraction(37, 4),
                track: Some(TrackId(1)),
                zone: TimelineZone::Track,
                target: Some(TimelineHit::ClipBody(ClipId(102))),
                x_px: 222,
                y_px: 82,
            })),
        );
        dispatch(
            &mut state,
            AppEvent::Timeline(TimelineEvent::PointerReleased(TimelineCursor {
                beat: BeatTime::from_fraction(37, 4),
                track: Some(TrackId(1)),
                zone: TimelineZone::Track,
                target: Some(TimelineHit::ClipBody(ClipId(102))),
                x_px: 222,
                y_px: 82,
            })),
        );

        assert_eq!(state.history.undo_stack.len(), 1);
        assert_eq!(
            state.clip(ClipId(102)).expect("clip exists").start,
            BeatTime::from_fraction(37, 4)
        );

        dispatch(&mut state, AppEvent::Undo);

        assert_eq!(
            state.clip(ClipId(102)).expect("clip exists").start,
            BeatTime::from_beats(8)
        );
        assert_eq!(state.history.redo_stack.len(), 1);
    }

    #[test]
    fn redo_reapplies_clip_drag_after_undo() {
        let mut state = StudioState::default();

        dispatch(
            &mut state,
            AppEvent::Timeline(TimelineEvent::PointerPressed(TimelineCursor {
                beat: BeatTime::from_beats(8),
                track: Some(TrackId(1)),
                zone: TimelineZone::Track,
                target: Some(TimelineHit::ClipBody(ClipId(102))),
                x_px: 180,
                y_px: 82,
            })),
        );
        dispatch(
            &mut state,
            AppEvent::Timeline(TimelineEvent::PointerMoved(TimelineCursor {
                beat: BeatTime::from_fraction(37, 4),
                track: Some(TrackId(1)),
                zone: TimelineZone::Track,
                target: Some(TimelineHit::ClipBody(ClipId(102))),
                x_px: 222,
                y_px: 82,
            })),
        );
        dispatch(
            &mut state,
            AppEvent::Timeline(TimelineEvent::PointerReleased(TimelineCursor {
                beat: BeatTime::from_fraction(37, 4),
                track: Some(TrackId(1)),
                zone: TimelineZone::Track,
                target: Some(TimelineHit::ClipBody(ClipId(102))),
                x_px: 222,
                y_px: 82,
            })),
        );
        dispatch(&mut state, AppEvent::Undo);
        dispatch(&mut state, AppEvent::Redo);

        assert_eq!(
            state.clip(ClipId(102)).expect("clip exists").start,
            BeatTime::from_fraction(37, 4)
        );
        assert!(state.history.redo_stack.is_empty());
    }

    #[test]
    fn selection_only_clip_click_does_not_create_history_entry() {
        let mut state = StudioState::default();

        dispatch(
            &mut state,
            AppEvent::Timeline(TimelineEvent::PointerPressed(TimelineCursor {
                beat: BeatTime::from_beats(8),
                track: Some(TrackId(1)),
                zone: TimelineZone::Track,
                target: Some(TimelineHit::ClipBody(ClipId(102))),
                x_px: 180,
                y_px: 82,
            })),
        );
        dispatch(
            &mut state,
            AppEvent::Timeline(TimelineEvent::PointerReleased(TimelineCursor {
                beat: BeatTime::from_beats(8),
                track: Some(TrackId(1)),
                zone: TimelineZone::Track,
                target: Some(TimelineHit::ClipBody(ClipId(102))),
                x_px: 181,
                y_px: 82,
            })),
        );

        assert!(state.history.undo_stack.is_empty());
        assert!(state.history.pending.is_none());
    }

    #[test]
    fn new_history_change_clears_redo_stack() {
        let mut state = StudioState::default();

        dispatch(&mut state, AppEvent::SetMasterIntensity(920));
        dispatch(&mut state, AppEvent::Undo);

        assert_eq!(state.history.redo_stack.len(), 1);

        dispatch(&mut state, AppEvent::SetMasterSpeed(760));

        assert!(state.history.redo_stack.is_empty());
        assert_eq!(state.history.undo_stack.len(), 1);
        assert_eq!(
            state.history.undo_stack.last().expect("undo entry").label,
            "Master Speed"
        );
    }

    #[test]
    fn duplicate_selected_clips_preserves_group_offset_and_selects_duplicates() {
        let mut state = StudioState::default();
        state.timeline.selection = SelectionState::Clip(ClipId(101));
        state.timeline.selected_clips = vec![ClipId(101), ClipId(102)];

        dispatch(&mut state, AppEvent::DuplicateSelectedClips);

        let duplicate_a = state.timeline.tracks[0]
            .clips
            .iter()
            .find(|clip| clip.title == "Opener Wash Copy")
            .expect("duplicate clip A");
        let duplicate_b = state.timeline.tracks[0]
            .clips
            .iter()
            .find(|clip| clip.title == "Drop Sweep Copy")
            .expect("duplicate clip B");

        assert_eq!(duplicate_a.start, BeatTime::from_beats(16));
        assert_eq!(duplicate_b.start, BeatTime::from_beats(24));
        assert_eq!(state.timeline.selected_clips.len(), 2);
        assert!(state.history.undo_stack.last().is_some());
    }

    #[test]
    fn split_selected_clip_at_playhead_creates_two_segments() {
        let mut state = StudioState::default();
        state.timeline.selection = SelectionState::Clip(ClipId(102));
        state.timeline.selected_clips = vec![ClipId(102)];
        state.engine.transport.playhead = BeatTime::from_beats(12);

        dispatch(&mut state, AppEvent::SplitSelectedClipsAtPlayhead);

        let left = state.clip(ClipId(102)).expect("left clip exists");
        let right = state.timeline.tracks[0]
            .clips
            .iter()
            .find(|clip| clip.title == "Drop Sweep B")
            .expect("right split clip");

        assert_eq!(left.duration, BeatTime::from_beats(4));
        assert_eq!(right.start, BeatTime::from_beats(12));
        assert_eq!(right.duration, BeatTime::from_beats(4));
        assert_eq!(state.timeline.selected_clips.len(), 2);
    }

    #[test]
    fn delete_selected_clips_clears_reverse_links() {
        let mut state = StudioState::default();
        state.timeline.selection = SelectionState::Clip(ClipId(102));
        state.timeline.selected_clips = vec![ClipId(102)];

        dispatch(&mut state, AppEvent::DeleteSelectedClips);

        assert!(state.clip(ClipId(102)).is_none());
        assert_eq!(state.cue(CueId(1)).expect("cue exists").linked_clip, None);
        assert_eq!(
            state.fx_layer(FxId(1)).expect("fx exists").linked_clip,
            None
        );
        assert_eq!(state.timeline.selection, SelectionState::None);
    }

    #[test]
    fn venture_save_as_and_delete_do_not_pollute_replay_log() {
        let mut state = StudioState::default();
        state.venture.directory = temp_venture_dir("venture-events")
            .to_string_lossy()
            .into_owned();

        dispatch(&mut state, AppEvent::RefreshVentures);
        dispatch(
            &mut state,
            AppEvent::SetVentureDraftName("Main Venture".to_owned()),
        );
        dispatch(&mut state, AppEvent::SaveCurrentVenture);

        let first_saved = state
            .venture
            .selected
            .clone()
            .expect("first venture selected after save");

        dispatch(
            &mut state,
            AppEvent::SetVentureDraftName("Main Venture Copy".to_owned()),
        );
        dispatch(&mut state, AppEvent::SaveCurrentVentureAs);

        assert_eq!(state.venture.ventures.len(), 2);
        assert_ne!(
            state.venture.selected.as_deref(),
            Some(first_saved.as_str())
        );

        dispatch(&mut state, AppEvent::DeleteSelectedVenture);

        assert_eq!(state.venture.ventures.len(), 1);
        assert!(state.venture.selected.is_none());
        assert!(state.replay_log.events.iter().all(|event| !matches!(
            event,
            AppEvent::RefreshVentures
                | AppEvent::SetVentureDraftName(_)
                | AppEvent::SaveCurrentVenture
                | AppEvent::SaveCurrentVentureAs
                | AppEvent::DeleteSelectedVenture
        )));

        let _ = fs::remove_dir_all(&state.venture.directory);
    }

    #[test]
    fn venture_rename_preserves_selected_id() {
        let mut state = StudioState::default();
        state.venture.directory = temp_venture_dir("venture-rename")
            .to_string_lossy()
            .into_owned();

        dispatch(
            &mut state,
            AppEvent::SetVentureDraftName("Rename Source".to_owned()),
        );
        dispatch(&mut state, AppEvent::SaveCurrentVenture);
        let selected_before = state.venture.selected.clone().expect("selected venture");

        dispatch(
            &mut state,
            AppEvent::SetVentureDraftName("Rename Target".to_owned()),
        );
        dispatch(&mut state, AppEvent::RenameSelectedVenture);

        assert_eq!(
            state.venture.selected.as_deref(),
            Some(selected_before.as_str())
        );
        assert_eq!(
            state.selected_venture().expect("renamed venture").name,
            "Rename Target"
        );

        let _ = fs::remove_dir_all(&state.venture.directory);
    }

    #[test]
    fn venture_dirty_state_and_recovery_restore_roundtrip() {
        let mut state = StudioState::default();
        state.venture.directory = temp_venture_dir("venture-dirty")
            .to_string_lossy()
            .into_owned();

        dispatch(
            &mut state,
            AppEvent::SetVentureDraftName("Recovery Demo".to_owned()),
        );
        dispatch(&mut state, AppEvent::SaveCurrentVenture);
        assert!(!state.venture.dirty);

        dispatch(&mut state, AppEvent::SetMasterIntensity(930));

        assert!(state.venture.dirty);
        assert!(!state.venture.recovery_slots.is_empty());

        let autosave_id = state
            .venture
            .selected_recovery
            .clone()
            .expect("autosave recovery selected");
        let intensity_after_edit = state.master.intensity;

        dispatch(&mut state, AppEvent::RestoreSelectedRecoverySlot);

        assert_eq!(state.master.intensity, intensity_after_edit);
        assert!(!state.venture.dirty);
        assert_eq!(
            state.venture.selected_recovery.as_deref(),
            Some(autosave_id.as_str())
        );

        let _ = fs::remove_dir_all(&state.venture.directory);
    }

    fn temp_venture_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "luma-switch-reducer-venture-{}-{}",
            label,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("unix time")
                .as_nanos()
        ))
    }
}
