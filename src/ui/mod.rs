pub mod fixture_view;
pub mod theme;
pub mod timeline;

use crate::core::{
    AppEvent, BeatTime, Chase, ChaseDirection, ChasePhase, ContextMenuAction, ContextMenuTarget,
    ControllerProfileKind, Cue, CueId, CuePhase, DmxBackendKind, DmxInterfaceKind,
    EngineDeckFollowMode, EngineDeckPhase, EngineLinkMode, EnginePrimeDevice, FixtureGroup,
    FixtureGroupId, FixturePatch, FixturePhase, FxKind, FxLayer, FxPhase, FxWaveform, MidiAction,
    MidiBinding, MidiBindingMessage, MidiControlHint, MidiLearnPhase, MidiMessageKind,
    MidiPortDescriptor, OutputUniverseMonitor, RgbaColor, SelectionState, SettingsTab, StudioState,
    Track, build_output_monitor_snapshot,
};
use iced::widget::{
    button, column, container, pick_list, row, scrollable, slider, text, text_input,
};
use iced::{Alignment, Color, Element, Length, Theme};
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct FixtureGroupChoice {
    id: Option<FixtureGroupId>,
    label: String,
}

impl fmt::Display for FixtureGroupChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SettingsTabChoice {
    value: SettingsTab,
    label: &'static str,
}

impl fmt::Display for SettingsTabChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DmxBackendChoice {
    value: DmxBackendKind,
    label: &'static str,
}

impl fmt::Display for DmxBackendChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EngineLinkModeChoice {
    value: EngineLinkMode,
    label: &'static str,
}

impl fmt::Display for EngineLinkModeChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EngineDeckFollowChoice {
    value: EngineDeckFollowMode,
    label: &'static str,
}

impl fmt::Display for EngineDeckFollowChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EngineDeviceChoice {
    id: Option<String>,
    label: String,
}

impl fmt::Display for EngineDeviceChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PortChoice {
    id: Option<String>,
    label: String,
}

impl fmt::Display for PortChoice {
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

fn fixture_group_choices(state: &StudioState) -> Vec<FixtureGroupChoice> {
    let mut options = vec![FixtureGroupChoice {
        id: None,
        label: "No Preview Group".to_owned(),
    }];
    options.extend(
        state
            .fixture_system
            .groups
            .iter()
            .map(|group| FixtureGroupChoice {
                id: Some(group.id),
                label: format!("{} ({})", group.name, group.id.0),
            }),
    );
    options
}

fn selected_fixture_group_choice(
    options: &[FixtureGroupChoice],
    group_id: Option<FixtureGroupId>,
) -> Option<FixtureGroupChoice> {
    options.iter().find(|option| option.id == group_id).cloned()
}

fn settings_tab_choices() -> [SettingsTabChoice; 5] {
    [
        SettingsTabChoice {
            value: SettingsTab::General,
            label: "General",
        },
        SettingsTabChoice {
            value: SettingsTab::Dmx,
            label: "DMX",
        },
        SettingsTabChoice {
            value: SettingsTab::Midi,
            label: "MIDI",
        },
        SettingsTabChoice {
            value: SettingsTab::Controllers,
            label: "Controllers",
        },
        SettingsTabChoice {
            value: SettingsTab::Engine,
            label: "Engine",
        },
    ]
}

fn engine_link_mode_choices() -> [EngineLinkModeChoice; 2] {
    [
        EngineLinkModeChoice {
            value: EngineLinkMode::Disabled,
            label: "Disabled",
        },
        EngineLinkModeChoice {
            value: EngineLinkMode::StageLinqExperimental,
            label: "StageLinq Experimental",
        },
    ]
}

fn engine_follow_choices() -> [EngineDeckFollowChoice; 5] {
    [
        EngineDeckFollowChoice {
            value: EngineDeckFollowMode::Disabled,
            label: "Disabled",
        },
        EngineDeckFollowChoice {
            value: EngineDeckFollowMode::Deck1,
            label: "Deck 1",
        },
        EngineDeckFollowChoice {
            value: EngineDeckFollowMode::Deck2,
            label: "Deck 2",
        },
        EngineDeckFollowChoice {
            value: EngineDeckFollowMode::MasterDeck,
            label: "Master Deck",
        },
        EngineDeckFollowChoice {
            value: EngineDeckFollowMode::AnyPlayingDeck,
            label: "Any Playing Deck",
        },
    ]
}

fn engine_device_choices(state: &StudioState) -> Vec<EngineDeviceChoice> {
    let mut options = vec![EngineDeviceChoice {
        id: None,
        label: "Auto / No Device".to_owned(),
    }];
    options.extend(
        state
            .settings
            .engine_link
            .devices
            .iter()
            .map(|device| EngineDeviceChoice {
                id: Some(device.id.clone()),
                label: format!("{} ({})", device.name, device.address),
            }),
    );
    options
}

fn selected_engine_device_choice(
    options: &[EngineDeviceChoice],
    device_id: Option<&str>,
) -> Option<EngineDeviceChoice> {
    options
        .iter()
        .find(|option| option.id.as_deref() == device_id)
        .cloned()
}

fn dmx_backend_choices() -> [DmxBackendChoice; 4] {
    [
        DmxBackendChoice {
            value: DmxBackendKind::Disabled,
            label: "Disabled",
        },
        DmxBackendChoice {
            value: DmxBackendKind::EnttecOpenDmx,
            label: "ENTTEC Open DMX",
        },
        DmxBackendChoice {
            value: DmxBackendKind::ArtNet,
            label: "Art-Net",
        },
        DmxBackendChoice {
            value: DmxBackendKind::Sacn,
            label: "sACN",
        },
    ]
}

fn dmx_interface_choices(state: &StudioState) -> Vec<PortChoice> {
    let mut options = vec![PortChoice {
        id: None,
        label: "No Interface".to_owned(),
    }];
    options.extend(
        state
            .settings
            .dmx
            .interfaces
            .iter()
            .map(|interface| PortChoice {
                id: Some(interface.id.clone()),
                label: format!("{}  |  {}", interface.name, interface.detail),
            }),
    );
    options
}

fn midi_port_choices(ports: &[MidiPortDescriptor], empty_label: &str) -> Vec<PortChoice> {
    let mut options = vec![PortChoice {
        id: None,
        label: empty_label.to_owned(),
    }];
    options.extend(ports.iter().map(|port| PortChoice {
        id: Some(port.id.clone()),
        label: port.detail.clone(),
    }));
    options
}

fn selected_port_choice(options: &[PortChoice], id: Option<&str>) -> Option<PortChoice> {
    options
        .iter()
        .find(|option| option.id.as_deref() == id)
        .cloned()
}

fn format_universe_labels(universes: &[u16]) -> String {
    if universes.is_empty() {
        "No universes".to_owned()
    } else {
        universes
            .iter()
            .map(|universe| format!("U{}", universe))
            .collect::<Vec<_>>()
            .join(", ")
    }
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

    let settings = container(settings_panel(state))
        .padding(14)
        .style(|_| theme::panel_tinted(theme::accent_playhead()));

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
        panel_header("Fixture View", "Library / Patch / Preview"),
        fixture_management_panel(state),
        text(
            state
                .selected_fixture_group()
                .map(|group| format!("Preview: {}", group.name))
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
            state,
            state.fixture_system.selected == Some(group.id),
        ));
    }
    let fixture = container(fixture_content)
        .padding(14)
        .style(|_| theme::panel_subtle());

    container(
        scrollable(
            column![
                venture,
                settings,
                clip_editor,
                context,
                cues,
                chases,
                fx,
                fixture
            ]
            .spacing(12),
        )
        .height(Length::Fill),
    )
    .padding(12)
    .style(|_| theme::panel())
    .height(Length::Fill)
    .into()
}

fn settings_panel(state: &StudioState) -> Element<'_, AppEvent> {
    let tab_bar = row(settings_tab_choices()
        .into_iter()
        .map(|choice| {
            let is_active = state.settings.selected_tab == choice.value;
            button(text(choice.label))
                .padding([6, 10])
                .style(move |_: &Theme, status| {
                    theme::toggle_button(status, is_active, theme::accent_playhead())
                })
                .on_press(AppEvent::SelectSettingsTab(choice.value))
                .into()
        })
        .collect::<Vec<Element<'_, AppEvent>>>())
    .spacing(8)
    .align_y(Alignment::Center);

    let refresh_button = {
        let button =
            button(text("Refresh Hardware"))
                .padding([8, 12])
                .style(move |_: &Theme, status| {
                    theme::toggle_button(
                        status,
                        state.can_refresh_hardware_inventory(),
                        theme::accent_blue(),
                    )
                });
        if state.can_refresh_hardware_inventory() {
            button.on_press(AppEvent::RefreshHardwareInventory)
        } else {
            button
        }
    };

    let summary = container(
        column![
            text(format!(
                "{} DMX  |  {} MIDI In  |  {} MIDI Out  |  {} Engine",
                state.settings.dmx.interfaces.len(),
                state.settings.midi.inputs.len(),
                state.settings.midi.outputs.len(),
                state.settings.engine_link.devices.len()
            ))
            .size(12)
            .color(theme::text_muted()),
            text(
                state
                    .output
                    .last_summary
                    .clone()
                    .or_else(|| state.settings.engine_link.last_summary.clone())
                    .clone()
                    .or_else(|| state.settings.midi.last_summary.clone())
                    .or_else(|| state.settings.dmx.last_summary.clone())
                    .unwrap_or_else(|| "Hardware-Status bereit.".to_owned())
            )
            .size(12)
            .color(theme::text_primary()),
        ]
        .spacing(6),
    )
    .padding([8, 10])
    .style(|_| theme::panel_inner());

    let content = match state.settings.selected_tab {
        SettingsTab::General => general_settings_panel(state),
        SettingsTab::Dmx => dmx_settings_panel(state),
        SettingsTab::Midi => midi_settings_panel(state),
        SettingsTab::Controllers => controller_settings_panel(state),
        SettingsTab::Engine => engine_settings_panel(state),
    };

    column![
        panel_header("Settings", "System / DMX / MIDI / Controllers / Engine"),
        row![refresh_button].align_y(Alignment::Center),
        tab_bar,
        summary,
        content,
    ]
    .spacing(10)
    .into()
}

fn general_settings_panel(state: &StudioState) -> Element<'_, AppEvent> {
    let toggles = column![
        settings_toggle_button(
            "FPS Overlay",
            state.settings.show_fps_overlay,
            AppEvent::SetShowFpsOverlay(!state.settings.show_fps_overlay),
            theme::success(),
        ),
        settings_toggle_button(
            "CPU Overlay",
            state.settings.show_cpu_overlay,
            AppEvent::SetShowCpuOverlay(!state.settings.show_cpu_overlay),
            theme::warning(),
        ),
        settings_toggle_button(
            "Smooth Playhead",
            state.settings.smooth_playhead,
            AppEvent::SetSmoothPlayhead(!state.settings.smooth_playhead),
            theme::accent_playhead(),
        ),
        settings_toggle_button(
            "Follow Playhead",
            state.settings.follow_playhead,
            AppEvent::SetFollowPlayhead(!state.settings.follow_playhead),
            theme::accent_blue(),
        ),
    ]
    .spacing(8);

    let runtime = container(
        column![
            text("Runtime").size(12).color(theme::text_muted()),
            text(format!(
                "Frame {}  |  {} FPS  |  CPU {}%  |  Budget {} ms",
                state.performance.frame_index,
                state.performance.fps,
                state.performance.cpu_load.0,
                state.performance.frame_budget_ms
            ))
            .size(12)
            .color(theme::text_primary()),
            text(format!(
                "Transport: {}  |  Zoom {:.2}x",
                state.engine.transport.position_label(),
                state.timeline.viewport.zoom.as_f32()
            ))
            .size(12)
            .color(theme::text_muted()),
        ]
        .spacing(6),
    )
    .padding([8, 10])
    .style(|_| theme::panel_inner());

    column![toggles, runtime, output_runtime_card(state)]
        .spacing(10)
        .into()
}

fn output_runtime_card(state: &StudioState) -> Element<'_, AppEvent> {
    let summary = state
        .output
        .last_summary
        .clone()
        .unwrap_or_else(|| "Noch kein Output-Dispatch".to_owned());
    let error = state
        .output
        .last_error
        .clone()
        .unwrap_or_else(|| "Kein Output-Fehler".to_owned());
    let backend = dmx_backend_choices()
        .into_iter()
        .find(|choice| choice.value == state.output.last_backend)
        .map(|choice| choice.label)
        .unwrap_or("Disabled");

    container(
        column![
            text("Output Runtime").size(12).color(theme::text_muted()),
            text(format!(
                "Phase: {}  |  Seq {}",
                output_phase_label(state.output.phase),
                state.output.sequence
            ))
            .size(12)
            .color(theme::text_primary()),
            text(format!(
                "Backend: {}  |  {} DMX frame(s)  |  {} MIDI message(s)",
                backend, state.output.last_dmx_frame_count, state.output.last_midi_message_count
            ))
            .size(11)
            .color(theme::text_muted()),
            text(summary).size(11).color(theme::text_primary()),
            text(error)
                .size(11)
                .color(if state.output.last_error.is_some() {
                    theme::warning()
                } else {
                    theme::text_muted()
                }),
        ]
        .spacing(6),
    )
    .padding([8, 10])
    .style(|_| theme::panel_inner())
    .into()
}

fn dmx_output_monitor_panel(state: &StudioState) -> Element<'_, AppEvent> {
    let monitor = build_output_monitor_snapshot(state);
    let header = match monitor.backend {
        DmxBackendKind::Disabled => "Preview only, DMX backend disabled".to_owned(),
        DmxBackendKind::EnttecOpenDmx => "ENTTEC Open DMX live monitor".to_owned(),
        DmxBackendKind::ArtNet => format!(
            "Art-Net live monitor @ {}",
            state.settings.dmx.artnet_target
        ),
        DmxBackendKind::Sacn => format!("sACN live monitor @ {}", state.settings.dmx.sacn_target),
    };

    let mut content = column![
        text("DMX Monitor").size(12).color(theme::text_muted()),
        text(header).size(12).color(theme::text_primary()),
        text(if monitor.blackout_applied {
            "Blackout On Stop ist aktiv, Universes zeigen aktuell Null-Frames."
        } else {
            "Aktuelle gerenderte Slot-Pegel aus Engine und Fixture-Patches."
        })
        .size(11)
        .color(theme::text_muted()),
    ]
    .spacing(8);

    if monitor.universe_monitors.is_empty() {
        content = content.push(
            container(text("Keine gepatchten oder gerenderten DMX-Universes."))
                .padding([8, 10])
                .style(|_| theme::panel_subtle()),
        );
    } else {
        for universe in monitor.universe_monitors.clone() {
            content = content.push(output_universe_monitor_card(universe));
        }
    }

    container(content)
        .padding([10, 12])
        .style(|_| theme::panel_inner())
        .into()
}

fn midi_feedback_monitor_panel(state: &StudioState) -> Element<'_, AppEvent> {
    let monitor = build_output_monitor_snapshot(state);
    let output_label = state
        .selected_midi_output()
        .map(|port| port.name.clone())
        .unwrap_or_else(|| "No MIDI Output".to_owned());

    let mut content = column![
        text("MIDI Feedback Monitor")
            .size(12)
            .color(theme::text_muted()),
        text(format!("Output: {}", output_label))
            .size(12)
            .color(theme::text_primary()),
    ]
    .spacing(8);

    if monitor.midi_feedback_monitors.is_empty() {
        content = content.push(
            container(text("Keine gelernten MIDI-Feedback-Bindings vorhanden."))
                .padding([8, 10])
                .style(|_| theme::panel_subtle()),
        );
    } else {
        for binding in monitor.midi_feedback_monitors.clone() {
            content = content.push(midi_feedback_monitor_card(binding));
        }
    }

    container(content)
        .padding([10, 12])
        .style(|_| theme::panel_inner())
        .into()
}

fn midi_feedback_monitor_card(
    binding: crate::core::MidiFeedbackMonitor,
) -> Element<'static, AppEvent> {
    let accent = if binding.active {
        theme::success()
    } else {
        theme::muted_chip()
    };

    container(
        row![
            container(text(""))
                .width(Length::Fixed(8.0))
                .style(move |_| theme::color_bar(accent)),
            column![
                text(binding.label).size(13).color(theme::text_primary()),
                text(format!(
                    "{}  |  {:>3}%",
                    binding.message,
                    ((binding.value as u32 * 100) / 16_383)
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
    .style(move |_| theme::track_card(accent, binding.active))
    .into()
}

fn output_universe_monitor_card(universe: OutputUniverseMonitor) -> Element<'static, AppEvent> {
    let internal_universe = universe.internal_universe;
    let destination = universe.destination;
    let patch_count = universe.patch_count;
    let enabled_patch_count = universe.enabled_patch_count;
    let occupied_channels = universe.occupied_channels;
    let active_slots = universe.active_slots;
    let peak_value = universe.peak_value;
    let segment_levels = universe.segment_levels;
    let patch_labels = universe.patch_labels;

    let active = peak_value > 0;
    let accent = if active {
        output_segment_color(segment_levels.iter().copied().max().unwrap_or(0))
    } else {
        theme::muted_chip()
    };

    container(
        column![
            row![
                text(format!("U{}", internal_universe))
                    .size(14)
                    .color(theme::text_primary()),
                text(destination).size(12).color(theme::text_muted()),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            text(format!(
                "{} patch(es)  |  {} enabled  |  {} occupied ch  |  {} active slot(s)  |  peak {}",
                patch_count, enabled_patch_count, occupied_channels, active_slots, peak_value
            ))
            .size(12)
            .color(theme::text_primary()),
            output_segment_strip(segment_levels),
            text(if patch_labels.is_empty() {
                "Keine Patch-Namen in diesem Universe.".to_owned()
            } else {
                format!("Patches: {}", patch_labels.join(", "))
            })
            .size(11)
            .color(theme::text_muted()),
        ]
        .spacing(6),
    )
    .padding([8, 10])
    .style(move |_| theme::track_card(accent, active))
    .into()
}

fn output_segment_strip(levels: Vec<u16>) -> Element<'static, AppEvent> {
    row(levels
        .into_iter()
        .map(|level| {
            container(text(" "))
                .width(Length::FillPortion(1))
                .height(Length::Fixed(8.0))
                .style(move |_| theme::color_bar(output_segment_color(level)))
                .into()
        })
        .collect::<Vec<Element<'static, AppEvent>>>())
    .spacing(3)
    .into()
}

fn output_segment_color(level: u16) -> Color {
    match level {
        901..=1000 => theme::success(),
        601..=900 => theme::accent_blue(),
        301..=600 => theme::warning(),
        1..=300 => theme::muted_chip(),
        _ => Color::from_rgba8(63, 74, 87, 0.2),
    }
}

fn dmx_settings_panel(state: &StudioState) -> Element<'_, AppEvent> {
    let backend_bar = row(dmx_backend_choices()
        .into_iter()
        .map(|choice| {
            let is_active = state.settings.dmx.backend == choice.value;
            button(text(choice.label))
                .padding([6, 10])
                .style(move |_: &Theme, status| {
                    theme::toggle_button(status, is_active, theme::accent_blue())
                })
                .on_press(AppEvent::SetDmxBackend(choice.value))
                .into()
        })
        .collect::<Vec<Element<'_, AppEvent>>>())
    .spacing(8)
    .align_y(Alignment::Center);

    let interface_choices = dmx_interface_choices(state);
    let selected_interface = selected_port_choice(
        &interface_choices,
        state.settings.dmx.selected_interface.as_deref(),
    );

    let mut content = column![
        backend_bar,
        settings_toggle_button(
            "DMX Output Enabled",
            state.settings.dmx.output_enabled,
            AppEvent::SetDmxOutputEnabled(!state.settings.dmx.output_enabled),
            theme::success(),
        ),
        settings_toggle_button(
            "Auto Connect",
            state.settings.dmx.auto_connect,
            AppEvent::SetDmxAutoConnect(!state.settings.dmx.auto_connect),
            theme::accent_playhead(),
        ),
        settings_toggle_button(
            "Blackout On Stop",
            state.settings.dmx.blackout_on_stop,
            AppEvent::SetDmxBlackoutOnStop(!state.settings.dmx.blackout_on_stop),
            theme::warning(),
        ),
        column![
            text("Interface").size(12).color(theme::text_muted()),
            pick_list(interface_choices, selected_interface, |choice| {
                AppEvent::SelectDmxInterface(choice.id)
            })
            .placeholder("DMX Interface wählen"),
        ]
        .spacing(6),
        column![
            text(format!(
                "Refresh Rate: {} Hz",
                state.settings.dmx.refresh_rate_hz
            ))
            .size(12)
            .color(theme::text_muted()),
            slider(
                1..=44,
                state.settings.dmx.refresh_rate_hz,
                AppEvent::SetDmxRefreshRate
            ),
        ]
        .spacing(6),
    ]
    .spacing(10);

    if let Some(interface) = state.selected_dmx_interface() {
        content = content.push(
            container(
                column![
                    text(&interface.name).size(13).color(theme::text_primary()),
                    text(format!(
                        "{}  |  {}",
                        dmx_interface_kind_label(interface.kind),
                        interface.detail
                    ))
                    .size(11)
                    .color(theme::text_muted()),
                ]
                .spacing(4),
            )
            .padding([8, 10])
            .style(|_| theme::panel_inner()),
        );
    }

    match state.settings.dmx.backend {
        DmxBackendKind::EnttecOpenDmx => {
            content = content
                .push(
                    container(
                        column![
                            text("ENTTEC Open DMX").size(12).color(theme::text_muted()),
                            text(
                                "Output-only Interface, 1 Universe / 512 Channels, Break/MAB timing ist direkt konfigurierbar."
                            )
                            .size(12)
                            .color(theme::text_primary()),
                        ]
                        .spacing(6),
                    )
                    .padding([8, 10])
                    .style(|_| theme::panel_inner()),
                )
                .push(
                    column![
                        text(format!(
                            "Break: {} µs",
                            state.settings.dmx.enttec_break_us
                        ))
                        .size(12)
                        .color(theme::text_muted()),
                        slider(
                            88..=1000,
                            state.settings.dmx.enttec_break_us,
                            AppEvent::SetEnttecBreakMicros,
                        ),
                    ]
                    .spacing(6),
                )
                .push(
                    column![
                        text(format!(
                            "Mark After Break: {} µs",
                            state.settings.dmx.enttec_mark_after_break_us
                        ))
                        .size(12)
                        .color(theme::text_muted()),
                        slider(
                            8..=1000,
                            state.settings.dmx.enttec_mark_after_break_us,
                            AppEvent::SetEnttecMabMicros,
                        ),
                    ]
                    .spacing(6),
                );
        }
        DmxBackendKind::ArtNet => {
            content = content.push(
                column![
                    text_input("Art-Net Target", &state.settings.dmx.artnet_target)
                        .on_input(AppEvent::SetArtNetTarget)
                        .padding([8, 10]),
                    text(format!(
                        "Art-Net Universe: {}",
                        state.settings.dmx.artnet_universe
                    ))
                    .size(12)
                    .color(theme::text_muted()),
                    slider(
                        1..=64,
                        state.settings.dmx.artnet_universe,
                        AppEvent::SetArtNetUniverse
                    ),
                ]
                .spacing(8),
            );
        }
        DmxBackendKind::Sacn => {
            content = content.push(
                column![
                    text_input("sACN Target", &state.settings.dmx.sacn_target)
                        .on_input(AppEvent::SetSacnTarget)
                        .padding([8, 10]),
                    text(format!(
                        "sACN Universe: {}",
                        state.settings.dmx.sacn_universe
                    ))
                    .size(12)
                    .color(theme::text_muted()),
                    slider(
                        1..=64,
                        state.settings.dmx.sacn_universe,
                        AppEvent::SetSacnUniverse
                    ),
                ]
                .spacing(8),
            );
        }
        DmxBackendKind::Disabled => {}
    }

    if let Some(error) = &state.settings.dmx.last_error {
        content = content.push(
            container(text(error).size(12).color(theme::warning()))
                .padding([8, 10])
                .style(|_| theme::panel_inner()),
        );
    }

    content
        .push(output_runtime_card(state))
        .push(dmx_output_monitor_panel(state))
        .into()
}

fn midi_settings_panel(state: &StudioState) -> Element<'_, AppEvent> {
    let input_choices = midi_port_choices(&state.settings.midi.inputs, "No MIDI Input");
    let output_choices = midi_port_choices(&state.settings.midi.outputs, "No MIDI Output");
    let selected_input = selected_port_choice(
        &input_choices,
        state.settings.midi.selected_input.as_deref(),
    );
    let selected_output = selected_port_choice(
        &output_choices,
        state.settings.midi.selected_output.as_deref(),
    );

    let learn_active = state.settings.midi.learn.phase != MidiLearnPhase::Idle;
    let automap_button = {
        let button =
            button(text("Apply Automap"))
                .padding([8, 12])
                .style(move |_: &Theme, status| {
                    theme::toggle_button(
                        status,
                        state.can_apply_controller_automap(),
                        theme::accent_blue(),
                    )
                });
        if state.can_apply_controller_automap() {
            button.on_press(AppEvent::ApplyDetectedControllerAutomap)
        } else {
            button
        }
    };

    let clear_button = {
        let button =
            button(text("Clear Bindings"))
                .padding([8, 12])
                .style(move |_: &Theme, status| {
                    theme::toggle_button(
                        status,
                        !state.settings.midi.bindings.is_empty(),
                        theme::warning(),
                    )
                });
        if state.settings.midi.bindings.is_empty() {
            button
        } else {
            button.on_press(AppEvent::ClearMidiBindings)
        }
    };

    let cancel_button = {
        let button =
            button(text("Cancel Learn"))
                .padding([8, 12])
                .style(move |_: &Theme, status| {
                    theme::toggle_button(status, learn_active, theme::warning())
                });
        if learn_active {
            button.on_press(AppEvent::CancelMidiLearn)
        } else {
            button
        }
    };

    let summary = container(
        column![
            text(format!(
                "Learn: {}",
                midi_learn_phase_label(state.settings.midi.learn.phase)
            ))
            .size(12)
            .color(theme::text_muted()),
            text(
                state
                    .settings
                    .midi
                    .learn
                    .target_binding
                    .and_then(|binding_id| state.midi_binding(binding_id))
                    .map(|binding| format!("Target: {}", binding.label))
                    .unwrap_or_else(|| "Target: none".to_owned())
            )
            .size(12)
            .color(theme::text_primary()),
            text(
                state
                    .settings
                    .midi
                    .last_message
                    .as_ref()
                    .map(midi_runtime_message_summary)
                    .unwrap_or_else(|| "Last MIDI: none".to_owned())
            )
            .size(11)
            .color(theme::text_muted()),
        ]
        .spacing(6),
    )
    .padding([8, 10])
    .style(|_| theme::panel_inner());

    let mut bindings_column = column![].spacing(8);
    for binding in &state.settings.midi.bindings {
        bindings_column = bindings_column.push(midi_binding_row(state, binding));
    }
    if state.settings.midi.bindings.is_empty() {
        bindings_column = bindings_column.push(
            container(text("Noch keine MIDI-Bindings vorhanden."))
                .padding([8, 10])
                .style(|_| theme::panel_inner()),
        );
    }

    let mut content = column![
        column![
            text("MIDI Input").size(12).color(theme::text_muted()),
            pick_list(input_choices, selected_input, |choice| {
                AppEvent::SelectMidiInput(choice.id)
            })
            .placeholder("MIDI Input wählen"),
        ]
        .spacing(6),
        column![
            text("MIDI Output").size(12).color(theme::text_muted()),
            pick_list(output_choices, selected_output, |choice| {
                AppEvent::SelectMidiOutput(choice.id)
            })
            .placeholder("MIDI Output wählen"),
        ]
        .spacing(6),
        settings_toggle_button(
            "MIDI Feedback",
            state.settings.midi.feedback_enabled,
            AppEvent::SetMidiFeedbackEnabled(!state.settings.midi.feedback_enabled),
            theme::success(),
        ),
        row![automap_button, clear_button, cancel_button]
            .spacing(8)
            .align_y(Alignment::Center),
        summary,
        bindings_column,
    ]
    .spacing(10);

    if let Some(error) = &state.settings.midi.last_error {
        content = content.push(
            container(text(error).size(12).color(theme::warning()))
                .padding([8, 10])
                .style(|_| theme::panel_inner()),
        );
    }

    content
        .push(output_runtime_card(state))
        .push(midi_feedback_monitor_panel(state))
        .into()
}

fn controller_settings_panel(state: &StudioState) -> Element<'_, AppEvent> {
    let detected = state
        .selected_controller_profile()
        .map(controller_profile_label)
        .unwrap_or("None");
    let remaining = state.settings.midi.learn.capture_queue.len();

    let profiles = [
        (
            ControllerProfileKind::Apc40Mk2,
            "Akai APC40 mkII",
            "5x8 Grid, Device-Knobs, Fader, Transport",
        ),
        (
            ControllerProfileKind::DenonPrime2,
            "Denon Prime 2",
            "2 Decks, Sweep FX, Filter, 16 Performance Pads, Pad Modes",
        ),
        (
            ControllerProfileKind::BehringerCmdDc1,
            "Behringer CMD DC-1",
            "16 Pads, 8 Encoder, FX-Buttons, Jog/Zoom",
        ),
        (
            ControllerProfileKind::BehringerCmdLc1,
            "Behringer CMD LC-1",
            "4x8 Grid, 8 Encoder, Transport, Master Macro",
        ),
    ];

    let mut content = column![
        container(
            column![
                text(format!("Detected: {}", detected))
                    .size(12)
                    .color(theme::text_primary()),
                text(format!("Automap Queue: {} remaining", remaining))
                    .size(11)
                    .color(theme::text_muted()),
            ]
            .spacing(6),
        )
        .padding([8, 10])
        .style(|_| theme::panel_inner()),
    ]
    .spacing(10);

    for (profile, title, summary) in profiles {
        let is_detected = state.selected_controller_profile() == Some(profile);
        content = content.push(
            container(
                column![
                    text(title).size(13).color(theme::text_primary()),
                    text(summary).size(11).color(theme::text_muted()),
                    text(format!(
                        "Automap: {}",
                        controller_profile_mapping_summary(profile)
                    ))
                    .size(11)
                    .color(theme::text_muted()),
                ]
                .spacing(6),
            )
            .padding([8, 10])
            .style(move |_| {
                theme::track_card(
                    if is_detected {
                        theme::accent_blue()
                    } else {
                        theme::muted_chip()
                    },
                    is_detected,
                )
            }),
        );
    }

    content.into()
}

fn engine_settings_panel(state: &StudioState) -> Element<'_, AppEvent> {
    let mode_choices = engine_link_mode_choices();
    let selected_mode = mode_choices
        .iter()
        .find(|choice| choice.value == state.settings.engine_link.mode)
        .copied();
    let follow_choices = engine_follow_choices();
    let selected_follow = follow_choices
        .iter()
        .find(|choice| choice.value == state.settings.engine_link.follow_mode)
        .copied();
    let device_choices = engine_device_choices(state);
    let selected_device = selected_engine_device_choice(
        &device_choices,
        state.settings.engine_link.selected_device.as_deref(),
    );

    let refresh_button = {
        let button =
            button(text("Refresh Engine Link"))
                .padding([8, 12])
                .style(move |_: &Theme, status| {
                    theme::toggle_button(
                        status,
                        state.can_refresh_engine_link_discovery(),
                        theme::accent_playhead(),
                    )
                });
        if state.can_refresh_engine_link_discovery() {
            button.on_press(AppEvent::RefreshEngineLinkDiscovery)
        } else {
            button
        }
    };

    let summary = container(
        column![
            text(format!(
                "Phase: {:?}  |  Discovery UDP {}",
                state.settings.engine_link.phase, state.settings.engine_link.discovery_port
            ))
            .size(12)
            .color(theme::text_primary()),
            text(
                state
                    .settings
                    .engine_link
                    .last_summary
                    .clone()
                    .unwrap_or_else(|| {
                        "Prime-/Engine-Discovery ist bereit. Erwartet Stagelinq-Announcements."
                            .to_owned()
                    })
            )
            .size(11)
            .color(theme::text_muted()),
            text(
                state
                    .settings
                    .engine_link
                    .last_error
                    .clone()
                    .unwrap_or_else(|| "Kein Engine-Link-Fehler.".to_owned())
            )
            .size(11)
            .color(theme::warning()),
        ]
        .spacing(6),
    )
    .padding([8, 10])
    .style(|_| theme::panel_inner());

    let device_list: Element<'_, AppEvent> = if state.settings.engine_link.devices.is_empty() {
        container(text("Noch keine Prime-/Engine-Devices entdeckt."))
            .padding([8, 10])
            .style(|_| theme::panel_inner())
            .into()
    } else {
        column(
            state
                .settings
                .engine_link
                .devices
                .iter()
                .map(|device| engine_device_card(device, state))
                .collect::<Vec<_>>(),
        )
        .spacing(8)
        .into()
    };

    let telemetry_panel: Element<'_, AppEvent> =
        if let Some(telemetry) = state.settings.engine_link.telemetry.as_ref() {
            let mut content = column![
                container(
                    column![
                        text(format!("Telemetry Device: {}", telemetry.device_id))
                            .size(12)
                            .color(theme::text_primary()),
                        text(telemetry.summary.clone())
                            .size(11)
                            .color(theme::text_muted()),
                    ]
                    .spacing(4),
                )
                .padding([8, 10])
                .style(|_| theme::panel_inner()),
            ]
            .spacing(8);

            for deck in &telemetry.decks {
                content = content.push(engine_deck_card(deck));
            }

            content = content.push(
                container(
                    text(format!(
                        "Mixer: Crossfader {}  |  Channel Faders {}",
                        telemetry.mixer.crossfader.permille(),
                        telemetry
                            .mixer
                            .channel_faders
                            .iter()
                            .map(|value| value.permille().to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))
                    .size(11)
                    .color(theme::text_muted()),
                )
                .padding([8, 10])
                .style(|_| theme::panel_inner()),
            );

            content.into()
        } else {
            container(
                text(
                    "Noch keine Session-Telemetrie. Discovery und Device-Auswahl sind aktiv; "
                        .to_owned()
                        + "der Runtime-Pfad wartet auf typisierte StageLinq-Service-Daten.",
                )
                .size(11)
                .color(theme::text_muted()),
            )
            .padding([8, 10])
            .style(|_| theme::panel_inner())
            .into()
        };

    column![
        row![
            settings_toggle_button(
                "Enabled",
                state.settings.engine_link.enabled,
                AppEvent::SetEngineLinkEnabled(!state.settings.engine_link.enabled),
                theme::accent_blue(),
            ),
            settings_toggle_button(
                "Auto Connect",
                state.settings.engine_link.auto_connect,
                AppEvent::SetEngineLinkAutoConnect(!state.settings.engine_link.auto_connect),
                theme::success(),
            ),
            settings_toggle_button(
                "Adopt Transport",
                state.settings.engine_link.adopt_transport,
                AppEvent::SetEngineLinkAdoptTransport(!state.settings.engine_link.adopt_transport),
                theme::accent_playhead(),
            ),
            refresh_button,
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        row![
            pick_list(mode_choices, selected_mode, |choice| {
                AppEvent::SetEngineLinkMode(choice.value)
            })
            .placeholder("Link Mode")
            .width(Length::FillPortion(2)),
            pick_list(follow_choices, selected_follow, |choice| {
                AppEvent::SetEngineLinkFollowMode(choice.value)
            })
            .placeholder("Follow")
            .width(Length::FillPortion(2)),
            pick_list(device_choices, selected_device, |choice| {
                AppEvent::SelectEngineLinkDevice(choice.id.clone())
            })
            .placeholder("Prime Device")
            .width(Length::FillPortion(3)),
        ]
        .spacing(8),
        summary,
        device_list,
        telemetry_panel,
    ]
    .spacing(10)
    .into()
}

fn engine_device_card<'a>(
    device: &'a EnginePrimeDevice,
    state: &'a StudioState,
) -> Element<'a, AppEvent> {
    let is_selected = state.settings.engine_link.selected_device.as_deref() == Some(&device.id);
    let services = if device.services.is_empty() {
        "Services: announce only".to_owned()
    } else {
        format!(
            "Services: {}",
            device
                .services
                .iter()
                .map(|service| format!("{}:{}", service.name, service.port))
                .collect::<Vec<_>>()
                .join("  |  ")
        )
    };

    container(
        column![
            text(format!("{}  |  {}", device.name, device.address))
                .size(12)
                .color(theme::text_primary()),
            text(format!(
                "{} {}  |  Service {:?}",
                device.software_name, device.software_version, device.service_port
            ))
            .size(11)
            .color(theme::text_muted()),
            text(services).size(11).color(theme::text_muted()),
        ]
        .spacing(4),
    )
    .padding([8, 10])
    .style(move |_| {
        theme::track_card(
            if is_selected {
                theme::accent_playhead()
            } else {
                theme::muted_chip()
            },
            is_selected,
        )
    })
    .into()
}

fn engine_deck_card(deck: &crate::core::EngineDeckTelemetry) -> Element<'_, AppEvent> {
    let accent = match deck.phase {
        EngineDeckPhase::Playing | EngineDeckPhase::Syncing => theme::success(),
        EngineDeckPhase::Paused | EngineDeckPhase::Cueing => theme::warning(),
        EngineDeckPhase::Idle => theme::muted_chip(),
    };

    container(
        column![
            text(format!(
                "Deck {}  |  {}  |  {:.2} BPM",
                deck.deck_index,
                deck.track_name,
                deck.bpm.as_f32()
            ))
            .size(12)
            .color(theme::text_primary()),
            text(format!(
                "Artist: {}  |  Beat {:.2}  |  {:?}",
                deck.artist_name,
                deck.beat.as_beats_f32(),
                deck.phase
            ))
            .size(11)
            .color(theme::text_muted()),
            text(format!(
                "Master: {}  |  Sync: {}",
                if deck.is_master { "Yes" } else { "No" },
                if deck.is_synced { "On" } else { "Off" }
            ))
            .size(11)
            .color(theme::text_muted()),
        ]
        .spacing(4),
    )
    .padding([8, 10])
    .style(move |_| theme::track_card(accent, matches!(deck.phase, EngineDeckPhase::Playing)))
    .into()
}

fn settings_toggle_button<'a>(
    label: &'a str,
    active: bool,
    event: AppEvent,
    accent: iced::Color,
) -> Element<'a, AppEvent> {
    button(text(format!(
        "{}  |  {}",
        label,
        if active { "On" } else { "Off" }
    )))
    .padding([8, 12])
    .style(move |_: &Theme, status| theme::toggle_button(status, active, accent))
    .on_press(event)
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

fn fixture_management_panel(state: &StudioState) -> Element<'_, AppEvent> {
    let import_ofl_button = {
        let button = button(text("Fetch OFL"))
            .padding([6, 10])
            .style(move |_: &Theme, status| {
                theme::toggle_button(
                    status,
                    state.can_import_fixture_from_ofl(),
                    theme::accent_blue(),
                )
            });
        if state.can_import_fixture_from_ofl() {
            button.on_press(AppEvent::RequestImportFixtureFromOfl)
        } else {
            button
        }
    };

    let import_qxf_button = {
        let button = button(text("Import QXF"))
            .padding([6, 10])
            .style(move |_: &Theme, status| {
                theme::toggle_button(
                    status,
                    state.can_import_fixture_from_qxf(),
                    theme::accent_playhead(),
                )
            });
        if state.can_import_fixture_from_qxf() {
            button.on_press(AppEvent::RequestImportFixtureFromQxfPath)
        } else {
            button
        }
    };

    let export_qxf_button = {
        let button = button(text("Export QXF"))
            .padding([6, 10])
            .style(move |_: &Theme, status| {
                theme::toggle_button(
                    status,
                    state.can_export_selected_fixture_profile(),
                    theme::success(),
                )
            });
        if state.can_export_selected_fixture_profile() {
            button.on_press(AppEvent::RequestExportSelectedFixtureAsQxf)
        } else {
            button
        }
    };

    let delete_profile_button = {
        let button =
            button(text("Delete Profile"))
                .padding([6, 10])
                .style(move |_: &Theme, status| {
                    theme::toggle_button(
                        status,
                        state.can_delete_selected_fixture_profile(),
                        theme::warning(),
                    )
                });
        if state.can_delete_selected_fixture_profile() {
            button.on_press(AppEvent::DeleteSelectedFixtureProfile)
        } else {
            button
        }
    };

    let add_patch_button = {
        let button = button(text("Add Patch"))
            .padding([6, 10])
            .style(move |_: &Theme, status| {
                theme::toggle_button(
                    status,
                    state.can_create_fixture_patch(),
                    theme::accent_blue(),
                )
            });
        if state.can_create_fixture_patch() {
            button.on_press(AppEvent::CreateFixturePatch)
        } else {
            button
        }
    };

    let delete_patch_button = {
        let button =
            button(text("Delete Patch"))
                .padding([6, 10])
                .style(move |_: &Theme, status| {
                    theme::toggle_button(
                        status,
                        state.can_delete_selected_fixture_patch(),
                        theme::warning(),
                    )
                });
        if state.can_delete_selected_fixture_patch() {
            button.on_press(AppEvent::DeleteSelectedFixturePatch)
        } else {
            button
        }
    };

    let status_text = state
        .fixture_system
        .library
        .last_error
        .clone()
        .or_else(|| state.fixture_system.library.last_summary.clone())
        .unwrap_or_else(|| "Fixture-Library bereit".to_owned());

    let mut profiles = column![text("Profiles").size(12).color(theme::text_muted())].spacing(8);
    if state.fixture_system.library.profiles.is_empty() {
        profiles = profiles.push(
            container(text("Noch keine Fixture-Profile importiert."))
                .padding([8, 10])
                .style(|_| theme::panel_subtle()),
        );
    } else {
        for profile in &state.fixture_system.library.profiles {
            profiles = profiles.push(fixture_profile_row(
                profile,
                state.fixture_system.library.selected_profile.as_deref()
                    == Some(profile.id.as_str()),
            ));
        }
    }

    let mut patches = column![text("Patches").size(12).color(theme::text_muted())].spacing(8);
    if state.fixture_system.library.patches.is_empty() {
        patches = patches.push(
            container(text("Noch keine Fixtures gepatcht."))
                .padding([8, 10])
                .style(|_| theme::panel_subtle()),
        );
    } else {
        for patch in &state.fixture_system.library.patches {
            patches = patches.push(fixture_patch_row(
                patch,
                state,
                state.fixture_system.library.selected_patch == Some(patch.id),
            ));
        }
    }

    container(
        column![
            text("Fixture Library").size(13).color(theme::text_muted()),
            row![
                text_input(
                    "ofl manufacturer key",
                    &state.fixture_system.library.ofl_manufacturer_key
                )
                .on_input(AppEvent::SetFixtureOflManufacturerKey)
                .padding([8, 10])
                .width(Length::FillPortion(1)),
                text_input(
                    "ofl fixture key",
                    &state.fixture_system.library.ofl_fixture_key
                )
                .on_input(AppEvent::SetFixtureOflFixtureKey)
                .padding([8, 10])
                .width(Length::FillPortion(1)),
            ]
            .spacing(8),
            row![
                text_input(
                    "import .qxf path",
                    &state.fixture_system.library.qxf_import_path
                )
                .on_input(AppEvent::SetFixtureQxfImportPath)
                .padding([8, 10])
                .width(Length::FillPortion(2)),
                import_ofl_button,
                import_qxf_button,
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            row![
                text_input(
                    "export .qxf path",
                    &state.fixture_system.library.qxf_export_path
                )
                .on_input(AppEvent::SetFixtureQxfExportPath)
                .padding([8, 10])
                .width(Length::FillPortion(2)),
                export_qxf_button,
                delete_profile_button,
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            container(text(status_text).size(12).color(
                if state.fixture_system.library.last_error.is_some() {
                    theme::warning()
                } else {
                    theme::text_primary()
                }
            ),)
            .padding([8, 10])
            .style(|_| theme::panel_subtle()),
            selected_fixture_profile_card(state),
            row![add_patch_button, delete_patch_button]
                .spacing(8)
                .align_y(Alignment::Center),
            selected_fixture_patch_card(state),
            selected_fixture_group_patch_card(state),
            fixture_universe_summary_panel(state),
            profiles,
            patches,
        ]
        .spacing(10),
    )
    .padding([10, 12])
    .style(|_| theme::panel_inner())
    .into()
}

fn selected_fixture_profile_card(state: &StudioState) -> Element<'_, AppEvent> {
    let Some(profile) = state.selected_fixture_profile() else {
        return container(text("Kein Fixture-Profil selektiert."))
            .padding([8, 10])
            .style(|_| theme::panel_subtle())
            .into();
    };

    let source_label = match profile.source.kind {
        crate::core::FixtureSourceKind::Demo => "demo",
        crate::core::FixtureSourceKind::OpenFixtureLibrary => "ofl",
        crate::core::FixtureSourceKind::Qxf => "qxf",
    };
    let dimensions = profile
        .physical
        .as_ref()
        .and_then(|physical| physical.dimensions_mm)
        .map(|[x, y, z]| format!("{x}x{y}x{z} mm"))
        .unwrap_or_else(|| "n/a".to_owned());

    container(
        column![
            text("Selected Profile").size(12).color(theme::text_muted()),
            text(format!("{} {}", profile.manufacturer, profile.model))
                .size(15)
                .color(theme::text_primary()),
            text(format!(
                "{}  |  {} mode(s)  |  {} channel(s)",
                source_label,
                profile.modes.len(),
                profile.channels.len()
            ))
            .size(12)
            .color(theme::text_muted()),
            text(format!(
                "Categories: {}  |  Physical: {}",
                profile.categories.join(", "),
                dimensions
            ))
            .size(12)
            .color(theme::text_primary()),
        ]
        .spacing(6),
    )
    .padding([10, 12])
    .style(|_| theme::panel_subtle())
    .into()
}

fn selected_fixture_patch_card(state: &StudioState) -> Element<'_, AppEvent> {
    let Some(patch) = state.selected_fixture_patch() else {
        return container(text("Kein Fixture-Patch selektiert."))
            .padding([8, 10])
            .style(|_| theme::panel_subtle())
            .into();
    };

    let mode_options = state
        .fixture_profile(&patch.profile_id)
        .map(|profile| {
            profile
                .modes
                .iter()
                .map(|mode| mode.name.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let group_options = fixture_group_choices(state);
    let footprint = state.fixture_patch_channel_count(patch).unwrap_or(0);
    let end_address = state
        .fixture_patch_end_address(patch)
        .unwrap_or(patch.address);
    let conflicts = state.fixture_patch_conflicts(patch.id);
    let group_label = patch
        .group_id
        .and_then(|group_id| state.fixture_group(group_id))
        .map(|group| group.name.clone())
        .unwrap_or_else(|| "No Preview Group".to_owned());

    container(
        column![
            text("Selected Patch").size(12).color(theme::text_muted()),
            text_input("Patch Name", &patch.name)
                .on_input(AppEvent::SetSelectedFixturePatchName)
                .padding([8, 10]),
            row![
                column![
                    text("Mode").size(12).color(theme::text_muted()),
                    pick_list(
                        mode_options,
                        Some(patch.mode_name.clone()),
                        AppEvent::SetSelectedFixturePatchMode
                    )
                    .placeholder("Mode"),
                ]
                .spacing(6)
                .width(Length::FillPortion(2)),
                column![
                    text("Group").size(12).color(theme::text_muted()),
                    pick_list(
                        group_options.clone(),
                        selected_fixture_group_choice(&group_options, patch.group_id),
                        |choice| AppEvent::SetSelectedFixturePatchGroup(choice.id)
                    )
                    .placeholder("Preview Group"),
                ]
                .spacing(6)
                .width(Length::FillPortion(2)),
            ]
            .spacing(10),
            row![
                column![
                    text("Universe").size(12).color(theme::text_muted()),
                    slider(1.0..=64.0, patch.universe as f32, |value| {
                        AppEvent::SetSelectedFixturePatchUniverse(value.round() as u16)
                    })
                    .step(1.0),
                    text(format!("U{}", patch.universe))
                        .size(12)
                        .color(theme::text_primary()),
                ]
                .spacing(6)
                .width(Length::FillPortion(1)),
                column![
                    text("Address").size(12).color(theme::text_muted()),
                    slider(1.0..=512.0, patch.address as f32, |value| {
                        AppEvent::SetSelectedFixturePatchAddress(value.round() as u16)
                    })
                    .step(1.0),
                    text(format!("{:03}", patch.address))
                        .size(12)
                        .color(theme::text_primary()),
                ]
                .spacing(6)
                .width(Length::FillPortion(1)),
            ]
            .spacing(10),
            text(format!(
                "Span U{}.{}-{}  |  {}ch  |  Group {}",
                patch.universe, patch.address, end_address, footprint, group_label
            ))
            .size(12)
            .color(theme::text_primary()),
            text(if conflicts.is_empty() {
                "Universe layout clean".to_owned()
            } else {
                format!("Overlap with patch ids {:?}", conflicts)
            })
            .size(12)
            .color(if conflicts.is_empty() {
                theme::text_muted()
            } else {
                theme::warning()
            }),
        ]
        .spacing(10),
    )
    .padding([10, 12])
    .style(|_| theme::panel_subtle())
    .into()
}

fn selected_fixture_group_patch_card(state: &StudioState) -> Element<'_, AppEvent> {
    let Some(group) = state.selected_fixture_group() else {
        return container(text("Keine Preview-Gruppe fuer Patch-Mapping selektiert."))
            .padding([8, 10])
            .style(|_| theme::panel_subtle())
            .into();
    };

    let summary = state.fixture_group_patch_summary(group.id);
    let mapped_names = state
        .fixture_patches_for_group(group.id)
        .into_iter()
        .map(|patch| patch.name.clone())
        .take(4)
        .collect::<Vec<_>>();

    container(
        column![
            text("Selected Group Patch Map")
                .size(12)
                .color(theme::text_muted()),
            text(format!(
                "{}  |  {} mapped patch(es)  |  {} enabled",
                group.name, summary.patch_count, summary.enabled_patch_count
            ))
            .size(15)
            .color(theme::text_primary()),
            text(format!(
                "{}  |  {} occupied DMX ch  |  {} footprint ch",
                format_universe_labels(&summary.universes),
                summary.occupied_channels,
                summary.footprint_channels
            ))
            .size(12)
            .color(theme::text_primary()),
            text(if summary.conflicting_patch_ids.is_empty() {
                "No patch overlaps in this preview group".to_owned()
            } else {
                format!("Overlap patch ids {:?}", summary.conflicting_patch_ids)
            })
            .size(12)
            .color(if summary.conflicting_patch_ids.is_empty() {
                theme::text_muted()
            } else {
                theme::warning()
            }),
            text(if mapped_names.is_empty() {
                "Noch keine gepatchten Fixtures in dieser Gruppe.".to_owned()
            } else {
                format!("Mapped: {}", mapped_names.join(", "))
            })
            .size(12)
            .color(theme::text_muted()),
        ]
        .spacing(6),
    )
    .padding([10, 12])
    .style(|_| theme::panel_subtle())
    .into()
}

fn fixture_universe_summary_panel(state: &StudioState) -> Element<'_, AppEvent> {
    let summaries = state.fixture_universe_summaries();
    if summaries.is_empty() {
        return container(text("Keine belegten DMX-Universes."))
            .padding([8, 10])
            .style(|_| theme::panel_subtle())
            .into();
    }

    let monitor = build_output_monitor_snapshot(state);
    let mut content = column![
        text("DMX Universe Routing")
            .size(12)
            .color(theme::text_muted())
    ]
    .spacing(8);
    for summary in summaries {
        let monitor_entry = monitor
            .universe_monitors
            .iter()
            .find(|entry| entry.internal_universe == summary.universe);
        let destination = monitor_entry
            .map(|entry| entry.destination.clone())
            .unwrap_or_else(|| "Preview only".to_owned());
        let active_slots = monitor_entry.map(|entry| entry.active_slots).unwrap_or(0);
        let peak_value = monitor_entry.map(|entry| entry.peak_value).unwrap_or(0);
        let segment_levels = monitor_entry
            .map(|entry| entry.segment_levels.clone())
            .unwrap_or_else(|| vec![0; 16]);
        let patch_labels = monitor_entry
            .map(|entry| entry.patch_labels.clone())
            .unwrap_or_default();

        content = content.push(
            container(
                column![
                    row![
                        text(format!("U{}", summary.universe))
                            .size(14)
                            .color(theme::text_primary()),
                        text(format!(
                            "{} patch(es)  |  {} enabled",
                            summary.patch_count, summary.enabled_patch_count
                        ))
                        .size(12)
                        .color(theme::text_muted()),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                    text(format!(
                        "{} occupied ch  |  {} footprint ch  |  max {}",
                        summary.occupied_channels,
                        summary.footprint_channels,
                        summary.highest_address
                    ))
                    .size(12)
                    .color(theme::text_primary()),
                    text(format!(
                        "Route: {}  |  {} active slot(s)  |  peak {}",
                        destination, active_slots, peak_value
                    ))
                    .size(12)
                    .color(theme::text_muted()),
                    output_segment_strip(segment_levels),
                    text(if summary.conflicting_patch_ids.is_empty() {
                        "Universe layout clean".to_owned()
                    } else {
                        format!("Overlap patch ids {:?}", summary.conflicting_patch_ids)
                    })
                    .size(12)
                    .color(if summary.conflicting_patch_ids.is_empty() {
                        theme::text_muted()
                    } else {
                        theme::warning()
                    }),
                    text(if patch_labels.is_empty() {
                        "Keine aktiven Patch-Namen fuer dieses Universe.".to_owned()
                    } else {
                        format!("Patches: {}", patch_labels.join(", "))
                    })
                    .size(11)
                    .color(theme::text_muted()),
                ]
                .spacing(6),
            )
            .padding([8, 10])
            .style(|_| theme::panel_subtle()),
        );
    }

    content.into()
}

fn fixture_profile_row(
    profile: &crate::core::FixtureProfile,
    is_selected: bool,
) -> Element<'_, AppEvent> {
    let accent = if is_selected {
        theme::accent_blue()
    } else {
        theme::muted_chip()
    };

    container(
        column![
            row![
                container(text(""))
                    .width(Length::Fixed(8.0))
                    .style(move |_| theme::color_bar(accent)),
                text(format!("{} {}", profile.manufacturer, profile.model))
                    .size(14)
                    .color(theme::text_primary()),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            row![
                button(text("Select"))
                    .padding([6, 10])
                    .style(move |_: &Theme, button_state| {
                        theme::toggle_button(button_state, is_selected, theme::accent_blue())
                    })
                    .on_press(AppEvent::SelectFixtureProfile(profile.id.clone())),
                text(format!(
                    "{} mode(s)  |  {} channel(s)",
                    profile.modes.len(),
                    profile.channels.len()
                ))
                .size(12)
                .color(theme::text_muted()),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(8),
    )
    .padding([10, 12])
    .style(move |_| theme::track_card(accent, is_selected))
    .into()
}

fn fixture_patch_row<'a>(
    patch: &'a FixturePatch,
    state: &'a StudioState,
    is_selected: bool,
) -> Element<'a, AppEvent> {
    let accent = if patch.enabled {
        theme::success()
    } else {
        theme::warning()
    };
    let profile_label = state
        .fixture_profile(&patch.profile_id)
        .map(|profile| format!("{} {}", profile.manufacturer, profile.model))
        .unwrap_or_else(|| patch.profile_id.clone());
    let footprint = state.fixture_patch_channel_count(patch).unwrap_or(0);
    let end_address = state
        .fixture_patch_end_address(patch)
        .unwrap_or(patch.address);
    let conflicts = state.fixture_patch_conflicts(patch.id);
    let group_label = patch
        .group_id
        .and_then(|group_id| state.fixture_group(group_id))
        .map(|group| group.name.clone())
        .unwrap_or_else(|| "No group".to_owned());

    container(
        column![
            row![
                container(text(""))
                    .width(Length::Fixed(8.0))
                    .style(move |_| theme::color_bar(accent)),
                text(&patch.name).size(14).color(theme::text_primary()),
                text(format!("U{}.{}", patch.universe, patch.address))
                    .size(12)
                    .color(theme::text_muted()),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            row![
                button(text("Select"))
                    .padding([6, 10])
                    .style(move |_: &Theme, button_state| {
                        theme::toggle_button(button_state, is_selected, theme::accent_blue())
                    })
                    .on_press(AppEvent::SelectFixturePatch(patch.id)),
                text(format!(
                    "{}  |  {}  |  {}  |  {}ch  |  {}-{}{}",
                    profile_label,
                    patch.mode_name,
                    group_label,
                    footprint,
                    patch.address,
                    end_address,
                    if conflicts.is_empty() { "" } else { " overlap" }
                ))
                .size(12)
                .color(if conflicts.is_empty() {
                    theme::text_muted()
                } else {
                    theme::warning()
                }),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(8),
    )
    .padding([10, 12])
    .style(move |_| theme::track_card(accent, is_selected))
    .into()
}

fn fixture_row<'a>(
    group: &'a FixtureGroup,
    state: &'a StudioState,
    is_selected: bool,
) -> Element<'a, AppEvent> {
    let accent = group.accent.to_iced();
    let status = match group.phase {
        FixturePhase::Uninitialized => "uninitialized",
        FixturePhase::Mapped => "mapped",
        FixturePhase::Active => "active",
        FixturePhase::Error => "error",
    };
    let summary = state.fixture_group_patch_summary(group.id);

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
                text(format!("Patches {}", summary.patch_count))
                    .size(12)
                    .color(theme::text_muted()),
                text(format!("{}", format_universe_labels(&summary.universes)))
                    .size(12)
                    .color(if summary.conflicting_patch_ids.is_empty() {
                        theme::text_muted()
                    } else {
                        theme::warning()
                    }),
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

fn dmx_interface_kind_label(kind: DmxInterfaceKind) -> &'static str {
    match kind {
        DmxInterfaceKind::EnttecOpenDmxCompatible => "ENTTEC Open DMX compatible",
        DmxInterfaceKind::UsbSerial => "USB Serial",
        DmxInterfaceKind::Unknown => "Unknown",
    }
}

fn midi_action_label(action: MidiAction) -> String {
    match action {
        MidiAction::TransportToggle => "Transport".to_owned(),
        MidiAction::MasterIntensity => "Master Intensity".to_owned(),
        MidiAction::MasterSpeed => "Master Speed".to_owned(),
        MidiAction::TimelineZoom => "Timeline Zoom".to_owned(),
        MidiAction::TriggerCueSlot(slot) => format!("Cue Slot {}", slot),
        MidiAction::TriggerChaseSlot(slot) => format!("Chase Slot {}", slot),
        MidiAction::FocusFixtureGroupSlot(slot) => format!("Fixture Group {}", slot),
        MidiAction::FxDepthSlot(slot) => format!("FX Depth {}", slot),
    }
}

fn midi_message_kind_label(kind: MidiMessageKind) -> &'static str {
    match kind {
        MidiMessageKind::Note => "Note",
        MidiMessageKind::ControlChange => "CC",
        MidiMessageKind::PitchBend => "Pitch",
    }
}

fn midi_binding_message_summary(message: &MidiBindingMessage) -> String {
    format!(
        "{} ch{} key{}",
        midi_message_kind_label(message.kind),
        message.channel,
        message.key
    )
}

fn midi_runtime_message_summary(message: &crate::core::MidiRuntimeMessage) -> String {
    format!(
        "Last MIDI: {} ch{} key{} val{}",
        midi_message_kind_label(message.kind),
        message.channel,
        message.key,
        message.value
    )
}

fn midi_control_hint_label(hint: MidiControlHint) -> &'static str {
    match hint {
        MidiControlHint::Button => "Button",
        MidiControlHint::Continuous => "Continuous",
        MidiControlHint::Any => "Any",
    }
}

fn midi_learn_phase_label(phase: MidiLearnPhase) -> &'static str {
    match phase {
        MidiLearnPhase::Idle => "Idle",
        MidiLearnPhase::Listening => "Listening",
        MidiLearnPhase::GuidedAutomap => "Guided Automap",
    }
}

fn output_phase_label(phase: crate::core::OutputDeliveryPhase) -> &'static str {
    match phase {
        crate::core::OutputDeliveryPhase::Idle => "Idle",
        crate::core::OutputDeliveryPhase::Dispatching => "Dispatching",
        crate::core::OutputDeliveryPhase::Delivered => "Delivered",
        crate::core::OutputDeliveryPhase::Error => "Error",
    }
}

fn controller_profile_label(profile: ControllerProfileKind) -> &'static str {
    match profile {
        ControllerProfileKind::Apc40Mk2 => "APC40 mkII",
        ControllerProfileKind::DenonPrime2 => "Denon Prime 2",
        ControllerProfileKind::BehringerCmdDc1 => "CMD DC-1",
        ControllerProfileKind::BehringerCmdLc1 => "CMD LC-1",
    }
}

fn controller_profile_mapping_summary(profile: ControllerProfileKind) -> &'static str {
    match profile {
        ControllerProfileKind::Apc40Mk2 => {
            "Master, Crossfader, 8 Device Knobs, Transport, 40 Cue Slots"
        }
        ControllerProfileKind::DenonPrime2 => {
            "2 Sweep FX, 2 Filter, 2 FX Select, View Encoder, 16 Cue Pads, 8 Chase Pad Modes"
        }
        ControllerProfileKind::BehringerCmdDc1 => {
            "Zoom, 8 Encoder, Transport, 16 Cue Pads, 8 Chase Buttons"
        }
        ControllerProfileKind::BehringerCmdLc1 => {
            "8 Encoder, Transport, Master Macro, 32 Cue Grid Slots"
        }
    }
}

fn midi_binding_row<'a>(state: &'a StudioState, binding: &'a MidiBinding) -> Element<'a, AppEvent> {
    let is_learning = state.settings.midi.learn.target_binding == Some(binding.id);
    let mapping = binding
        .message
        .as_ref()
        .map(midi_binding_message_summary)
        .unwrap_or_else(|| "Pending learn".to_owned());
    let subtitle = format!(
        "{}  |  {}  |  {}",
        midi_action_label(binding.action),
        midi_control_hint_label(binding.hint),
        if binding.learned {
            "Learned"
        } else {
            "Template"
        }
    );

    container(
        row![
            column![
                text(&binding.label).size(13).color(theme::text_primary()),
                text(subtitle).size(11).color(theme::text_muted()),
                text(mapping).size(11).color(theme::text_muted()),
            ]
            .spacing(4)
            .width(Length::Fill),
            button(text(if is_learning { "Listening" } else { "Learn" }))
                .padding([6, 10])
                .style(move |_: &Theme, status| {
                    theme::toggle_button(status, is_learning, theme::accent_blue())
                })
                .on_press(AppEvent::StartMidiLearn(binding.id)),
            button(text("Clear"))
                .padding([6, 10])
                .style(|_: &Theme, status| {
                    theme::toggle_button(status, binding.message.is_some(), theme::warning())
                })
                .on_press(AppEvent::RemoveMidiBinding(binding.id)),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([8, 10])
    .style(move |_| {
        theme::track_card(
            if is_learning {
                theme::accent_blue()
            } else {
                theme::muted_chip()
            },
            is_learning,
        )
    })
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

    let mut chips = row![].spacing(8).align_y(Alignment::Center);
    if state.settings.show_fps_overlay {
        chips = chips.push(perf_chip("FPS", state.performance.fps.to_string()));
    }
    if state.settings.show_cpu_overlay {
        chips = chips.push(perf_chip(
            "CPU",
            format!("{}%", state.performance.cpu_load.0),
        ));
    }

    container(
        row![container(status).width(Length::Fill), chips]
            .spacing(12)
            .align_y(Alignment::Center),
    )
    .padding([10, 14])
    .style(|_| theme::status_bar())
    .into()
}
