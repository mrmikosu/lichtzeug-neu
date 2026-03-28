use luma_switch::core::{
    AppEvent, BeatTime, ChaseId, ClipId, ControllerProfileKind, CueId, DmxBackendKind,
    DmxInterfaceDescriptor, DmxInterfaceKind, EngineDeckFollowMode, EngineDeckPhase,
    EngineLinkMode, EngineMixerTelemetry, EnginePhase, EnginePrimeDevice, EngineServiceDescriptor,
    EngineTelemetryFrame, FixtureChannel, FixtureGroupId, FixtureMode, FixturePatch,
    FixturePhysical, FixtureProfile, FixtureSourceInfo, FixtureSourceKind, FxId, FxWaveform,
    HardwareInventorySnapshot, IntensityLevel, MidiMessageKind, MidiPortDescriptor,
    MidiPortDirection, MidiRuntimeMessage, PPQ, SelectionState, StudioState, TempoBpm,
    TimelineCursor, TimelineEvent, TimelineHit, TimelineZone, TrackId,
    build_output_monitor_snapshot, build_runtime_output_snapshot, dispatch, export_project_json,
    foundation_spec, foundation_spec_json, import_ofl_fixture, import_project_json, list_ventures,
    load_recovery_registry, load_venture, replay_events, save_venture, validate_state,
};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn integration_project_structure_exports_core_modules() {
    let spec = foundation_spec();
    assert!(
        spec.project_structure
            .iter()
            .any(|path| path == "src/core/state.rs")
    );
    assert!(
        spec.project_structure
            .iter()
            .any(|path| path == "src/core/reducer.rs")
    );
    assert!(
        spec.project_structure
            .iter()
            .any(|path| path == "src/core/automation.rs")
    );
    assert!(
        spec.project_structure
            .iter()
            .any(|path| path == "src/core/hardware.rs")
    );
    assert!(
        spec.project_structure
            .iter()
            .any(|path| path == "src/core/engine_link.rs")
    );
    assert!(
        spec.project_structure
            .iter()
            .any(|path| path == "src/core/output.rs")
    );
    assert!(
        spec.project_structure
            .iter()
            .any(|path| path == "src/core/project.rs")
    );
    assert!(
        spec.project_structure
            .iter()
            .any(|path| path == "src/ui/fixture_view.rs")
    );
}

#[test]
fn integration_queue_drives_reducer_without_reordering() {
    let script = vec![
        AppEvent::Tick,
        AppEvent::SetMasterIntensity(900),
        AppEvent::Tick,
    ];
    let state = replay_events(&script);

    assert!(state.event_queue.completed.len() >= 3);
    assert!(
        state
            .event_queue
            .completed
            .windows(2)
            .all(|pair| pair[0].sequence < pair[1].sequence)
    );
}

#[test]
fn simulation_engine_tick_updates_clip_phases() {
    let mut state = luma_switch::core::StudioState::default();
    state.engine.phase = EnginePhase::Running;
    state.engine.transport.playhead = BeatTime::from_ticks((PPQ * 8) - 5);

    luma_switch::core::dispatch(&mut state, AppEvent::Tick);

    let clip = state.clip(ClipId(102)).expect("clip exists");
    assert!(matches!(
        clip.phase,
        luma_switch::core::ClipPhase::Triggered | luma_switch::core::ClipPhase::Active
    ));
}

#[test]
fn scenario_drag_zoom_scrub_keeps_state_valid() {
    let script = vec![
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
            x_px: 220,
            y_px: 82,
        })),
        AppEvent::SetTimelineZoom(1325),
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

    let state = replay_events(&script);
    let report = validate_state(&state);

    assert!(report.valid);
    assert!(matches!(
        state.timeline.selection,
        SelectionState::Clip(ClipId(102))
    ));
}

#[test]
fn replay_produces_identical_state_snapshot() {
    let script = vec![
        AppEvent::Tick,
        AppEvent::SetMasterIntensity(910),
        AppEvent::SetMasterSpeed(720),
        AppEvent::Timeline(TimelineEvent::Scrolled {
            delta_lines: -2,
            anchor_x_px: 200,
            anchor_beat: BeatTime::from_beats(8),
        }),
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);

    assert_eq!(
        serde_json::to_string(&left).expect("serialize left"),
        serde_json::to_string(&right).expect("serialize right")
    );
}

#[test]
fn integration_runtime_output_snapshot_replays_deterministically() {
    let mut left = StudioState::default();
    left.settings.dmx.output_enabled = true;
    left.settings.dmx.backend = DmxBackendKind::ArtNet;
    left.engine.clock.monotonic_ns = 33_333_334;
    left.fixture_system.library.profiles = vec![test_output_profile()];
    left.fixture_system.library.patches = vec![FixturePatch {
        id: 1,
        profile_id: "integration-output".to_owned(),
        name: "Front Wash".to_owned(),
        mode_name: "8bit".to_owned(),
        universe: 1,
        address: 1,
        group_id: Some(FixtureGroupId(1)),
        enabled: true,
    }];

    let right = left.clone();
    let left_snapshot = build_runtime_output_snapshot(&left).expect("left snapshot");
    let right_snapshot = build_runtime_output_snapshot(&right).expect("right snapshot");

    assert_eq!(
        serde_json::to_string(&left_snapshot).expect("serialize left snapshot"),
        serde_json::to_string(&right_snapshot).expect("serialize right snapshot")
    );
}

#[test]
fn integration_output_monitor_snapshot_is_deterministic() {
    let mut left = StudioState::default();
    left.settings.dmx.backend = DmxBackendKind::ArtNet;
    left.settings.dmx.artnet_universe = 6;
    left.fixture_system.library.profiles = vec![test_output_profile()];
    left.fixture_system.library.patches = vec![FixturePatch {
        id: 1,
        profile_id: "integration-output".to_owned(),
        name: "Front Wash".to_owned(),
        mode_name: "8bit".to_owned(),
        universe: 2,
        address: 1,
        group_id: Some(FixtureGroupId(1)),
        enabled: true,
    }];

    let right = left.clone();
    let left_monitor = build_output_monitor_snapshot(&left);
    let right_monitor = build_output_monitor_snapshot(&right);

    assert_eq!(
        serde_json::to_string(&left_monitor).expect("serialize left monitor"),
        serde_json::to_string(&right_monitor).expect("serialize right monitor")
    );
}

#[test]
fn integration_engine_link_discovery_and_transport_follow_replays_deterministically() {
    let device = EnginePrimeDevice {
        id: "denon-prime-2-192-168-1-99-50010".to_owned(),
        name: "Denon Prime 2".to_owned(),
        address: "192.168.1.99".to_owned(),
        software_name: "Engine DJ".to_owned(),
        software_version: "4.1.0".to_owned(),
        announce_port: 51_337,
        service_port: Some(50_010),
        token_hint: Some("abcd1234".to_owned()),
        services: vec![
            EngineServiceDescriptor {
                name: "BeatInfo".to_owned(),
                port: 50_020,
                detail: "Beat telemetry".to_owned(),
            },
            EngineServiceDescriptor {
                name: "StateMap".to_owned(),
                port: 50_030,
                detail: "State telemetry".to_owned(),
            },
        ],
        detail: "Prime 2 StageLinq".to_owned(),
        last_seen_frame: 0,
    };
    let telemetry = EngineTelemetryFrame {
        device_id: device.id.clone(),
        decks: vec![
            luma_switch::core::EngineDeckTelemetry {
                deck_index: 1,
                track_name: "Intro".to_owned(),
                artist_name: "Artist".to_owned(),
                bpm: TempoBpm::from_whole_bpm(124),
                beat: BeatTime::from_beats(12),
                phase: EngineDeckPhase::Playing,
                is_master: true,
                is_synced: true,
            },
            luma_switch::core::EngineDeckTelemetry {
                deck_index: 2,
                track_name: "Drop".to_owned(),
                artist_name: "Artist".to_owned(),
                bpm: TempoBpm::from_whole_bpm(128),
                beat: BeatTime::from_beats(24),
                phase: EngineDeckPhase::Paused,
                is_master: false,
                is_synced: false,
            },
        ],
        mixer: EngineMixerTelemetry {
            crossfader: IntensityLevel::from_permille(500),
            channel_faders: vec![
                IntensityLevel::from_permille(1000),
                IntensityLevel::from_permille(820),
            ],
        },
        summary: "Prime 2 session frame".to_owned(),
    };
    let script = vec![
        AppEvent::SetEngineLinkMode(EngineLinkMode::StageLinqExperimental),
        AppEvent::SetEngineLinkEnabled(true),
        AppEvent::SetEngineLinkAdoptTransport(true),
        AppEvent::SetEngineLinkFollowMode(EngineDeckFollowMode::MasterDeck),
        AppEvent::ApplyEngineLinkDiscoveryDevice(device),
        AppEvent::SelectEngineLinkDevice(Some("denon-prime-2-192-168-1-99-50010".to_owned())),
        AppEvent::ApplyEngineLinkTelemetry(telemetry),
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);

    assert_eq!(
        serde_json::to_string(&left).expect("serialize left"),
        serde_json::to_string(&right).expect("serialize right")
    );
    assert_eq!(left.engine.transport.bpm, TempoBpm::from_whole_bpm(124));
    assert_eq!(left.engine.transport.playhead, BeatTime::from_beats(12));
    assert_eq!(
        left.settings.engine_link.selected_device.as_deref(),
        Some("denon-prime-2-192-168-1-99-50010")
    );
}

#[test]
fn integration_small_pointer_jitter_does_not_move_clip() {
    let script = vec![
        AppEvent::Timeline(TimelineEvent::PointerPressed(TimelineCursor {
            beat: BeatTime::from_beats(8),
            track: Some(TrackId(1)),
            zone: TimelineZone::Track,
            target: Some(TimelineHit::ClipBody(ClipId(102))),
            x_px: 180,
            y_px: 82,
        })),
        AppEvent::Timeline(TimelineEvent::PointerMoved(TimelineCursor {
            beat: BeatTime::from_beats_f32(8.03),
            track: Some(TrackId(1)),
            zone: TimelineZone::Track,
            target: Some(TimelineHit::ClipBody(ClipId(102))),
            x_px: 183,
            y_px: 84,
        })),
        AppEvent::Timeline(TimelineEvent::PointerReleased(TimelineCursor {
            beat: BeatTime::from_beats_f32(8.03),
            track: Some(TrackId(1)),
            zone: TimelineZone::Track,
            target: Some(TimelineHit::ClipBody(ClipId(102))),
            x_px: 183,
            y_px: 84,
        })),
    ];

    let state = replay_events(&script);

    assert_eq!(
        state.clip(ClipId(102)).expect("clip exists").start,
        BeatTime::from_beats(8)
    );
    assert!(validate_state(&state).valid);
}

#[test]
fn integration_zoom_anchor_replays_deterministically() {
    let script = vec![AppEvent::Timeline(TimelineEvent::Scrolled {
        delta_lines: -4,
        anchor_x_px: 200,
        anchor_beat: BeatTime::from_beats(8),
    })];

    let left = replay_events(&script);
    let right = replay_events(&script);
    let left_anchor = BeatTime::from_ticks(
        (left.timeline.viewport.scroll.ticks() as f32
            + ((200.0 / (40.0 * left.timeline.viewport.zoom.as_f32())) * PPQ as f32))
            .round() as u32,
    );
    let right_anchor = BeatTime::from_ticks(
        (right.timeline.viewport.scroll.ticks() as f32
            + ((200.0 / (40.0 * right.timeline.viewport.zoom.as_f32())) * PPQ as f32))
            .round() as u32,
    );

    assert_eq!(left_anchor, BeatTime::from_beats(8));
    assert_eq!(left_anchor, right_anchor);
    assert_eq!(
        serde_json::to_string(&left.timeline).expect("serialize left timeline"),
        serde_json::to_string(&right.timeline).expect("serialize right timeline")
    );
}

#[test]
fn integration_box_selection_replays_deterministically() {
    let script = vec![
        AppEvent::Timeline(TimelineEvent::PointerPressed(TimelineCursor {
            beat: BeatTime::from_fraction(1, 5),
            track: Some(TrackId(1)),
            zone: TimelineZone::Track,
            target: None,
            x_px: 10,
            y_px: 44,
        })),
        AppEvent::Timeline(TimelineEvent::PointerMoved(TimelineCursor {
            beat: BeatTime::from_beats(9),
            track: Some(TrackId(1)),
            zone: TimelineZone::Track,
            target: Some(TimelineHit::ClipBody(ClipId(102))),
            x_px: 420,
            y_px: 114,
        })),
        AppEvent::Timeline(TimelineEvent::PointerReleased(TimelineCursor {
            beat: BeatTime::from_beats(9),
            track: Some(TrackId(1)),
            zone: TimelineZone::Track,
            target: Some(TimelineHit::ClipBody(ClipId(102))),
            x_px: 420,
            y_px: 114,
        })),
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);

    assert_eq!(left.timeline.selection, SelectionState::Clip(ClipId(101)));
    assert_eq!(left.timeline.selected_clips, vec![ClipId(101), ClipId(102)]);
    assert_eq!(
        serde_json::to_string(&left.timeline).expect("serialize left timeline"),
        serde_json::to_string(&right.timeline).expect("serialize right timeline")
    );
}

#[test]
fn integration_settings_hardware_automap_replays_deterministically() {
    let midi_input_id = "midi-in::0::akai-apc40-mkii".to_owned();
    let midi_output_id = "midi-out::0::akai-apc40-mkii".to_owned();
    let dmx_id = "dmx::enttec-open-dmx-usb".to_owned();
    let inventory = HardwareInventorySnapshot {
        dmx_interfaces: vec![DmxInterfaceDescriptor {
            id: dmx_id.clone(),
            name: "ENTTEC Open DMX USB".to_owned(),
            kind: DmxInterfaceKind::EnttecOpenDmxCompatible,
            port_name: "/dev/cu.usbserial-enttec".to_owned(),
            manufacturer: Some("ENTTEC".to_owned()),
            product: Some("Open DMX USB".to_owned()),
            serial_number: Some("enttec-demo-1".to_owned()),
            detail: "Open DMX USB / ENTTEC @ /dev/cu.usbserial-enttec".to_owned(),
            universe_capacity: 1,
        }],
        midi_inputs: vec![MidiPortDescriptor {
            id: midi_input_id.clone(),
            name: "Akai APC40 mkII".to_owned(),
            direction: MidiPortDirection::Input,
            profile_hint: Some(ControllerProfileKind::Apc40Mk2),
            detail: "Akai APC40 mkII (APC40 mkII)".to_owned(),
        }],
        midi_outputs: vec![MidiPortDescriptor {
            id: midi_output_id.clone(),
            name: "Akai APC40 mkII".to_owned(),
            direction: MidiPortDirection::Output,
            profile_hint: Some(ControllerProfileKind::Apc40Mk2),
            detail: "Akai APC40 mkII (APC40 mkII)".to_owned(),
        }],
    };
    let script = vec![
        AppEvent::ApplyHardwareInventory(inventory),
        AppEvent::SetDmxBackend(DmxBackendKind::EnttecOpenDmx),
        AppEvent::SelectDmxInterface(Some(dmx_id.clone())),
        AppEvent::SelectMidiInput(Some(midi_input_id.clone())),
        AppEvent::SelectMidiOutput(Some(midi_output_id.clone())),
        AppEvent::ApplyDetectedControllerAutomap,
        AppEvent::StartMidiLearn(1),
        AppEvent::CompleteMidiLearn(MidiRuntimeMessage {
            timestamp_micros: 1,
            kind: MidiMessageKind::ControlChange,
            channel: 1,
            key: 7,
            value: 64,
        }),
        AppEvent::StartMidiLearn(11),
        AppEvent::CompleteMidiLearn(MidiRuntimeMessage {
            timestamp_micros: 2,
            kind: MidiMessageKind::Note,
            channel: 1,
            key: 91,
            value: 127,
        }),
        AppEvent::ReceiveMidiRuntimeMessage(MidiRuntimeMessage {
            timestamp_micros: 3,
            kind: MidiMessageKind::ControlChange,
            channel: 1,
            key: 7,
            value: 127,
        }),
        AppEvent::ReceiveMidiRuntimeMessage(MidiRuntimeMessage {
            timestamp_micros: 4,
            kind: MidiMessageKind::Note,
            channel: 1,
            key: 91,
            value: 127,
        }),
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);

    assert_eq!(
        serde_json::to_string(&left.settings).expect("serialize left settings"),
        serde_json::to_string(&right.settings).expect("serialize right settings")
    );
    assert_eq!(
        left.settings.midi.bindings[0].message,
        right.settings.midi.bindings[0].message
    );
    assert_eq!(left.master.intensity, IntensityLevel::from_permille(1000));
    assert_eq!(left.engine.phase, EnginePhase::Paused);
    assert_eq!(
        left.settings.dmx.selected_interface.as_deref(),
        Some(dmx_id.as_str())
    );
    assert!(validate_state(&left).valid);
}

#[test]
fn integration_denon_prime_2_automap_replays_deterministically() {
    let midi_input_id = "midi-in::0::denon-dj-prime-2-midi".to_owned();
    let midi_output_id = "midi-out::0::denon-dj-prime-2-midi".to_owned();
    let inventory = HardwareInventorySnapshot {
        dmx_interfaces: Vec::new(),
        midi_inputs: vec![MidiPortDescriptor {
            id: midi_input_id.clone(),
            name: "Denon DJ PRIME 2 MIDI".to_owned(),
            direction: MidiPortDirection::Input,
            profile_hint: Some(ControllerProfileKind::DenonPrime2),
            detail: "Denon DJ PRIME 2 MIDI (Denon Prime 2)".to_owned(),
        }],
        midi_outputs: vec![MidiPortDescriptor {
            id: midi_output_id.clone(),
            name: "Denon DJ PRIME 2 MIDI".to_owned(),
            direction: MidiPortDirection::Output,
            profile_hint: Some(ControllerProfileKind::DenonPrime2),
            detail: "Denon DJ PRIME 2 MIDI (Denon Prime 2)".to_owned(),
        }],
    };
    let script = vec![
        AppEvent::ApplyHardwareInventory(inventory),
        AppEvent::SelectMidiInput(Some(midi_input_id.clone())),
        AppEvent::SelectMidiOutput(Some(midi_output_id.clone())),
        AppEvent::ApplyDetectedControllerAutomap,
        AppEvent::StartMidiLearn(1),
        AppEvent::CompleteMidiLearn(MidiRuntimeMessage {
            timestamp_micros: 1,
            kind: MidiMessageKind::ControlChange,
            channel: 1,
            key: 12,
            value: 64,
        }),
        AppEvent::StartMidiLearn(14),
        AppEvent::CompleteMidiLearn(MidiRuntimeMessage {
            timestamp_micros: 2,
            kind: MidiMessageKind::Note,
            channel: 1,
            key: 36,
            value: 127,
        }),
        AppEvent::StartMidiLearn(33),
        AppEvent::CompleteMidiLearn(MidiRuntimeMessage {
            timestamp_micros: 3,
            kind: MidiMessageKind::Note,
            channel: 1,
            key: 70,
            value: 127,
        }),
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);

    assert_eq!(
        left.settings.midi.detected_controller,
        Some(ControllerProfileKind::DenonPrime2)
    );
    assert_eq!(left.settings.midi.bindings.len(), 37);
    assert_eq!(left.settings.midi.bindings[0].label, "Deck 1 Sweep FX");
    assert_eq!(
        left.settings.midi.bindings[13].label,
        "Deck 1 Performance Pad 1"
    );
    assert_eq!(left.settings.midi.bindings[36].label, "Deck 2 Pad Mode 4");
    assert_eq!(
        serde_json::to_string(&left.settings.midi).expect("serialize left midi"),
        serde_json::to_string(&right.settings.midi).expect("serialize right midi")
    );
    assert!(validate_state(&left).valid);
}

#[test]
fn integration_cue_trigger_updates_fixture_and_clip_views() {
    let state = replay_events(&[AppEvent::TriggerCue(CueId(3))]);

    assert!(matches!(
        state.clip(ClipId(201)).expect("clip exists").cue_state,
        luma_switch::core::CueVisualState::Active
    ));
    assert!(matches!(
        state
            .fixture_group(FixtureGroupId(3))
            .expect("fixture exists")
            .phase,
        luma_switch::core::FixturePhase::Active
    ));
}

#[test]
fn integration_timeline_hotspots_dispatch_show_actions() {
    let script = vec![
        AppEvent::Timeline(TimelineEvent::PointerPressed(TimelineCursor {
            beat: BeatTime::from_beats(8),
            track: Some(TrackId(1)),
            zone: TimelineZone::Track,
            target: Some(TimelineHit::ClipCueHotspot(ClipId(102), CueId(1))),
            x_px: 188,
            y_px: 92,
        })),
        AppEvent::Timeline(TimelineEvent::PointerPressed(TimelineCursor {
            beat: BeatTime::from_beats(8),
            track: Some(TrackId(1)),
            zone: TimelineZone::Track,
            target: Some(TimelineHit::ClipFxHotspot(ClipId(102), FxId(1))),
            x_px: 248,
            y_px: 92,
        })),
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);

    assert_eq!(left.timeline.selection, SelectionState::Clip(ClipId(102)));
    assert_eq!(left.fx_system.selected, Some(FxId(1)));
    assert!(matches!(
        left.cue(CueId(1)).expect("cue exists").phase,
        luma_switch::core::CuePhase::Triggered | luma_switch::core::CuePhase::Active
    ));
    assert_eq!(
        serde_json::to_string(&left.cue_system).expect("serialize left cue system"),
        serde_json::to_string(&right.cue_system).expect("serialize right cue system")
    );
}

#[test]
fn integration_inline_parameter_drag_replays_deterministically() {
    let script = vec![
        AppEvent::Timeline(TimelineEvent::PointerPressed(TimelineCursor {
            beat: BeatTime::from_beats(8),
            track: Some(TrackId(1)),
            zone: TimelineZone::Track,
            target: Some(TimelineHit::ClipParamHandle(
                ClipId(102),
                luma_switch::core::ClipInlineParameterKind::Intensity,
            )),
            x_px: 284,
            y_px: 58,
        })),
        AppEvent::Timeline(TimelineEvent::PointerMoved(TimelineCursor {
            beat: BeatTime::from_beats(8),
            track: Some(TrackId(1)),
            zone: TimelineZone::Track,
            target: Some(TimelineHit::ClipParamHandle(
                ClipId(102),
                luma_switch::core::ClipInlineParameterKind::Intensity,
            )),
            x_px: 284,
            y_px: 100,
        })),
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);

    assert!(
        left.clip(ClipId(102))
            .expect("clip exists")
            .params
            .intensity
            .permille()
            < 300
    );
    assert_eq!(
        serde_json::to_string(&left.timeline).expect("serialize left timeline"),
        serde_json::to_string(&right.timeline).expect("serialize right timeline")
    );
    assert_eq!(
        serde_json::to_string(&left.fx_system).expect("serialize left fx system"),
        serde_json::to_string(&right.fx_system).expect("serialize right fx system")
    );
}

#[test]
fn simulation_chase_step_progress_is_deterministic() {
    let script = vec![
        AppEvent::ToggleChase(ChaseId(2)),
        AppEvent::ReverseChase(ChaseId(2)),
        AppEvent::Tick,
        AppEvent::Tick,
        AppEvent::Tick,
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);

    assert_eq!(
        left.chase(ChaseId(2)).expect("left chase").current_step,
        right.chase(ChaseId(2)).expect("right chase").current_step
    );
    assert_eq!(
        serde_json::to_string(&left.chase_system).expect("serialize left chase system"),
        serde_json::to_string(&right.chase_system).expect("serialize right chase system")
    );
}

#[test]
fn integration_fx_depth_event_is_clamped_and_replayed() {
    let state = replay_events(&[
        AppEvent::SelectFx(FxId(1)),
        AppEvent::SetFxDepth(FxId(1), 1400),
        AppEvent::Tick,
    ]);

    assert_eq!(
        state.fx_layer(FxId(1)).expect("fx exists").depth_permille,
        1000
    );
    assert!(validate_state(&state).valid);
}

#[test]
fn integration_fx_waveform_and_fixture_preview_replay_is_deterministic() {
    let script = vec![
        AppEvent::SelectFx(FxId(1)),
        AppEvent::SetFxRate(FxId(1), 1340),
        AppEvent::SetFxSpread(FxId(1), 910),
        AppEvent::SetFxPhaseOffset(FxId(1), 620),
        AppEvent::SetFxWaveform(FxId(1), FxWaveform::Saw),
        AppEvent::TriggerCue(CueId(1)),
        AppEvent::Tick,
        AppEvent::Tick,
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);

    assert!(validate_state(&left).valid);
    assert_eq!(
        serde_json::to_string(&left.fx_system).expect("serialize left fx system"),
        serde_json::to_string(&right.fx_system).expect("serialize right fx system")
    );
    assert_eq!(
        serde_json::to_string(&left.fixture_system).expect("serialize left fixture system"),
        serde_json::to_string(&right.fixture_system).expect("serialize right fixture system")
    );
}

#[test]
fn scenario_fixture_selection_stays_valid_after_show_updates() {
    let script = vec![
        AppEvent::SelectFixtureGroup(FixtureGroupId(2)),
        AppEvent::TriggerCue(CueId(2)),
        AppEvent::ToggleFx(FxId(2)),
        AppEvent::Tick,
        AppEvent::Tick,
    ];

    let state = replay_events(&script);
    let report = validate_state(&state);

    assert!(report.valid);
    assert_eq!(state.fixture_system.selected, Some(FixtureGroupId(2)));
}

#[test]
fn replay_produces_identical_show_state_snapshot() {
    let script = vec![
        AppEvent::TriggerCue(CueId(2)),
        AppEvent::ToggleChase(ChaseId(1)),
        AppEvent::SetFxDepth(FxId(2), 735),
        AppEvent::SelectFixtureGroup(FixtureGroupId(1)),
        AppEvent::Tick,
        AppEvent::Tick,
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);

    assert_eq!(
        serde_json::to_string(&left.cue_system).expect("serialize left cue system"),
        serde_json::to_string(&right.cue_system).expect("serialize right cue system")
    );
    assert_eq!(
        serde_json::to_string(&left.chase_system).expect("serialize left chase system"),
        serde_json::to_string(&right.chase_system).expect("serialize right chase system")
    );
    assert_eq!(
        serde_json::to_string(&left.fx_system).expect("serialize left fx system"),
        serde_json::to_string(&right.fx_system).expect("serialize right fx system")
    );
    assert_eq!(
        serde_json::to_string(&left.fixture_system).expect("serialize left fixture system"),
        serde_json::to_string(&right.fixture_system).expect("serialize right fixture system")
    );
}

#[test]
fn clip_editor_overlay_and_replay_are_deterministic() {
    let script = vec![
        AppEvent::OpenClipEditor(ClipId(102)),
        AppEvent::SetClipEditorIntensity(930),
        AppEvent::SetClipEditorSpeed(1080),
        AppEvent::SetClipEditorFxDepth(760),
        AppEvent::SetClipEditorCue(Some(CueId(2))),
        AppEvent::SetClipEditorChase(Some(ChaseId(2))),
        AppEvent::Tick,
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);

    assert_eq!(
        serde_json::to_string(&left.clip_editor).expect("serialize left clip editor"),
        serde_json::to_string(&right.clip_editor).expect("serialize right clip editor")
    );
    assert_eq!(
        serde_json::to_string(&left.timeline).expect("serialize left timeline"),
        serde_json::to_string(&right.timeline).expect("serialize right timeline")
    );
    assert_eq!(
        serde_json::to_string(&left.chase_system).expect("serialize left chase system"),
        serde_json::to_string(&right.chase_system).expect("serialize right chase system")
    );
}

#[test]
fn cue_and_chase_authoring_replay_is_deterministic() {
    let script = vec![
        AppEvent::SelectCue(CueId(2)),
        AppEvent::SetSelectedCueName("Build Cue".to_owned()),
        AppEvent::SetSelectedCueFadeDuration(BeatTime::from_fraction(3, 4)),
        AppEvent::CreateChase,
        AppEvent::SetSelectedChaseName("Build Chase".to_owned()),
        AppEvent::AddSelectedChaseStep,
        AppEvent::SelectChaseStep(Some(1)),
        AppEvent::SetSelectedChaseStepCue(Some(CueId(2))),
        AppEvent::SetSelectedChaseStepDuration(BeatTime::from_beats(1)),
        AppEvent::MoveSelectedChaseStepLeft,
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);

    assert_eq!(left.selected_cue().expect("selected cue").name, "Build Cue");
    assert_eq!(
        left.selected_chase().expect("selected chase").name,
        "Build Chase"
    );
    assert_eq!(
        serde_json::to_string(&left.cue_system).expect("serialize left cue system"),
        serde_json::to_string(&right.cue_system).expect("serialize right cue system")
    );
    assert_eq!(
        serde_json::to_string(&left.chase_system).expect("serialize left chase system"),
        serde_json::to_string(&right.chase_system).expect("serialize right chase system")
    );
}

#[test]
fn integration_undo_redo_replays_deterministically() {
    let script = vec![
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
        AppEvent::Timeline(TimelineEvent::PointerReleased(TimelineCursor {
            beat: BeatTime::from_fraction(37, 4),
            track: Some(TrackId(1)),
            zone: TimelineZone::Track,
            target: Some(TimelineHit::ClipBody(ClipId(102))),
            x_px: 222,
            y_px: 82,
        })),
        AppEvent::Undo,
        AppEvent::Redo,
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);

    assert_eq!(
        left.clip(ClipId(102)).expect("left clip exists").start,
        BeatTime::from_fraction(37, 4)
    );
    assert_eq!(left.history.undo_stack.len(), 1);
    assert!(left.history.redo_stack.is_empty());
    assert_eq!(
        serde_json::to_string(&left).expect("serialize left state"),
        serde_json::to_string(&right).expect("serialize right state")
    );
}

#[test]
fn integration_duplicate_selected_clips_replays_deterministically() {
    let script = vec![AppEvent::DuplicateSelectedClips];

    let left = replay_events(&script);
    let right = replay_events(&script);

    let duplicate = left.timeline.tracks[0]
        .clips
        .iter()
        .find(|clip| clip.title == "Drop Sweep Copy")
        .expect("duplicated clip");

    assert_eq!(duplicate.start, BeatTime::from_beats(16));
    assert_eq!(left.timeline.selected_clips, vec![duplicate.id]);
    assert_eq!(
        serde_json::to_string(&left.timeline).expect("serialize left timeline"),
        serde_json::to_string(&right.timeline).expect("serialize right timeline")
    );
}

#[test]
fn integration_split_selected_clips_replays_deterministically() {
    let script = vec![
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
        AppEvent::SplitSelectedClipsAtPlayhead,
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);

    let right_segment = left.timeline.tracks[0]
        .clips
        .iter()
        .find(|clip| clip.title == "Drop Sweep B")
        .expect("split clip");

    assert_eq!(
        left.clip(ClipId(102)).expect("left segment").duration,
        BeatTime::from_beats(4)
    );
    assert_eq!(right_segment.start, BeatTime::from_beats(12));
    assert_eq!(
        serde_json::to_string(&left.timeline).expect("serialize left timeline"),
        serde_json::to_string(&right.timeline).expect("serialize right timeline")
    );
}

#[test]
fn integration_delete_selected_clips_replays_deterministically() {
    let script = vec![AppEvent::DeleteSelectedClips];

    let left = replay_events(&script);
    let right = replay_events(&script);

    assert!(left.clip(ClipId(102)).is_none());
    assert_eq!(left.cue(CueId(1)).expect("cue exists").linked_clip, None);
    assert_eq!(left.fx_layer(FxId(1)).expect("fx exists").linked_clip, None);
    assert_eq!(
        serde_json::to_string(&left).expect("serialize left state"),
        serde_json::to_string(&right).expect("serialize right state")
    );
}

#[test]
fn integration_clipboard_paste_replays_deterministically() {
    let script = vec![
        AppEvent::CopySelectedClips,
        AppEvent::Timeline(TimelineEvent::PointerPressed(TimelineCursor {
            beat: BeatTime::from_beats(20),
            track: None,
            zone: TimelineZone::Header,
            target: Some(TimelineHit::Playhead),
            x_px: 420,
            y_px: 12,
        })),
        AppEvent::Timeline(TimelineEvent::PointerReleased(TimelineCursor {
            beat: BeatTime::from_beats(20),
            track: None,
            zone: TimelineZone::Header,
            target: Some(TimelineHit::Playhead),
            x_px: 420,
            y_px: 12,
        })),
        AppEvent::PasteClipboardAtPlayhead,
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);
    let pasted_id = *left
        .timeline
        .selected_clips
        .first()
        .expect("pasted clip selected");
    let pasted = left.clip(pasted_id).expect("pasted clip exists");

    assert_eq!(pasted.start, BeatTime::from_beats(20));
    assert_eq!(
        serde_json::to_string(&left.timeline).expect("serialize left timeline"),
        serde_json::to_string(&right.timeline).expect("serialize right timeline")
    );
}

#[test]
fn integration_context_menu_nudge_replays_deterministically() {
    let script = vec![
        AppEvent::Timeline(TimelineEvent::SecondaryPressed(TimelineCursor {
            beat: BeatTime::from_beats(8),
            track: Some(TrackId(1)),
            zone: TimelineZone::Track,
            target: Some(TimelineHit::ClipBody(ClipId(102))),
            x_px: 188,
            y_px: 92,
        })),
        AppEvent::ApplyContextMenuAction(luma_switch::core::ContextMenuAction::NudgeRight),
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);

    assert_eq!(
        left.clip(ClipId(102)).expect("clip exists").start,
        BeatTime::from_fraction(33, 4)
    );
    assert_eq!(
        serde_json::to_string(&left.timeline).expect("serialize left timeline"),
        serde_json::to_string(&right.timeline).expect("serialize right timeline")
    );
}

#[test]
fn integration_project_export_import_roundtrip_is_deterministic() {
    let state = replay_events(&[
        AppEvent::CopySelectedClips,
        AppEvent::PasteClipboardAtPlayhead,
        AppEvent::OpenClipEditor(ClipId(102)),
        AppEvent::SetClipEditorAutomationTarget(luma_switch::core::AutomationTarget::Intensity),
    ]);
    let json = export_project_json(&state);
    let restored = import_project_json(&json).expect("import project");

    assert_eq!(
        serde_json::to_string(&state.timeline.tracks).expect("serialize left tracks"),
        serde_json::to_string(&restored.timeline.tracks).expect("serialize restored tracks")
    );
    assert_eq!(state.timeline.selection, restored.timeline.selection);
    assert_eq!(
        state.timeline.selected_clips,
        restored.timeline.selected_clips
    );
    assert_eq!(state.clip_editor, restored.clip_editor);
}

#[test]
fn integration_venture_save_load_roundtrip_is_deterministic() {
    let directory = temp_venture_dir("integration");
    let state = replay_events(&[
        AppEvent::DuplicateSelectedClips,
        AppEvent::SetMasterSpeed(740),
        AppEvent::TriggerCue(CueId(2)),
    ]);
    let saved = save_venture(&state, &directory, None, "Festival Rig").expect("save venture");
    let listed = list_ventures(&directory).expect("list ventures");
    let (loaded, descriptor) = load_venture(&directory, &saved.id).expect("load venture");

    assert_eq!(listed.len(), 1);
    assert_eq!(descriptor.name, "Festival Rig");
    assert_eq!(
        serde_json::to_string(&state.timeline.tracks).expect("serialize source tracks"),
        serde_json::to_string(&loaded.timeline.tracks).expect("serialize loaded tracks")
    );
    assert_eq!(state.master.speed, loaded.master.speed);

    let _ = fs::remove_dir_all(directory);
}

#[test]
fn integration_autosave_recovery_restore_is_deterministic() {
    let directory = temp_venture_dir("autosave-restore");
    let mut state = StudioState::default();
    state.venture.directory = directory.to_string_lossy().into_owned();

    dispatch(
        &mut state,
        AppEvent::SetVentureDraftName("Autosave Venture".to_owned()),
    );
    dispatch(&mut state, AppEvent::SaveCurrentVenture);
    dispatch(&mut state, AppEvent::SetMasterIntensity(930));

    let recovery_id = state
        .venture
        .selected_recovery
        .clone()
        .expect("selected recovery after autosave");
    let registry = load_recovery_registry(&directory).expect("load recovery registry");
    let intensity_after_edit = state.master.intensity;

    assert_eq!(registry.slots.len(), 1);
    assert!(state.venture.dirty);

    dispatch(&mut state, AppEvent::RestoreSelectedRecoverySlot);

    assert_eq!(state.master.intensity, intensity_after_edit);
    assert_eq!(
        state.venture.selected_recovery.as_deref(),
        Some(recovery_id.as_str())
    );
    assert!(!state.venture.dirty);

    let _ = fs::remove_dir_all(directory);
}

#[test]
fn integration_fixture_profile_import_and_patch_replay_is_deterministic() {
    let profile = import_ofl_fixture(
        r#"{
          "$schema":"https://raw.githubusercontent.com/OpenLightingProject/open-fixture-library/master/schemas/fixture.json",
          "name":"Replay Spot",
          "categories":["Moving Head"],
          "meta":{"authors":["Tester"],"createDate":"2024-01-01","lastModifyDate":"2024-01-02"},
          "availableChannels":{
            "Dimmer":{"capability":{"type":"Intensity"}},
            "Pan":{"capability":{"type":"Pan"}},
            "Tilt":{"capability":{"type":"Tilt"}}
          },
          "modes":[{"name":"3ch","channels":["Dimmer","Pan","Tilt"]}]
        }"#,
        Some("demo"),
        Some("replay-spot"),
    )
    .expect("fixture profile");
    let script = vec![
        AppEvent::ApplyImportedFixtureProfile(profile),
        AppEvent::CreateFixturePatch,
        AppEvent::SetSelectedFixturePatchAddress(37),
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);

    assert_eq!(left.fixture_system.library.profiles.len(), 1);
    assert_eq!(left.fixture_system.library.patches.len(), 1);
    assert_eq!(left.fixture_system.library.patches[0].address, 37);
    assert_eq!(
        serde_json::to_string(&left.fixture_system.library)
            .expect("serialize left fixture library"),
        serde_json::to_string(&right.fixture_system.library)
            .expect("serialize right fixture library")
    );
}

#[test]
fn integration_fixture_patch_stage_summaries_are_deterministic() {
    let profile = import_ofl_fixture(
        r#"{
          "$schema":"https://raw.githubusercontent.com/OpenLightingProject/open-fixture-library/master/schemas/fixture.json",
          "name":"Patch Stage Wash",
          "categories":["Wash"],
          "meta":{"authors":["Tester"],"createDate":"2024-01-01","lastModifyDate":"2024-01-02"},
          "availableChannels":{
            "Dimmer":{"capability":{"type":"Intensity"}},
            "Red":{"capability":{"type":"ColorIntensity","color":"Red"}},
            "Green":{"capability":{"type":"ColorIntensity","color":"Green"}},
            "Blue":{"capability":{"type":"ColorIntensity","color":"Blue"}}
          },
          "modes":[{"name":"4ch","channels":["Dimmer","Red","Green","Blue"]}]
        }"#,
        Some("demo"),
        Some("patch-stage-wash"),
    )
    .expect("fixture profile");
    let script = vec![
        AppEvent::ApplyImportedFixtureProfile(profile),
        AppEvent::SelectFixtureGroup(FixtureGroupId(2)),
        AppEvent::CreateFixturePatch,
        AppEvent::CreateFixturePatch,
        AppEvent::SetSelectedFixturePatchAddress(4),
    ];

    let left = replay_events(&script);
    let right = replay_events(&script);
    let left_group = left.fixture_group_patch_summary(FixtureGroupId(2));
    let right_group = right.fixture_group_patch_summary(FixtureGroupId(2));
    let left_universe = left
        .fixture_universe_summaries()
        .into_iter()
        .find(|summary| summary.universe == 1)
        .expect("universe 1 summary");
    let right_universe = right
        .fixture_universe_summaries()
        .into_iter()
        .find(|summary| summary.universe == 1)
        .expect("universe 1 summary");

    assert_eq!(left_group.patch_count, 2);
    assert_eq!(left_group.occupied_channels, 7);
    assert_eq!(left_group.conflicting_patch_ids, vec![1, 2]);
    assert_eq!(left_universe.footprint_channels, 8);
    assert_eq!(left_universe.occupied_channels, 7);
    assert_eq!(
        serde_json::to_string(&left_group).expect("serialize left group summary"),
        serde_json::to_string(&right_group).expect("serialize right group summary")
    );
    assert_eq!(
        serde_json::to_string(&left_universe).expect("serialize left universe summary"),
        serde_json::to_string(&right_universe).expect("serialize right universe summary")
    );
}

#[test]
fn machine_readable_spec_roundtrip() {
    let json = foundation_spec_json();
    let parsed: luma_switch::core::MachineReadableSection =
        serde_json::from_str(&json).expect("json roundtrip");

    assert_eq!(parsed.modules.len(), 19);
    assert!(
        parsed
            .modules
            .iter()
            .any(|module| module.name == "SettingsHardwareSystem")
    );
    assert!(
        parsed
            .modules
            .iter()
            .any(|module| module.name == "EngineLinkSystem")
    );
    assert!(
        parsed
            .modules
            .iter()
            .any(|module| module.name == "OutputRuntimeSystem")
    );
}

fn test_output_profile() -> FixtureProfile {
    FixtureProfile {
        id: "integration-output".to_owned(),
        manufacturer: "Integration".to_owned(),
        model: "RGB".to_owned(),
        short_name: "RGB".to_owned(),
        categories: vec!["Color Changer".to_owned()],
        physical: Some(FixturePhysical {
            dimensions_mm: None,
            weight_grams: None,
            power_watts: None,
            dmx_connector: None,
        }),
        channels: vec![
            FixtureChannel {
                name: "Dimmer".to_owned(),
                group: "Intensity".to_owned(),
                byte: 0,
                default_value: 0,
                highlight_value: 255,
                capabilities: Vec::new(),
            },
            FixtureChannel {
                name: "Red".to_owned(),
                group: "Color".to_owned(),
                byte: 0,
                default_value: 0,
                highlight_value: 255,
                capabilities: Vec::new(),
            },
            FixtureChannel {
                name: "Green".to_owned(),
                group: "Color".to_owned(),
                byte: 0,
                default_value: 0,
                highlight_value: 255,
                capabilities: Vec::new(),
            },
            FixtureChannel {
                name: "Blue".to_owned(),
                group: "Color".to_owned(),
                byte: 0,
                default_value: 0,
                highlight_value: 255,
                capabilities: Vec::new(),
            },
        ],
        modes: vec![FixtureMode {
            name: "8bit".to_owned(),
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
    }
}

fn temp_venture_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "luma-switch-integration-venture-{}-{}",
        label,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("unix time")
            .as_nanos()
    ))
}
