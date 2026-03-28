use crate::core::event::StateDiff;
use crate::core::state::{
    ClipPhase, CpuLoad, EnginePhase, EngineResumeTarget, EngineState, MasterState, StudioState,
};
use crate::core::time::{BeatTime, PPQ};
use crate::core::{advance_clip_editor, advance_show_frame};

const BEAT_DENOMINATOR: u128 = 60_000_000_000u128 * 100u128 * 1000u128;

pub fn toggle_transport(engine: &mut EngineState) -> StateDiff {
    engine.phase = match engine.phase {
        EnginePhase::Stopped | EnginePhase::Paused => EnginePhase::Running,
        EnginePhase::Running => EnginePhase::Paused,
        EnginePhase::Syncing => match engine.resume_target {
            EngineResumeTarget::Stopped => EnginePhase::Running,
            EngineResumeTarget::Running => EnginePhase::Paused,
            EngineResumeTarget::Paused => EnginePhase::Running,
        },
        EnginePhase::Error => EnginePhase::Error,
    };

    if matches!(engine.phase, EnginePhase::Running) {
        engine.resume_target = EngineResumeTarget::Running;
    } else if matches!(engine.phase, EnginePhase::Paused) {
        engine.resume_target = EngineResumeTarget::Paused;
    }

    StateDiff::Engine
}

pub fn enter_sync_phase(engine: &mut EngineState) {
    engine.resume_target = match engine.phase {
        EnginePhase::Stopped => EngineResumeTarget::Stopped,
        EnginePhase::Running => EngineResumeTarget::Running,
        EnginePhase::Paused | EnginePhase::Syncing | EnginePhase::Error => {
            EngineResumeTarget::Paused
        }
    };

    if engine.phase != EnginePhase::Error {
        engine.phase = EnginePhase::Syncing;
    }
}

pub fn resume_after_sync(engine: &mut EngineState) {
    if engine.phase == EnginePhase::Error {
        return;
    }

    engine.phase = match engine.resume_target {
        EngineResumeTarget::Stopped => EnginePhase::Stopped,
        EngineResumeTarget::Running => EnginePhase::Running,
        EngineResumeTarget::Paused => EnginePhase::Paused,
    };
}

pub fn advance_engine_frame(state: &mut StudioState) -> Vec<StateDiff> {
    let previous_playhead = state.engine.transport.playhead;
    let mut delta = BeatTime::ZERO;
    state.engine.clock.advance_frame();
    state.performance.frame_index = state.engine.clock.frame_index;
    state.performance.fps = 60;

    if state.engine.phase == EnginePhase::Running {
        delta = transport_delta(&mut state.engine, &state.master);
        state.engine.transport.playhead = state
            .engine
            .transport
            .playhead
            .wrapping_add(delta, state.engine.transport.song_length);
    }

    state.performance.cpu_load = estimate_cpu_load(state);
    update_clip_phases(state, previous_playhead);
    let show_diffs = advance_show_frame(state, delta);
    let clip_editor_diffs = advance_clip_editor(state);

    let mut diffs = vec![StateDiff::Engine, StateDiff::Performance];

    if previous_playhead != state.engine.transport.playhead {
        diffs.push(StateDiff::Playhead);
    }

    diffs.extend(show_diffs);
    diffs.extend(clip_editor_diffs);
    diffs
}

fn transport_delta(engine: &mut EngineState, master: &MasterState) -> BeatTime {
    let numerator = engine.transport.bpm.centi_bpm() as u128
        * master.speed.permille() as u128
        * PPQ as u128
        * engine.clock.frame_interval_ns as u128;

    engine.clock.beat_carry = engine.clock.beat_carry.saturating_add(numerator);
    let ticks = (engine.clock.beat_carry / BEAT_DENOMINATOR) as u32;
    engine.clock.beat_carry %= BEAT_DENOMINATOR;

    BeatTime::from_ticks(ticks)
}

fn update_clip_phases(state: &mut StudioState, previous_playhead: BeatTime) {
    let current = state.engine.transport.playhead;

    for track in &mut state.timeline.tracks {
        for clip in &mut track.clips {
            let clip_end = clip.start.saturating_add(clip.duration);
            let was_in_clip = previous_playhead >= clip.start && previous_playhead < clip_end;
            let is_in_clip = current >= clip.start && current < clip_end;

            clip.phase = match (was_in_clip, is_in_clip) {
                (false, true) => ClipPhase::Triggered,
                (true, true) => ClipPhase::Active,
                (true, false) => ClipPhase::Completed,
                (false, false) => ClipPhase::Inactive,
            };
        }
    }
}

fn estimate_cpu_load(state: &StudioState) -> CpuLoad {
    let base = 10u16;
    let queue_cost = (state.event_queue.queue.len() as u16) * 3;
    let interaction_cost = match state.timeline.phase {
        crate::core::TimelinePhase::Idle => 4,
        crate::core::TimelinePhase::Dragging => 11,
        crate::core::TimelinePhase::Zooming => 8,
        crate::core::TimelinePhase::Snapping => 12,
        crate::core::TimelinePhase::Rendering => 14,
    };

    CpuLoad((base + queue_cost + interaction_cost).min(100))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::StudioState;

    #[test]
    fn engine_advances_playhead_deterministically() {
        let mut state = StudioState::default();
        let before = state.engine.transport.playhead;

        advance_engine_frame(&mut state);
        advance_engine_frame(&mut state);

        assert!(state.engine.transport.playhead > before);
        assert_eq!(state.engine.clock.frame_index, 2);
    }
}
