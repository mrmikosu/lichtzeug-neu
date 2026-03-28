use crate::core::{
    AppEvent, AutomationInterpolation, AutomationLane, AutomationTarget, BeatTime, Chase, ChaseId,
    ChaseSystemState, Clip, ClipEditorPhase, ClipEditorState, ClipId, ClipInlineParameterKind,
    ContextMenuAction, ContextMenuState, Cue, CueId, CueSystemState, CueVisualState, FxKind,
    FxLayer, FxSystemState, FxWaveform, HoverTarget, PPQ, SelectionState, SnapGuide, SnapPhase,
    SnapResolution, StudioState, TIMELINE_CLIP_HEIGHT_PX, TIMELINE_CLIP_TOP_INSET_PX,
    TIMELINE_HEADER_HEIGHT_PX, TIMELINE_TRACK_GAP_PX, TIMELINE_TRACK_HEIGHT_PX, TimelineCursor,
    TimelineEvent, TimelineHit, TimelineInteraction, TimelineState, TimelineZone, TrackId,
    TransportState,
};
use crate::ui::theme;
use iced::widget::canvas::{self, Canvas, Fill, Path, Stroke, Text, gradient};
use iced::widget::{button, column, container, pick_list, row, slider, text};
use iced::{
    Color, Element, Length, Pixels, Point, Rectangle, Renderer, Size, Theme, alignment, border,
    mouse,
};
use std::cell::Cell;
use std::fmt;

pub const HEADER_HEIGHT: f32 = TIMELINE_HEADER_HEIGHT_PX as f32;
pub const TRACK_HEIGHT: f32 = TIMELINE_TRACK_HEIGHT_PX as f32;
pub const TRACK_GAP: f32 = TIMELINE_TRACK_GAP_PX as f32;
const CLIP_HEIGHT: f32 = TIMELINE_CLIP_HEIGHT_PX as f32;
const CLIP_TOP_INSET: f32 = TIMELINE_CLIP_TOP_INSET_PX as f32;
const HANDLE_WIDTH: f32 = 10.0;
const PLAYHEAD_HIT_RADIUS: f32 = 6.0;
const SNAP_DASH: [f32; 2] = [6.0, 6.0];
const INLINE_PARAM_TRACK_WIDTH: f32 = 10.0;
const INLINE_PARAM_TRACK_HEIGHT: f32 = 30.0;
const INLINE_PARAM_SPACING: f32 = 8.0;
const INLINE_PARAM_KNOB_SIZE: f32 = 8.0;
const CONTEXT_MENU_WIDTH: f32 = 168.0;
const CONTEXT_MENU_ITEM_HEIGHT: f32 = 24.0;

#[derive(Debug, Clone)]
pub struct TimelineProgram {
    transport: TransportState,
    timeline: TimelineState,
    clip_editor: ClipEditorState,
    context_menu: ContextMenuState,
    cue_system: CueSystemState,
    chase_system: ChaseSystemState,
    fx_system: FxSystemState,
    frame_index: u64,
    grid_revision: u64,
    clip_revision: u64,
}

#[derive(Debug, Default)]
pub struct TimelineCanvasState {
    grid_cache: canvas::Cache,
    clip_cache: canvas::Cache,
    grid_revision: Cell<u64>,
    clip_revision: Cell<u64>,
}

#[derive(Debug, Clone)]
struct ClipHotspot {
    hit: TimelineHit,
    rect: Rectangle,
    accent: Color,
    label: String,
}

#[derive(Debug, Clone)]
struct ClipParamHandle {
    kind: ClipInlineParameterKind,
    track_rect: Rectangle,
    knob_rect: Rectangle,
    accent: Color,
    label: &'static str,
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
}

fn automation_normalized_value(target: AutomationTarget, value: u16) -> f32 {
    match target {
        AutomationTarget::Intensity | AutomationTarget::FxDepth => value as f32 / 1000.0,
        AutomationTarget::Speed => ((value.saturating_sub(200)) as f32 / 1300.0).clamp(0.0, 1.0),
    }
}

pub fn canvas(state: &StudioState) -> Canvas<TimelineProgram, AppEvent> {
    Canvas::new(TimelineProgram {
        transport: state.engine.transport.clone(),
        timeline: state.timeline.clone(),
        clip_editor: state.clip_editor.clone(),
        context_menu: state.context_menu.clone(),
        cue_system: state.cue_system.clone(),
        chase_system: state.chase_system.clone(),
        fx_system: state.fx_system.clone(),
        frame_index: state.performance.frame_index,
        grid_revision: state.revisions.grid,
        clip_revision: state.revisions.clips,
    })
    .width(Length::Fill)
    .height(Length::Fill)
}

pub fn view(state: &StudioState) -> Element<'_, AppEvent> {
    let timeline_canvas = container(canvas(state))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| theme::timeline_shell());

    if state.clip_editor.phase == ClipEditorPhase::Closed {
        return timeline_canvas.into();
    }

    container(column![timeline_canvas, clip_editor_panel(state)].spacing(12))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CueOption {
    id: Option<CueId>,
    label: String,
}

impl fmt::Display for CueOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChaseOption {
    id: Option<ChaseId>,
    label: String,
}

impl fmt::Display for ChaseOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

fn automation_points_row(
    lane: Option<&AutomationLane>,
    selected: Option<usize>,
) -> Element<'_, AppEvent> {
    let Some(lane) = lane else {
        return container(text("Keine Punkte"))
            .padding([8, 10])
            .style(|_| theme::panel_inner())
            .into();
    };

    let mut points = row![].spacing(8);
    for (index, point) in lane.points.iter().enumerate() {
        let is_selected = selected == Some(index);
        points = points.push(
            button(text(format!("{:.1}b", point.offset.as_beats_f32())).size(11))
                .padding([6, 10])
                .style(move |_: &Theme, button_state| {
                    theme::toggle_button(button_state, is_selected, theme::accent_blue())
                })
                .on_press(AppEvent::SelectClipEditorAutomationPoint(Some(index))),
        );
    }

    container(points).padding([2, 0]).into()
}

fn automation_value_slider(
    target: AutomationTarget,
    selected_point: Option<(usize, &crate::core::AutomationPoint)>,
) -> Element<'_, AppEvent> {
    let Some((_, point)) = selected_point else {
        return container(text("Wert folgt dem selektierten Punkt"))
            .padding([10, 12])
            .style(|_| theme::panel_inner())
            .into();
    };

    let (min, max) = match target {
        AutomationTarget::Intensity | AutomationTarget::FxDepth => (0.0, 1000.0),
        AutomationTarget::Speed => (200.0, 1500.0),
    };

    column![
        slider(min..=max, point.value as f32, |value| {
            AppEvent::SetClipEditorAutomationPointValue(value.round() as u16)
        })
        .step(1.0),
        text(automation_value_label(target, point.value))
            .size(12)
            .color(theme::text_primary()),
    ]
    .spacing(8)
    .into()
}

fn automation_value_label(target: AutomationTarget, value: u16) -> String {
    match target {
        AutomationTarget::Intensity | AutomationTarget::FxDepth => format!("{}%", value / 10),
        AutomationTarget::Speed => format!("{}%", value / 10),
    }
}

fn clip_editor_panel(state: &StudioState) -> Element<'_, AppEvent> {
    let Some(clip) = state.editor_clip() else {
        return container(text("Clip-Editor ohne selektierten Clip"))
            .padding(12)
            .style(|_| theme::panel_subtle())
            .into();
    };

    let cue_options = std::iter::once(CueOption {
        id: None,
        label: "No Cue".to_owned(),
    })
    .chain(state.cue_system.cues.iter().map(|cue| CueOption {
        id: Some(cue.id),
        label: cue.name.clone(),
    }))
    .collect::<Vec<_>>();

    let selected_cue = cue_options
        .iter()
        .find(|option| option.id == clip.linked_cue)
        .cloned()
        .or_else(|| cue_options.first().cloned());

    let active_chase = state
        .chase_system
        .chases
        .iter()
        .find(|chase| chase.linked_clip == Some(clip.id));

    let chase_options = std::iter::once(ChaseOption {
        id: None,
        label: "No Chase".to_owned(),
    })
    .chain(state.chase_system.chases.iter().map(|chase| ChaseOption {
        id: Some(chase.id),
        label: chase.name.clone(),
    }))
    .collect::<Vec<_>>();

    let selected_chase = chase_options
        .iter()
        .find(|option| option.id == active_chase.map(|chase| chase.id))
        .cloned()
        .or_else(|| chase_options.first().cloned());

    let grid_options = vec![
        SnapResolution::Beat,
        SnapResolution::HalfBeat,
        SnapResolution::QuarterBeat,
        SnapResolution::EighthBeat,
    ];
    let automation_targets = vec![
        AutomationTarget::Intensity,
        AutomationTarget::Speed,
        AutomationTarget::FxDepth,
    ];
    let automation_modes = vec![
        AutomationInterpolation::Linear,
        AutomationInterpolation::Step,
    ];
    let active_lane = clip
        .automation
        .iter()
        .find(|lane| lane.target == state.clip_editor.automation_target);
    let selected_point = active_lane.and_then(|lane| {
        state
            .clip_editor
            .selected_automation_point
            .and_then(|index| lane.points.get(index).map(|point| (index, point)))
    });
    let selected_point_card: Element<'_, AppEvent> = if let Some((index, point)) = selected_point {
        container(
            text(format!(
                "#{}  @ {:.2}b  |  {}",
                index + 1,
                point.offset.as_beats_f32(),
                automation_value_label(state.clip_editor.automation_target, point.value)
            ))
            .size(12)
            .color(theme::text_primary()),
        )
        .padding([10, 12])
        .style(|_: &Theme| theme::panel_inner())
        .into()
    } else {
        container(text("Kein Automation-Punkt selektiert"))
            .padding([10, 12])
            .style(|_: &Theme| theme::panel_inner())
            .into()
    };

    let title = row![
        text(format!("Clip Editor  |  {}", clip.title))
            .size(16)
            .color(theme::text_primary()),
        text(match state.clip_editor.phase {
            ClipEditorPhase::Open => "open",
            ClipEditorPhase::Adjusting => "adjusting",
            ClipEditorPhase::Previewing => "previewing",
            ClipEditorPhase::Closed => "closed",
        })
        .size(12)
        .color(theme::text_muted()),
        button(text("Close"))
            .padding([6, 10])
            .style(|_: &Theme, button_state| {
                theme::toggle_button(button_state, false, theme::muted_chip())
            })
            .on_press(AppEvent::CloseClipEditor),
    ]
    .spacing(12)
    .align_y(iced::Alignment::Center);

    let cue_action: Element<'_, AppEvent> = if let Some(cue_id) = clip.linked_cue {
        button(text("Go Cue"))
            .padding([8, 12])
            .style(|_: &Theme, button_state| {
                theme::toggle_button(button_state, true, theme::warning())
            })
            .on_press(AppEvent::TriggerCue(cue_id))
            .into()
    } else {
        container(text("Kein Cue verknüpft"))
            .padding([8, 12])
            .style(|_| theme::panel_inner())
            .into()
    };

    let chase_action: Element<'_, AppEvent> = if let Some(chase) = active_chase {
        row![
            button(text(
                if matches!(
                    chase.phase,
                    crate::core::ChasePhase::Playing
                        | crate::core::ChasePhase::Looping
                        | crate::core::ChasePhase::Reversing
                ) {
                    "Stop Chase"
                } else {
                    "Play Chase"
                }
            ))
            .padding([8, 12])
            .style(|_: &Theme, button_state| {
                theme::toggle_button(button_state, true, theme::accent_blue())
            })
            .on_press(AppEvent::ToggleChase(chase.id)),
            button(text("Reverse"))
                .padding([8, 12])
                .style(|_: &Theme, button_state| {
                    theme::toggle_button(button_state, true, theme::muted_chip())
                })
                .on_press(AppEvent::ReverseChase(chase.id)),
        ]
        .spacing(10)
        .into()
    } else {
        container(text("Kein Chase verknüpft"))
            .padding([8, 12])
            .style(|_| theme::panel_inner())
            .into()
    };

    let body = column![
        title,
        row![
            column![
                text("Intensity").size(12).color(theme::text_muted()),
                slider(0.0..=1.0, clip.params.intensity.as_f32(), |value| {
                    AppEvent::SetClipEditorIntensity((value * 1000.0).round() as u16)
                })
                .step(0.001),
                text(format!("{:>3.0}%", clip.params.intensity.as_f32() * 100.0))
                    .size(12)
                    .color(theme::text_primary()),
            ]
            .spacing(8)
            .width(Length::FillPortion(1)),
            column![
                text("Speed").size(12).color(theme::text_muted()),
                slider(0.2..=1.5, clip.params.speed.as_f32(), |value| {
                    AppEvent::SetClipEditorSpeed((value * 1000.0).round() as u16)
                })
                .step(0.001),
                text(format!("{:>3.0}%", clip.params.speed.as_f32() * 100.0))
                    .size(12)
                    .color(theme::text_primary()),
            ]
            .spacing(8)
            .width(Length::FillPortion(1)),
            column![
                text("FX Depth").size(12).color(theme::text_muted()),
                slider(0.0..=1.0, clip.params.fx_depth.as_f32(), |value| {
                    AppEvent::SetClipEditorFxDepth((value * 1000.0).round() as u16)
                })
                .step(0.001),
                text(format!("{:>3.0}%", clip.params.fx_depth.as_f32() * 100.0))
                    .size(12)
                    .color(theme::text_primary()),
            ]
            .spacing(8)
            .width(Length::FillPortion(1)),
        ]
        .spacing(14),
        row![
            column![
                text("Cue Link").size(12).color(theme::text_muted()),
                pick_list(cue_options, selected_cue, |choice| {
                    AppEvent::SetClipEditorCue(choice.id)
                })
                .placeholder("Cue"),
            ]
            .spacing(8)
            .width(Length::FillPortion(1)),
            column![
                text("Chase Link").size(12).color(theme::text_muted()),
                pick_list(chase_options, selected_chase, |choice| {
                    AppEvent::SetClipEditorChase(choice.id)
                })
                .placeholder("Chase"),
            ]
            .spacing(8)
            .width(Length::FillPortion(1)),
            column![
                text("BPM Grid").size(12).color(theme::text_muted()),
                pick_list(
                    grid_options,
                    Some(clip.params.bpm_grid),
                    AppEvent::SetClipEditorGrid
                )
                .placeholder("Grid"),
            ]
            .spacing(8)
            .width(Length::FillPortion(1)),
            column![
                text("Live Preview").size(12).color(theme::text_muted()),
                container(
                    text(format!(
                        "Cue {:?}  |  Chase {:?}  |  FX {}  |  Grid {}",
                        clip.linked_cue.map(|cue_id| cue_id.0),
                        active_chase.map(|chase| chase.id.0),
                        clip.params.fx_depth.permille(),
                        clip.params.bpm_grid
                    ))
                    .size(12)
                    .color(theme::text_primary())
                )
                .padding([10, 12])
                .style(|_| theme::panel_inner()),
            ]
            .spacing(8)
            .width(Length::FillPortion(1)),
        ]
        .spacing(14),
        row![
            column![
                text("Automation Lane").size(12).color(theme::text_muted()),
                pick_list(
                    automation_targets,
                    Some(state.clip_editor.automation_target),
                    AppEvent::SetClipEditorAutomationTarget
                )
                .placeholder("Lane"),
                button(text(if active_lane.is_some_and(|lane| lane.enabled) {
                    "Lane On"
                } else {
                    "Lane Off"
                }))
                .padding([8, 12])
                .style(|_: &Theme, button_state| {
                    theme::toggle_button(button_state, true, theme::accent_blue())
                })
                .on_press(AppEvent::ToggleClipEditorAutomationLane),
            ]
            .spacing(8)
            .width(Length::FillPortion(1)),
            column![
                text("Interpolation").size(12).color(theme::text_muted()),
                pick_list(
                    automation_modes,
                    active_lane.map(|lane| lane.interpolation),
                    AppEvent::SetClipEditorAutomationMode
                )
                .placeholder("Mode"),
                button(text("Point @ Playhead"))
                    .padding([8, 12])
                    .style(|_: &Theme, button_state| {
                        theme::toggle_button(button_state, true, theme::success())
                    })
                    .on_press(AppEvent::AddClipEditorAutomationPointAtPlayhead),
            ]
            .spacing(8)
            .width(Length::FillPortion(1)),
            column![
                text("Points").size(12).color(theme::text_muted()),
                automation_points_row(active_lane, state.clip_editor.selected_automation_point),
                text(
                    active_lane
                        .map(|lane| {
                            format!(
                                "{} point(s)  |  {}",
                                lane.points.len(),
                                if lane.enabled { "enabled" } else { "disabled" }
                            )
                        })
                        .unwrap_or_else(|| "Keine Lane".to_owned())
                )
                .size(12)
                .color(theme::text_primary()),
            ]
            .spacing(8)
            .width(Length::FillPortion(2)),
        ]
        .spacing(14),
        row![
            column![
                text("Selected Point").size(12).color(theme::text_muted()),
                selected_point_card,
            ]
            .spacing(8)
            .width(Length::FillPortion(1)),
            column![
                text("Value").size(12).color(theme::text_muted()),
                automation_value_slider(state.clip_editor.automation_target, selected_point)
            ]
            .spacing(8)
            .width(Length::FillPortion(2)),
            column![
                text("Edit Point").size(12).color(theme::text_muted()),
                row![
                    button(text("<"))
                        .padding([8, 10])
                        .style(|_: &Theme, button_state| {
                            theme::toggle_button(button_state, true, theme::accent_playhead())
                        })
                        .on_press(AppEvent::NudgeClipEditorAutomationPointLeft),
                    button(text(">"))
                        .padding([8, 10])
                        .style(|_: &Theme, button_state| {
                            theme::toggle_button(button_state, true, theme::accent_playhead())
                        })
                        .on_press(AppEvent::NudgeClipEditorAutomationPointRight),
                    button(text("Delete"))
                        .padding([8, 12])
                        .style(|_: &Theme, button_state| {
                            theme::toggle_button(button_state, true, theme::warning())
                        })
                        .on_press(AppEvent::DeleteClipEditorAutomationPoint),
                ]
                .spacing(8),
            ]
            .spacing(8)
            .width(Length::FillPortion(1)),
        ]
        .spacing(14),
        row![cue_action, chase_action].spacing(10),
    ]
    .spacing(14);

    container(body)
        .padding(14)
        .style(|_| theme::panel_tinted(theme::accent_blue()))
        .into()
}

impl canvas::Program<AppEvent> for TimelineProgram {
    type State = TimelineCanvasState;

    fn update(
        &self,
        _state: &mut Self::State,
        event: canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (canvas::event::Status, Option<AppEvent>) {
        match event {
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some(cursor) = self
                    .cursor_info(bounds, cursor)
                    .or_else(|| self.cursor_info_anywhere(bounds, cursor))
                {
                    (
                        canvas::event::Status::Captured,
                        Some(AppEvent::Timeline(TimelineEvent::PointerMoved(cursor))),
                    )
                } else {
                    (
                        canvas::event::Status::Captured,
                        Some(AppEvent::Timeline(TimelineEvent::PointerExited)),
                    )
                }
            }
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => self
                .cursor_info(bounds, cursor)
                .map_or((canvas::event::Status::Ignored, None), |cursor| {
                    (
                        canvas::event::Status::Captured,
                        Some(AppEvent::Timeline(TimelineEvent::PointerPressed(cursor))),
                    )
                }),
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => self
                .cursor_info(bounds, cursor)
                .map_or((canvas::event::Status::Ignored, None), |cursor| {
                    (
                        canvas::event::Status::Captured,
                        Some(AppEvent::Timeline(TimelineEvent::SecondaryPressed(cursor))),
                    )
                }),
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => self
                .cursor_info(bounds, cursor)
                .or_else(|| self.cursor_info_anywhere(bounds, cursor))
                .map_or(
                    (
                        canvas::event::Status::Captured,
                        Some(AppEvent::Timeline(TimelineEvent::PointerExited)),
                    ),
                    |cursor| {
                        (
                            canvas::event::Status::Captured,
                            Some(AppEvent::Timeline(TimelineEvent::PointerReleased(cursor))),
                        )
                    },
                ),
            canvas::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let Some(anchor) = self.cursor_info(bounds, cursor) else {
                    return (canvas::event::Status::Ignored, None);
                };
                let delta_lines = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => y.round() as i16,
                    mouse::ScrollDelta::Pixels { y, .. } => (y / 48.0).round() as i16,
                };

                (
                    canvas::event::Status::Captured,
                    Some(AppEvent::Timeline(TimelineEvent::Scrolled {
                        delta_lines,
                        anchor_x_px: anchor.x_px,
                        anchor_beat: anchor.beat,
                    })),
                )
            }
            canvas::Event::Mouse(mouse::Event::CursorLeft) => {
                if self.timeline.interaction.captures_pointer() {
                    (canvas::event::Status::Ignored, None)
                } else {
                    (
                        canvas::event::Status::Captured,
                        Some(AppEvent::Timeline(TimelineEvent::PointerExited)),
                    )
                }
            }
            _ => (canvas::event::Status::Ignored, None),
        }
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        if state.grid_revision.get() != self.grid_revision {
            state.grid_cache.clear();
            state.grid_revision.set(self.grid_revision);
        }

        if state.clip_revision.get() != self.clip_revision {
            state.clip_cache.clear();
            state.clip_revision.set(self.clip_revision);
        }

        let background = state.grid_cache.draw(renderer, bounds.size(), |frame| {
            self.draw_background(frame, bounds.size());
        });

        let clips = state.clip_cache.draw(renderer, bounds.size(), |frame| {
            self.draw_clips(frame, bounds.size());
        });

        let mut overlay = canvas::Frame::new(renderer, bounds.size());
        self.draw_overlay(&mut overlay, bounds.size());

        vec![background, clips, overlay.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        match self.timeline.interaction {
            TimelineInteraction::PendingBoxSelection { .. }
            | TimelineInteraction::BoxSelecting { .. } => return mouse::Interaction::Crosshair,
            TimelineInteraction::PendingClipDrag { .. } => return mouse::Interaction::Grab,
            TimelineInteraction::DragClip { .. } => return mouse::Interaction::Grabbing,
            TimelineInteraction::PendingResizeClipStart { .. }
            | TimelineInteraction::PendingResizeClipEnd { .. } => {
                return mouse::Interaction::ResizingHorizontally;
            }
            TimelineInteraction::ResizeClipStart { .. }
            | TimelineInteraction::ResizeClipEnd { .. } => {
                return mouse::Interaction::ResizingHorizontally;
            }
            TimelineInteraction::AdjustClipParameter { .. } => {
                return mouse::Interaction::ResizingVertically;
            }
            TimelineInteraction::ScrubPlayhead => return mouse::Interaction::Crosshair,
            TimelineInteraction::Idle => {}
        }

        match self
            .cursor_info(bounds, cursor)
            .and_then(|cursor| cursor.target)
        {
            Some(TimelineHit::ContextAction(_)) => mouse::Interaction::Pointer,
            Some(TimelineHit::ClipParamHandle(_, _)) => mouse::Interaction::ResizingVertically,
            Some(
                TimelineHit::ClipCueHotspot(_, _)
                | TimelineHit::ClipChaseHotspot(_, _)
                | TimelineHit::ClipFxHotspot(_, _),
            ) => mouse::Interaction::Pointer,
            Some(TimelineHit::ClipStartHandle(_)) | Some(TimelineHit::ClipEndHandle(_)) => {
                mouse::Interaction::ResizingHorizontally
            }
            Some(TimelineHit::ClipBody(_)) => mouse::Interaction::Grab,
            Some(TimelineHit::Playhead) => mouse::Interaction::Crosshair,
            None if cursor.is_over(bounds) => {
                self.cursor_info(bounds, cursor)
                    .map_or(mouse::Interaction::Pointer, |cursor| match cursor.zone {
                        TimelineZone::Header => mouse::Interaction::Crosshair,
                        TimelineZone::Track if cursor.target.is_none() => {
                            mouse::Interaction::Crosshair
                        }
                        TimelineZone::Track | TimelineZone::Empty => mouse::Interaction::Pointer,
                    })
            }
            None => mouse::Interaction::None,
        }
    }
}

impl TimelineProgram {
    fn draw_background(&self, frame: &mut canvas::Frame<Renderer>, size: Size) {
        frame.fill_rectangle(Point::ORIGIN, size, theme::timeline_background());
        frame.fill_rectangle(
            Point::ORIGIN,
            size,
            Fill::from(
                gradient::Linear::new(Point::new(0.0, 0.0), Point::new(size.width, size.height))
                    .add_stop(0.0, Color::from_rgba8(44, 76, 122, 0.14))
                    .add_stop(0.36, Color::from_rgba8(26, 51, 76, 0.06))
                    .add_stop(1.0, Color::from_rgba8(0, 0, 0, 0.0)),
            ),
        );
        frame.fill_rectangle(
            Point::ORIGIN,
            Size::new(size.width, HEADER_HEIGHT),
            theme::timeline_header(),
        );
        frame.fill_rectangle(
            Point::ORIGIN,
            Size::new(size.width, HEADER_HEIGHT),
            Fill::from(
                gradient::Linear::new(Point::new(0.0, 0.0), Point::new(0.0, HEADER_HEIGHT))
                    .add_stop(0.0, Color::from_rgba8(255, 255, 255, 0.08))
                    .add_stop(1.0, Color::from_rgba8(255, 255, 255, 0.0)),
            ),
        );
        frame.fill_rectangle(
            Point::new(0.0, HEADER_HEIGHT - 1.0),
            Size::new(size.width, 1.0),
            Color::from_rgba8(142, 164, 189, 0.22),
        );

        let start_ticks = self.timeline.viewport.scroll.ticks();
        let end_ticks =
            start_ticks + ((size.width / self.pixels_per_beat()) * PPQ as f32).ceil() as u32 + PPQ;
        let bar_ticks = PPQ * 4;
        let first_bar = (start_ticks / bar_ticks).saturating_sub(1);
        let last_bar = (end_ticks / bar_ticks) + 1;

        for bar_index in first_bar..=last_bar {
            let bar_start = BeatTime::from_ticks(bar_index * bar_ticks);
            let bar_end = BeatTime::from_ticks((bar_index + 1) * bar_ticks);
            let x = self.beat_to_x(bar_start);
            let next_x = self.beat_to_x(bar_end);
            let width = (next_x - x).max(1.0);
            let band_color = if bar_index % 2 == 0 {
                Color::from_rgba8(255, 255, 255, 0.025)
            } else {
                Color::from_rgba8(114, 148, 201, 0.045)
            };

            frame.fill_rectangle(
                Point::new(x, 0.0),
                Size::new(width, HEADER_HEIGHT),
                band_color,
            );
            frame.fill_rectangle(
                Point::new(x, 0.0),
                Size::new(width, 3.0),
                Color::from_rgba8(151, 182, 226, if bar_index % 2 == 0 { 0.08 } else { 0.14 }),
            );
        }

        let step = self.subdivision_step().ticks().max(1);
        let first_step = (start_ticks / step).saturating_sub(1);
        let last_step = (end_ticks / step) + 1;

        for step_index in first_step..=last_step {
            let beat = BeatTime::from_ticks(step_index * step);
            let x = self.beat_to_x(beat);
            let is_bar = beat.ticks() % (PPQ * 4) == 0;
            let is_beat = beat.ticks() % PPQ == 0;
            let color = if is_bar {
                theme::grid_bar()
            } else if is_beat {
                theme::grid_beat()
            } else {
                theme::grid_subdivision()
            };
            let width = if is_bar {
                1.4
            } else if is_beat {
                1.0
            } else {
                0.8
            };

            frame.stroke(
                &Path::line(Point::new(x, 0.0), Point::new(x, size.height)),
                Stroke::default().with_color(color).with_width(width),
            );

            let tick_top = if is_bar {
                6.0
            } else if is_beat {
                16.0
            } else {
                24.0
            };
            frame.stroke(
                &Path::line(Point::new(x, tick_top), Point::new(x, HEADER_HEIGHT - 4.0)),
                Stroke::default()
                    .with_color(Color::from_rgba(color.r, color.g, color.b, 0.82))
                    .with_width(if is_bar { 1.4 } else { 1.0 }),
            );

            if is_bar && x > -48.0 && x < size.width + 48.0 {
                let pill = Path::rounded_rectangle(
                    Point::new(x + 7.0, 8.0),
                    Size::new(34.0, 14.0),
                    border::Radius::new(7.0),
                );
                frame.fill(&pill, Color::from_rgba8(255, 255, 255, 0.05));
                frame.fill_text(Text {
                    content: format!("{:02}.1", (beat.ticks() / (PPQ * 4)) + 1),
                    position: Point::new(x + 24.0, 15.0),
                    color: theme::text_primary(),
                    size: Pixels(10.5),
                    horizontal_alignment: alignment::Horizontal::Center,
                    vertical_alignment: alignment::Vertical::Center,
                    ..Text::default()
                });
            } else if is_beat && self.pixels_per_beat() > 54.0 && x > -28.0 && x < size.width + 28.0
            {
                frame.fill_text(Text {
                    content: format!("{}", (beat.ticks() / PPQ) + 1),
                    position: Point::new(x + 4.0, 28.0),
                    color: theme::text_muted(),
                    size: Pixels(10.0),
                    ..Text::default()
                });
            }
        }

        for (track_index, track) in self.timeline.tracks.iter().enumerate() {
            let y = self.track_y(track_index);
            let row_color = if track_index % 2 == 0 {
                Color::from_rgba8(19, 24, 31, 0.92)
            } else {
                Color::from_rgba8(22, 28, 36, 0.96)
            };

            frame.fill_rectangle(
                Point::new(0.0, y),
                Size::new(size.width, TRACK_HEIGHT),
                row_color,
            );

            if self.is_track_selected(track.id) {
                let accent = track.color.to_iced();
                frame.fill_rectangle(
                    Point::new(0.0, y),
                    Size::new(size.width, TRACK_HEIGHT),
                    Color::from_rgba(accent.r, accent.g, accent.b, 0.08),
                );
            }

            frame.stroke(
                &Path::line(
                    Point::new(0.0, y + TRACK_HEIGHT),
                    Point::new(size.width, y + TRACK_HEIGHT),
                ),
                Stroke::default()
                    .with_color(Color::from_rgba8(57, 68, 82, 0.72))
                    .with_width(1.0),
            );
        }

        frame.stroke(
            &Path::line(
                Point::new(0.0, HEADER_HEIGHT),
                Point::new(size.width, HEADER_HEIGHT),
            ),
            Stroke::default()
                .with_color(theme::border_strong())
                .with_width(1.0),
        );
    }

    fn draw_clips(&self, frame: &mut canvas::Frame<Renderer>, size: Size) {
        for (track_index, track) in self.timeline.tracks.iter().enumerate() {
            for clip in &track.clips {
                let mut rect = self.clip_rect(track_index, clip);

                if rect.x + rect.width < 0.0 || rect.x > size.width {
                    continue;
                }

                let is_selected = self.timeline.selected_clips.contains(&clip.id);
                let is_hovered = matches!(
                    self.timeline.hover,
                    Some(HoverTarget::ClipBody(clip_id)
                        | HoverTarget::ClipStartHandle(clip_id)
                        | HoverTarget::ClipEndHandle(clip_id))
                        if clip_id == clip.id
                );
                let is_active = self.timeline.interaction.active_clip() == Some(clip.id);

                if let Some(jitter) = self.snap_jitter(clip.id) {
                    rect.x += jitter;
                }

                let radius = border::Radius::new(10.0);
                let shadow_shape =
                    Path::rounded_rectangle(Point::new(rect.x, rect.y + 4.0), rect.size(), radius);
                let clip_shape = Path::rounded_rectangle(rect.position(), rect.size(), radius);
                let inner_sheen = Path::rounded_rectangle(
                    Point::new(rect.x + 1.0, rect.y + 1.0),
                    Size::new(rect.width - 2.0, rect.height * 0.48),
                    border::Radius::new(9.0),
                );

                let gradient = gradient::Linear::new(
                    Point::new(rect.x, rect.y),
                    Point::new(rect.x, rect.y + rect.height),
                )
                .add_stop(0.0, clip.palette.highlight.to_iced())
                .add_stop(1.0, clip.palette.base.to_iced());

                frame.fill(
                    &shadow_shape,
                    Color::from_rgba8(
                        0,
                        0,
                        0,
                        if is_selected {
                            0.2
                        } else if is_hovered {
                            0.15
                        } else {
                            0.1
                        },
                    ),
                );
                frame.fill(&clip_shape, Fill::from(gradient));
                frame.fill(&inner_sheen, Color::from_rgba8(255, 255, 255, 0.07));
                frame.fill_rectangle(
                    Point::new(rect.x + 1.0, rect.y + rect.height - 7.0),
                    Size::new(rect.width - 2.0, 5.0),
                    Color::from_rgba8(0, 0, 0, 0.12),
                );

                let edge = clip.palette.edge.to_iced();
                let border_color = if is_selected {
                    let pulse = 0.62 + (self.animation_phase() * 5.4).sin().abs() * 0.28;
                    Color::from_rgba(edge.r, edge.g, edge.b, pulse)
                } else if is_hovered {
                    Color::from_rgba(edge.r, edge.g, edge.b, 0.68)
                } else {
                    Color::from_rgba(edge.r, edge.g, edge.b, 0.34)
                };

                frame.stroke(
                    &clip_shape,
                    Stroke::default()
                        .with_color(border_color)
                        .with_width(if is_selected {
                            2.4
                        } else if is_active {
                            2.0
                        } else {
                            1.2
                        }),
                );

                self.draw_handles(frame, &rect, clip, is_selected || is_hovered || is_active);
                self.draw_markers(frame, &rect, clip);
                self.draw_semantic_badges(frame, &rect, clip);
                self.draw_fx_waveforms(frame, &rect, clip, is_selected);
                self.draw_automation_lanes(frame, &rect, clip, is_selected || is_active);
                self.draw_inline_parameter_handles(frame, &rect, clip, is_selected || is_active);
                self.draw_param_preview(frame, &rect, clip);
                self.draw_clip_text(frame, &rect, clip, is_selected);
            }
        }
    }

    fn draw_overlay(&self, frame: &mut canvas::Frame<Renderer>, size: Size) {
        self.draw_editor_focus(frame);
        self.draw_box_selection(frame);
        self.draw_context_menu(frame, size);

        if let Some(guide) = &self.timeline.snap.guide {
            self.draw_snap_guide(frame, size, guide);
        }

        let playhead_x = self.beat_to_x(self.transport.playhead);
        let highlight_playhead = self.timeline.interaction == TimelineInteraction::ScrubPlayhead;
        let show_playhead_label =
            highlight_playhead || matches!(self.timeline.hover, Some(HoverTarget::Playhead));
        let glow = Path::line(
            Point::new(playhead_x, 0.0),
            Point::new(playhead_x, size.height),
        );
        frame.stroke(
            &glow,
            Stroke::default()
                .with_width(5.0)
                .with_color(Color::from_rgba(
                    theme::accent_playhead().r,
                    theme::accent_playhead().g,
                    theme::accent_playhead().b,
                    if highlight_playhead { 0.2 } else { 0.12 },
                )),
        );
        frame.stroke(
            &glow,
            Stroke::default()
                .with_width(if highlight_playhead { 2.8 } else { 2.2 })
                .with_color(theme::accent_playhead()),
        );

        let cap_label = if show_playhead_label {
            self.transport.position_label()
        } else {
            "P".to_owned()
        };
        let cap_width = if show_playhead_label { 62.0 } else { 20.0 };
        let cap = Path::rounded_rectangle(
            Point::new(playhead_x - cap_width / 2.0, 6.0),
            Size::new(cap_width, 18.0),
            border::Radius::new(8.0),
        );
        frame.fill(&cap, theme::accent_playhead());
        frame.fill_text(Text {
            content: cap_label,
            position: Point::new(playhead_x, 15.5),
            color: Color::from_rgb8(18, 24, 30),
            size: Pixels(if show_playhead_label { 10.0 } else { 11.0 }),
            horizontal_alignment: alignment::Horizontal::Center,
            vertical_alignment: alignment::Vertical::Center,
            ..Text::default()
        });
    }

    fn draw_snap_guide(&self, frame: &mut canvas::Frame<Renderer>, size: Size, guide: &SnapGuide) {
        let x = self.beat_to_x(guide.beat);
        let alpha = (guide.strength_permille as f32 / 1000.0).clamp(0.0, 1.0) * 0.95;
        let locked = self.timeline.snap.phase == SnapPhase::Locked;
        let color = Color::from_rgba(
            theme::accent_snap().r,
            theme::accent_snap().g,
            theme::accent_snap().b,
            alpha,
        );
        if let Some(track_id) = guide.track
            && let Some(track_index) = self
                .timeline
                .tracks
                .iter()
                .position(|track| track.id == track_id)
        {
            let track_y = self.track_y(track_index);
            frame.fill_rectangle(
                Point::new(0.0, track_y),
                Size::new(size.width, TRACK_HEIGHT),
                Color::from_rgba(color.r, color.g, color.b, if locked { 0.08 } else { 0.05 }),
            );
        }

        frame.fill_rectangle(
            Point::new(x - 2.0, HEADER_HEIGHT),
            Size::new(4.0, size.height - HEADER_HEIGHT),
            Color::from_rgba(color.r, color.g, color.b, if locked { 0.16 } else { 0.08 }),
        );
        let path = Path::line(Point::new(x, HEADER_HEIGHT), Point::new(x, size.height));

        frame.stroke(
            &path,
            Stroke {
                style: canvas::Style::Solid(color),
                width: if locked { 2.4 } else { 1.8 },
                line_dash: canvas::LineDash {
                    segments: &SNAP_DASH,
                    offset: 0,
                },
                ..Stroke::default()
            },
        );

        let label = if locked { "LOCK" } else { "SNAP" };
        let marker = Path::rounded_rectangle(
            Point::new(x - 18.0, HEADER_HEIGHT + 6.0),
            Size::new(36.0, 16.0),
            border::Radius::new(8.0),
        );
        frame.fill(
            &marker,
            Color::from_rgba(color.r, color.g, color.b, if locked { 0.3 } else { 0.22 }),
        );
        frame.fill_text(Text {
            content: label.to_owned(),
            position: Point::new(x, HEADER_HEIGHT + 14.0),
            color,
            size: Pixels(10.0),
            horizontal_alignment: alignment::Horizontal::Center,
            vertical_alignment: alignment::Vertical::Center,
            ..Text::default()
        });
    }

    fn draw_box_selection(&self, frame: &mut canvas::Frame<Renderer>) {
        let Some(rect) = self.box_selection_rect() else {
            return;
        };

        let shape = Path::rounded_rectangle(
            Point::new(rect.x as f32, rect.y as f32),
            Size::new(rect.width as f32, rect.height as f32),
            border::Radius::new(8.0),
        );
        frame.fill(
            &shape,
            Color::from_rgba(
                theme::accent_blue().r,
                theme::accent_blue().g,
                theme::accent_blue().b,
                0.14,
            ),
        );
        frame.stroke(
            &shape,
            Stroke::default()
                .with_width(1.2)
                .with_color(Color::from_rgba(
                    theme::accent_blue().r,
                    theme::accent_blue().g,
                    theme::accent_blue().b,
                    0.74,
                )),
        );
    }

    fn draw_context_menu(&self, frame: &mut canvas::Frame<Renderer>, size: Size) {
        let Some(menu_rect) = self.context_menu_rect(size) else {
            return;
        };

        let background = Path::rounded_rectangle(
            menu_rect.position(),
            menu_rect.size(),
            border::Radius::new(10.0),
        );
        frame.fill(&background, Color::from_rgba8(19, 24, 31, 0.96));
        frame.stroke(
            &background,
            Stroke::default()
                .with_width(1.0)
                .with_color(Color::from_rgba8(255, 255, 255, 0.08)),
        );

        for (index, action) in self.context_menu_actions().iter().enumerate() {
            let item_rect = self.context_menu_item_rect(menu_rect, index);
            let item_shape = Path::rounded_rectangle(
                item_rect.position(),
                item_rect.size(),
                border::Radius::new(7.0),
            );
            frame.fill(
                &item_shape,
                Color::from_rgba8(255, 255, 255, if index % 2 == 0 { 0.04 } else { 0.02 }),
            );
            frame.fill_text(Text {
                content: self.context_action_label(*action).to_owned(),
                position: Point::new(item_rect.x + 10.0, item_rect.y + item_rect.height / 2.0),
                color: theme::text_primary(),
                size: Pixels(11.0),
                vertical_alignment: alignment::Vertical::Center,
                ..Text::default()
            });
        }
    }

    fn draw_handles(
        &self,
        frame: &mut canvas::Frame<Renderer>,
        rect: &Rectangle,
        clip: &Clip,
        emphasized: bool,
    ) {
        let edge = clip.palette.edge.to_iced();
        let handle_color = if emphasized {
            Color::from_rgba(edge.r, edge.g, edge.b, 0.82)
        } else {
            Color::from_rgba(edge.r, edge.g, edge.b, 0.44)
        };

        for x in [rect.x + 4.0, rect.x + rect.width - HANDLE_WIDTH - 4.0] {
            let handle = Path::rounded_rectangle(
                Point::new(x, rect.y + 6.0),
                Size::new(HANDLE_WIDTH, rect.height - 12.0),
                border::Radius::new(6.0),
            );
            frame.fill(&handle, Color::from_rgba8(255, 255, 255, 0.08));
            frame.stroke(
                &handle,
                Stroke::default().with_width(1.0).with_color(handle_color),
            );
        }
    }

    fn draw_markers(&self, frame: &mut canvas::Frame<Renderer>, rect: &Rectangle, clip: &Clip) {
        for marker in &clip.markers {
            let x = rect.x
                + (marker.offset.as_beats_f32() * self.pixels_per_beat()).min(rect.width - 18.0);
            frame.fill_rectangle(
                Point::new(x, rect.y + 7.0),
                Size::new(3.0, 12.0),
                marker.color.to_iced(),
            );
        }

        let cue_color = match clip.cue_state {
            CueVisualState::Active => theme::success(),
            CueVisualState::Ready => theme::warning(),
            CueVisualState::Inactive => theme::muted_chip(),
        };

        let cue_marker = Path::rounded_rectangle(
            Point::new(rect.x + rect.width - 26.0, rect.y + 7.0),
            Size::new(12.0, 12.0),
            border::Radius::new(6.0),
        );
        frame.fill(&cue_marker, cue_color);
    }

    fn draw_semantic_badges(
        &self,
        frame: &mut canvas::Frame<Renderer>,
        rect: &Rectangle,
        clip: &Clip,
    ) {
        for hotspot in self.clip_hotspots(rect, clip) {
            let badge = Path::rounded_rectangle(
                hotspot.rect.position(),
                hotspot.rect.size(),
                border::Radius::new(7.0),
            );
            frame.fill(
                &badge,
                Color::from_rgba(hotspot.accent.r, hotspot.accent.g, hotspot.accent.b, 0.2),
            );
            frame.stroke(
                &badge,
                Stroke::default()
                    .with_width(1.0)
                    .with_color(Color::from_rgba(
                        hotspot.accent.r,
                        hotspot.accent.g,
                        hotspot.accent.b,
                        0.48,
                    )),
            );
            frame.fill_text(Text {
                content: hotspot.label,
                position: Point::new(hotspot.rect.x + 8.0, hotspot.rect.y + 8.0),
                color: hotspot.accent,
                size: Pixels(9.0),
                vertical_alignment: alignment::Vertical::Center,
                ..Text::default()
            });
        }
    }

    fn draw_fx_waveforms(
        &self,
        frame: &mut canvas::Frame<Renderer>,
        rect: &Rectangle,
        clip: &Clip,
        clip_selected: bool,
    ) {
        let fx_layers = self.fx_layers_for_clip(clip.id);
        if fx_layers.is_empty() {
            return;
        }

        for (index, layer) in fx_layers.into_iter().take(2).enumerate() {
            let track_rect = Rectangle {
                x: rect.x + 16.0,
                y: rect.y + 24.0 + index as f32 * 10.0,
                width: (rect.width - 68.0).max(24.0).min(126.0),
                height: 8.0,
            };
            let accent = self.fx_accent(layer.kind);
            let alpha = 0.18 + (layer.output_level as f32 / 1000.0) * 0.52;

            frame.fill_rectangle(
                Point::new(track_rect.x, track_rect.y),
                Size::new(track_rect.width, track_rect.height),
                Color::from_rgba8(255, 255, 255, 0.05),
            );

            let waveform = Path::new(|builder| {
                let samples = 18;
                for sample in 0..=samples {
                    let sample_phase = sample as f32 / samples as f32;
                    let animated_phase = (sample_phase
                        + (self.frame_index as f32 * layer.rate.as_f32() / 420.0)
                        + (layer.phase_offset_permille as f32 / 1000.0))
                        .fract();
                    let y = track_rect.y
                        + track_rect.height
                            * (0.15
                                + self.waveform_preview_sample(layer.waveform, animated_phase)
                                    * (0.45 + layer.spread_permille as f32 / 2000.0));
                    let x = track_rect.x + sample_phase * track_rect.width;

                    if sample == 0 {
                        builder.move_to(Point::new(x, y));
                    } else {
                        builder.line_to(Point::new(x, y));
                    }
                }
            });

            frame.stroke(
                &waveform,
                Stroke::default()
                    .with_color(Color::from_rgba(accent.r, accent.g, accent.b, alpha))
                    .with_width(if clip_selected { 1.8 } else { 1.3 }),
            );

            let output_meter = Rectangle {
                x: rect.x + rect.width - 16.0,
                y: rect.y + 22.0 + index as f32 * 12.0,
                width: 4.0,
                height: 10.0,
            };
            frame.fill_rectangle(
                Point::new(output_meter.x, output_meter.y),
                Size::new(output_meter.width, output_meter.height),
                Color::from_rgba8(255, 255, 255, 0.08),
            );
            frame.fill_rectangle(
                Point::new(
                    output_meter.x,
                    output_meter.y
                        + output_meter.height * (1.0 - layer.output_level as f32 / 1000.0),
                ),
                Size::new(
                    output_meter.width,
                    output_meter.height * (layer.output_level as f32 / 1000.0),
                ),
                accent,
            );

            frame.fill_text(Text {
                content: format!("{} {}", layer.name, layer.output_level / 10),
                position: Point::new(track_rect.x + track_rect.width + 8.0, track_rect.y + 5.5),
                color: Color::from_rgba8(228, 235, 242, 0.72),
                size: Pixels(8.8),
                ..Text::default()
            });
        }
    }

    fn draw_automation_lanes(
        &self,
        frame: &mut canvas::Frame<Renderer>,
        rect: &Rectangle,
        clip: &Clip,
        emphasized: bool,
    ) {
        let lane_width = (rect.width - 76.0).max(28.0).min(140.0);

        for (index, lane) in clip
            .automation
            .iter()
            .filter(|lane| lane.enabled)
            .take(3)
            .enumerate()
        {
            if lane.points.len() < 2 {
                continue;
            }

            let lane_rect = Rectangle {
                x: rect.x + 18.0,
                y: rect.y + rect.height - 24.0 - index as f32 * 10.0,
                width: lane_width,
                height: 8.0,
            };
            let accent = match lane.target {
                AutomationTarget::Intensity => theme::warning(),
                AutomationTarget::Speed => theme::accent_blue(),
                AutomationTarget::FxDepth => theme::success(),
            };

            frame.fill_rectangle(
                Point::new(lane_rect.x, lane_rect.y),
                Size::new(lane_rect.width, lane_rect.height),
                Color::from_rgba8(255, 255, 255, 0.04),
            );

            let path = Path::new(|builder| {
                for (point_index, point) in lane.points.iter().enumerate() {
                    let normalized_x =
                        point.offset.ticks() as f32 / clip.duration.ticks().max(1) as f32;
                    let normalized_y = automation_normalized_value(lane.target, point.value);
                    let x = lane_rect.x + normalized_x * lane_rect.width;
                    let y = lane_rect.y + (1.0 - normalized_y) * lane_rect.height;

                    if point_index == 0 {
                        builder.move_to(Point::new(x, y));
                    } else {
                        builder.line_to(Point::new(x, y));
                    }
                }
            });

            frame.stroke(
                &path,
                Stroke::default()
                    .with_width(if emphasized { 1.5 } else { 1.1 })
                    .with_color(Color::from_rgba(
                        accent.r,
                        accent.g,
                        accent.b,
                        if emphasized { 0.88 } else { 0.62 },
                    )),
            );
        }
    }

    fn draw_inline_parameter_handles(
        &self,
        frame: &mut canvas::Frame<Renderer>,
        rect: &Rectangle,
        clip: &Clip,
        emphasized: bool,
    ) {
        for handle in self.clip_param_handles(rect, clip) {
            let knob_center_y = handle.knob_rect.y + handle.knob_rect.height / 2.0;
            let track_shape = Path::rounded_rectangle(
                handle.track_rect.position(),
                handle.track_rect.size(),
                border::Radius::new(5.0),
            );
            frame.fill(
                &track_shape,
                Color::from_rgba8(255, 255, 255, if emphasized { 0.13 } else { 0.08 }),
            );
            frame.stroke(
                &track_shape,
                Stroke::default()
                    .with_width(1.0)
                    .with_color(Color::from_rgba(
                        handle.accent.r,
                        handle.accent.g,
                        handle.accent.b,
                        if emphasized { 0.62 } else { 0.34 },
                    )),
            );

            let fill_height = (handle.track_rect.y + handle.track_rect.height) - knob_center_y;
            frame.fill_rectangle(
                Point::new(handle.track_rect.x + 2.0, knob_center_y),
                Size::new(handle.track_rect.width - 4.0, fill_height.max(0.0)),
                Color::from_rgba(handle.accent.r, handle.accent.g, handle.accent.b, 0.24),
            );

            let knob = Path::rounded_rectangle(
                handle.knob_rect.position(),
                handle.knob_rect.size(),
                border::Radius::new(4.0),
            );
            frame.fill(&knob, handle.accent);
            frame.stroke(
                &knob,
                Stroke::default()
                    .with_width(1.0)
                    .with_color(Color::from_rgba8(16, 22, 28, 0.9)),
            );

            frame.fill_text(Text {
                content: handle.label.to_owned(),
                position: Point::new(
                    handle.track_rect.x + handle.track_rect.width / 2.0,
                    handle.track_rect.y - 6.0,
                ),
                color: Color::from_rgba8(228, 235, 242, if emphasized { 0.82 } else { 0.62 }),
                size: Pixels(8.8),
                horizontal_alignment: alignment::Horizontal::Center,
                vertical_alignment: alignment::Vertical::Center,
                ..Text::default()
            });
        }
    }

    fn draw_param_preview(
        &self,
        frame: &mut canvas::Frame<Renderer>,
        rect: &Rectangle,
        clip: &Clip,
    ) {
        let bars = [
            (
                theme::warning(),
                clip.params.intensity.permille() as f32 / 1000.0,
            ),
            (
                theme::accent_blue(),
                clip.params.speed.permille() as f32 / 1500.0,
            ),
            (
                theme::success(),
                clip.params.fx_depth.permille() as f32 / 1000.0,
            ),
        ];

        for (index, (accent, amount)) in bars.iter().enumerate() {
            let track = Rectangle {
                x: rect.x + 16.0 + index as f32 * 46.0,
                y: rect.y + rect.height - 8.0,
                width: 34.0,
                height: 3.0,
            };
            frame.fill_rectangle(
                Point::new(track.x, track.y),
                Size::new(track.width, track.height),
                Color::from_rgba8(255, 255, 255, 0.12),
            );
            frame.fill_rectangle(
                Point::new(track.x, track.y),
                Size::new(track.width * amount.clamp(0.0, 1.0), track.height),
                *accent,
            );
        }
    }

    fn draw_clip_text(
        &self,
        frame: &mut canvas::Frame<Renderer>,
        rect: &Rectangle,
        clip: &Clip,
        selected: bool,
    ) {
        frame.fill_text(Text {
            content: clip.title.clone(),
            position: Point::new(rect.x + 16.0, rect.y + 16.0),
            color: theme::text_primary(),
            size: Pixels(15.0),
            ..Text::default()
        });

        let descriptor = format!(
            "{}  |  {:.2} beats  |  {}  |  {}  |  {}",
            if selected { "Selected" } else { "Timeline FX" },
            clip.duration.as_beats_f32(),
            clip.params.bpm_grid,
            self.fx_layers_for_clip(clip.id)
                .into_iter()
                .take(2)
                .map(|fx| format!("{}:{}%", fx.waveform, fx.output_level / 10))
                .collect::<Vec<_>>()
                .join(" · "),
            clip.markers
                .iter()
                .map(|marker| marker.label.as_str())
                .collect::<Vec<_>>()
                .join(" · ")
        );

        frame.fill_text(Text {
            content: descriptor,
            position: Point::new(rect.x + 16.0, rect.y + 36.0),
            color: Color::from_rgba8(228, 235, 242, 0.75),
            size: Pixels(11.0),
            ..Text::default()
        });
    }

    fn draw_editor_focus(&self, frame: &mut canvas::Frame<Renderer>) {
        if self.clip_editor.phase == ClipEditorPhase::Closed {
            return;
        }

        let Some(clip_id) = self.clip_editor.clip_id else {
            return;
        };

        let Some((track_index, clip)) = self.clip_with_track(clip_id) else {
            return;
        };

        let rect = self.clip_rect(track_index, clip);
        let glow = Path::rounded_rectangle(
            Point::new(rect.x - 6.0, rect.y - 6.0),
            Size::new(rect.width + 12.0, rect.height + 12.0),
            border::Radius::new(14.0),
        );

        frame.stroke(
            &glow,
            Stroke::default()
                .with_width(2.0)
                .with_color(Color::from_rgba(
                    theme::accent_blue().r,
                    theme::accent_blue().g,
                    theme::accent_blue().b,
                    0.72,
                )),
        );
    }

    fn cursor_info(&self, bounds: Rectangle, cursor: mouse::Cursor) -> Option<TimelineCursor> {
        let position = cursor.position_in(bounds)?;
        Some(self.cursor_info_at(position))
    }

    fn cursor_info_anywhere(
        &self,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<TimelineCursor> {
        if !self.timeline.interaction.captures_pointer() {
            return None;
        }

        let position = cursor.position_from(Point::new(bounds.x, bounds.y))?;
        Some(self.cursor_info_at(position))
    }

    fn cursor_info_at(&self, position: Point) -> TimelineCursor {
        let beat = self.x_to_beat(position.x).max(BeatTime::ZERO);
        let zone = if position.y <= HEADER_HEIGHT {
            TimelineZone::Header
        } else if self.track_at_y(position.y).is_some() {
            TimelineZone::Track
        } else {
            TimelineZone::Empty
        };
        let track = self.track_at_y(position.y);
        let target = self
            .context_menu_hit(position)
            .or_else(|| self.hit_test(position));

        TimelineCursor {
            beat,
            track,
            zone,
            target,
            x_px: position.x.round() as i32,
            y_px: position.y.round() as i32,
        }
    }

    fn context_menu_hit(&self, position: Point) -> Option<TimelineHit> {
        let rect = self.context_menu_rect(Size::new(10_000.0, 10_000.0))?;
        if !rect.contains(position) {
            return None;
        }

        self.context_menu_actions()
            .iter()
            .enumerate()
            .find_map(|(index, action)| {
                self.context_menu_item_rect(rect, index)
                    .contains(position)
                    .then_some(TimelineHit::ContextAction(*action))
            })
    }

    fn hit_test(&self, position: Point) -> Option<TimelineHit> {
        if let Some(track_id) = self.track_at_y(position.y) {
            if let Some((_, track_index, clip)) = self
                .timeline
                .tracks
                .iter()
                .enumerate()
                .flat_map(|(track_index, track)| {
                    track
                        .clips
                        .iter()
                        .map(move |clip| (track.id, track_index, clip))
                })
                .find(|(current_track, track_index, clip)| {
                    if *current_track != track_id {
                        return false;
                    }
                    self.clip_rect(*track_index, clip).contains(position)
                })
            {
                let rect = self.clip_rect(track_index, clip);
                if let Some(hotspot) = self
                    .clip_hotspots(&rect, clip)
                    .into_iter()
                    .find(|hotspot| hotspot.rect.contains(position))
                {
                    return Some(hotspot.hit);
                }
                if let Some(handle) =
                    self.clip_param_handles(&rect, clip)
                        .into_iter()
                        .find(|handle| {
                            handle.knob_rect.contains(position)
                                || handle.track_rect.contains(position)
                        })
                {
                    return Some(TimelineHit::ClipParamHandle(clip.id, handle.kind));
                }
                if position.x <= rect.x + HANDLE_WIDTH + 8.0 {
                    return Some(TimelineHit::ClipStartHandle(clip.id));
                }
                if position.x >= rect.x + rect.width - HANDLE_WIDTH - 8.0 {
                    return Some(TimelineHit::ClipEndHandle(clip.id));
                }
                return Some(TimelineHit::ClipBody(clip.id));
            }
        }

        let playhead_x = self.beat_to_x(self.transport.playhead);
        if (position.x - playhead_x).abs() <= PLAYHEAD_HIT_RADIUS {
            return Some(TimelineHit::Playhead);
        }

        None
    }

    fn track_at_y(&self, y: f32) -> Option<TrackId> {
        self.timeline
            .tracks
            .iter()
            .enumerate()
            .find_map(|(index, track)| {
                let track_y = self.track_y(index);
                let in_track = y >= track_y && y <= track_y + TRACK_HEIGHT;
                in_track.then_some(track.id)
            })
    }

    fn context_menu_actions(&self) -> Vec<ContextMenuAction> {
        vec![
            ContextMenuAction::Copy,
            ContextMenuAction::Cut,
            ContextMenuAction::Paste,
            ContextMenuAction::Duplicate,
            ContextMenuAction::Split,
            ContextMenuAction::Delete,
            ContextMenuAction::NudgeLeft,
            ContextMenuAction::NudgeRight,
            ContextMenuAction::SelectAllOnTrack,
            ContextMenuAction::TrimToPlayhead,
            ContextMenuAction::Close,
        ]
    }

    fn context_action_label(&self, action: ContextMenuAction) -> &'static str {
        match action {
            ContextMenuAction::Copy => "Copy",
            ContextMenuAction::Cut => "Cut",
            ContextMenuAction::Paste => "Paste @ Playhead",
            ContextMenuAction::Duplicate => "Duplicate",
            ContextMenuAction::Split => "Split @ Playhead",
            ContextMenuAction::Delete => "Delete",
            ContextMenuAction::NudgeLeft => "Nudge Left",
            ContextMenuAction::NudgeRight => "Nudge Right",
            ContextMenuAction::SelectAllOnTrack => "Select Track Clips",
            ContextMenuAction::TrimToPlayhead => "Trim To Playhead",
            ContextMenuAction::Close => "Close",
        }
    }

    fn context_menu_rect(&self, size: Size) -> Option<Rectangle> {
        if !self.context_menu.open {
            return None;
        }

        let height = self.context_menu_actions().len() as f32 * CONTEXT_MENU_ITEM_HEIGHT + 12.0;
        let x = (self.context_menu.x_px as f32)
            .min(size.width - CONTEXT_MENU_WIDTH - 8.0)
            .max(8.0);
        let y = (self.context_menu.y_px as f32)
            .min(size.height - height - 8.0)
            .max(8.0);

        Some(Rectangle {
            x,
            y,
            width: CONTEXT_MENU_WIDTH,
            height,
        })
    }

    fn context_menu_item_rect(&self, menu_rect: Rectangle, index: usize) -> Rectangle {
        Rectangle {
            x: menu_rect.x + 6.0,
            y: menu_rect.y + 6.0 + index as f32 * CONTEXT_MENU_ITEM_HEIGHT,
            width: menu_rect.width - 12.0,
            height: CONTEXT_MENU_ITEM_HEIGHT - 2.0,
        }
    }

    fn clip_rect(&self, track_index: usize, clip: &Clip) -> Rectangle {
        Rectangle {
            x: self.beat_to_x(clip.start),
            y: self.track_y(track_index) + CLIP_TOP_INSET,
            width: (clip.duration.as_beats_f32() * self.pixels_per_beat()).max(36.0),
            height: CLIP_HEIGHT,
        }
    }

    fn cue_for_clip(&self, clip: &Clip) -> Option<&Cue> {
        let cue_id = clip.linked_cue?;
        self.cue_system.cues.iter().find(|cue| cue.id == cue_id)
    }

    fn chases_for_clip(&self, clip_id: ClipId) -> Vec<&Chase> {
        self.chase_system
            .chases
            .iter()
            .filter(|chase| chase.linked_clip == Some(clip_id))
            .collect()
    }

    fn fx_layers_for_clip(&self, clip_id: ClipId) -> Vec<&FxLayer> {
        self.fx_system
            .layers
            .iter()
            .filter(|layer| layer.linked_clip == Some(clip_id))
            .collect()
    }

    fn clip_with_track(&self, clip_id: ClipId) -> Option<(usize, &Clip)> {
        self.timeline
            .tracks
            .iter()
            .enumerate()
            .find_map(|(track_index, track)| {
                track
                    .clips
                    .iter()
                    .find(|clip| clip.id == clip_id)
                    .map(|clip| (track_index, clip))
            })
    }

    fn clip_hotspots(&self, rect: &Rectangle, clip: &Clip) -> Vec<ClipHotspot> {
        let mut hotspots = Vec::new();
        let mut x = rect.x + 12.0;
        let y = rect.y + rect.height - 20.0;

        if let Some(cue) = self.cue_for_clip(clip) {
            let label = format!("CUE {}", cue.name);
            let width = self.hotspot_width(&label);
            hotspots.push(ClipHotspot {
                hit: TimelineHit::ClipCueHotspot(clip.id, cue.id),
                rect: Rectangle {
                    x,
                    y,
                    width,
                    height: 14.0,
                },
                accent: theme::warning(),
                label,
            });
            x += width + 6.0;
        }

        if let Some(chase) = self.chases_for_clip(clip.id).first() {
            let label = format!("CHASE {}", chase.name);
            let width = self.hotspot_width(&label);
            hotspots.push(ClipHotspot {
                hit: TimelineHit::ClipChaseHotspot(clip.id, chase.id),
                rect: Rectangle {
                    x,
                    y,
                    width,
                    height: 14.0,
                },
                accent: theme::accent_blue(),
                label,
            });
            x += width + 6.0;
        }

        for fx in self.fx_layers_for_clip(clip.id).into_iter().take(2) {
            let label = format!("FX {} {:>02}%", fx.waveform, fx.output_level / 10);
            let width = self.hotspot_width(&label);
            hotspots.push(ClipHotspot {
                hit: TimelineHit::ClipFxHotspot(clip.id, fx.id),
                rect: Rectangle {
                    x,
                    y,
                    width,
                    height: 14.0,
                },
                accent: self.fx_accent(fx.kind),
                label,
            });
            x += width + 6.0;
        }

        hotspots
            .into_iter()
            .filter(|hotspot| hotspot.rect.x + hotspot.rect.width <= rect.x + rect.width - 58.0)
            .collect()
    }

    fn clip_param_handles(&self, rect: &Rectangle, clip: &Clip) -> Vec<ClipParamHandle> {
        let kinds = [
            ClipInlineParameterKind::Intensity,
            ClipInlineParameterKind::Speed,
            ClipInlineParameterKind::FxDepth,
        ];
        let labels = ["I", "S", "FX"];
        let accents = [theme::warning(), theme::accent_blue(), theme::success()];
        let total_width = (INLINE_PARAM_TRACK_WIDTH * 3.0) + (INLINE_PARAM_SPACING * 2.0) + 6.0;
        let start_x = (rect.x + rect.width - total_width - 10.0).max(rect.x + 16.0);
        let track_y = rect.y + 12.0;

        kinds
            .into_iter()
            .enumerate()
            .map(|(index, kind)| {
                let amount = self.clip_param_amount(clip, kind).clamp(0.0, 1.0);
                let x = start_x + index as f32 * (INLINE_PARAM_TRACK_WIDTH + INLINE_PARAM_SPACING);
                let knob_y = track_y + (1.0 - amount) * INLINE_PARAM_TRACK_HEIGHT;
                ClipParamHandle {
                    kind,
                    track_rect: Rectangle {
                        x,
                        y: track_y,
                        width: INLINE_PARAM_TRACK_WIDTH,
                        height: INLINE_PARAM_TRACK_HEIGHT,
                    },
                    knob_rect: Rectangle {
                        x: x - 1.0,
                        y: knob_y - INLINE_PARAM_KNOB_SIZE / 2.0,
                        width: INLINE_PARAM_TRACK_WIDTH + 2.0,
                        height: INLINE_PARAM_KNOB_SIZE,
                    },
                    accent: accents[index],
                    label: labels[index],
                }
            })
            .collect()
    }

    fn clip_param_amount(&self, clip: &Clip, kind: ClipInlineParameterKind) -> f32 {
        match kind {
            ClipInlineParameterKind::Intensity => clip.params.intensity.permille() as f32 / 1000.0,
            ClipInlineParameterKind::Speed => {
                (clip.params.speed.permille().saturating_sub(200)) as f32 / 1300.0
            }
            ClipInlineParameterKind::FxDepth => clip.params.fx_depth.permille() as f32 / 1000.0,
        }
    }

    fn hotspot_width(&self, label: &str) -> f32 {
        22.0 + (label.len() as f32 * 4.5)
    }

    fn pixels_per_beat(&self) -> f32 {
        40.0 * self.timeline.viewport.zoom.as_f32()
    }

    fn x_to_beat(&self, x: f32) -> BeatTime {
        let ticks = self.timeline.viewport.scroll.ticks() as f32
            + (x / self.pixels_per_beat() * PPQ as f32);
        BeatTime::from_ticks(ticks.max(0.0).round() as u32)
    }

    fn beat_to_x(&self, beat: BeatTime) -> f32 {
        (beat.ticks() as f32 - self.timeline.viewport.scroll.ticks() as f32) / PPQ as f32
            * self.pixels_per_beat()
    }

    fn track_y(&self, track_index: usize) -> f32 {
        HEADER_HEIGHT + (track_index as f32 * (TRACK_HEIGHT + TRACK_GAP))
    }

    fn subdivision_step(&self) -> BeatTime {
        let pixels_per_beat = self.pixels_per_beat();

        if pixels_per_beat >= 120.0 {
            BeatTime::from_fraction(1, 4)
        } else if pixels_per_beat >= 72.0 {
            BeatTime::from_fraction(1, 2)
        } else {
            BeatTime::from_beats(1)
        }
    }

    fn is_track_selected(&self, track_id: TrackId) -> bool {
        match self.timeline.selection {
            SelectionState::Track(selected_track) => selected_track == track_id,
            SelectionState::Clip(_) => self
                .timeline
                .tracks
                .iter()
                .find(|track| track.id == track_id)
                .map(|track| {
                    track
                        .clips
                        .iter()
                        .any(|clip| self.timeline.selected_clips.contains(&clip.id))
                })
                .unwrap_or(false),
            SelectionState::None => false,
        }
    }

    fn box_selection_rect(&self) -> Option<PixelRect> {
        match self.timeline.interaction {
            TimelineInteraction::PendingBoxSelection {
                origin_x_px,
                origin_y_px,
                current_x_px,
                current_y_px,
                ..
            }
            | TimelineInteraction::BoxSelecting {
                origin_x_px,
                origin_y_px,
                current_x_px,
                current_y_px,
            } => Some(PixelRect::from_points(
                origin_x_px,
                origin_y_px,
                current_x_px,
                current_y_px,
            )),
            _ => None,
        }
    }

    fn snap_jitter(&self, clip_id: ClipId) -> Option<f32> {
        let active_clip = self.timeline.interaction.active_clip()?;
        let guide = self.timeline.snap.guide.as_ref()?;

        if active_clip != clip_id {
            return None;
        }

        let strength = guide.strength_permille as f32 / 1000.0;
        Some((self.animation_phase() * 48.0).sin() * strength * 1.4)
    }

    fn animation_phase(&self) -> f32 {
        self.frame_index as f32 / 60.0
    }

    fn fx_accent(&self, kind: FxKind) -> Color {
        match kind {
            FxKind::Color => theme::success(),
            FxKind::Intensity => theme::warning(),
            FxKind::Position => theme::accent_blue(),
        }
    }

    fn waveform_preview_sample(&self, waveform: FxWaveform, phase: f32) -> f32 {
        let phase = phase.fract();
        match waveform {
            FxWaveform::Sine => (phase * std::f32::consts::TAU).sin() * 0.5 + 0.5,
            FxWaveform::Triangle => {
                if phase < 0.5 {
                    phase * 2.0
                } else {
                    (1.0 - phase) * 2.0
                }
            }
            FxWaveform::Saw => phase,
            FxWaveform::Pulse => {
                if phase < 0.32 {
                    1.0
                } else {
                    0.18
                }
            }
        }
        .clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;

    #[test]
    fn clip_hotspots_are_generated_for_linked_entities() {
        let state = StudioState::default();
        let program = TimelineProgram {
            transport: state.engine.transport.clone(),
            timeline: state.timeline.clone(),
            clip_editor: state.clip_editor.clone(),
            context_menu: state.context_menu.clone(),
            cue_system: state.cue_system.clone(),
            chase_system: state.chase_system.clone(),
            fx_system: state.fx_system.clone(),
            frame_index: 32,
            grid_revision: state.revisions.grid,
            clip_revision: state.revisions.clips,
        };

        let clip = state.clip(ClipId(102)).expect("clip exists");
        let rect = Rectangle {
            x: 100.0,
            y: 40.0,
            width: 240.0,
            height: 52.0,
        };
        let hotspots = program.clip_hotspots(&rect, clip);

        assert!(
            hotspots
                .iter()
                .any(|hotspot| matches!(hotspot.hit, TimelineHit::ClipCueHotspot(ClipId(102), _)))
        );
        assert!(
            hotspots
                .iter()
                .any(|hotspot| matches!(hotspot.hit, TimelineHit::ClipFxHotspot(ClipId(102), _)))
        );
    }

    #[test]
    fn clip_param_handles_are_generated_for_inline_editing() {
        let state = StudioState::default();
        let program = TimelineProgram {
            transport: state.engine.transport.clone(),
            timeline: state.timeline.clone(),
            clip_editor: state.clip_editor.clone(),
            context_menu: state.context_menu.clone(),
            cue_system: state.cue_system.clone(),
            chase_system: state.chase_system.clone(),
            fx_system: state.fx_system.clone(),
            frame_index: 32,
            grid_revision: state.revisions.grid,
            clip_revision: state.revisions.clips,
        };

        let clip = state.clip(ClipId(102)).expect("clip exists");
        let rect = Rectangle {
            x: 100.0,
            y: 40.0,
            width: 240.0,
            height: 52.0,
        };
        let handles = program.clip_param_handles(&rect, clip);

        assert_eq!(handles.len(), 3);
        assert!(handles.iter().any(|handle| {
            handle.kind == ClipInlineParameterKind::Intensity && handle.knob_rect.y >= rect.y
        }));
    }

    #[test]
    fn waveform_preview_sample_is_deterministic_and_bounded() {
        let state = StudioState::default();
        let program = TimelineProgram {
            transport: state.engine.transport.clone(),
            timeline: state.timeline.clone(),
            clip_editor: state.clip_editor.clone(),
            context_menu: state.context_menu.clone(),
            cue_system: state.cue_system.clone(),
            chase_system: state.chase_system.clone(),
            fx_system: state.fx_system.clone(),
            frame_index: 32,
            grid_revision: state.revisions.grid,
            clip_revision: state.revisions.clips,
        };

        let left = program.waveform_preview_sample(FxWaveform::Triangle, 0.35);
        let right = program.waveform_preview_sample(FxWaveform::Triangle, 0.35);

        assert_eq!(left, right);
        assert!((0.0..=1.0).contains(&left));
    }

    #[test]
    fn cursor_info_anywhere_supports_dragging_outside_canvas_bounds() {
        let state = StudioState::default();
        let mut timeline = state.timeline.clone();
        timeline.interaction = TimelineInteraction::DragClip {
            clip_id: ClipId(102),
            origin_track: TrackId(1),
            origin_start: BeatTime::from_beats(8),
            pointer_origin: BeatTime::from_beats(8),
        };

        let program = TimelineProgram {
            transport: state.engine.transport.clone(),
            timeline,
            clip_editor: state.clip_editor.clone(),
            context_menu: state.context_menu.clone(),
            cue_system: state.cue_system.clone(),
            chase_system: state.chase_system.clone(),
            fx_system: state.fx_system.clone(),
            frame_index: 32,
            grid_revision: state.revisions.grid,
            clip_revision: state.revisions.clips,
        };

        let bounds = Rectangle {
            x: 40.0,
            y: 20.0,
            width: 320.0,
            height: 260.0,
        };
        let cursor = mouse::Cursor::Available(Point::new(bounds.x + bounds.width + 24.0, 112.0));

        let timeline_cursor = program
            .cursor_info_anywhere(bounds, cursor)
            .expect("cursor available outside bounds");

        assert!(timeline_cursor.x_px > bounds.width as i32);
        assert_eq!(timeline_cursor.zone, TimelineZone::Track);
    }

    #[test]
    fn box_selection_rect_is_available_during_marquee_selection() {
        let state = StudioState::default();
        let mut timeline = state.timeline.clone();
        timeline.interaction = TimelineInteraction::BoxSelecting {
            origin_x_px: 12,
            origin_y_px: 44,
            current_x_px: 420,
            current_y_px: 114,
        };

        let program = TimelineProgram {
            transport: state.engine.transport.clone(),
            timeline,
            clip_editor: state.clip_editor.clone(),
            context_menu: state.context_menu.clone(),
            cue_system: state.cue_system.clone(),
            chase_system: state.chase_system.clone(),
            fx_system: state.fx_system.clone(),
            frame_index: 32,
            grid_revision: state.revisions.grid,
            clip_revision: state.revisions.clips,
        };

        let rect = program.box_selection_rect().expect("selection rect");

        assert_eq!(rect.x, 12);
        assert_eq!(rect.width, 408);
        assert_eq!(rect.height, 70);
    }
}
