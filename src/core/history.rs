use crate::core::event::{AppEvent, StateDiff};
use crate::core::state::{
    HistoryEntry, HistoryPhase, HistorySnapshot, HistoryState, HistoryTimelineSnapshot,
    PendingHistoryEntry, SnapPhase, StudioState, TimelineInteraction, TimelinePhase,
};

pub fn capture_history_snapshot(state: &StudioState) -> HistorySnapshot {
    HistorySnapshot {
        master: state.master.clone(),
        timeline: HistoryTimelineSnapshot {
            viewport: state.timeline.viewport.clone(),
            snap_enabled: state.timeline.snap.enabled,
            snap_resolution: state.timeline.snap.resolution,
            tracks: state.timeline.tracks.clone(),
            selection: state.timeline.selection,
            selected_clips: state.timeline.selected_clips.clone(),
        },
        clip_editor: state.clip_editor.clone(),
        cue_system: state.cue_system.clone(),
        chase_system: state.chase_system.clone(),
        fx_system: state.fx_system.clone(),
        fixture_system: state.fixture_system.clone(),
        settings: state.settings.clone(),
    }
}

pub fn begin_history_transaction(state: &mut HistoryState, label: String, before: HistorySnapshot) {
    state.pending = Some(PendingHistoryEntry { label, before });
    state.phase = HistoryPhase::Tracking;
}

pub fn record_history_entry(
    state: &mut HistoryState,
    label: String,
    trigger: AppEvent,
    before: HistorySnapshot,
    after: HistorySnapshot,
) -> bool {
    if before == after {
        state.phase = HistoryPhase::Idle;
        return false;
    }

    state.undo_stack.push(HistoryEntry {
        label,
        trigger,
        before,
        after,
    });
    if state.undo_stack.len() > state.capacity {
        let overflow = state.undo_stack.len() - state.capacity;
        state.undo_stack.drain(0..overflow);
    }
    state.redo_stack.clear();
    state.phase = HistoryPhase::Idle;
    true
}

pub fn commit_history_transaction(
    state: &mut HistoryState,
    trigger: AppEvent,
    after: HistorySnapshot,
) -> bool {
    let Some(pending) = state.pending.take() else {
        return false;
    };

    record_history_entry(state, pending.label, trigger, pending.before, after)
}

pub fn clear_pending_history(state: &mut HistoryState) {
    state.pending = None;
    if state.phase == HistoryPhase::Tracking {
        state.phase = HistoryPhase::Idle;
    }
}

pub fn apply_undo(state: &mut StudioState) -> Vec<StateDiff> {
    let Some(entry) = state.history.undo_stack.pop() else {
        return Vec::new();
    };

    state.history.pending = None;
    let before = entry.before.clone();
    state.history.redo_stack.push(entry);
    if state.history.redo_stack.len() > state.history.capacity {
        let overflow = state.history.redo_stack.len() - state.history.capacity;
        state.history.redo_stack.drain(0..overflow);
    }
    restore_history_snapshot(state, &before);
    state.history.phase = HistoryPhase::UndoApplied;
    vec![StateDiff::History]
}

pub fn apply_redo(state: &mut StudioState) -> Vec<StateDiff> {
    let Some(entry) = state.history.redo_stack.pop() else {
        return Vec::new();
    };

    state.history.pending = None;
    let after = entry.after.clone();
    state.history.undo_stack.push(entry);
    if state.history.undo_stack.len() > state.history.capacity {
        let overflow = state.history.undo_stack.len() - state.history.capacity;
        state.history.undo_stack.drain(0..overflow);
    }
    restore_history_snapshot(state, &after);
    state.history.phase = HistoryPhase::RedoApplied;
    vec![StateDiff::History]
}

pub fn restore_history_snapshot(state: &mut StudioState, snapshot: &HistorySnapshot) {
    state.master = snapshot.master.clone();
    state.timeline.viewport = snapshot.timeline.viewport.clone();
    state.timeline.snap.enabled = snapshot.timeline.snap_enabled;
    state.timeline.snap.resolution = snapshot.timeline.snap_resolution;
    state.timeline.snap.phase = SnapPhase::Free;
    state.timeline.snap.guide = None;
    state.timeline.tracks = snapshot.timeline.tracks.clone();
    state.timeline.selection = snapshot.timeline.selection;
    state.timeline.selected_clips = snapshot.timeline.selected_clips.clone();
    state.timeline.hover = None;
    state.timeline.interaction = TimelineInteraction::Idle;
    state.timeline.phase = TimelinePhase::Rendering;
    state.clip_editor = snapshot.clip_editor.clone();
    state.cue_system = snapshot.cue_system.clone();
    state.chase_system = snapshot.chase_system.clone();
    state.fx_system = snapshot.fx_system.clone();
    state.fixture_system = snapshot.fixture_system.clone();
    state.settings = snapshot.settings.clone();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{AppEvent, ClipId, StudioState};

    #[test]
    fn record_history_entry_clears_redo_and_respects_capacity() {
        let mut history = HistoryState::default();
        history.capacity = 1;
        history.redo_stack.push(HistoryEntry {
            label: "redo".to_owned(),
            trigger: AppEvent::SetMasterIntensity(100),
            before: capture_history_snapshot(&StudioState::default()),
            after: capture_history_snapshot(&StudioState::default()),
        });

        let state = StudioState::default();
        let before = capture_history_snapshot(&state);
        let mut changed = state.clone();
        changed.timeline.selected_clips = vec![ClipId(101)];
        let after = capture_history_snapshot(&changed);

        assert!(record_history_entry(
            &mut history,
            "select".to_owned(),
            AppEvent::OpenClipEditor(ClipId(101)),
            before.clone(),
            after.clone()
        ));
        assert!(history.redo_stack.is_empty());

        let mut changed_again = changed.clone();
        changed_again.timeline.selected_clips = vec![ClipId(102)];
        assert!(record_history_entry(
            &mut history,
            "select-2".to_owned(),
            AppEvent::OpenClipEditor(ClipId(102)),
            after,
            capture_history_snapshot(&changed_again)
        ));
        assert_eq!(history.undo_stack.len(), 1);
        assert_eq!(history.undo_stack[0].label, "select-2");
    }
}
