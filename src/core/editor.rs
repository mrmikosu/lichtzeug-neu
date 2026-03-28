use crate::core::automation::{
    clamp_automation_value, clip_parameter_value, effective_clip_parameters, ensure_lane,
    sort_lane_points,
};
use crate::core::event::StateDiff;
use crate::core::ids::{ChaseId, ClipId, CueId};
use crate::core::state::{
    AutomationInterpolation, AutomationTarget, Clip, ClipEditorPhase, CueVisualState,
    SelectionState, SnapResolution, StudioState,
};
use crate::core::time::{BeatTime, IntensityLevel, SpeedRatio};

pub fn open_clip_editor(state: &mut StudioState, clip_id: ClipId) -> Vec<StateDiff> {
    if state.clip(clip_id).is_none() {
        return Vec::new();
    }

    state.timeline.selection = SelectionState::Clip(clip_id);
    state.timeline.selected_clips = vec![clip_id];
    state.clip_editor.phase = ClipEditorPhase::Open;
    state.clip_editor.clip_id = Some(clip_id);
    state.clip_editor.selected_automation_point = None;

    vec![StateDiff::Selection, StateDiff::ClipEditor]
}

pub fn close_clip_editor(state: &mut StudioState) -> Vec<StateDiff> {
    if state.clip_editor.phase == ClipEditorPhase::Closed {
        return Vec::new();
    }

    state.clip_editor.phase = ClipEditorPhase::Closed;
    state.clip_editor.clip_id = None;
    state.clip_editor.selected_automation_point = None;
    vec![StateDiff::ClipEditor]
}

pub fn set_clip_editor_automation_target(
    state: &mut StudioState,
    target: AutomationTarget,
) -> Vec<StateDiff> {
    if state.clip_editor.clip_id.is_none() {
        return Vec::new();
    }

    state.clip_editor.automation_target = target;
    state.clip_editor.selected_automation_point = None;
    vec![StateDiff::ClipEditor]
}

pub fn toggle_clip_editor_automation_lane(state: &mut StudioState) -> Vec<StateDiff> {
    let Some(clip_id) = state.clip_editor.clip_id else {
        return Vec::new();
    };
    let target = state.clip_editor.automation_target;
    let Some(clip) = clip_mut(state, clip_id) else {
        return Vec::new();
    };

    let lane = ensure_lane(clip, target);
    lane.enabled = !lane.enabled;
    sort_lane_points(lane);
    state.clip_editor.phase = ClipEditorPhase::Previewing;

    vec![StateDiff::ClipGeometry(clip_id), StateDiff::ClipEditor]
}

pub fn set_clip_editor_automation_mode(
    state: &mut StudioState,
    mode: AutomationInterpolation,
) -> Vec<StateDiff> {
    let Some(clip_id) = state.clip_editor.clip_id else {
        return Vec::new();
    };
    let target = state.clip_editor.automation_target;
    let Some(clip) = clip_mut(state, clip_id) else {
        return Vec::new();
    };

    let lane = ensure_lane(clip, target);
    lane.interpolation = mode;
    sort_lane_points(lane);
    state.clip_editor.phase = ClipEditorPhase::Previewing;

    vec![StateDiff::ClipGeometry(clip_id), StateDiff::ClipEditor]
}

pub fn add_clip_editor_automation_point(state: &mut StudioState) -> Vec<StateDiff> {
    let Some(clip_id) = state.clip_editor.clip_id else {
        return Vec::new();
    };
    let Some(clip_snapshot) = state.clip(clip_id).cloned() else {
        return Vec::new();
    };
    let target = state.clip_editor.automation_target;
    let local = state
        .engine
        .transport
        .playhead
        .saturating_sub(clip_snapshot.start)
        .clamp(BeatTime::ZERO, clip_snapshot.duration);
    let value = clip_parameter_value(effective_clip_parameters(&clip_snapshot, local), target);

    let Some(clip) = clip_mut(state, clip_id) else {
        return Vec::new();
    };
    let lane = ensure_lane(clip, target);
    lane.points.push(crate::core::AutomationPoint {
        offset: local,
        value,
    });
    sort_lane_points(lane);
    state.clip_editor.selected_automation_point = lane
        .points
        .iter()
        .position(|point| point.offset == local && point.value == value);
    state.clip_editor.phase = ClipEditorPhase::Previewing;

    vec![StateDiff::ClipGeometry(clip_id), StateDiff::ClipEditor]
}

pub fn select_clip_editor_automation_point(
    state: &mut StudioState,
    index: Option<usize>,
) -> Vec<StateDiff> {
    if state.clip_editor.clip_id.is_none() {
        return Vec::new();
    }

    state.clip_editor.selected_automation_point = index;
    vec![StateDiff::ClipEditor]
}

pub fn set_clip_editor_automation_value(state: &mut StudioState, value: u16) -> Vec<StateDiff> {
    let Some(clip_id) = state.clip_editor.clip_id else {
        return Vec::new();
    };
    let target = state.clip_editor.automation_target;
    let Some(index) = state.clip_editor.selected_automation_point else {
        return Vec::new();
    };
    let Some(clip) = clip_mut(state, clip_id) else {
        return Vec::new();
    };
    let lane = ensure_lane(clip, target);
    if let Some(point) = lane.points.get_mut(index) {
        point.value = clamp_automation_value(target, value);
        sort_lane_points(lane);
        state.clip_editor.phase = ClipEditorPhase::Previewing;
        return vec![StateDiff::ClipGeometry(clip_id), StateDiff::ClipEditor];
    }

    Vec::new()
}

pub fn nudge_clip_editor_automation_point(
    state: &mut StudioState,
    direction: i8,
) -> Vec<StateDiff> {
    let Some(clip_id) = state.clip_editor.clip_id else {
        return Vec::new();
    };
    let target = state.clip_editor.automation_target;
    let Some(index) = state.clip_editor.selected_automation_point else {
        return Vec::new();
    };
    let step = state
        .clip(clip_id)
        .map(|clip| clip.params.bpm_grid.step())
        .unwrap_or_else(|| BeatTime::from_fraction(1, 4));
    let nudge_step = if state.input_modifiers.shift {
        BeatTime::from_ticks((step.ticks() / 2).max(1))
    } else {
        step
    };

    let Some(clip) = clip_mut(state, clip_id) else {
        return Vec::new();
    };
    let duration = clip.duration;
    let lane = ensure_lane(clip, target);
    if let Some(point) = lane.points.get_mut(index) {
        let new_offset = if direction < 0 {
            point.offset.saturating_sub(nudge_step)
        } else {
            point.offset.saturating_add(nudge_step).min(duration)
        };
        point.offset = new_offset;
        let value = point.value;
        sort_lane_points(lane);
        state.clip_editor.selected_automation_point = lane
            .points
            .iter()
            .position(|candidate| candidate.offset == new_offset && candidate.value == value);
        state.clip_editor.phase = ClipEditorPhase::Previewing;
        return vec![StateDiff::ClipGeometry(clip_id), StateDiff::ClipEditor];
    }

    Vec::new()
}

pub fn delete_clip_editor_automation_point(state: &mut StudioState) -> Vec<StateDiff> {
    let Some(clip_id) = state.clip_editor.clip_id else {
        return Vec::new();
    };
    let target = state.clip_editor.automation_target;
    let Some(index) = state.clip_editor.selected_automation_point else {
        return Vec::new();
    };
    let Some(clip) = clip_mut(state, clip_id) else {
        return Vec::new();
    };
    let lane = ensure_lane(clip, target);
    if index >= lane.points.len() {
        return Vec::new();
    }

    lane.points.remove(index);
    sort_lane_points(lane);
    state.clip_editor.selected_automation_point = lane.points.get(index).map(|_| index);
    state.clip_editor.phase = ClipEditorPhase::Previewing;

    vec![StateDiff::ClipGeometry(clip_id), StateDiff::ClipEditor]
}

pub fn set_clip_editor_intensity(state: &mut StudioState, value: u16) -> Vec<StateDiff> {
    update_clip_editor(state, |clip| {
        clip.params.intensity = IntensityLevel::from_permille(value);
    })
}

pub fn set_clip_editor_speed(state: &mut StudioState, value: u16) -> Vec<StateDiff> {
    update_clip_editor(state, |clip| {
        clip.params.speed = SpeedRatio::from_permille(value);
    })
}

pub fn set_clip_editor_fx_depth(state: &mut StudioState, value: u16) -> Vec<StateDiff> {
    update_clip_editor(state, |clip| {
        clip.params.fx_depth = IntensityLevel::from_permille(value);
    })
}

pub fn set_clip_editor_grid(state: &mut StudioState, value: SnapResolution) -> Vec<StateDiff> {
    update_clip_editor(state, |clip| {
        clip.params.bpm_grid = value;
    })
}

pub fn set_clip_editor_cue(state: &mut StudioState, cue_id: Option<CueId>) -> Vec<StateDiff> {
    let visual = cue_id
        .and_then(|id| state.cue(id))
        .map(|cue| CueVisualState::from_phase(cue.phase))
        .unwrap_or(CueVisualState::Inactive);

    let Some(clip_id) = state.clip_editor.clip_id else {
        return Vec::new();
    };

    state.clip_editor.phase = ClipEditorPhase::Adjusting;

    {
        let Some(clip) = clip_mut(state, clip_id) else {
            return Vec::new();
        };

        clip.linked_cue = cue_id;
        clip.cue_state = visual;
    }
    state.clip_editor.phase = ClipEditorPhase::Previewing;

    if let Some(cue_id) = cue_id {
        state.cue_system.selected = Some(cue_id);
        vec![
            StateDiff::ClipGeometry(clip_id),
            StateDiff::ClipEditor,
            StateDiff::Cue(cue_id),
        ]
    } else {
        vec![StateDiff::ClipGeometry(clip_id), StateDiff::ClipEditor]
    }
}

pub fn set_clip_editor_chase(state: &mut StudioState, chase_id: Option<ChaseId>) -> Vec<StateDiff> {
    let Some(clip_id) = state.clip_editor.clip_id else {
        return Vec::new();
    };

    if let Some(chase_id) = chase_id
        && state.chase(chase_id).is_none()
    {
        return Vec::new();
    }

    state.clip_editor.phase = ClipEditorPhase::Adjusting;

    let mut touched = Vec::new();
    for chase in &mut state.chase_system.chases {
        let was_linked = chase.linked_clip == Some(clip_id);
        let should_be_linked = chase_id == Some(chase.id);

        if was_linked && !should_be_linked {
            chase.linked_clip = None;
            touched.push(chase.id);
        } else if should_be_linked {
            if !was_linked {
                chase.linked_clip = Some(clip_id);
            }
            touched.push(chase.id);
            state.chase_system.selected = Some(chase.id);
        }
    }

    if chase_id.is_none()
        && state.chase_system.selected.is_some_and(|selected| {
            state
                .chase(selected)
                .map(|chase| chase.linked_clip != Some(clip_id))
                .unwrap_or(false)
        })
    {
        state.chase_system.selected = None;
    }

    state.clip_editor.phase = ClipEditorPhase::Previewing;

    let mut diffs = vec![StateDiff::ClipEditor];
    for chase_id in touched {
        diffs.push(StateDiff::Chase(chase_id));
    }
    diffs
}

pub fn advance_clip_editor(state: &mut StudioState) -> Vec<StateDiff> {
    match state.clip_editor.phase {
        ClipEditorPhase::Closed => Vec::new(),
        ClipEditorPhase::Open | ClipEditorPhase::Adjusting => {
            if let Some(clip_id) = state.clip_editor.clip_id {
                if state.clip(clip_id).is_none()
                    || state.timeline.selection != SelectionState::Clip(clip_id)
                    || state.timeline.selected_clips != vec![clip_id]
                {
                    return close_clip_editor(state);
                }
            }
            Vec::new()
        }
        ClipEditorPhase::Previewing => {
            state.clip_editor.phase = ClipEditorPhase::Open;
            vec![StateDiff::ClipEditor]
        }
    }
}

fn update_clip_editor(state: &mut StudioState, update: impl FnOnce(&mut Clip)) -> Vec<StateDiff> {
    let Some(clip_id) = state.clip_editor.clip_id else {
        return Vec::new();
    };

    state.clip_editor.phase = ClipEditorPhase::Adjusting;

    {
        let Some(clip) = clip_mut(state, clip_id) else {
            return Vec::new();
        };

        update(clip);
    }
    state.clip_editor.phase = ClipEditorPhase::Previewing;

    vec![StateDiff::ClipGeometry(clip_id), StateDiff::ClipEditor]
}

fn clip_mut(state: &mut StudioState, clip_id: ClipId) -> Option<&mut Clip> {
    let (track_index, clip_index) = state.clip_location(clip_id)?;
    state
        .timeline
        .tracks
        .get_mut(track_index)?
        .clips
        .get_mut(clip_index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{ChaseId, ClipEditorPhase, CueId, StudioState};

    #[test]
    fn opening_clip_editor_selects_clip() {
        let mut state = StudioState::default();

        let diffs = open_clip_editor(&mut state, ClipId(201));

        assert!(diffs.contains(&StateDiff::ClipEditor));
        assert_eq!(state.clip_editor.phase, ClipEditorPhase::Open);
        assert_eq!(state.clip_editor.clip_id, Some(ClipId(201)));
        assert_eq!(state.timeline.selection, SelectionState::Clip(ClipId(201)));
    }

    #[test]
    fn clip_editor_parameter_change_enters_previewing() {
        let mut state = StudioState::default();
        open_clip_editor(&mut state, ClipId(102));

        let diffs = set_clip_editor_fx_depth(&mut state, 930);

        assert!(diffs.contains(&StateDiff::ClipEditor));
        assert_eq!(state.clip_editor.phase, ClipEditorPhase::Previewing);
        assert_eq!(
            state
                .clip(ClipId(102))
                .expect("clip exists")
                .params
                .fx_depth
                .permille(),
            930
        );
    }

    #[test]
    fn clip_editor_can_relink_cue() {
        let mut state = StudioState::default();
        open_clip_editor(&mut state, ClipId(101));

        let diffs = set_clip_editor_cue(&mut state, Some(CueId(3)));

        assert!(diffs.contains(&StateDiff::Cue(CueId(3))));
        assert_eq!(
            state.clip(ClipId(101)).expect("clip exists").linked_cue,
            Some(CueId(3))
        );
        assert_eq!(state.cue_system.selected, Some(CueId(3)));
    }

    #[test]
    fn clip_editor_can_relink_chase() {
        let mut state = StudioState::default();
        open_clip_editor(&mut state, ClipId(101));

        let diffs = set_clip_editor_chase(&mut state, Some(ChaseId(2)));

        assert!(diffs.contains(&StateDiff::Chase(ChaseId(2))));
        assert_eq!(
            state.chase(ChaseId(2)).expect("chase exists").linked_clip,
            Some(ClipId(101))
        );
        assert_eq!(state.chase_system.selected, Some(ChaseId(2)));
        assert!(
            state
                .chase_system
                .chases
                .iter()
                .filter(|chase| chase.linked_clip == Some(ClipId(101)))
                .count()
                == 1
        );
    }
}
