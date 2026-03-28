use crate::core::automation::effective_clip_parameters;
use crate::core::event::StateDiff;
use crate::core::ids::{ChaseId, CueId, FixtureGroupId, FxId};
use crate::core::state::{
    Chase, ChaseDirection, ChasePhase, ChaseStep, Cue, CuePhase, CueVisualState, FixturePhase,
    FxPhase, FxWaveform, MIN_CLIP_DURATION, StudioState,
};
use crate::core::time::{BeatTime, RgbaColor, SpeedRatio};
use std::f32::consts::TAU;

pub fn select_cue(state: &mut StudioState, cue_id: CueId) -> Vec<StateDiff> {
    if state.cue(cue_id).is_none() {
        return Vec::new();
    }

    state.cue_system.selected = Some(cue_id);
    vec![StateDiff::Cue(cue_id)]
}

pub fn create_cue(state: &mut StudioState) -> Vec<StateDiff> {
    let cue_id = state.next_cue_id();
    let color = state
        .selected_cue()
        .map(|cue| cue.color)
        .unwrap_or(RgbaColor::rgb(255, 196, 120));
    let fade_duration = state
        .selected_cue()
        .map(|cue| cue.fade_duration)
        .unwrap_or(BeatTime::from_fraction(1, 2));

    state.cue_system.cues.push(Cue {
        id: cue_id,
        name: format!("Cue {}", cue_id.0),
        phase: CuePhase::Stored,
        linked_clip: None,
        color,
        fade_duration,
        elapsed: BeatTime::ZERO,
    });
    state.cue_system.selected = Some(cue_id);

    vec![StateDiff::Cue(cue_id)]
}

pub fn delete_selected_cue(state: &mut StudioState) -> Vec<StateDiff> {
    let Some(cue_id) = state.cue_system.selected else {
        return Vec::new();
    };
    let Some(index) = state
        .cue_system
        .cues
        .iter()
        .position(|cue| cue.id == cue_id)
    else {
        return Vec::new();
    };

    state.cue_system.cues.remove(index);
    state.cue_system.selected = state
        .cue_system
        .cues
        .get(index)
        .or_else(|| {
            index
                .checked_sub(1)
                .and_then(|prev| state.cue_system.cues.get(prev))
        })
        .map(|cue| cue.id);

    let mut diffs = vec![StateDiff::Cue(cue_id)];

    for track in &mut state.timeline.tracks {
        for clip in &mut track.clips {
            if clip.linked_cue == Some(cue_id) {
                clip.linked_cue = None;
                clip.cue_state = CueVisualState::Inactive;
                diffs.push(StateDiff::ClipGeometry(clip.id));
            }
        }
    }

    for chase in &mut state.chase_system.chases {
        let mut changed = false;
        for step in &mut chase.steps {
            if step.cue_id == Some(cue_id) {
                step.cue_id = None;
                changed = true;
            }
        }
        if changed {
            diffs.push(StateDiff::Chase(chase.id));
        }
    }

    for group in &mut state.fixture_system.groups {
        if group.linked_cue == Some(cue_id) {
            group.linked_cue = None;
            diffs.push(StateDiff::Fixture(group.id));
        }
    }

    state.cue_system.active = state
        .cue_system
        .cues
        .iter()
        .find(|cue| matches!(cue.phase, CuePhase::Triggered | CuePhase::Active))
        .map(|cue| cue.id);

    sync_clip_cue_states(state);
    diffs.extend(refresh_fixture_groups(state));
    diffs.push(StateDiff::ClipEditor);
    diffs
}

pub fn set_selected_cue_name(state: &mut StudioState, name: String) -> Vec<StateDiff> {
    let Some(cue_id) = state.cue_system.selected else {
        return Vec::new();
    };
    let Some(cue) = state
        .cue_system
        .cues
        .iter_mut()
        .find(|cue| cue.id == cue_id)
    else {
        return Vec::new();
    };

    if cue.name == name {
        return Vec::new();
    }

    cue.name = name;
    vec![StateDiff::Cue(cue_id)]
}

pub fn set_selected_cue_color(state: &mut StudioState, color: RgbaColor) -> Vec<StateDiff> {
    let Some(cue_id) = state.cue_system.selected else {
        return Vec::new();
    };
    let Some(cue) = state
        .cue_system
        .cues
        .iter_mut()
        .find(|cue| cue.id == cue_id)
    else {
        return Vec::new();
    };

    if cue.color == color {
        return Vec::new();
    }

    cue.color = color;
    vec![StateDiff::Cue(cue_id)]
}

pub fn set_selected_cue_fade_duration(
    state: &mut StudioState,
    fade_duration: BeatTime,
) -> Vec<StateDiff> {
    let Some(cue_id) = state.cue_system.selected else {
        return Vec::new();
    };
    let Some(cue) = state
        .cue_system
        .cues
        .iter_mut()
        .find(|cue| cue.id == cue_id)
    else {
        return Vec::new();
    };

    if cue.fade_duration == fade_duration {
        return Vec::new();
    }

    cue.fade_duration = fade_duration;
    vec![StateDiff::Cue(cue_id)]
}

pub fn arm_cue(state: &mut StudioState, cue_id: CueId) -> Vec<StateDiff> {
    let Some(index) = state
        .cue_system
        .cues
        .iter()
        .position(|cue| cue.id == cue_id)
    else {
        return Vec::new();
    };

    state.cue_system.selected = Some(cue_id);
    if !matches!(state.cue_system.cues[index].phase, CuePhase::Active) {
        state.cue_system.cues[index].phase = CuePhase::Armed;
        state.cue_system.cues[index].elapsed = BeatTime::ZERO;
    }

    sync_clip_cue_states(state);
    refresh_fixture_groups(state);

    vec![StateDiff::Cue(cue_id)]
}

pub fn trigger_cue(state: &mut StudioState, cue_id: CueId) -> Vec<StateDiff> {
    trigger_cue_internal(state, cue_id)
}

pub fn select_chase(state: &mut StudioState, chase_id: ChaseId) -> Vec<StateDiff> {
    let Some(selected_step) = state.chase(chase_id).map(|chase| {
        (!chase.steps.is_empty())
            .then_some(chase.current_step.min(chase.steps.len().saturating_sub(1)))
    }) else {
        return Vec::new();
    };

    state.chase_system.selected = Some(chase_id);
    state.chase_system.selected_step = selected_step;
    vec![StateDiff::Chase(chase_id)]
}

pub fn create_chase(state: &mut StudioState) -> Vec<StateDiff> {
    let chase_id = state.next_chase_id();
    let cue_id = state.cue_system.selected;
    let color = cue_id
        .and_then(|selected| state.cue(selected))
        .map(|cue| cue.color)
        .unwrap_or(RgbaColor::rgb(204, 218, 255));
    let name = format!("Chase {}", chase_id.0);

    state.chase_system.chases.push(Chase {
        id: chase_id,
        name,
        phase: ChasePhase::Stopped,
        direction: ChaseDirection::Forward,
        current_step: 0,
        progress: BeatTime::ZERO,
        loop_enabled: true,
        linked_clip: None,
        steps: vec![ChaseStep {
            label: "Step 1".to_owned(),
            cue_id,
            duration: BeatTime::from_fraction(1, 2),
            color,
        }],
    });
    state.chase_system.selected = Some(chase_id);
    state.chase_system.selected_step = Some(0);

    vec![StateDiff::Chase(chase_id)]
}

pub fn delete_selected_chase(state: &mut StudioState) -> Vec<StateDiff> {
    let Some(chase_id) = state.chase_system.selected else {
        return Vec::new();
    };
    let Some(index) = state
        .chase_system
        .chases
        .iter()
        .position(|chase| chase.id == chase_id)
    else {
        return Vec::new();
    };

    state.chase_system.chases.remove(index);
    state.chase_system.selected = state
        .chase_system
        .chases
        .get(index)
        .or_else(|| {
            index
                .checked_sub(1)
                .and_then(|prev| state.chase_system.chases.get(prev))
        })
        .map(|chase| chase.id);
    state.chase_system.selected_step = state
        .chase_system
        .selected
        .and_then(|selected| state.chase(selected))
        .and_then(|chase| {
            (!chase.steps.is_empty()).then_some(chase.current_step.min(chase.steps.len() - 1))
        });

    vec![StateDiff::Chase(chase_id), StateDiff::ClipEditor]
}

pub fn set_selected_chase_name(state: &mut StudioState, name: String) -> Vec<StateDiff> {
    let Some(chase_id) = state.chase_system.selected else {
        return Vec::new();
    };
    let Some(chase) = state
        .chase_system
        .chases
        .iter_mut()
        .find(|chase| chase.id == chase_id)
    else {
        return Vec::new();
    };

    if chase.name == name {
        return Vec::new();
    }

    chase.name = name;
    vec![StateDiff::Chase(chase_id)]
}

pub fn set_selected_chase_direction(
    state: &mut StudioState,
    direction: ChaseDirection,
) -> Vec<StateDiff> {
    let Some(chase_id) = state.chase_system.selected else {
        return Vec::new();
    };
    let Some(chase) = state
        .chase_system
        .chases
        .iter_mut()
        .find(|chase| chase.id == chase_id)
    else {
        return Vec::new();
    };

    if chase.direction == direction {
        return Vec::new();
    }

    chase.direction = direction;
    if matches!(
        chase.phase,
        ChasePhase::Playing | ChasePhase::Looping | ChasePhase::Reversing
    ) {
        chase.phase = match direction {
            ChaseDirection::Forward => ChasePhase::Playing,
            ChaseDirection::Reverse => ChasePhase::Reversing,
        };
    }

    vec![StateDiff::Chase(chase_id)]
}

pub fn set_selected_chase_loop(state: &mut StudioState, loop_enabled: bool) -> Vec<StateDiff> {
    let Some(chase_id) = state.chase_system.selected else {
        return Vec::new();
    };
    let Some(chase) = state
        .chase_system
        .chases
        .iter_mut()
        .find(|chase| chase.id == chase_id)
    else {
        return Vec::new();
    };

    if chase.loop_enabled == loop_enabled {
        return Vec::new();
    }

    chase.loop_enabled = loop_enabled;
    vec![StateDiff::Chase(chase_id)]
}

pub fn select_chase_step(state: &mut StudioState, index: Option<usize>) -> Vec<StateDiff> {
    let Some((chase_id, step_count)) = state
        .selected_chase()
        .map(|chase| (chase.id, chase.steps.len()))
    else {
        state.chase_system.selected_step = None;
        return Vec::new();
    };

    let Some(index) = index else {
        state.chase_system.selected_step = None;
        return vec![StateDiff::Chase(chase_id)];
    };
    if index >= step_count {
        return Vec::new();
    }

    if state.chase_system.selected_step == Some(index) {
        return Vec::new();
    }

    state.chase_system.selected_step = Some(index);
    vec![StateDiff::Chase(chase_id)]
}

pub fn add_selected_chase_step(state: &mut StudioState) -> Vec<StateDiff> {
    let Some(chase_id) = state.chase_system.selected else {
        return Vec::new();
    };

    let selected_cue = state.cue_system.selected;
    let selected_color = selected_cue
        .and_then(|cue_id| state.cue(cue_id))
        .map(|cue| cue.color);

    let Some(chase) = state
        .chase_system
        .chases
        .iter_mut()
        .find(|chase| chase.id == chase_id)
    else {
        return Vec::new();
    };

    let insert_at = state
        .chase_system
        .selected_step
        .map(|index| (index + 1).min(chase.steps.len()))
        .unwrap_or(chase.steps.len());
    let inherited = chase.steps.get(insert_at.saturating_sub(1));
    let step = ChaseStep {
        label: format!("Step {}", insert_at + 1),
        cue_id: selected_cue.or_else(|| inherited.and_then(|step| step.cue_id)),
        duration: inherited
            .map(|step| step.duration)
            .unwrap_or(BeatTime::from_fraction(1, 2)),
        color: selected_color
            .or_else(|| inherited.map(|step| step.color))
            .unwrap_or(RgbaColor::rgb(204, 218, 255)),
    };

    chase.steps.insert(insert_at, step);
    if chase.current_step >= insert_at {
        chase.current_step += 1;
    }
    state.chase_system.selected_step = Some(insert_at);

    vec![StateDiff::Chase(chase_id)]
}

pub fn delete_selected_chase_step(state: &mut StudioState) -> Vec<StateDiff> {
    let Some(chase_id) = state.chase_system.selected else {
        return Vec::new();
    };
    let Some(selected_step) = state.chase_system.selected_step else {
        return Vec::new();
    };
    let Some(chase) = state
        .chase_system
        .chases
        .iter_mut()
        .find(|chase| chase.id == chase_id)
    else {
        return Vec::new();
    };
    if chase.steps.len() <= 1 || selected_step >= chase.steps.len() {
        return Vec::new();
    }

    chase.steps.remove(selected_step);
    if chase.current_step > selected_step {
        chase.current_step -= 1;
    } else if chase.current_step >= chase.steps.len() {
        chase.current_step = chase.steps.len().saturating_sub(1);
    }
    state.chase_system.selected_step = Some(selected_step.min(chase.steps.len() - 1));

    vec![StateDiff::Chase(chase_id)]
}

pub fn move_selected_chase_step(state: &mut StudioState, direction: i8) -> Vec<StateDiff> {
    let Some(chase_id) = state.chase_system.selected else {
        return Vec::new();
    };
    let Some(selected_step) = state.chase_system.selected_step else {
        return Vec::new();
    };
    let Some(chase) = state
        .chase_system
        .chases
        .iter_mut()
        .find(|chase| chase.id == chase_id)
    else {
        return Vec::new();
    };

    let target_index = if direction < 0 {
        selected_step.checked_sub(1)
    } else if selected_step + 1 < chase.steps.len() {
        Some(selected_step + 1)
    } else {
        None
    };
    let Some(target_index) = target_index else {
        return Vec::new();
    };

    chase.steps.swap(selected_step, target_index);
    if chase.current_step == selected_step {
        chase.current_step = target_index;
    } else if chase.current_step == target_index {
        chase.current_step = selected_step;
    }
    state.chase_system.selected_step = Some(target_index);

    vec![StateDiff::Chase(chase_id)]
}

pub fn set_selected_chase_step_label(state: &mut StudioState, label: String) -> Vec<StateDiff> {
    let Some((chase_index, step_index, chase_id)) = selected_chase_step_locator(state) else {
        return Vec::new();
    };
    let step = &mut state.chase_system.chases[chase_index].steps[step_index];
    if step.label == label {
        return Vec::new();
    }

    step.label = label;
    vec![StateDiff::Chase(chase_id)]
}

pub fn set_selected_chase_step_cue(
    state: &mut StudioState,
    cue_id: Option<CueId>,
) -> Vec<StateDiff> {
    if cue_id.is_some_and(|cue_id| state.cue(cue_id).is_none()) {
        return Vec::new();
    }

    let Some((chase_index, step_index, chase_id)) = selected_chase_step_locator(state) else {
        return Vec::new();
    };
    let step = &mut state.chase_system.chases[chase_index].steps[step_index];
    if step.cue_id == cue_id {
        return Vec::new();
    }

    step.cue_id = cue_id;
    let _ = step;
    if let Some(cue_id) = cue_id {
        state.cue_system.selected = Some(cue_id);
    }
    vec![StateDiff::Chase(chase_id)]
}

pub fn set_selected_chase_step_duration(
    state: &mut StudioState,
    duration: BeatTime,
) -> Vec<StateDiff> {
    let Some((chase_index, step_index, chase_id)) = selected_chase_step_locator(state) else {
        return Vec::new();
    };
    let duration = duration.max(MIN_CLIP_DURATION);
    let step = &mut state.chase_system.chases[chase_index].steps[step_index];
    if step.duration == duration {
        return Vec::new();
    }

    step.duration = duration;
    vec![StateDiff::Chase(chase_id)]
}

pub fn set_selected_chase_step_color(state: &mut StudioState, color: RgbaColor) -> Vec<StateDiff> {
    let Some((chase_index, step_index, chase_id)) = selected_chase_step_locator(state) else {
        return Vec::new();
    };
    let step = &mut state.chase_system.chases[chase_index].steps[step_index];
    if step.color == color {
        return Vec::new();
    }

    step.color = color;
    vec![StateDiff::Chase(chase_id)]
}

pub fn toggle_chase(state: &mut StudioState, chase_id: ChaseId) -> Vec<StateDiff> {
    let Some(index) = state
        .chase_system
        .chases
        .iter()
        .position(|chase| chase.id == chase_id)
    else {
        return Vec::new();
    };

    state.chase_system.selected = Some(chase_id);
    let mut cue_to_trigger = None;

    {
        let chase = &mut state.chase_system.chases[index];
        match chase.phase {
            ChasePhase::Playing | ChasePhase::Looping | ChasePhase::Reversing => {
                chase.phase = ChasePhase::Stopped;
                chase.progress = BeatTime::ZERO;
            }
            ChasePhase::Idle | ChasePhase::Stopped => {
                chase.phase = match chase.direction {
                    ChaseDirection::Forward => ChasePhase::Playing,
                    ChaseDirection::Reverse => ChasePhase::Reversing,
                };
                chase.progress = BeatTime::ZERO;
                cue_to_trigger = chase
                    .steps
                    .get(chase.current_step)
                    .and_then(|step| step.cue_id);
            }
        }
    }

    let mut diffs = vec![StateDiff::Chase(chase_id)];
    if let Some(cue_id) = cue_to_trigger {
        diffs.extend(trigger_cue_internal(state, cue_id));
    } else {
        refresh_fixture_groups(state);
    }

    diffs
}

pub fn reverse_chase(state: &mut StudioState, chase_id: ChaseId) -> Vec<StateDiff> {
    let Some(index) = state
        .chase_system
        .chases
        .iter()
        .position(|chase| chase.id == chase_id)
    else {
        return Vec::new();
    };

    state.chase_system.selected = Some(chase_id);
    let chase = &mut state.chase_system.chases[index];
    chase.direction = match chase.direction {
        ChaseDirection::Forward => ChaseDirection::Reverse,
        ChaseDirection::Reverse => ChaseDirection::Forward,
    };
    chase.phase = match chase.direction {
        ChaseDirection::Forward => ChasePhase::Playing,
        ChaseDirection::Reverse => ChasePhase::Reversing,
    };
    chase.progress = BeatTime::ZERO;

    vec![StateDiff::Chase(chase_id)]
}

fn selected_chase_step_locator(state: &StudioState) -> Option<(usize, usize, ChaseId)> {
    let chase_id = state.chase_system.selected?;
    let selected_step = state.chase_system.selected_step?;
    let chase_index = state
        .chase_system
        .chases
        .iter()
        .position(|chase| chase.id == chase_id)?;
    (selected_step < state.chase_system.chases[chase_index].steps.len()).then_some((
        chase_index,
        selected_step,
        chase_id,
    ))
}

pub fn select_fx(state: &mut StudioState, fx_id: FxId) -> Vec<StateDiff> {
    if state.fx_system.layers.iter().any(|layer| layer.id == fx_id) {
        state.fx_system.selected = Some(fx_id);
        return vec![StateDiff::Fx(fx_id)];
    }

    Vec::new()
}

pub fn toggle_fx(state: &mut StudioState, fx_id: FxId) -> Vec<StateDiff> {
    let Some(index) = state
        .fx_system
        .layers
        .iter()
        .position(|layer| layer.id == fx_id)
    else {
        return Vec::new();
    };

    let layer = &mut state.fx_system.layers[index];
    layer.enabled = !layer.enabled;
    layer.phase = if layer.enabled {
        FxPhase::Processing
    } else {
        layer.output_level = 0;
        FxPhase::Idle
    };
    state.fx_system.selected = Some(fx_id);
    refresh_fixture_groups(state);

    vec![StateDiff::Fx(fx_id)]
}

pub fn set_fx_depth(state: &mut StudioState, fx_id: FxId, depth_permille: u16) -> Vec<StateDiff> {
    let Some(index) = state
        .fx_system
        .layers
        .iter()
        .position(|layer| layer.id == fx_id)
    else {
        return Vec::new();
    };

    let layer = &mut state.fx_system.layers[index];
    layer.depth_permille = depth_permille.min(1000);
    layer.phase = if layer.enabled {
        FxPhase::Processing
    } else {
        FxPhase::Idle
    };
    state.fx_system.selected = Some(fx_id);
    refresh_fixture_groups(state);

    vec![StateDiff::Fx(fx_id)]
}

pub fn set_fx_rate(state: &mut StudioState, fx_id: FxId, rate_permille: u16) -> Vec<StateDiff> {
    let Some(index) = state
        .fx_system
        .layers
        .iter()
        .position(|layer| layer.id == fx_id)
    else {
        return Vec::new();
    };

    let layer = &mut state.fx_system.layers[index];
    layer.rate = SpeedRatio::from_permille(rate_permille);
    layer.phase = if layer.enabled {
        FxPhase::Processing
    } else {
        FxPhase::Idle
    };
    state.fx_system.selected = Some(fx_id);
    refresh_fixture_groups(state);

    vec![StateDiff::Fx(fx_id)]
}

pub fn set_fx_spread(state: &mut StudioState, fx_id: FxId, spread_permille: u16) -> Vec<StateDiff> {
    let Some(index) = state
        .fx_system
        .layers
        .iter()
        .position(|layer| layer.id == fx_id)
    else {
        return Vec::new();
    };

    let layer = &mut state.fx_system.layers[index];
    layer.spread_permille = spread_permille.min(1000);
    layer.phase = if layer.enabled {
        FxPhase::Processing
    } else {
        FxPhase::Idle
    };
    state.fx_system.selected = Some(fx_id);
    refresh_fixture_groups(state);

    vec![StateDiff::Fx(fx_id)]
}

pub fn set_fx_phase_offset(
    state: &mut StudioState,
    fx_id: FxId,
    phase_offset_permille: u16,
) -> Vec<StateDiff> {
    let Some(index) = state
        .fx_system
        .layers
        .iter()
        .position(|layer| layer.id == fx_id)
    else {
        return Vec::new();
    };

    let layer = &mut state.fx_system.layers[index];
    layer.phase_offset_permille = phase_offset_permille.min(1000);
    layer.phase = if layer.enabled {
        FxPhase::Processing
    } else {
        FxPhase::Idle
    };
    state.fx_system.selected = Some(fx_id);
    refresh_fixture_groups(state);

    vec![StateDiff::Fx(fx_id)]
}

pub fn set_fx_waveform(
    state: &mut StudioState,
    fx_id: FxId,
    waveform: FxWaveform,
) -> Vec<StateDiff> {
    let Some(index) = state
        .fx_system
        .layers
        .iter()
        .position(|layer| layer.id == fx_id)
    else {
        return Vec::new();
    };

    let layer = &mut state.fx_system.layers[index];
    layer.waveform = waveform;
    layer.phase = if layer.enabled {
        FxPhase::Processing
    } else {
        FxPhase::Idle
    };
    state.fx_system.selected = Some(fx_id);
    refresh_fixture_groups(state);

    vec![StateDiff::Fx(fx_id)]
}

pub fn select_fixture_group(
    state: &mut StudioState,
    fixture_group_id: FixtureGroupId,
) -> Vec<StateDiff> {
    if state
        .fixture_system
        .groups
        .iter()
        .any(|group| group.id == fixture_group_id)
    {
        state.fixture_system.selected = Some(fixture_group_id);
        return vec![StateDiff::Fixture(fixture_group_id)];
    }

    Vec::new()
}

pub fn advance_show_frame(state: &mut StudioState, delta: BeatTime) -> Vec<StateDiff> {
    let mut diffs = Vec::new();

    diffs.extend(advance_cues(state, delta));
    diffs.extend(advance_chases(state, delta));
    diffs.extend(advance_fx_layers(state));
    diffs.extend(refresh_fixture_groups(state));
    sync_clip_cue_states(state);

    diffs
}

fn trigger_cue_internal(state: &mut StudioState, cue_id: CueId) -> Vec<StateDiff> {
    let Some(target_index) = state
        .cue_system
        .cues
        .iter()
        .position(|cue| cue.id == cue_id)
    else {
        return Vec::new();
    };

    let mut diffs = Vec::new();

    for cue in &mut state.cue_system.cues {
        if cue.id == cue_id {
            cue.phase = CuePhase::Triggered;
            cue.elapsed = BeatTime::ZERO;
        } else if matches!(cue.phase, CuePhase::Active | CuePhase::Triggered) {
            cue.phase = CuePhase::Fading;
            cue.elapsed = BeatTime::ZERO;
            diffs.push(StateDiff::Cue(cue.id));
        } else if cue.phase == CuePhase::Fading {
            cue.elapsed = BeatTime::ZERO;
        }
    }

    state.cue_system.selected = Some(cue_id);
    state.cue_system.active = Some(state.cue_system.cues[target_index].id);
    diffs.push(StateDiff::Cue(cue_id));

    sync_clip_cue_states(state);
    diffs.extend(refresh_fixture_groups(state));
    diffs
}

fn advance_cues(state: &mut StudioState, delta: BeatTime) -> Vec<StateDiff> {
    let mut diffs = Vec::new();

    for cue in &mut state.cue_system.cues {
        let before = cue.phase;
        match cue.phase {
            CuePhase::Triggered => {
                cue.phase = CuePhase::Active;
                cue.elapsed = BeatTime::ZERO;
            }
            CuePhase::Fading => {
                cue.elapsed = cue.elapsed.saturating_add(delta);
                if cue.elapsed >= cue.fade_duration {
                    cue.phase = CuePhase::Stored;
                    cue.elapsed = BeatTime::ZERO;
                }
            }
            CuePhase::Stored | CuePhase::Armed | CuePhase::Active => {}
        }

        if cue.phase != before {
            diffs.push(StateDiff::Cue(cue.id));
        }
    }

    state.cue_system.active = state
        .cue_system
        .cues
        .iter()
        .find(|cue| matches!(cue.phase, CuePhase::Triggered | CuePhase::Active))
        .map(|cue| cue.id);

    diffs
}

fn advance_chases(state: &mut StudioState, delta: BeatTime) -> Vec<StateDiff> {
    let mut diffs = Vec::new();
    let mut triggered_cues = Vec::new();
    let clip_speeds = state
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.clips.iter())
        .map(|clip| {
            let local = state
                .engine
                .transport
                .playhead
                .saturating_sub(clip.start)
                .clamp(BeatTime::ZERO, clip.duration);
            let effective = effective_clip_parameters(clip, local);
            (clip.id, effective.speed.permille())
        })
        .collect::<Vec<_>>();

    for chase in &mut state.chase_system.chases {
        if !matches!(
            chase.phase,
            ChasePhase::Playing | ChasePhase::Looping | ChasePhase::Reversing
        ) {
            continue;
        }

        if chase.steps.is_empty() {
            chase.phase = ChasePhase::Stopped;
            diffs.push(StateDiff::Chase(chase.id));
            continue;
        }

        let scaled_delta = chase
            .linked_clip
            .and_then(|clip_id| {
                clip_speeds
                    .iter()
                    .find_map(|(id, speed)| (*id == clip_id).then_some(*speed))
            })
            .map(|speed| {
                BeatTime::from_ticks(((delta.ticks() as u64 * speed as u64) / 1000) as u32)
            })
            .unwrap_or(delta);

        chase.progress = chase.progress.saturating_add(scaled_delta);
        let mut changed = false;

        loop {
            let step_duration = chase.steps[chase.current_step].duration;
            if chase.progress < step_duration {
                break;
            }

            chase.progress = chase.progress.saturating_sub(step_duration);
            changed = true;

            match chase.direction {
                ChaseDirection::Forward => {
                    if chase.current_step + 1 < chase.steps.len() {
                        chase.current_step += 1;
                        chase.phase = ChasePhase::Playing;
                    } else if chase.loop_enabled {
                        chase.current_step = 0;
                        chase.phase = ChasePhase::Looping;
                    } else {
                        chase.phase = ChasePhase::Stopped;
                        chase.progress = BeatTime::ZERO;
                        break;
                    }
                }
                ChaseDirection::Reverse => {
                    if chase.current_step > 0 {
                        chase.current_step -= 1;
                        chase.phase = ChasePhase::Reversing;
                    } else if chase.loop_enabled {
                        chase.current_step = chase.steps.len().saturating_sub(1);
                        chase.phase = ChasePhase::Reversing;
                    } else {
                        chase.phase = ChasePhase::Stopped;
                        chase.progress = BeatTime::ZERO;
                        break;
                    }
                }
            }

            if let Some(cue_id) = chase.steps[chase.current_step].cue_id {
                triggered_cues.push(cue_id);
            }
        }

        if changed {
            diffs.push(StateDiff::Chase(chase.id));
        }
    }

    for cue_id in triggered_cues {
        diffs.extend(trigger_cue_internal(state, cue_id));
    }

    diffs
}

fn advance_fx_layers(state: &mut StudioState) -> Vec<StateDiff> {
    let enabled_count = state
        .fx_system
        .layers
        .iter()
        .filter(|layer| layer.enabled)
        .count();
    let clip_profiles = state
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.clips.iter())
        .map(|clip| {
            let local = state
                .engine
                .transport
                .playhead
                .saturating_sub(clip.start)
                .clamp(BeatTime::ZERO, clip.duration);
            let effective = effective_clip_parameters(clip, local);
            (
                clip.id,
                matches!(
                    clip.phase,
                    crate::core::ClipPhase::Triggered | crate::core::ClipPhase::Active
                ),
                effective.fx_depth.permille(),
                effective.speed.permille(),
            )
        })
        .collect::<Vec<_>>();
    let frame_index = state.performance.frame_index;
    let master_intensity = state.master.intensity.permille();
    let mut diffs = Vec::new();

    for layer in &mut state.fx_system.layers {
        let clip_state = layer.linked_clip.and_then(|clip_id| {
            clip_profiles.iter().find_map(|(id, active, depth, speed)| {
                (*id == clip_id).then_some((*active, *depth, *speed))
            })
        });
        let active_clip = clip_state.map(|(active, _, _)| active).unwrap_or(false);
        let clip_fx_depth = clip_state.map(|(_, depth, _)| depth).unwrap_or(1000);
        let clip_speed = clip_state.map(|(_, _, speed)| speed).unwrap_or(1000);
        let effective_rate = ((layer.rate.permille() as u32 * clip_speed as u32) / 1000)
            .clamp(SpeedRatio::MIN as u32, SpeedRatio::MAX as u32)
            as u16;

        let before_phase = layer.phase;
        let before_output = layer.output_level;
        let modulation = waveform_modulation(
            frame_index,
            effective_rate,
            layer.phase_offset_permille,
            layer.spread_permille,
            layer.waveform,
        );
        let base_depth = ((layer.depth_permille as u32 * clip_fx_depth as u32) / 1000) as u16;

        if !layer.enabled {
            layer.phase = FxPhase::Idle;
            layer.output_level = 0;
        } else if active_clip {
            layer.phase = if enabled_count > 1 {
                FxPhase::Composed
            } else {
                FxPhase::Applied
            };
            let base_output = (base_depth as u32 * master_intensity as u32) / 1000;
            let factor = 400u32 + ((modulation as u32 * 600u32) / 1000);
            layer.output_level = ((base_output * factor) / 1000) as u16;
        } else {
            layer.phase = FxPhase::Processing;
            let factor = 180u32 + ((modulation as u32 * 220u32) / 1000);
            layer.output_level = ((base_depth as u32 * factor) / 1000) as u16;
        }

        if layer.phase != before_phase || layer.output_level != before_output {
            diffs.push(StateDiff::Fx(layer.id));
        }
    }

    diffs
}

fn waveform_modulation(
    frame_index: u64,
    rate_permille: u16,
    phase_offset_permille: u16,
    spread_permille: u16,
    waveform: FxWaveform,
) -> u16 {
    let phase = (((frame_index as u128 * rate_permille as u128 * 7) / 5)
        + phase_offset_permille as u128)
        % 1000;
    let raw = match waveform {
        FxWaveform::Sine => {
            let radians = (phase as f32 / 1000.0) * TAU;
            ((radians.sin() * 0.5 + 0.5) * 1000.0).round() as u16
        }
        FxWaveform::Triangle => {
            if phase < 500 {
                (phase as u16) * 2
            } else {
                ((1000 - phase) as u16) * 2
            }
        }
        FxWaveform::Saw => phase as u16,
        FxWaveform::Pulse => {
            if phase < 320 {
                1000
            } else {
                180
            }
        }
    };

    let centered = raw as i32 - 500;
    (500 + (centered * spread_permille as i32) / 1000).clamp(0, 1000) as u16
}

fn refresh_fixture_groups(state: &mut StudioState) -> Vec<StateDiff> {
    let active_cues = state
        .cue_system
        .cues
        .iter()
        .map(|cue| {
            let output = cue
                .linked_clip
                .and_then(|clip_id| state.clip(clip_id))
                .map(|clip| {
                    let local = state
                        .engine
                        .transport
                        .playhead
                        .saturating_sub(clip.start)
                        .clamp(BeatTime::ZERO, clip.duration);
                    effective_clip_parameters(clip, local).intensity.permille()
                })
                .unwrap_or(state.master.intensity.permille());
            (
                cue.id,
                matches!(cue.phase, CuePhase::Triggered | CuePhase::Active),
                output,
            )
        })
        .collect::<Vec<_>>();
    let fx_outputs = state
        .fx_system
        .layers
        .iter()
        .map(|layer| (layer.id, layer.output_level))
        .collect::<Vec<_>>();
    let mut diffs = Vec::new();

    for group in &mut state.fixture_system.groups {
        let cue_drive = group
            .linked_cue
            .and_then(|cue_id| {
                active_cues
                    .iter()
                    .find_map(|(id, active, output)| (*id == cue_id && *active).then_some(*output))
            })
            .unwrap_or(0);
        let fx_output = group
            .linked_fx
            .and_then(|fx_id| {
                fx_outputs
                    .iter()
                    .find_map(|(id, output)| (*id == fx_id).then_some(*output))
            })
            .unwrap_or(0);

        let before_phase = group.phase;
        let before_output = group.output_level;

        if group.online == 0 {
            group.phase = FixturePhase::Error;
            group.output_level = 0;
        } else if cue_drive > 0 || fx_output > 0 {
            group.phase = FixturePhase::Active;
            group.output_level = cue_drive.max(fx_output);
        } else if group.phase == FixturePhase::Uninitialized {
            group.output_level = 0;
        } else {
            group.phase = FixturePhase::Mapped;
            group.output_level = (group.fixture_count as u16 * 20).min(400);
        }

        if group.phase != before_phase || group.output_level != before_output {
            diffs.push(StateDiff::Fixture(group.id));
        }
    }

    diffs
}

fn sync_clip_cue_states(state: &mut StudioState) {
    let cue_states = state
        .cue_system
        .cues
        .iter()
        .map(|cue| (cue.id, CueVisualState::from_phase(cue.phase)))
        .collect::<Vec<_>>();

    for track in &mut state.timeline.tracks {
        for clip in &mut track.clips {
            if let Some(cue_id) = clip.linked_cue {
                clip.cue_state = cue_states
                    .iter()
                    .find_map(|(id, visual)| (*id == cue_id).then_some(*visual))
                    .unwrap_or(CueVisualState::Inactive);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{ClipId, StudioState};

    #[test]
    fn trigger_cue_moves_previous_active_to_fading() {
        let mut state = StudioState::default();

        let diffs = trigger_cue(&mut state, CueId(3));

        assert!(diffs.contains(&StateDiff::Cue(CueId(1))));
        assert!(diffs.contains(&StateDiff::Cue(CueId(3))));
        assert_eq!(
            state.cue(CueId(1)).expect("cue exists").phase,
            CuePhase::Fading
        );
        assert_eq!(
            state.cue(CueId(3)).expect("cue exists").phase,
            CuePhase::Triggered
        );
        assert_eq!(
            state.clip(ClipId(201)).expect("clip exists").cue_state,
            CueVisualState::Active
        );
    }

    #[test]
    fn chase_advances_and_triggers_linked_cue() {
        let mut state = StudioState::default();
        state.chase_system.chases[0].current_step = 0;
        state.chase_system.chases[0].progress = BeatTime::ZERO;

        let diffs = advance_show_frame(&mut state, BeatTime::from_fraction(1, 2));

        assert!(
            diffs
                .iter()
                .any(|diff| matches!(diff, StateDiff::Chase(ChaseId(1))))
        );
        assert_eq!(
            state.chase(ChaseId(1)).expect("chase exists").current_step,
            1
        );
        assert_eq!(
            state.cue(CueId(1)).expect("cue exists").phase,
            CuePhase::Triggered
        );
    }

    #[test]
    fn delete_selected_cue_clears_clip_fixture_and_chase_links() {
        let mut state = StudioState::default();
        state.cue_system.selected = Some(CueId(1));

        let diffs = delete_selected_cue(&mut state);

        assert!(diffs.contains(&StateDiff::Cue(CueId(1))));
        assert!(state.cue(CueId(1)).is_none());
        assert!(
            state
                .timeline
                .tracks
                .iter()
                .flat_map(|track| track.clips.iter())
                .all(|clip| clip.linked_cue != Some(CueId(1)))
        );
        assert!(
            state
                .fixture_system
                .groups
                .iter()
                .all(|group| group.linked_cue != Some(CueId(1)))
        );
        assert!(
            state
                .chase_system
                .chases
                .iter()
                .flat_map(|chase| chase.steps.iter())
                .all(|step| step.cue_id != Some(CueId(1)))
        );
    }

    #[test]
    fn create_chase_and_edit_steps_updates_selection_deterministically() {
        let mut state = StudioState::default();

        let create_diffs = create_chase(&mut state);
        let chase_id = state.chase_system.selected.expect("selected chase");
        assert!(create_diffs.contains(&StateDiff::Chase(chase_id)));

        add_selected_chase_step(&mut state);
        set_selected_chase_step_label(&mut state, "Accent".to_owned());
        set_selected_chase_step_duration(&mut state, BeatTime::from_beats(1));
        move_selected_chase_step(&mut state, -1);

        let chase = state.chase(chase_id).expect("chase exists");
        assert_eq!(chase.steps.len(), 2);
        assert_eq!(state.chase_system.selected_step, Some(0));
        assert_eq!(chase.steps[0].label, "Accent");
        assert_eq!(chase.steps[0].duration, BeatTime::from_beats(1));
    }

    #[test]
    fn set_selected_chase_step_cue_updates_selected_cue_and_clamps_duration() {
        let mut state = StudioState::default();
        select_chase(&mut state, ChaseId(1));
        select_chase_step(&mut state, Some(0));

        set_selected_chase_step_cue(&mut state, Some(CueId(4)));
        set_selected_chase_step_duration(&mut state, BeatTime::ZERO);

        let chase = state.chase(ChaseId(1)).expect("chase exists");
        assert_eq!(state.cue_system.selected, Some(CueId(4)));
        assert_eq!(chase.steps[0].cue_id, Some(CueId(4)));
        assert_eq!(chase.steps[0].duration, MIN_CLIP_DURATION);
    }

    #[test]
    fn fx_output_follows_master_intensity_for_active_clip() {
        let mut state = StudioState::default();
        state.timeline.tracks[0].clips[1].phase = crate::core::ClipPhase::Active;
        state.performance.frame_index = 24;

        let diffs = advance_fx_layers(&mut state);
        let layer = state.fx_layer(FxId(1)).expect("fx exists");
        let clip = state.clip(ClipId(102)).expect("clip exists");
        let effective_rate =
            ((layer.rate.permille() as u32 * clip.params.speed.permille() as u32) / 1000)
                .clamp(SpeedRatio::MIN as u32, SpeedRatio::MAX as u32) as u16;
        let modulation = waveform_modulation(
            24,
            effective_rate,
            layer.phase_offset_permille,
            layer.spread_permille,
            layer.waveform,
        );
        let base_depth =
            ((layer.depth_permille as u32 * clip.params.fx_depth.permille() as u32) / 1000) as u16;
        let base_output = (base_depth as u32 * state.master.intensity.permille() as u32) / 1000;
        let expected = ((base_output * (400 + ((modulation as u32 * 600) / 1000))) / 1000) as u16;

        assert!(
            diffs
                .iter()
                .any(|diff| matches!(diff, StateDiff::Fx(FxId(1))))
        );
        assert_eq!(layer.output_level, expected);
    }

    #[test]
    fn fx_waveform_settings_change_modulated_output_deterministically() {
        let mut state = StudioState::default();
        state.timeline.tracks[0].clips[1].phase = crate::core::ClipPhase::Active;
        state.performance.frame_index = 12;
        state.fx_system.layers[0].waveform = FxWaveform::Saw;
        state.fx_system.layers[0].spread_permille = 1000;
        state.fx_system.layers[0].phase_offset_permille = 500;

        advance_fx_layers(&mut state);

        let output = state.fx_layer(FxId(1)).expect("fx exists").output_level;
        assert!(output > 0);
        assert!(output <= 1000);
    }

    #[test]
    fn fixture_group_becomes_active_from_linked_sources() {
        let mut state = StudioState::default();
        trigger_cue(&mut state, CueId(1));
        state.fixture_system.groups[0].phase = FixturePhase::Mapped;
        state.fixture_system.groups[0].output_level = 0;
        state.fx_system.layers[0].output_level = 900;

        let diffs = refresh_fixture_groups(&mut state);

        assert!(
            diffs
                .iter()
                .any(|diff| matches!(diff, StateDiff::Fixture(FixtureGroupId(1))))
        );
        assert_eq!(
            state
                .fixture_group(FixtureGroupId(1))
                .expect("group exists")
                .phase,
            FixturePhase::Active
        );
    }
}
