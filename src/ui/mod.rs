pub mod fixture_view;
pub mod theme;
pub mod timeline;

use crate::core::{
    AppEvent, BeatTime, Chase, ChaseDirection, ChasePhase, ContextMenuAction, ContextMenuTarget,
    Cue, CueId, CuePhase, FixtureGroup, FixturePhase, FxKind, FxLayer, FxPhase, FxWaveform,
    RgbaColor, SelectionState, StudioState, Track,
};
use iced::widget::{
    button, column, container, pick_list, row, scrollable, slider, text, text_input,
};
use iced::{Alignment, Element, Length, Theme};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
struct CueChoice {
    id: Option<CueId>,
    label: String,
}

impl fmt::Display for CueChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BeatTimeChoice {
    value: BeatTime,
    label: String,
}

impl fmt::Display for BeatTimeChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ColorChoice {
    value: RgbaColor,
    label: String,
}

impl fmt::Display for ColorChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

fn cue_choices(state: &StudioState) -> Vec<CueChoice> {
    let mut options = vec![CueChoice {
        id: None,
        label: "No Cue".to_owned(),
    }];
    options.extend(state.cue_system.cues.iter().map(|cue| CueChoice {
        id: Some(cue.id),
        label: format!("{} ({})", cue.name, cue.id.0),
    }));
    options
}

fn selected_cue_choice(options: &[CueChoice], cue_id: Option<CueId>) -> Option<CueChoice> {
    options.iter().find(|option| option.id == cue_id).cloned()
}

fn beat_time_choices(current: Option<BeatTime>) -> Vec<BeatTimeChoice> {
    let mut options = Vec::new();
    let presets = [
        ("1/4 Beat", BeatTime::from_fraction(1, 4)),
        ("1/2 Beat", BeatTime::from_fraction(1, 2)),
        ("3/4 Beat", BeatTime::from_fraction(3, 4)),
        ("1 Beat", BeatTime::from_beats(1)),
        ("2 Beats", BeatTime::from_beats(2)),
    ];

    if let Some(current) = current
        && presets.iter().all(|(_, value)| *value != current)
    {
        options.push(BeatTimeChoice {
            value: current,
            label: format!("{:.2} Beats", current.as_beats_f32()),
        });
    }

    options.extend(presets.into_iter().map(|(label, value)| BeatTimeChoice {
        value,
        label: label.to_owned(),
    }));
    options
}

fn selected_beat_time_choice(
    options: &[BeatTimeChoice],
    value: BeatTime,
) -> Option<BeatTimeChoice> {
    options.iter().find(|option| option.value == value).cloned()
}

fn color_choices(current: Option<RgbaColor>) -> Vec<ColorChoice> {
    let mut options = Vec::new();
    let presets = [
        ("Amber", RgbaColor::rgb(255, 196, 120)),
        ("Aqua", RgbaColor::rgb(117, 234, 214)),
        ("Rose", RgbaColor::rgb(255, 138, 153)),
        ("Gold", RgbaColor::rgb(255, 221, 159)),
        ("Ice", RgbaColor::rgb(204, 218, 255)),
        ("Sky", RgbaColor::rgb(104, 164, 255)),
        ("Warm", RgbaColor::rgb(255, 232, 191)),
    ];

    if let Some(current) = current
        && presets.iter().all(|(_, value)| *value != current)
    {
        options.push(ColorChoice {
            value: current,
            label: format!("Current ({}, {}, {})", current.r, current.g, current.b),
        });
    }

    options.extend(presets.into_iter().map(|(label, value)| ColorChoice {
        value,
        label: label.to_owned(),
    }));
    options
}

fn selected_color_choice(options: &[ColorChoice], value: RgbaColor) -> Option<ColorChoice> {
    options.iter().find(|option| option.value == value).cloned()
}

pub fn view(state: &StudioState) -> Element<'_, AppEvent> {
    let top_bar = top_bar(state);

    let center = row![
        container(left_panel(state)).width(Length::Fixed(248.0)),
        container(timeline::view(state))
            .width(Length::FillPortion(5))
            .height(Length::Fill),
        container(right_panel(state)).width(Length::Fixed(320.0)),
    ]
    .spacing(12)
    .height(Length::Fill);

    let status = status_bar(state);

    container(column![top_bar, center, status].spacing(12))
        .padding(12)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn top_bar(state: &StudioState) -> Element<'_, AppEvent> {
    let play_label = if state.engine.is_running() {
        "Pause"
    } else {
        "Play"
    };
    let undo_hint = state.undo_label().unwrap_or("Keine Historie");
    let redo_hint = state.redo_label().unwrap_or("Keine Historie");

    let transport_group = container(
        row![
            button(text(play_label).size(16))
                .padding([10, 18])
                .style(move |_: &Theme, status| {
                    theme::transport_button(status, state.engine.is_running())
                })
                .on_press(AppEvent::ToggleTransport),
            container(
                column![
                    text("Transport").size(12).color(theme::text_muted()),
                    text(state.engine.transport.position_label())
                        .size(24)
                        .color(theme::text_primary()),
                ]
                .spacing(4),
            )
            .padding([2, 4]),
            container(
                column![
                    text("BPM").size(12).color(theme::text_muted()),
                    text(format!("{:.1}", state.engine.transport.bpm.as_f32()))
                        .size(20)
                        .color(theme::text_primary()),
                ]
                .spacing(4),
            )
            .padding([2, 4]),
        ]
        .spacing(16)
        .align_y(Alignment::Center),
    )
    .padding(16)
    .style(|_| theme::panel_tinted(theme::accent_playhead()));

    let undo_button = {
        let button =
            button(text("Undo").size(14))
                .padding([10, 16])
                .style(move |_: &Theme, status| {
                    theme::toggle_button(status, state.can_undo(), theme::warning())
                });

        if state.can_undo() {
            button.on_press(AppEvent::Undo)
        } else {
            button
        }
    };

    let redo_button = {
        let button =
            button(text("Redo").size(14))
                .padding([10, 16])
                .style(move |_: &Theme, status| {
                    theme::toggle_button(status, state.can_redo(), theme::accent_blue())
                });

        if state.can_redo() {
            button.on_press(AppEvent::Redo)
        } else {
            button
        }
    };

    let history_group = container(
        column![
            text("History").size(12).color(theme::text_muted()),
            row![undo_button, redo_button]
                .spacing(10)
                .align_y(Alignment::Center),
            text(format!("Undo: {}", undo_hint))
                .size(12)
                .color(theme::text_primary()),
            text(format!("Redo: {}", redo_hint))
                .size(12)
                .color(theme::text_muted()),
        ]
        .spacing(10),
    )
    .padding(16)
    .style(|_| theme::panel_tinted(theme::warning()));

    let duplicate_button = {
        let button =
            button(text("Duplicate").size(14))
                .padding([10, 14])
                .style(move |_: &Theme, status| {
                    theme::toggle_button(
                        status,
                        state.can_duplicate_selected_clips(),
                        theme::accent_blue(),
                    )
                });

        if state.can_duplicate_selected_clips() {
            button.on_press(AppEvent::DuplicateSelectedClips)
        } else {
            button
        }
    };

    let split_button = {
        let button =
            button(text("Split").size(14))
                .padding([10, 14])
                .style(move |_: &Theme, status| {
                    theme::toggle_button(
                        status,
                        state.can_split_selected_clips_at_playhead(),
                        theme::accent_playhead(),
                    )
                });

        if state.can_split_selected_clips_at_playhead() {
            button.on_press(AppEvent::SplitSelectedClipsAtPlayhead)
        } else {
            button
        }
    };

    let delete_button = {
        let button =
            button(text("Delete").size(14))
                .padding([10, 14])
                .style(move |_: &Theme, status| {
                    theme::toggle_button(
                        status,
                        state.can_delete_selected_clips(),
                        theme::warning(),
                    )
                });

        if state.can_delete_selected_clips() {
            button.on_press(AppEvent::DeleteSelectedClips)
        } else {
            button
        }
    };

    let copy_button = {
        let button =
            button(text("Copy").size(14))
                .padding([10, 14])
                .style(move |_: &Theme, status| {
                    theme::toggle_button(
                        status,
                        state.can_copy_selected_clips(),
                        theme::muted_chip(),
                    )
                });

        if state.can_copy_selected_clips() {
            button.on_press(AppEvent::CopySelectedClips)
        } else {
            button
        }
    };

    let cut_button = {
        let button =
            button(text("Cut").size(14))
                .padding([10, 14])
                .style(move |_: &Theme, status| {
                    theme::toggle_button(status, state.can_copy_selected_clips(), theme::warning())
                });

        if state.can_copy_selected_clips() {
            button.on_press(AppEvent::CutSelectedClips)
        } else {
            button
        }
    };

    let paste_button = {
        let button =
            button(text("Paste").size(14))
                .padding([10, 14])
                .style(move |_: &Theme, status| {
                    theme::toggle_button(status, state.can_paste_clipboard(), theme::accent_blue())
                });

        if state.can_paste_clipboard() {
            button.on_press(AppEvent::PasteClipboardAtPlayhead)
        } else {
            button
        }
    };

    let nudge_left_button = {
        let button =
            button(text("<").size(14))
                .padding([10, 12])
                .style(move |_: &Theme, status| {
                    theme::toggle_button(
                        status,
                        state.can_copy_selected_clips(),
                        theme::accent_playhead(),
                    )
                });

        if state.can_copy_selected_clips() {
            button.on_press(AppEvent::NudgeSelectedClipsLeft)
        } else {
            button
        }
    };

    let nudge_right_button = {
        let button =
            button(text(">").size(14))
                .padding([10, 12])
                .style(move |_: &Theme, status| {
                    theme::toggle_button(
                        status,
                        state.can_copy_selected_clips(),
                        theme::accent_playhead(),
                    )
                });

        if state.can_copy_selected_clips() {
            button.on_press(AppEvent::NudgeSelectedClipsRight)
        } else {
            button
        }
    };

    let edit_group = container(
        column![
            text("Edit").size(12).color(theme::text_muted()),
            row![duplicate_button, split_button, delete_button]
                .spacing(10)
                .align_y(Alignment::Center),
            row![
                copy_button,
                cut_button,
                paste_button,
                nudge_left_button,
                nudge_right_button
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            text(format!("Selection: {}", state.selected_summary()))
                .size(12)
                .color(theme::text_primary()),
            text(format!(
                "Playhead: {}  |  Modifiers S:{} A:{} Cmd:{}",
                state.engine.transport.position_label(),
                if state.input_modifiers.shift {
                    "on"
                } else {
                    "off"
                },
                if state.input_modifiers.alt {
                    "on"
                } else {
                    "off"
                },
                if state.input_modifiers.command {
                    "on"
                } else {
                    "off"
                }
            ))
            .size(12)
            .color(theme::text_muted()),
        ]
        .spacing(10),
    )
    .padding(16)
    .style(|_| theme::panel_tinted(theme::accent_playhead()));

    let master_group = container(
        column![
            text("Master Control").size(12).color(theme::text_muted()),
            row![
                slider(0.0..=1.0, state.master.intensity.as_f32(), |value| {
                    AppEvent::SetMasterIntensity((value * 1000.0).round() as u16)
                },)
                .step(0.001),
                text(format!(
                    "Intensity {:>3.0}%",
                    state.master.intensity.as_f32() * 100.0
                ))
                .size(14)
                .width(Length::Fixed(120.0))
                .color(theme::text_primary()),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
            row![
                slider(0.2..=1.5, state.master.speed.as_f32(), |value| {
                    AppEvent::SetMasterSpeed((value * 1000.0).round() as u16)
                },)
                .step(0.001),
                text(format!(
                    "Speed {:>3.0}%",
                    state.master.speed.as_f32() * 100.0
                ))
                .size(14)
                .width(Length::Fixed(120.0))
                .color(theme::text_primary()),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        ]
        .spacing(12),
    )
    .padding(16)
    .style(|_| theme::panel_tinted(theme::accent_blue()));

    let zoom_group = container(
        column![
            text("Timeline Zoom").size(12).color(theme::text_muted()),
            row![
                slider(0.45..=2.4, state.timeline.viewport.zoom.as_f32(), |value| {
                    AppEvent::SetTimelineZoom((value * 1000.0).round() as u16)
                },)
                .step(0.001),
                text(format!(
                    "{:.0}%",
                    state.timeline.viewport.zoom.as_f32() * 100.0
                ))
                .size(14)
                .width(Length::Fixed(72.0))
                .color(theme::text_primary()),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
            row![
                perf_chip("FPS", format!("{}", state.performance.fps)),
                perf_chip("CPU", format!("{}%", state.performance.cpu_load.0)),
            ]
            .spacing(8),
        ]
        .spacing(12),
    )
    .padding(16)
    .style(|_| theme::panel_tinted(theme::success()));

    row![
        transport_group,
        history_group,
        edit_group,
        master_group,
        zoom_group
    ]
    .spacing(12)
    .into()
}

fn left_panel(state: &StudioState) -> Element<'_, AppEvent> {
    let mut tracks = column![panel_header("Tracks", "Mute / Solo / Layer")].spacing(12);

    tracks = tracks.push(track_column_header());

    for (index, track) in state.timeline.tracks.iter().enumerate() {
        let is_selected = match state.timeline.selection {
            SelectionState::Track(track_id) => track_id == track.id,
            SelectionState::Clip(_) => track
                .clips
                .iter()
                .any(|clip| state.is_clip_selected(clip.id)),
            SelectionState::None => false,
        };

        tracks = tracks.push(track_card(track, index, is_selected));
    }

    container(tracks)
        .padding(12)
        .style(|_| theme::panel())
        .height(Length::Fill)
        .into()
}

fn track_column_header() -> Element<'static, AppEvent> {
    container(
        row![
            text("CH").size(11).color(theme::text_muted()),
            text("TRACK").size(11).color(theme::text_muted()),
            text("MODE").size(11).color(theme::text_muted()),
            text("METER").size(11).color(theme::text_muted()),
        ]
        .spacing(16)
        .align_y(Alignment::Center),
    )
    .padding([10, 12])
    .height(Length::Fixed(timeline::HEADER_HEIGHT))
    .style(|_| theme::panel_tinted(theme::accent_blue()))
    .into()
}

fn track_card(track: &Track, index: usize, is_selected: bool) -> Element<'_, AppEvent> {
    let accent = track.color.to_iced();
    let meter_fill = ((track.clips.len() as f32 / 4.0).clamp(0.2, 1.0) * 1000.0) as u16;

    let card = row![
        container(text(""))
            .width(Length::Fixed(5.0))
            .style(move |_| theme::color_bar(accent)),
        container(
            text(format!("{:02}", index + 1))
                .size(15)
                .color(if is_selected {
                    theme::text_primary()
                } else {
                    theme::text_muted()
                }),
        )
        .padding([10, 10])
        .style(move |_| theme::track_card(accent, is_selected)),
        column![
            row![
                text(&track.name).size(18).color(theme::text_primary()),
                text(format!("{} Clips", track.clips.len()))
                    .size(12)
                    .color(theme::text_muted()),
                text(if track.solo {
                    "SOLO"
                } else if track.muted {
                    "MUTED"
                } else {
                    "LIVE"
                })
                .size(11)
                .color(if track.solo {
                    theme::success()
                } else if track.muted {
                    theme::warning()
                } else {
                    theme::accent_blue()
                }),
            ]
            .spacing(8),
            row![
                button(text("M"))
                    .padding([6, 10])
                    .style(move |_: &Theme, status| {
                        theme::toggle_button(status, track.muted, theme::warning())
                    })
                    .on_press(AppEvent::ToggleTrackMute(track.id)),
                button(text("S"))
                    .padding([6, 10])
                    .style(move |_: &Theme, status| {
                        theme::toggle_button(status, track.solo, theme::success())
                    })
                    .on_press(AppEvent::ToggleTrackSolo(track.id)),
                text(if track.solo { "Focused" } else { "Shared" })
                    .size(12)
                    .color(theme::text_muted()),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            row![
                text(
                    track
                        .clips
                        .first()
                        .map(|clip| format!("In {:.1}b", clip.start.as_beats_f32()))
                        .unwrap_or_else(|| "Empty".to_owned()),
                )
                .size(11)
                .color(theme::text_muted()),
                meter_bar(accent, meter_fill),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        ]
        .spacing(14)
        .width(Length::Fill),
    ]
    .spacing(12)
    .height(Length::Fixed(timeline::TRACK_HEIGHT));

    container(card)
        .padding(12)
        .style(move |_| theme::track_card(accent, is_selected))
        .into()
}

fn meter_bar<'a>(accent: iced::Color, fill_permille: u16) -> Element<'a, AppEvent> {
    let mut bars = row![].spacing(3).align_y(Alignment::Center);

    for index in 0..8 {
        let threshold = ((index + 1) * 125) as u16;
        let active = fill_permille >= threshold;
        let color = if active {
            accent
        } else {
            iced::Color::from_rgba8(255, 255, 255, 0.08)
        };

        bars = bars.push(
            container(text(""))
                .width(Length::Fixed(10.0))
                .height(Length::Fixed(5.0 + index as f32 * 0.7))
                .style(move |_| theme::color_bar(color)),
        );
    }

    container(bars).padding([2, 0]).into()
}

fn right_panel(state: &StudioState) -> Element<'_, AppEvent> {
    let venture = container(venture_panel(state))
        .padding(14)
        .style(|_| theme::panel_tinted(theme::success()));

    let clip_editor_action: Element<'_, AppEvent> = if state.has_multi_clip_selection() {
        container(text(format!(
            "{} Clips selektiert. Einzelauswahl oeffnet den Clip-Editor.",
            state.selected_clip_count()
        )))
        .style(|_| theme::panel_inner())
        .padding([8, 10])
        .into()
    } else if let Some(clip) = state.selected_clip() {
        button(text(
            if state.clip_editor.clip_id == Some(clip.id)
                && state.clip_editor.phase != crate::core::ClipEditorPhase::Closed
            {
                "Editor schließen"
            } else {
                "Editor öffnen"
            },
        ))
        .padding([8, 12])
        .style(|_: &Theme, button_state| {
            theme::toggle_button(button_state, true, theme::accent_blue())
        })
        .on_press(
            if state.clip_editor.clip_id == Some(clip.id)
                && state.clip_editor.phase != crate::core::ClipEditorPhase::Closed
            {
                AppEvent::CloseClipEditor
            } else {
                AppEvent::OpenClipEditor(clip.id)
            },
        )
        .into()
    } else {
        container(text(
            "Clip wählen, um den Editor im Timeline-Bereich zu öffnen.",
        ))
        .style(|_| theme::panel_inner())
        .padding([8, 10])
        .into()
    };

    let clip_editor = container(
        column![
            panel_header("Clip Editor", "Timeline / Live Preview"),
            text(if state.has_multi_clip_selection() {
                format!("Selected: {} Clips", state.selected_clip_count())
            } else {
                state
                    .selected_clip()
                    .map(|clip| format!("Selected: {}", clip.title))
                    .unwrap_or_else(|| "Kein Clip selektiert.".to_owned())
            },)
            .size(12)
            .color(theme::text_muted()),
            clip_editor_action
        ]
        .spacing(10),
    )
    .padding(14)
    .style(|_| theme::panel_tinted(theme::accent_blue()));

    let context = container(context_panel(state))
        .padding(14)
        .style(|_| theme::panel_subtle());

    let mut cues_content = column![
        panel_header("Cues", "Stored / Armed / Active"),
        text(
            state
                .selected_cue()
                .map(|cue| format!("Selected: {}", cue.name))
                .unwrap_or_else(|| "Kein Cue selektiert.".to_owned()),
        )
        .size(12)
        .color(theme::text_muted()),
        row![
            button(text("New Cue"))
                .padding([6, 10])
                .style(|_: &Theme, status| {
                    theme::toggle_button(status, true, theme::accent_blue())
                })
                .on_press(AppEvent::CreateCue),
            {
                let button =
                    button(text("Delete Cue"))
                        .padding([6, 10])
                        .style(move |_: &Theme, status| {
                            theme::toggle_button(
                                status,
                                state.can_delete_selected_cue(),
                                theme::warning(),
                            )
                        });
                if state.can_delete_selected_cue() {
                    button.on_press(AppEvent::DeleteSelectedCue)
                } else {
                    button
                }
            },
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        cue_inspector(state),
    ]
    .spacing(10);
    for cue in &state.cue_system.cues {
        cues_content = cues_content.push(cue_row(cue, state.cue_system.selected == Some(cue.id)));
    }
    let cues = container(cues_content)
        .padding(14)
        .style(|_| theme::panel_subtle());

    let mut chase_content = column![
        panel_header("Chases", "Steps / Loop / Direction"),
        text(
            state
                .selected_chase()
                .map(|chase| format!("Selected: {}", chase.name))
                .unwrap_or_else(|| "Kein Chase selektiert.".to_owned()),
        )
        .size(12)
        .color(theme::text_muted()),
        row![
            button(text("New Chase"))
                .padding([6, 10])
                .style(|_: &Theme, status| {
                    theme::toggle_button(status, true, theme::accent_blue())
                })
                .on_press(AppEvent::CreateChase),
            {
                let button = button(text("Delete Chase")).padding([6, 10]).style(
                    move |_: &Theme, status| {
                        theme::toggle_button(
                            status,
                            state.can_delete_selected_chase(),
                            theme::warning(),
                        )
                    },
                );
                if state.can_delete_selected_chase() {
                    button.on_press(AppEvent::DeleteSelectedChase)
                } else {
                    button
                }
            },
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        chase_inspector(state),
    ]
    .spacing(10);
    for chase in &state.chase_system.chases {
        chase_content = chase_content.push(chase_row(
            chase,
            state.chase_system.selected == Some(chase.id),
        ));
    }
    let chases = container(chase_content)
        .padding(14)
        .style(|_| theme::panel_subtle());

    let mut fx_content = column![
        panel_header("FX", "Color / Intensity / BPM"),
        text(
            state
                .selected_fx()
                .map(|layer| format!("Selected: {}", layer.name))
                .unwrap_or_else(|| "Kein FX selektiert.".to_owned()),
        )
        .size(12)
        .color(theme::text_muted()),
        fx_inspector(state),
    ]
    .spacing(10);
    for layer in &state.fx_system.layers {
        fx_content = fx_content.push(fx_row(layer, state.fx_system.selected == Some(layer.id)));
    }
    let fx = container(fx_content)
        .padding(14)
        .style(|_| theme::panel_subtle());

    let mut fixture_content = column![
        panel_header("Fixture View", "Mapped / Active / Error"),
        text(
            state
                .selected_fixture_group()
                .map(|group| format!("Selected: {}", group.name))
                .unwrap_or_else(|| "Keine Fixture-Gruppe selektiert.".to_owned()),
        )
        .size(12)
        .color(theme::text_muted()),
        fixture_view::view(state),
    ]
    .spacing(10);
    for group in &state.fixture_system.groups {
        fixture_content = fixture_content.push(fixture_row(
            group,
            state.fixture_system.selected == Some(group.id),
        ));
    }
    let fixture = container(fixture_content)
        .padding(14)
        .style(|_| theme::panel_subtle());

    container(
        scrollable(column![venture, clip_editor, context, cues, chases, fx, fixture].spacing(12))
            .height(Length::Fill),
    )
    .padding(12)
    .style(|_| theme::panel())
    .height(Length::Fill)
    .into()
}

fn venture_panel(state: &StudioState) -> Element<'_, AppEvent> {
    let selected_venture = state.selected_venture().cloned();
    let selected_recovery = state.selected_recovery_slot().cloned();
    let save_button = {
        let button = button(text("Save"))
            .padding([8, 12])
            .style(move |_: &Theme, status| {
                theme::toggle_button(status, state.can_save_venture(), theme::success())
            });

        if state.can_save_venture() {
            button.on_press(AppEvent::SaveCurrentVenture)
        } else {
            button
        }
    };

    let save_as_button = {
        let button = button(text("Save As"))
            .padding([8, 12])
            .style(move |_: &Theme, status| {
                theme::toggle_button(status, state.can_save_venture_as(), theme::accent_blue())
            });

        if state.can_save_venture_as() {
            button.on_press(AppEvent::SaveCurrentVentureAs)
        } else {
            button
        }
    };

    let rename_button = {
        let button = button(text("Rename"))
            .padding([8, 12])
            .style(move |_: &Theme, status| {
                theme::toggle_button(
                    status,
                    state.can_rename_selected_venture(),
                    theme::accent_playhead(),
                )
            });

        if state.can_rename_selected_venture() {
            button.on_press(AppEvent::RenameSelectedVenture)
        } else {
            button
        }
    };

    let load_button = {
        let button = button(text("Load"))
            .padding([8, 12])
            .style(move |_: &Theme, status| {
                theme::toggle_button(
                    status,
                    state.can_load_selected_venture(),
                    theme::accent_blue(),
                )
            });

        if state.can_load_selected_venture() {
            button.on_press(AppEvent::LoadSelectedVenture)
        } else {
            button
        }
    };

    let delete_button = {
        let button = button(text("Delete"))
            .padding([8, 12])
            .style(move |_: &Theme, status| {
                theme::toggle_button(
                    status,
                    state.can_delete_selected_venture(),
                    theme::warning(),
                )
            });

        if state.can_delete_selected_venture() {
            button.on_press(AppEvent::DeleteSelectedVenture)
        } else {
            button
        }
    };

    let restore_button = {
        let button =
            button(text("Restore Recovery"))
                .padding([8, 12])
                .style(move |_: &Theme, status| {
                    theme::toggle_button(
                        status,
                        state.can_restore_selected_recovery(),
                        theme::warning(),
                    )
                });

        if state.can_restore_selected_recovery() {
            button.on_press(AppEvent::RestoreSelectedRecoverySlot)
        } else {
            button
        }
    };

    let ventures: Element<'_, AppEvent> = if state.venture.ventures.is_empty() {
        container(text("Noch keine Ventures gespeichert."))
            .padding([8, 10])
            .style(|_: &Theme| theme::panel_inner())
            .into()
    } else {
        column(
            state
                .venture
                .ventures
                .iter()
                .map(|venture| {
                    let is_selected =
                        state.venture.selected.as_deref() == Some(venture.id.as_str());
                    container(
                        row![
                            text(&venture.name).size(14).color(theme::text_primary()),
                            text(format!("{}  |  {}", venture.id, venture.filename))
                                .size(11)
                                .color(theme::text_muted()),
                        ]
                        .spacing(10)
                        .align_y(Alignment::Center),
                    )
                    .padding([8, 10])
                    .style(move |_| theme::track_card(theme::accent_blue(), is_selected))
                    .into()
                })
                .collect::<Vec<Element<'_, AppEvent>>>(),
        )
        .spacing(8)
        .into()
    };

    let venture_health: Element<'_, AppEvent> =
        if state.venture.registry_issues.is_empty() && state.venture.recovery_issues.is_empty() {
            container(
                text(format!(
                    "{}  |  {}  |  Last Saved: {}",
                    state.dirty_summary(),
                    state.venture_issue_summary(),
                    state
                        .selected_venture()
                        .map(|venture| venture.name.clone())
                        .unwrap_or_else(|| "kein persistierter Slot".to_owned())
                ))
                .size(12)
                .color(if state.venture.dirty {
                    theme::warning()
                } else {
                    theme::text_muted()
                }),
            )
            .padding([8, 10])
            .style(|_: &Theme| theme::panel_inner())
            .into()
        } else {
            let mut issue_column = column![
                text(state.dirty_summary())
                    .size(12)
                    .color(if state.venture.dirty {
                        theme::warning()
                    } else {
                        theme::success()
                    }),
                text(state.venture_issue_summary())
                    .size(12)
                    .color(theme::warning()),
                text(state.recovery_issue_summary())
                    .size(12)
                    .color(theme::warning()),
            ]
            .spacing(6);

            for issue in &state.venture.registry_issues {
                issue_column = issue_column.push(text(issue).size(11).color(theme::text_muted()));
            }
            for issue in &state.venture.recovery_issues {
                issue_column = issue_column.push(text(issue).size(11).color(theme::text_muted()));
            }

            container(issue_column)
                .padding([8, 10])
                .style(|_: &Theme| theme::panel_inner())
                .into()
        };

    let recovery_summary: Element<'_, AppEvent> = container(
        column![
            text(format!(
                "Autosave: {}  |  Recovery Slots: {}",
                if state.venture.autosave_enabled {
                    "On"
                } else {
                    "Off"
                },
                state.venture.recovery_slots.len()
            ))
            .size(12)
            .color(theme::text_muted()),
            text(
                state
                    .selected_recovery_slot()
                    .map(|slot| format!("Selected Recovery: {}", slot.label))
                    .or_else(|| {
                        state
                            .venture
                            .last_autosave
                            .as_ref()
                            .map(|slot_id| format!("Last Autosave Slot: {}", slot_id))
                    })
                    .unwrap_or_else(|| "Noch kein Recovery-Slot vorhanden.".to_owned())
            )
            .size(12)
            .color(theme::text_primary()),
        ]
        .spacing(6),
    )
    .padding([8, 10])
    .style(|_: &Theme| theme::panel_inner())
    .into();

    column![
        panel_header("Ventures", "Save / Load / Session Registry"),
        text(match state.venture.phase {
            crate::core::VenturePhase::Idle => format!(
                "{} Venture(s) im Verzeichnis {}",
                state.venture.ventures.len(),
                state.venture.directory
            ),
            crate::core::VenturePhase::Saving => "Venture wird gespeichert".to_owned(),
            crate::core::VenturePhase::Loading => "Venture wird geladen".to_owned(),
            crate::core::VenturePhase::Error => state
                .venture
                .last_error
                .clone()
                .unwrap_or_else(|| "Venture-Fehler".to_owned()),
        })
        .size(12)
        .color(theme::text_muted()),
        text_input("Venture Name", &state.venture.draft_name)
            .on_input(AppEvent::SetVentureDraftName)
            .padding([8, 10]),
        pick_list(
            state.venture.ventures.clone(),
            selected_venture,
            |venture| { AppEvent::SelectVenture(venture.id) }
        )
        .placeholder("Gespeichertes Venture wählen"),
        pick_list(
            state.venture.recovery_slots.clone(),
            selected_recovery,
            |slot| { AppEvent::SelectRecoverySlot(slot.id) }
        )
        .placeholder("Recovery-Slot wählen"),
        row![
            button(text("New"))
                .padding([8, 12])
                .style(|_: &Theme, status| {
                    theme::toggle_button(status, true, theme::accent_playhead())
                })
                .on_press(AppEvent::CreateNewVenture),
            save_button,
            save_as_button,
            rename_button,
            load_button,
            delete_button,
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        row![
            restore_button,
            button(text("Refresh"))
                .padding([8, 12])
                .style(|_: &Theme, status| {
                    theme::toggle_button(status, true, theme::muted_chip())
                })
                .on_press(AppEvent::RefreshVentures),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        venture_health,
        recovery_summary,
        ventures,
    ]
    .spacing(10)
    .into()
}

fn context_panel(state: &StudioState) -> Element<'_, AppEvent> {
    let target_label = match state.context_menu.target {
        Some(ContextMenuTarget::Clip(clip_id)) => state
            .clip(clip_id)
            .map(|clip| format!("Clip: {}", clip.title))
            .unwrap_or_else(|| format!("Clip {}", clip_id.0)),
        Some(ContextMenuTarget::Track(track_id)) => state
            .track(track_id)
            .map(|track| format!("Track: {}", track.name))
            .unwrap_or_else(|| format!("Track {}", track_id.0)),
        Some(ContextMenuTarget::Timeline) => "Timeline".to_owned(),
        None => "Kein Kontext offen".to_owned(),
    };

    let clipboard_label = if state.clipboard.clips.is_empty() {
        "Clipboard leer".to_owned()
    } else {
        format!(
            "Clipboard: {} Clip(s)  |  Span {:.2} Beats",
            state.clipboard.clips.len(),
            state.clipboard.span.as_beats_f32()
        )
    };

    let actions = row![
        button(text("Copy"))
            .padding([6, 10])
            .style(|_: &Theme, status| theme::toggle_button(status, true, theme::muted_chip()))
            .on_press(AppEvent::ApplyContextMenuAction(ContextMenuAction::Copy)),
        button(text("Cut"))
            .padding([6, 10])
            .style(|_: &Theme, status| theme::toggle_button(status, true, theme::warning()))
            .on_press(AppEvent::ApplyContextMenuAction(ContextMenuAction::Cut)),
        button(text("Paste"))
            .padding([6, 10])
            .style(|_: &Theme, status| theme::toggle_button(status, true, theme::accent_blue()))
            .on_press(AppEvent::ApplyContextMenuAction(ContextMenuAction::Paste)),
    ]
    .spacing(8);

    column![
        panel_header("Context", "Right Click / Clipboard / Shortcuts"),
        text(if state.context_menu.open {
            format!("Open: {}", target_label)
        } else {
            target_label
        })
        .size(12)
        .color(theme::text_primary()),
        text(clipboard_label).size(12).color(theme::text_muted()),
        actions
    ]
    .spacing(10)
    .into()
}

fn cue_phase_label(phase: CuePhase) -> &'static str {
    match phase {
        CuePhase::Stored => "stored",
        CuePhase::Armed => "armed",
        CuePhase::Triggered => "triggered",
        CuePhase::Fading => "fading",
        CuePhase::Active => "active",
    }
}

fn chase_phase_label(phase: ChasePhase) -> &'static str {
    match phase {
        ChasePhase::Idle => "idle",
        ChasePhase::Playing => "playing",
        ChasePhase::Looping => "looping",
        ChasePhase::Reversing => "reverse",
        ChasePhase::Stopped => "stopped",
    }
}

fn cue_inspector(state: &StudioState) -> Element<'_, AppEvent> {
    let Some(cue) = state.selected_cue() else {
        return container(text("Cue-Inspector bereit, sobald ein Cue selektiert ist."))
            .padding([10, 12])
            .style(|_| theme::panel_inner())
            .into();
    };

    let color_options = color_choices(Some(cue.color));
    let fade_options = beat_time_choices(Some(cue.fade_duration));

    container(
        column![
            text("Cue Inspector").size(13).color(theme::text_muted()),
            text_input("Cue Name", &cue.name)
                .on_input(AppEvent::SetSelectedCueName)
                .padding([8, 10]),
            row![
                column![
                    text("Color").size(12).color(theme::text_muted()),
                    pick_list(
                        color_options.clone(),
                        selected_color_choice(&color_options, cue.color),
                        |choice| AppEvent::SetSelectedCueColor(choice.value)
                    )
                    .placeholder("Color"),
                ]
                .spacing(6)
                .width(Length::FillPortion(1)),
                column![
                    text("Fade").size(12).color(theme::text_muted()),
                    pick_list(
                        fade_options.clone(),
                        selected_beat_time_choice(&fade_options, cue.fade_duration),
                        |choice| AppEvent::SetSelectedCueFadeDuration(choice.value)
                    )
                    .placeholder("Fade"),
                ]
                .spacing(6)
                .width(Length::FillPortion(1)),
            ]
            .spacing(10),
            text(format!(
                "State: {}  |  Linked Clip: {}  |  Fade: {:.2} Beats",
                cue_phase_label(cue.phase),
                cue.linked_clip
                    .map(|clip_id| clip_id.0.to_string())
                    .unwrap_or_else(|| "none".to_owned()),
                cue.fade_duration.as_beats_f32()
            ))
            .size(12)
            .color(theme::text_primary()),
        ]
        .spacing(10),
    )
    .padding([10, 12])
    .style(|_| theme::panel_inner())
    .into()
}

fn chase_inspector(state: &StudioState) -> Element<'_, AppEvent> {
    let Some(chase) = state.selected_chase() else {
        return container(text(
            "Chase-Inspector bereit, sobald ein Chase selektiert ist.",
        ))
        .padding([10, 12])
        .style(|_| theme::panel_inner())
        .into();
    };

    let cue_options = cue_choices(state);
    let mut steps_column = column![text("Steps").size(12).color(theme::text_muted())].spacing(8);
    for (index, step) in chase.steps.iter().enumerate() {
        let is_selected = state.selected_chase_step_index() == Some(index);
        let cue_label = step
            .cue_id
            .and_then(|cue_id| state.cue(cue_id))
            .map(|cue| cue.name.clone())
            .unwrap_or_else(|| "No Cue".to_owned());
        steps_column = steps_column.push(
            container(
                row![
                    button(text(format!("{:02}", index + 1)).size(12))
                        .padding([5, 8])
                        .style(move |_: &Theme, status| {
                            theme::toggle_button(status, is_selected, step.color.to_iced())
                        })
                        .on_press(AppEvent::SelectChaseStep(Some(index))),
                    column![
                        text(&step.label).size(13).color(theme::text_primary()),
                        text(format!(
                            "{}  |  {:.2} Beats",
                            cue_label,
                            step.duration.as_beats_f32()
                        ))
                        .size(11)
                        .color(theme::text_muted()),
                    ]
                    .spacing(4)
                    .width(Length::Fill),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            )
            .padding([8, 10])
            .style(move |_| theme::track_card(step.color.to_iced(), is_selected)),
        );
    }

    let step_editor: Element<'_, AppEvent> = if let Some(step) = state.selected_chase_step() {
        let duration_options = beat_time_choices(Some(step.duration));
        let color_options = color_choices(Some(step.color));

        container(
            column![
                text("Selected Step").size(12).color(theme::text_muted()),
                text_input("Step Label", &step.label)
                    .on_input(AppEvent::SetSelectedChaseStepLabel)
                    .padding([8, 10]),
                row![
                    column![
                        text("Cue").size(12).color(theme::text_muted()),
                        pick_list(
                            cue_options.clone(),
                            selected_cue_choice(&cue_options, step.cue_id),
                            |choice| AppEvent::SetSelectedChaseStepCue(choice.id)
                        )
                        .placeholder("Cue"),
                    ]
                    .spacing(6)
                    .width(Length::FillPortion(1)),
                    column![
                        text("Duration").size(12).color(theme::text_muted()),
                        pick_list(
                            duration_options.clone(),
                            selected_beat_time_choice(&duration_options, step.duration),
                            |choice| AppEvent::SetSelectedChaseStepDuration(choice.value)
                        )
                        .placeholder("Duration"),
                    ]
                    .spacing(6)
                    .width(Length::FillPortion(1)),
                    column![
                        text("Color").size(12).color(theme::text_muted()),
                        pick_list(
                            color_options.clone(),
                            selected_color_choice(&color_options, step.color),
                            |choice| AppEvent::SetSelectedChaseStepColor(choice.value)
                        )
                        .placeholder("Color"),
                    ]
                    .spacing(6)
                    .width(Length::FillPortion(1)),
                ]
                .spacing(10),
            ]
            .spacing(10),
        )
        .padding([10, 12])
        .style(|_| theme::panel_subtle())
        .into()
    } else {
        container(text("Kein Step selektiert."))
            .padding([8, 10])
            .style(|_| theme::panel_subtle())
            .into()
    };

    container(
        column![
            text("Chase Inspector").size(13).color(theme::text_muted()),
            text_input("Chase Name", &chase.name)
                .on_input(AppEvent::SetSelectedChaseName)
                .padding([8, 10]),
            row![
                button(text("Forward"))
                    .padding([6, 10])
                    .style(|_: &Theme, status| {
                        theme::toggle_button(
                            status,
                            chase.direction == ChaseDirection::Forward,
                            theme::success(),
                        )
                    })
                    .on_press(AppEvent::SetSelectedChaseDirection(ChaseDirection::Forward)),
                button(text("Reverse"))
                    .padding([6, 10])
                    .style(|_: &Theme, status| {
                        theme::toggle_button(
                            status,
                            chase.direction == ChaseDirection::Reverse,
                            theme::accent_blue(),
                        )
                    })
                    .on_press(AppEvent::SetSelectedChaseDirection(ChaseDirection::Reverse)),
                button(text("Loop"))
                    .padding([6, 10])
                    .style(|_: &Theme, status| {
                        theme::toggle_button(status, chase.loop_enabled, theme::warning())
                    })
                    .on_press(AppEvent::SetSelectedChaseLoop(!chase.loop_enabled)),
            ]
            .spacing(8),
            text(format!(
                "State: {}  |  Current Step: {} / {}  |  Linked Clip: {}",
                chase_phase_label(chase.phase),
                chase.current_step + 1,
                chase.steps.len(),
                chase
                    .linked_clip
                    .map(|clip_id| clip_id.0.to_string())
                    .unwrap_or_else(|| "none".to_owned())
            ))
            .size(12)
            .color(theme::text_primary()),
            row![
                button(text("Add Step"))
                    .padding([6, 10])
                    .style(|_: &Theme, status| {
                        theme::toggle_button(status, true, theme::accent_blue())
                    })
                    .on_press(AppEvent::AddSelectedChaseStep),
                {
                    let button = button(text("Delete Step")).padding([6, 10]).style(
                        move |_: &Theme, status| {
                            theme::toggle_button(
                                status,
                                state.can_delete_selected_chase_step(),
                                theme::warning(),
                            )
                        },
                    );
                    if state.can_delete_selected_chase_step() {
                        button.on_press(AppEvent::DeleteSelectedChaseStep)
                    } else {
                        button
                    }
                },
                {
                    let button = button(text("Move Left")).padding([6, 10]).style(
                        move |_: &Theme, status| {
                            theme::toggle_button(
                                status,
                                state.can_move_selected_chase_step_left(),
                                theme::muted_chip(),
                            )
                        },
                    );
                    if state.can_move_selected_chase_step_left() {
                        button.on_press(AppEvent::MoveSelectedChaseStepLeft)
                    } else {
                        button
                    }
                },
                {
                    let button = button(text("Move Right")).padding([6, 10]).style(
                        move |_: &Theme, status| {
                            theme::toggle_button(
                                status,
                                state.can_move_selected_chase_step_right(),
                                theme::muted_chip(),
                            )
                        },
                    );
                    if state.can_move_selected_chase_step_right() {
                        button.on_press(AppEvent::MoveSelectedChaseStepRight)
                    } else {
                        button
                    }
                },
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            steps_column,
            step_editor,
        ]
        .spacing(10),
    )
    .padding([10, 12])
    .style(|_| theme::panel_inner())
    .into()
}

fn cue_row(cue: &Cue, is_selected: bool) -> Element<'_, AppEvent> {
    let (accent, status) = match cue.phase {
        CuePhase::Active => (theme::success(), "active"),
        CuePhase::Triggered => (theme::success(), "triggered"),
        CuePhase::Fading => (theme::warning(), "fading"),
        CuePhase::Armed => (theme::warning(), "armed"),
        CuePhase::Stored => (theme::muted_chip(), "stored"),
    };

    container(
        column![
            row![
                container(text(""))
                    .width(Length::Fixed(8.0))
                    .style(move |_| theme::color_bar(accent)),
                text(&cue.name).size(15).color(theme::text_primary()),
                text(status).size(12).color(theme::text_muted()),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            row![
                button(text("Select"))
                    .padding([6, 10])
                    .style(move |_: &Theme, button_state| {
                        theme::toggle_button(button_state, is_selected, theme::accent_blue())
                    })
                    .on_press(AppEvent::SelectCue(cue.id)),
                button(text("Arm"))
                    .padding([6, 10])
                    .style(move |_: &Theme, button_state| {
                        theme::toggle_button(
                            button_state,
                            matches!(cue.phase, CuePhase::Armed),
                            theme::warning(),
                        )
                    })
                    .on_press(AppEvent::ArmCue(cue.id)),
                button(text("Go"))
                    .padding([6, 10])
                    .style(move |_: &Theme, button_state| {
                        theme::toggle_button(
                            button_state,
                            matches!(cue.phase, CuePhase::Triggered | CuePhase::Active),
                            theme::success(),
                        )
                    })
                    .on_press(AppEvent::TriggerCue(cue.id)),
                text(format!(
                    "{}  |  Fade {:.2}",
                    cue.linked_clip
                        .map(|clip_id| format!("Clip {}", clip_id.0))
                        .unwrap_or_else(|| "No clip".to_owned()),
                    cue.fade_duration.as_beats_f32()
                ),)
                .size(12)
                .color(theme::text_muted()),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(10),
    )
    .padding([10, 12])
    .style(move |_| theme::track_card(accent, is_selected))
    .into()
}

fn chase_row(chase: &Chase, is_selected: bool) -> Element<'_, AppEvent> {
    let accent = match chase.phase {
        ChasePhase::Playing | ChasePhase::Looping => theme::success(),
        ChasePhase::Reversing => theme::accent_blue(),
        ChasePhase::Stopped | ChasePhase::Idle => theme::muted_chip(),
    };
    let status = match chase.phase {
        ChasePhase::Playing => "playing",
        ChasePhase::Looping => "looping",
        ChasePhase::Reversing => "reverse",
        ChasePhase::Stopped => "stopped",
        ChasePhase::Idle => "idle",
    };

    container(
        column![
            row![
                container(text(""))
                    .width(Length::Fixed(8.0))
                    .style(move |_| theme::color_bar(accent)),
                text(&chase.name).size(15).color(theme::text_primary()),
                text(status).size(12).color(theme::text_muted()),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            row![
                button(text("Select"))
                    .padding([6, 10])
                    .style(move |_: &Theme, button_state| {
                        theme::toggle_button(button_state, is_selected, theme::accent_blue())
                    })
                    .on_press(AppEvent::SelectChase(chase.id)),
                button(text("Play"))
                    .padding([6, 10])
                    .style(move |_: &Theme, button_state| {
                        theme::toggle_button(
                            button_state,
                            matches!(
                                chase.phase,
                                ChasePhase::Playing | ChasePhase::Looping | ChasePhase::Reversing
                            ),
                            theme::success(),
                        )
                    })
                    .on_press(AppEvent::ToggleChase(chase.id)),
                button(text("Rev"))
                    .padding([6, 10])
                    .style(move |_: &Theme, button_state| {
                        theme::toggle_button(
                            button_state,
                            matches!(chase.phase, ChasePhase::Reversing),
                            theme::accent_blue(),
                        )
                    })
                    .on_press(AppEvent::ReverseChase(chase.id)),
                text(format!(
                    "Step {}/{}  |  {}  |  {}",
                    chase.current_step + 1,
                    chase.steps.len(),
                    if chase.loop_enabled {
                        "loop"
                    } else {
                        "one-shot"
                    },
                    match chase.direction {
                        ChaseDirection::Forward => "forward",
                        ChaseDirection::Reverse => "reverse",
                    }
                ))
                .size(12)
                .color(theme::text_muted()),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(10),
    )
    .padding([10, 12])
    .style(move |_| theme::track_card(accent, is_selected))
    .into()
}

fn fx_inspector(state: &StudioState) -> Element<'_, AppEvent> {
    let Some(layer) = state.selected_fx() else {
        return container(text(
            "FX wählen, um Rate, Spread, Phase und Waveform zu bearbeiten.",
        ))
        .padding([10, 12])
        .style(|_| theme::panel_inner())
        .into();
    };

    let waveform_options = vec![
        FxWaveform::Sine,
        FxWaveform::Triangle,
        FxWaveform::Saw,
        FxWaveform::Pulse,
    ];

    let linked_clip_action: Element<'_, AppEvent> = if let Some(clip_id) = layer.linked_clip {
        button(text(format!("Linked Clip {}", clip_id.0)))
            .padding([6, 10])
            .style(|_: &Theme, button_state| {
                theme::toggle_button(button_state, true, theme::accent_blue())
            })
            .on_press(AppEvent::OpenClipEditor(clip_id))
            .into()
    } else {
        container(text("Kein verknüpfter Clip"))
            .padding([6, 10])
            .style(|_| theme::panel_inner())
            .into()
    };

    container(
        column![
            row![
                text(format!("Inspector: {}", layer.name))
                    .size(14)
                    .color(theme::text_primary()),
                text(format!("{} / {}%", layer.waveform, layer.output_level / 10))
                    .size(12)
                    .color(theme::text_muted()),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            row![
                column![
                    text("Depth").size(12).color(theme::text_muted()),
                    slider(
                        0.0..=1.0,
                        layer.depth_permille as f32 / 1000.0,
                        move |value| {
                            AppEvent::SetFxDepth(layer.id, (value * 1000.0).round() as u16)
                        }
                    )
                    .step(0.001),
                    text(format!("{:>3}%", layer.depth_permille / 10))
                        .size(12)
                        .color(theme::text_primary()),
                ]
                .spacing(8)
                .width(Length::FillPortion(1)),
                column![
                    text("Rate").size(12).color(theme::text_muted()),
                    slider(0.2..=1.5, layer.rate.as_f32(), move |value| {
                        AppEvent::SetFxRate(layer.id, (value * 1000.0).round() as u16)
                    })
                    .step(0.001),
                    text(format!("{:>3.0}%", layer.rate.as_f32() * 100.0))
                        .size(12)
                        .color(theme::text_primary()),
                ]
                .spacing(8)
                .width(Length::FillPortion(1)),
            ]
            .spacing(12),
            row![
                column![
                    text("Spread").size(12).color(theme::text_muted()),
                    slider(
                        0.0..=1.0,
                        layer.spread_permille as f32 / 1000.0,
                        move |value| {
                            AppEvent::SetFxSpread(layer.id, (value * 1000.0).round() as u16)
                        }
                    )
                    .step(0.001),
                    text(format!("{:>3}%", layer.spread_permille / 10))
                        .size(12)
                        .color(theme::text_primary()),
                ]
                .spacing(8)
                .width(Length::FillPortion(1)),
                column![
                    text("Phase Offset").size(12).color(theme::text_muted()),
                    slider(
                        0.0..=1.0,
                        layer.phase_offset_permille as f32 / 1000.0,
                        move |value| {
                            AppEvent::SetFxPhaseOffset(layer.id, (value * 1000.0).round() as u16)
                        }
                    )
                    .step(0.001),
                    text(format!("{:>3}%", layer.phase_offset_permille / 10))
                        .size(12)
                        .color(theme::text_primary()),
                ]
                .spacing(8)
                .width(Length::FillPortion(1)),
            ]
            .spacing(12),
            row![
                column![
                    text("Waveform").size(12).color(theme::text_muted()),
                    pick_list(waveform_options, Some(layer.waveform), move |waveform| {
                        AppEvent::SetFxWaveform(layer.id, waveform)
                    })
                    .placeholder("Waveform"),
                ]
                .spacing(8)
                .width(Length::FillPortion(1)),
                column![
                    text("Timeline Link").size(12).color(theme::text_muted()),
                    linked_clip_action,
                ]
                .spacing(8)
                .width(Length::FillPortion(1)),
            ]
            .spacing(12),
        ]
        .spacing(12),
    )
    .padding([10, 12])
    .style(|_| theme::panel_inner())
    .into()
}

fn fx_row(layer: &FxLayer, is_selected: bool) -> Element<'_, AppEvent> {
    let accent = match layer.kind {
        FxKind::Color => theme::success(),
        FxKind::Intensity => theme::warning(),
        FxKind::Position => theme::accent_blue(),
    };
    let status = match layer.phase {
        FxPhase::Idle => "idle",
        FxPhase::Processing => "processing",
        FxPhase::Applied => "applied",
        FxPhase::Composed => "composed",
    };
    let depth_value = layer.depth_permille as f32 / 1000.0;

    container(
        column![
            row![
                container(text(""))
                    .width(Length::Fixed(8.0))
                    .style(move |_| theme::color_bar(accent)),
                text(&layer.name).size(15).color(theme::text_primary()),
                text(status).size(12).color(theme::text_muted()),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            slider(0.0..=1.0, depth_value, move |value| {
                AppEvent::SetFxDepth(layer.id, (value * 1000.0).round() as u16)
            })
            .step(0.001),
            row![
                button(text(if layer.enabled { "On" } else { "Off" }))
                    .padding([6, 10])
                    .style(move |_: &Theme, button_state| {
                        theme::toggle_button(button_state, layer.enabled, accent)
                    })
                    .on_press(AppEvent::ToggleFx(layer.id)),
                button(text("Focus"))
                    .padding([6, 10])
                    .style(move |_: &Theme, button_state| {
                        theme::toggle_button(button_state, is_selected, theme::accent_blue())
                    })
                    .on_press(AppEvent::SelectFx(layer.id)),
                text(format!(
                    "Out {:>3}%  |  {}  |  Rate {:>3.0}%",
                    layer.output_level / 10,
                    layer.waveform,
                    layer.rate.as_f32() * 100.0
                ))
                .size(12)
                .color(theme::text_muted()),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(10),
    )
    .padding([10, 12])
    .style(move |_| theme::track_card(accent, is_selected))
    .into()
}

fn fixture_row(group: &FixtureGroup, is_selected: bool) -> Element<'_, AppEvent> {
    let accent = group.accent.to_iced();
    let status = match group.phase {
        FixturePhase::Uninitialized => "uninitialized",
        FixturePhase::Mapped => "mapped",
        FixturePhase::Active => "active",
        FixturePhase::Error => "error",
    };

    container(
        column![
            row![
                container(text(""))
                    .width(Length::Fixed(10.0))
                    .style(move |_| theme::color_bar(accent)),
                text(&group.name).size(15).color(theme::text_primary()),
                text(status).size(12).color(theme::text_muted()),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            row![
                button(text("Focus"))
                    .padding([6, 10])
                    .style(move |_: &Theme, button_state| {
                        theme::toggle_button(button_state, is_selected, theme::accent_blue())
                    })
                    .on_press(AppEvent::SelectFixtureGroup(group.id)),
                text(format!("{}/{} online", group.online, group.fixture_count))
                    .size(12)
                    .color(theme::text_muted()),
                text(format!("Nodes {}", group.preview_nodes.len()))
                    .size(12)
                    .color(theme::text_muted()),
                text(format!("Out {:>3}%", group.output_level / 10))
                    .size(12)
                    .color(theme::text_muted()),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(10),
    )
    .padding([10, 12])
    .style(move |_| theme::track_card(accent, is_selected))
    .into()
}

fn panel_header<'a>(title: &'a str, subtitle: &'a str) -> Element<'a, AppEvent> {
    column![
        text(title).size(13).color(theme::text_muted()),
        text(subtitle).size(18).color(theme::text_primary()),
    ]
    .spacing(4)
    .into()
}

fn perf_chip<'a>(label: &'a str, value: String) -> Element<'a, AppEvent> {
    container(
        row![
            text(label).size(11).color(theme::text_muted()),
            text(value).size(14).color(theme::text_primary()),
        ]
        .spacing(8),
    )
    .padding([8, 12])
    .style(|_| theme::panel_inner())
    .into()
}

fn status_bar(state: &StudioState) -> Element<'_, AppEvent> {
    let status = row![
        text(state.selected_summary())
            .size(13)
            .color(theme::text_primary()),
        text("•").size(13).color(theme::text_muted()),
        text(state.status.hint.clone())
            .size(13)
            .color(theme::text_muted()),
        text("•").size(13).color(theme::text_muted()),
        text(state.diff_summary())
            .size(12)
            .color(theme::text_muted()),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    container(status)
        .padding([10, 14])
        .style(|_| theme::status_bar())
        .into()
}
