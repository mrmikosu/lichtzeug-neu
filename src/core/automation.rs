use crate::core::state::{
    AutomationInterpolation, AutomationLane, AutomationPoint, AutomationTarget, Clip,
    ClipParameters,
};
use crate::core::time::{BeatTime, IntensityLevel, SpeedRatio};

pub fn clip_parameter_value(params: ClipParameters, target: AutomationTarget) -> u16 {
    match target {
        AutomationTarget::Intensity => params.intensity.permille(),
        AutomationTarget::Speed => params.speed.permille(),
        AutomationTarget::FxDepth => params.fx_depth.permille(),
    }
}

pub fn clamp_automation_value(target: AutomationTarget, value: u16) -> u16 {
    match target {
        AutomationTarget::Intensity | AutomationTarget::FxDepth => value.min(1000),
        AutomationTarget::Speed => value.clamp(SpeedRatio::MIN, SpeedRatio::MAX),
    }
}

pub fn lane_mut(clip: &mut Clip, target: AutomationTarget) -> Option<&mut AutomationLane> {
    clip.automation
        .iter_mut()
        .find(|lane| lane.target == target)
}

pub fn lane(clip: &Clip, target: AutomationTarget) -> Option<&AutomationLane> {
    clip.automation.iter().find(|lane| lane.target == target)
}

pub fn ensure_lane(clip: &mut Clip, target: AutomationTarget) -> &mut AutomationLane {
    let base = clip_parameter_value(clip.params, target);
    let duration = clip.duration;

    if clip.automation.iter().all(|lane| lane.target != target) {
        clip.automation.push(AutomationLane {
            target,
            interpolation: AutomationInterpolation::Linear,
            enabled: true,
            points: vec![
                AutomationPoint {
                    offset: BeatTime::ZERO,
                    value: base,
                },
                AutomationPoint {
                    offset: duration,
                    value: base,
                },
            ],
        });
    }

    clip.automation
        .iter_mut()
        .find(|lane| lane.target == target)
        .expect("lane inserted")
}

pub fn sort_lane_points(lane: &mut AutomationLane) {
    lane.points.sort_by_key(|point| point.offset.ticks());
    lane.points.dedup_by_key(|point| point.offset.ticks());

    for point in &mut lane.points {
        point.value = clamp_automation_value(lane.target, point.value);
    }
}

pub fn effective_clip_parameters(clip: &Clip, clip_local_time: BeatTime) -> ClipParameters {
    let intensity = evaluate_lane_value(clip, AutomationTarget::Intensity, clip_local_time)
        .map(IntensityLevel::from_permille)
        .unwrap_or(clip.params.intensity);
    let speed = evaluate_lane_value(clip, AutomationTarget::Speed, clip_local_time)
        .map(SpeedRatio::from_permille)
        .unwrap_or(clip.params.speed);
    let fx_depth = evaluate_lane_value(clip, AutomationTarget::FxDepth, clip_local_time)
        .map(IntensityLevel::from_permille)
        .unwrap_or(clip.params.fx_depth);

    ClipParameters {
        intensity,
        speed,
        fx_depth,
        bpm_grid: clip.params.bpm_grid,
    }
}

pub fn evaluate_lane_value(
    clip: &Clip,
    target: AutomationTarget,
    clip_local_time: BeatTime,
) -> Option<u16> {
    let lane = lane(clip, target)?;
    if !lane.enabled || lane.points.is_empty() {
        return None;
    }

    let local = clip_local_time.clamp(BeatTime::ZERO, clip.duration);
    let first = lane.points.first()?;
    let last = lane.points.last()?;

    if local <= first.offset {
        return Some(clamp_automation_value(target, first.value));
    }

    if local >= last.offset {
        return Some(clamp_automation_value(target, last.value));
    }

    let mut previous = first;
    for next in lane.points.iter().skip(1) {
        if local <= next.offset {
            return Some(match lane.interpolation {
                AutomationInterpolation::Step => clamp_automation_value(target, previous.value),
                AutomationInterpolation::Linear => {
                    interpolate_points(target, previous, next, local)
                }
            });
        }
        previous = next;
    }

    Some(clamp_automation_value(target, last.value))
}

fn interpolate_points(
    target: AutomationTarget,
    left: &AutomationPoint,
    right: &AutomationPoint,
    position: BeatTime,
) -> u16 {
    let span = right
        .offset
        .ticks()
        .saturating_sub(left.offset.ticks())
        .max(1);
    let progress = position.ticks().saturating_sub(left.offset.ticks());
    let delta = right.value as i32 - left.value as i32;
    let value = left.value as i32 + ((delta * progress as i32) / span as i32);
    clamp_automation_value(target, value.max(0) as u16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{AutomationPoint, ClipId, ClipPalette, ClipPhase, CueVisualState, RgbaColor};

    fn test_clip() -> Clip {
        Clip {
            id: ClipId(1),
            title: "Test".to_owned(),
            phase: ClipPhase::Inactive,
            start: BeatTime::ZERO,
            duration: BeatTime::from_beats(8),
            params: ClipParameters {
                intensity: IntensityLevel::from_permille(500),
                speed: SpeedRatio::from_permille(1000),
                fx_depth: IntensityLevel::from_permille(600),
                bpm_grid: crate::core::SnapResolution::QuarterBeat,
            },
            automation: vec![AutomationLane {
                target: AutomationTarget::Intensity,
                interpolation: AutomationInterpolation::Linear,
                enabled: true,
                points: vec![
                    AutomationPoint {
                        offset: BeatTime::ZERO,
                        value: 250,
                    },
                    AutomationPoint {
                        offset: BeatTime::from_beats(4),
                        value: 750,
                    },
                ],
            }],
            palette: ClipPalette {
                base: RgbaColor::rgb(0, 0, 0),
                highlight: RgbaColor::rgb(0, 0, 0),
                edge: RgbaColor::rgb(0, 0, 0),
            },
            markers: Vec::new(),
            linked_cue: None,
            cue_state: CueVisualState::Inactive,
        }
    }

    #[test]
    fn linear_automation_interpolates_deterministically() {
        let clip = test_clip();

        let value =
            evaluate_lane_value(&clip, AutomationTarget::Intensity, BeatTime::from_beats(2))
                .expect("value");

        assert_eq!(value, 500);
    }

    #[test]
    fn effective_parameters_fallback_to_clip_defaults() {
        let clip = test_clip();

        let effective = effective_clip_parameters(&clip, BeatTime::from_beats(2));

        assert_eq!(effective.intensity.permille(), 500);
        assert_eq!(effective.speed.permille(), 1000);
        assert_eq!(effective.fx_depth.permille(), 600);
    }
}
