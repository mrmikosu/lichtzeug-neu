use crate::core::hardware::midi_port_id;
use crate::core::state::{
    ChasePhase, CuePhase, DmxBackendKind, EnginePhase, FixtureChannel, FixtureGroup, MidiAction,
    MidiBindingMessage, MidiMessageKind, MidiPortDirection, OutputDispatchReport, StudioState,
};
use crate::core::time::RgbaColor;
use midir::MidiOutput;
use serde::{Deserialize, Serialize};
use serialport::{DataBits, FlowControl, Parity, StopBits};
use std::collections::BTreeMap;
use std::io::Write;
use std::net::UdpSocket;
use std::thread;
use std::time::Duration;

const DMX_SLOT_COUNT: usize = 512;
const SACN_SOURCE_NAME: &str = "Luma Switch Studio";
const SACN_CID: [u8; 16] = [
    0x4c, 0x75, 0x6d, 0x61, 0x53, 0x77, 0x69, 0x74, 0x63, 0x68, 0x53, 0x74, 0x75, 0x64, 0x69, 0x6f,
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeOutputSnapshot {
    pub sequence: u64,
    pub dmx_backend: DmxBackendKind,
    pub dmx_frames: Vec<DmxUniverseFrame>,
    pub enttec_port_name: Option<String>,
    pub enttec_break_us: u16,
    pub enttec_mark_after_break_us: u16,
    pub artnet_target: String,
    pub artnet_base_universe: u16,
    pub sacn_target: String,
    pub sacn_base_universe: u16,
    pub midi_output_id: Option<String>,
    pub midi_feedback_packets: Vec<MidiFeedbackPacket>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DmxUniverseFrame {
    pub universe: u16,
    pub slots: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiFeedbackPacket {
    pub message: MidiBindingMessage,
    pub value: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputDispatchFailure {
    pub sequence: u64,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputMonitorSnapshot {
    pub backend: DmxBackendKind,
    pub blackout_applied: bool,
    pub universe_monitors: Vec<OutputUniverseMonitor>,
    pub midi_feedback_monitors: Vec<MidiFeedbackMonitor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputUniverseMonitor {
    pub internal_universe: u16,
    pub routed_universe: Option<u16>,
    pub destination: String,
    pub patch_count: usize,
    pub enabled_patch_count: usize,
    pub occupied_channels: u16,
    pub active_slots: u16,
    pub peak_value: u8,
    pub segment_levels: Vec<u16>,
    pub patch_labels: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiFeedbackMonitor {
    pub binding_id: u32,
    pub label: String,
    pub message: String,
    pub value: u16,
    pub active: bool,
}

pub fn build_runtime_output_snapshot(state: &StudioState) -> Option<RuntimeOutputSnapshot> {
    if !state.should_dispatch_runtime_outputs() || !output_dispatch_due(state) {
        return None;
    }

    let dmx_frames = if state.settings.dmx.output_enabled
        && !matches!(state.settings.dmx.backend, DmxBackendKind::Disabled)
    {
        build_dmx_universe_frames(state)
    } else {
        Vec::new()
    };

    let midi_feedback_packets = if state.settings.midi.feedback_enabled {
        build_midi_feedback_packets(state)
    } else {
        Vec::new()
    };

    if dmx_frames.is_empty() && midi_feedback_packets.is_empty() {
        return None;
    }

    Some(RuntimeOutputSnapshot {
        sequence: state.output.sequence.saturating_add(1),
        dmx_backend: state.settings.dmx.backend,
        dmx_frames,
        enttec_port_name: state
            .selected_dmx_interface()
            .map(|interface| interface.port_name.clone()),
        enttec_break_us: state.settings.dmx.enttec_break_us,
        enttec_mark_after_break_us: state.settings.dmx.enttec_mark_after_break_us,
        artnet_target: state.settings.dmx.artnet_target.clone(),
        artnet_base_universe: state.settings.dmx.artnet_universe,
        sacn_target: state.settings.dmx.sacn_target.clone(),
        sacn_base_universe: state.settings.dmx.sacn_universe,
        midi_output_id: state.selected_midi_output().map(|port| port.id.clone()),
        midi_feedback_packets,
    })
}

pub fn deliver_runtime_outputs(
    snapshot: RuntimeOutputSnapshot,
) -> Result<OutputDispatchReport, OutputDispatchFailure> {
    let dmx_frame_count = snapshot.dmx_frames.len() as u16;
    let midi_message_count = snapshot.midi_feedback_packets.len() as u16;

    match snapshot.dmx_backend {
        DmxBackendKind::Disabled => {}
        DmxBackendKind::EnttecOpenDmx => deliver_enttec_open_dmx(&snapshot)?,
        DmxBackendKind::ArtNet => deliver_artnet(&snapshot)?,
        DmxBackendKind::Sacn => deliver_sacn(&snapshot)?,
    }

    if !snapshot.midi_feedback_packets.is_empty() {
        deliver_midi_feedback(&snapshot)?;
    }

    Ok(OutputDispatchReport {
        sequence: snapshot.sequence,
        dmx_backend: snapshot.dmx_backend,
        dmx_frame_count,
        midi_message_count,
        summary: dispatch_summary(&snapshot),
    })
}

pub fn build_output_monitor_snapshot(state: &StudioState) -> OutputMonitorSnapshot {
    let dmx_frames = build_dmx_universe_frames(state);
    let summaries = state.fixture_universe_summaries();
    let blackout_applied =
        state.settings.dmx.blackout_on_stop && state.engine.phase != EnginePhase::Running;

    let mut frame_map = BTreeMap::<u16, DmxUniverseFrame>::new();
    for frame in dmx_frames {
        frame_map.insert(frame.universe, frame);
    }

    let mut universe_ids = summaries
        .iter()
        .map(|summary| summary.universe)
        .collect::<Vec<_>>();
    universe_ids.extend(frame_map.keys().copied());
    universe_ids.sort_unstable();
    universe_ids.dedup();

    let universe_monitors = universe_ids
        .into_iter()
        .map(|universe| {
            let summary = summaries
                .iter()
                .find(|summary| summary.universe == universe)
                .cloned();
            let frame = frame_map.get(&universe);
            let patches = state
                .fixture_system
                .library
                .patches
                .iter()
                .filter(|patch| patch.universe == universe)
                .collect::<Vec<_>>();
            let patch_labels = patches
                .iter()
                .take(4)
                .map(|patch| patch.name.clone())
                .collect::<Vec<_>>();
            let slots = frame.map(|frame| frame.slots.as_slice()).unwrap_or(&[]);
            let (active_slots, peak_value, segment_levels) = slot_monitor_statistics(slots);

            OutputUniverseMonitor {
                internal_universe: universe,
                routed_universe: routed_wire_universe(state, universe),
                destination: output_destination_label(state, universe),
                patch_count: summary
                    .as_ref()
                    .map(|summary| summary.patch_count)
                    .unwrap_or(0),
                enabled_patch_count: summary
                    .as_ref()
                    .map(|summary| summary.enabled_patch_count)
                    .unwrap_or(0),
                occupied_channels: summary
                    .as_ref()
                    .map(|summary| summary.occupied_channels)
                    .unwrap_or(0),
                active_slots,
                peak_value,
                segment_levels,
                patch_labels,
            }
        })
        .collect::<Vec<_>>();

    OutputMonitorSnapshot {
        backend: state.settings.dmx.backend,
        blackout_applied,
        universe_monitors,
        midi_feedback_monitors: build_midi_feedback_monitors(state),
    }
}

fn output_dispatch_due(state: &StudioState) -> bool {
    let refresh_hz = state.settings.dmx.refresh_rate_hz.max(1) as u64;
    let interval_ns = 1_000_000_000u64 / refresh_hz;
    let next_due_ns = state.output.sequence.saturating_mul(interval_ns.max(1));
    state.engine.clock.monotonic_ns >= next_due_ns
}

fn slot_monitor_statistics(slots: &[u8]) -> (u16, u8, Vec<u16>) {
    if slots.is_empty() {
        return (0, 0, vec![0; 16]);
    }

    let active_slots = slots.iter().filter(|value| **value > 0).count() as u16;
    let peak_value = slots.iter().copied().max().unwrap_or(0);
    let segment_size = (DMX_SLOT_COUNT / 16).max(1);
    let mut segment_levels = Vec::with_capacity(16);

    for segment in 0..16 {
        let start = segment * segment_size;
        let end = ((segment + 1) * segment_size).min(slots.len());
        let peak = if start < end {
            slots[start..end].iter().copied().max().unwrap_or(0)
        } else {
            0
        };
        segment_levels.push(((peak as u32 * 1000) / 255) as u16);
    }

    (active_slots, peak_value, segment_levels)
}

fn routed_wire_universe(state: &StudioState, internal_universe: u16) -> Option<u16> {
    match state.settings.dmx.backend {
        DmxBackendKind::Disabled => None,
        DmxBackendKind::EnttecOpenDmx => (internal_universe == 1).then_some(1),
        DmxBackendKind::ArtNet => Some(artnet_wire_universe(
            state.settings.dmx.artnet_universe,
            internal_universe,
        )),
        DmxBackendKind::Sacn => Some(
            state
                .settings
                .dmx
                .sacn_universe
                .saturating_add(internal_universe.saturating_sub(1)),
        ),
    }
}

fn output_destination_label(state: &StudioState, internal_universe: u16) -> String {
    match state.settings.dmx.backend {
        DmxBackendKind::Disabled => "Preview only (DMX disabled)".to_owned(),
        DmxBackendKind::EnttecOpenDmx => {
            if internal_universe == 1 {
                format!(
                    "ENTTEC Open DMX @ {}",
                    state
                        .selected_dmx_interface()
                        .map(|interface| interface.name.clone())
                        .unwrap_or_else(|| "No interface".to_owned())
                )
            } else {
                "Not routable on single ENTTEC interface".to_owned()
            }
        }
        DmxBackendKind::ArtNet => format!(
            "Art-Net U{} @ {}",
            artnet_wire_universe(state.settings.dmx.artnet_universe, internal_universe),
            state.settings.dmx.artnet_target
        ),
        DmxBackendKind::Sacn => format!(
            "sACN U{} @ {}",
            state
                .settings
                .dmx
                .sacn_universe
                .saturating_add(internal_universe.saturating_sub(1)),
            state.settings.dmx.sacn_target
        ),
    }
}

fn build_dmx_universe_frames(state: &StudioState) -> Vec<DmxUniverseFrame> {
    let blackout =
        state.settings.dmx.blackout_on_stop && state.engine.phase != EnginePhase::Running;
    let mut patches = state
        .fixture_system
        .library
        .patches
        .iter()
        .filter(|patch| patch.enabled)
        .collect::<Vec<_>>();
    patches.sort_by_key(|patch| (patch.universe, patch.address, patch.id));

    let mut universes = BTreeMap::<u16, Vec<u8>>::new();
    for patch in patches {
        let Some(profile) = state.fixture_profile(&patch.profile_id) else {
            continue;
        };
        let Some(mode) = profile
            .modes
            .iter()
            .find(|mode| mode.name == patch.mode_name)
        else {
            continue;
        };
        let frame = universes
            .entry(patch.universe)
            .or_insert_with(|| vec![0; DMX_SLOT_COUNT]);
        if blackout {
            continue;
        }

        let group = patch
            .group_id
            .and_then(|group_id| state.fixture_group(group_id));
        let group_output = group.map(|group| group.output_level).unwrap_or(0);
        let centroid = fixture_group_centroid(group);

        for (channel_index, channel_name) in mode.channels.iter().enumerate() {
            let Some(slot_index) = patch_slot_index(patch.address, channel_index) else {
                continue;
            };
            let Some(channel) = profile
                .channels
                .iter()
                .find(|channel| channel.name == *channel_name)
            else {
                continue;
            };
            let value = render_channel_value(state, group, group_output, centroid, channel);
            frame[slot_index] = frame[slot_index].max(value);
        }
    }

    universes
        .into_iter()
        .map(|(universe, slots)| DmxUniverseFrame { universe, slots })
        .collect()
}

fn patch_slot_index(address: u16, channel_index: usize) -> Option<usize> {
    let base = address.checked_sub(1)? as usize;
    let slot = base.saturating_add(channel_index);
    (slot < DMX_SLOT_COUNT).then_some(slot)
}

fn fixture_group_centroid(group: Option<&FixtureGroup>) -> (u16, u16) {
    let Some(group) = group else {
        return (500, 500);
    };
    if group.preview_nodes.is_empty() {
        return (500, 500);
    }

    let count = group.preview_nodes.len() as u32;
    let sum_x = group
        .preview_nodes
        .iter()
        .map(|node| node.x_permille as u32)
        .sum::<u32>();
    let sum_y = group
        .preview_nodes
        .iter()
        .map(|node| node.y_permille as u32)
        .sum::<u32>();
    ((sum_x / count) as u16, (sum_y / count) as u16)
}

fn render_channel_value(
    state: &StudioState,
    group: Option<&FixtureGroup>,
    group_output: u16,
    centroid: (u16, u16),
    channel: &FixtureChannel,
) -> u8 {
    let classification = classify_channel(channel);
    let accent = group
        .map(|group| group.accent)
        .unwrap_or(RgbaColor::rgb(255, 255, 255));
    let intensity = scale_u8(255, group_output);
    let (pan_coarse, pan_fine) = pan_tilt_bytes(centroid.0);
    let (tilt_coarse, tilt_fine) = pan_tilt_bytes(centroid.1);
    let speed = scale_range(
        state.master.speed.permille(),
        crate::core::time::SpeedRatio::MIN,
        crate::core::time::SpeedRatio::MAX,
    );
    let default = channel.default_value.min(255) as u8;
    let highlight = channel.highlight_value.min(255) as u8;

    match classification {
        ChannelClassification::Intensity => intensity,
        ChannelClassification::Red => scale_u8(accent.r, group_output),
        ChannelClassification::Green => scale_u8(accent.g, group_output),
        ChannelClassification::Blue => scale_u8(accent.b, group_output),
        ChannelClassification::White => intensity,
        ChannelClassification::Amber => scale_u8(191, group_output),
        ChannelClassification::Uv => scale_u8(128, group_output),
        ChannelClassification::PanCoarse => pan_coarse,
        ChannelClassification::PanFine => pan_fine,
        ChannelClassification::TiltCoarse => tilt_coarse,
        ChannelClassification::TiltFine => tilt_fine,
        ChannelClassification::Speed => speed,
        ChannelClassification::Shutter => {
            if group_output > 0 {
                highlight.max(default)
            } else {
                0
            }
        }
        ChannelClassification::Generic => default,
    }
}

fn scale_u8(component: u8, permille: u16) -> u8 {
    ((component as u32 * permille.min(1000) as u32) / 1000) as u8
}

fn scale_range(value: u16, min: u16, max: u16) -> u8 {
    if max <= min {
        return 0;
    }
    let normalized = value.clamp(min, max).saturating_sub(min) as u32;
    let range = (max - min) as u32;
    ((normalized * 255) / range) as u8
}

fn pan_tilt_bytes(permille: u16) -> (u8, u8) {
    let full = ((permille.min(1000) as u32) * 65_535 / 1000) as u16;
    ((full >> 8) as u8, (full & 0xff) as u8)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChannelClassification {
    Intensity,
    Red,
    Green,
    Blue,
    White,
    Amber,
    Uv,
    PanCoarse,
    PanFine,
    TiltCoarse,
    TiltFine,
    Speed,
    Shutter,
    Generic,
}

fn classify_channel(channel: &FixtureChannel) -> ChannelClassification {
    let label = format!("{} {}", channel.group, channel.name).to_ascii_lowercase();

    if label.contains("dimmer") || label.contains("intensity") {
        return ChannelClassification::Intensity;
    }
    if label.contains("red") {
        return ChannelClassification::Red;
    }
    if label.contains("green") {
        return ChannelClassification::Green;
    }
    if label.contains("blue") {
        return ChannelClassification::Blue;
    }
    if label.contains("white") {
        return ChannelClassification::White;
    }
    if label.contains("amber") {
        return ChannelClassification::Amber;
    }
    if label.contains("uv") || label.contains("ultra violet") {
        return ChannelClassification::Uv;
    }
    if label.contains("pan") && (channel.byte > 0 || label.contains("fine")) {
        return ChannelClassification::PanFine;
    }
    if label.contains("pan") {
        return ChannelClassification::PanCoarse;
    }
    if label.contains("tilt") && (channel.byte > 0 || label.contains("fine")) {
        return ChannelClassification::TiltFine;
    }
    if label.contains("tilt") {
        return ChannelClassification::TiltCoarse;
    }
    if label.contains("speed") || label.contains("rate") {
        return ChannelClassification::Speed;
    }
    if label.contains("shutter") || label.contains("strobe") {
        return ChannelClassification::Shutter;
    }

    ChannelClassification::Generic
}

fn build_midi_feedback_packets(state: &StudioState) -> Vec<MidiFeedbackPacket> {
    if state.selected_midi_output().is_none() {
        return Vec::new();
    }

    let mut bindings = state
        .settings
        .midi
        .bindings
        .iter()
        .filter_map(|binding| {
            let message = binding.message.clone()?;
            Some((binding.id, binding.action, message))
        })
        .collect::<Vec<_>>();
    bindings.sort_by_key(|(binding_id, _, _)| *binding_id);

    bindings
        .into_iter()
        .map(|(_, action, message)| MidiFeedbackPacket {
            value: midi_feedback_value(state, action),
            message,
        })
        .collect()
}

fn build_midi_feedback_monitors(state: &StudioState) -> Vec<MidiFeedbackMonitor> {
    let mut bindings = state
        .settings
        .midi
        .bindings
        .iter()
        .filter_map(|binding| {
            let message = binding.message.as_ref()?;
            Some(MidiFeedbackMonitor {
                binding_id: binding.id,
                label: binding.label.clone(),
                message: format_midi_binding_message(message),
                value: midi_feedback_value(state, binding.action),
                active: midi_feedback_value(state, binding.action) > 0,
            })
        })
        .collect::<Vec<_>>();
    bindings.sort_by_key(|binding| binding.binding_id);
    bindings
}

fn format_midi_binding_message(message: &MidiBindingMessage) -> String {
    match message.kind {
        MidiMessageKind::Note => format!("Note ch{} key{}", message.channel, message.key),
        MidiMessageKind::ControlChange => format!("CC ch{} ctrl{}", message.channel, message.key),
        MidiMessageKind::PitchBend => format!("Pitch ch{}", message.channel),
    }
}

fn midi_feedback_value(state: &StudioState, action: MidiAction) -> u16 {
    match action {
        MidiAction::TransportToggle => {
            if state.engine.phase == EnginePhase::Running {
                16_383
            } else {
                0
            }
        }
        MidiAction::MasterIntensity => scale_permille_to_14bit(state.master.intensity.permille()),
        MidiAction::MasterSpeed => {
            let value = scale_range(
                state.master.speed.permille(),
                crate::core::time::SpeedRatio::MIN,
                crate::core::time::SpeedRatio::MAX,
            );
            ((value as u32 * 16_383) / 255) as u16
        }
        MidiAction::TimelineZoom => {
            let value = scale_range(
                state.timeline.viewport.zoom.permille(),
                crate::core::time::ZoomFactor::MIN,
                crate::core::time::ZoomFactor::MAX,
            );
            ((value as u32 * 16_383) / 255) as u16
        }
        MidiAction::TriggerCueSlot(slot) => {
            let index = slot.saturating_sub(1) as usize;
            state
                .cue_system
                .cues
                .get(index)
                .map(|cue| match cue.phase {
                    CuePhase::Armed => 8_191,
                    CuePhase::Triggered | CuePhase::Fading | CuePhase::Active => 16_383,
                    CuePhase::Stored => 0,
                })
                .unwrap_or(0)
        }
        MidiAction::TriggerChaseSlot(slot) => {
            let index = slot.saturating_sub(1) as usize;
            state
                .chase_system
                .chases
                .get(index)
                .map(|chase| {
                    if matches!(
                        chase.phase,
                        ChasePhase::Playing | ChasePhase::Looping | ChasePhase::Reversing
                    ) {
                        16_383
                    } else {
                        0
                    }
                })
                .unwrap_or(0)
        }
        MidiAction::FocusFixtureGroupSlot(slot) => {
            let index = slot.saturating_sub(1) as usize;
            state
                .fixture_system
                .groups
                .get(index)
                .map(|group| {
                    if state.fixture_system.selected == Some(group.id) {
                        16_383
                    } else {
                        0
                    }
                })
                .unwrap_or(0)
        }
        MidiAction::FxDepthSlot(slot) => {
            let index = slot.saturating_sub(1) as usize;
            state
                .fx_system
                .layers
                .get(index)
                .map(|layer| scale_permille_to_14bit(layer.depth_permille))
                .unwrap_or(0)
        }
    }
}

fn scale_permille_to_14bit(permille: u16) -> u16 {
    ((permille.min(1000) as u32 * 16_383) / 1000) as u16
}

fn deliver_enttec_open_dmx(snapshot: &RuntimeOutputSnapshot) -> Result<(), OutputDispatchFailure> {
    if snapshot.dmx_frames.is_empty() {
        return Ok(());
    }
    if snapshot.dmx_frames.len() != 1 {
        return Err(output_failure(
            snapshot.sequence,
            format!(
                "ENTTEC Open DMX unterstützt genau 1 Universum, aktuell {}.",
                snapshot.dmx_frames.len()
            ),
        ));
    }

    let port_name = snapshot.enttec_port_name.as_deref().ok_or_else(|| {
        output_failure(
            snapshot.sequence,
            "ENTTEC Open DMX benötigt ein selektiertes serielles Interface.".to_owned(),
        )
    })?;

    let frame = &snapshot.dmx_frames[0];
    let mut payload = Vec::with_capacity(DMX_SLOT_COUNT + 1);
    payload.push(0);
    payload.extend_from_slice(&frame.slots);

    let mut port = serialport::new(port_name, 250_000)
        .data_bits(DataBits::Eight)
        .flow_control(FlowControl::None)
        .parity(Parity::None)
        .stop_bits(StopBits::Two)
        .timeout(Duration::from_millis(100))
        .open()
        .map_err(|error| {
            output_failure(
                snapshot.sequence,
                format!(
                    "ENTTEC-Interface {} konnte nicht geöffnet werden: {}",
                    port_name, error
                ),
            )
        })?;

    port.set_break().map_err(|error| {
        output_failure(
            snapshot.sequence,
            format!("DMX Break konnte nicht gesetzt werden: {}", error),
        )
    })?;
    thread::sleep(Duration::from_micros(snapshot.enttec_break_us as u64));
    port.clear_break().map_err(|error| {
        output_failure(
            snapshot.sequence,
            format!("DMX Break konnte nicht beendet werden: {}", error),
        )
    })?;
    thread::sleep(Duration::from_micros(
        snapshot.enttec_mark_after_break_us as u64,
    ));
    port.write_all(&payload).map_err(|error| {
        output_failure(
            snapshot.sequence,
            format!("DMX-Frame konnte nicht geschrieben werden: {}", error),
        )
    })?;
    port.flush().map_err(|error| {
        output_failure(
            snapshot.sequence,
            format!("DMX-Frame konnte nicht geflusht werden: {}", error),
        )
    })?;
    Ok(())
}

fn deliver_artnet(snapshot: &RuntimeOutputSnapshot) -> Result<(), OutputDispatchFailure> {
    if snapshot.dmx_frames.is_empty() {
        return Ok(());
    }
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|error| {
        output_failure(
            snapshot.sequence,
            format!("Art-Net Socket konnte nicht geöffnet werden: {}", error),
        )
    })?;
    socket.set_broadcast(true).map_err(|error| {
        output_failure(
            snapshot.sequence,
            format!("Art-Net Broadcast konnte nicht aktiviert werden: {}", error),
        )
    })?;

    for frame in &snapshot.dmx_frames {
        let packet = build_artnet_dmx_packet(
            artnet_wire_universe(snapshot.artnet_base_universe, frame.universe),
            snapshot.sequence as u8,
            &frame.slots,
        );
        socket
            .send_to(&packet, &snapshot.artnet_target)
            .map_err(|error| {
                output_failure(
                    snapshot.sequence,
                    format!(
                        "Art-Net Versand an {} fehlgeschlagen: {}",
                        snapshot.artnet_target, error
                    ),
                )
            })?;
    }
    Ok(())
}

fn artnet_wire_universe(base_universe: u16, frame_universe: u16) -> u16 {
    base_universe
        .saturating_sub(1)
        .saturating_add(frame_universe.saturating_sub(1))
}

fn deliver_sacn(snapshot: &RuntimeOutputSnapshot) -> Result<(), OutputDispatchFailure> {
    if snapshot.dmx_frames.is_empty() {
        return Ok(());
    }
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|error| {
        output_failure(
            snapshot.sequence,
            format!("sACN Socket konnte nicht geöffnet werden: {}", error),
        )
    })?;

    for frame in &snapshot.dmx_frames {
        let packet = build_sacn_data_packet(
            snapshot
                .sacn_base_universe
                .saturating_add(frame.universe.saturating_sub(1)),
            snapshot.sequence as u8,
            &frame.slots,
        );
        socket
            .send_to(&packet, &snapshot.sacn_target)
            .map_err(|error| {
                output_failure(
                    snapshot.sequence,
                    format!(
                        "sACN Versand an {} fehlgeschlagen: {}",
                        snapshot.sacn_target, error
                    ),
                )
            })?;
    }
    Ok(())
}

fn deliver_midi_feedback(snapshot: &RuntimeOutputSnapshot) -> Result<(), OutputDispatchFailure> {
    if snapshot.midi_feedback_packets.is_empty() {
        return Ok(());
    }

    let selected_output_id = snapshot.midi_output_id.as_deref().ok_or_else(|| {
        output_failure(
            snapshot.sequence,
            "MIDI-Feedback benötigt einen selektierten MIDI-Output.".to_owned(),
        )
    })?;

    let midi_out = MidiOutput::new("Luma Switch MIDI Output").map_err(|error| {
        output_failure(
            snapshot.sequence,
            format!("MIDI-Output konnte nicht initialisiert werden: {}", error),
        )
    })?;
    let ports = midi_out.ports();
    let selected_port = ports.iter().enumerate().find_map(|(index, port)| {
        let name = midi_out.port_name(port).ok()?;
        (midi_port_id(MidiPortDirection::Output, index, &name) == selected_output_id)
            .then_some(port.clone())
    });
    let Some(port) = selected_port else {
        return Err(output_failure(
            snapshot.sequence,
            format!(
                "Selektierter MIDI-Output {} ist nicht verfügbar.",
                selected_output_id
            ),
        ));
    };

    let mut connection = midi_out
        .connect(&port, "luma-switch-midi-feedback")
        .map_err(|error| {
            output_failure(
                snapshot.sequence,
                format!("MIDI-Output konnte nicht verbunden werden: {}", error),
            )
        })?;

    for packet in &snapshot.midi_feedback_packets {
        let bytes = encode_midi_feedback_message(packet);
        connection.send(&bytes).map_err(|error| {
            output_failure(
                snapshot.sequence,
                format!("MIDI-Feedback konnte nicht gesendet werden: {}", error),
            )
        })?;
    }

    Ok(())
}

fn encode_midi_feedback_message(packet: &MidiFeedbackPacket) -> Vec<u8> {
    match packet.message.kind {
        MidiMessageKind::Note => vec![
            0x90 | packet.message.channel.saturating_sub(1),
            packet.message.key,
            if packet.value > 0 { 127 } else { 0 },
        ],
        MidiMessageKind::ControlChange => vec![
            0xb0 | packet.message.channel.saturating_sub(1),
            packet.message.key,
            ((packet.value as u32 * 127) / 16_383) as u8,
        ],
        MidiMessageKind::PitchBend => {
            let clamped = packet.value.min(16_383);
            vec![
                0xe0 | packet.message.channel.saturating_sub(1),
                (clamped & 0x7f) as u8,
                ((clamped >> 7) & 0x7f) as u8,
            ]
        }
    }
}

fn build_artnet_dmx_packet(universe: u16, sequence: u8, slots: &[u8]) -> Vec<u8> {
    let length = slots.len().clamp(2, DMX_SLOT_COUNT);
    let length = if length % 2 == 0 { length } else { length + 1 };
    let mut packet = Vec::with_capacity(18 + length);
    packet.extend_from_slice(b"Art-Net\0");
    packet.extend_from_slice(&[0x00, 0x50]);
    packet.extend_from_slice(&[0x00, 14]);
    packet.push(sequence);
    packet.push(0);
    packet.push((universe & 0xff) as u8);
    packet.push((universe >> 8) as u8);
    packet.push(((length >> 8) & 0xff) as u8);
    packet.push((length & 0xff) as u8);
    packet.extend_from_slice(&slots[..length.min(slots.len())]);
    while packet.len() < 18 + length {
        packet.push(0);
    }
    packet
}

fn build_sacn_data_packet(universe: u16, sequence: u8, slots: &[u8]) -> Vec<u8> {
    let slot_count = slots.len().clamp(1, DMX_SLOT_COUNT);
    let property_value_count = 1u16.saturating_add(slot_count as u16);
    let dmp_pdu_length = 10u16.saturating_add(property_value_count);
    let framing_pdu_length = 77u16.saturating_add(dmp_pdu_length);
    let root_pdu_length = 22u16.saturating_add(framing_pdu_length);

    let mut packet = Vec::with_capacity(126 + slot_count);
    packet.extend_from_slice(&0x0010u16.to_be_bytes());
    packet.extend_from_slice(&0x0000u16.to_be_bytes());
    packet.extend_from_slice(b"ASC-E1.17\0\0\0");
    packet.extend_from_slice(&acn_flags_and_length(root_pdu_length));
    packet.extend_from_slice(&0x0000_0004u32.to_be_bytes());
    packet.extend_from_slice(&SACN_CID);
    packet.extend_from_slice(&acn_flags_and_length(framing_pdu_length));
    packet.extend_from_slice(&0x0000_0002u32.to_be_bytes());

    let mut source_name = [0u8; 64];
    let source_bytes = SACN_SOURCE_NAME.as_bytes();
    let copy_len = source_bytes.len().min(source_name.len());
    source_name[..copy_len].copy_from_slice(&source_bytes[..copy_len]);
    packet.extend_from_slice(&source_name);
    packet.push(100);
    packet.extend_from_slice(&0u16.to_be_bytes());
    packet.push(sequence);
    packet.push(0);
    packet.extend_from_slice(&universe.to_be_bytes());

    packet.extend_from_slice(&acn_flags_and_length(dmp_pdu_length));
    packet.push(0x02);
    packet.push(0xa1);
    packet.extend_from_slice(&0u16.to_be_bytes());
    packet.extend_from_slice(&1u16.to_be_bytes());
    packet.extend_from_slice(&property_value_count.to_be_bytes());
    packet.push(0);
    packet.extend_from_slice(&slots[..slot_count]);

    packet
}

fn acn_flags_and_length(length: u16) -> [u8; 2] {
    let value = 0x7000u16 | (length & 0x0fff);
    value.to_be_bytes()
}

fn dispatch_summary(snapshot: &RuntimeOutputSnapshot) -> String {
    let dmx_summary = if snapshot.dmx_frames.is_empty() {
        "0 DMX".to_owned()
    } else {
        format!(
            "{} DMX universe(s) via {}",
            snapshot.dmx_frames.len(),
            dmx_backend_label(snapshot.dmx_backend)
        )
    };
    let midi_summary = if snapshot.midi_feedback_packets.is_empty() {
        "0 MIDI".to_owned()
    } else {
        format!("{} MIDI feedback", snapshot.midi_feedback_packets.len())
    };
    format!("Output {}  |  {}", dmx_summary, midi_summary)
}

fn dmx_backend_label(backend: DmxBackendKind) -> &'static str {
    match backend {
        DmxBackendKind::Disabled => "Disabled",
        DmxBackendKind::EnttecOpenDmx => "ENTTEC Open DMX",
        DmxBackendKind::ArtNet => "Art-Net",
        DmxBackendKind::Sacn => "sACN",
    }
}

fn output_failure(sequence: u64, detail: String) -> OutputDispatchFailure {
    OutputDispatchFailure { sequence, detail }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::FixtureGroupId;
    use crate::core::state::{
        FixtureCapability, FixtureMode, FixturePatch, FixturePhysical, FixtureProfile,
        FixtureSourceInfo, FixtureSourceKind, MidiBinding, MidiControlHint,
    };
    use crate::core::time::SpeedRatio;

    #[test]
    fn runtime_output_snapshot_renders_group_output_into_patched_universe() {
        let mut state = StudioState::default();
        state.settings.dmx.output_enabled = true;
        state.settings.dmx.backend = DmxBackendKind::ArtNet;
        state.engine.clock.monotonic_ns = 33_333_334;
        state.fixture_system.library.profiles = vec![rgb_profile()];
        state.fixture_system.library.patches = vec![FixturePatch {
            id: 1,
            profile_id: "demo-rgb".to_owned(),
            name: "Front Wash".to_owned(),
            mode_name: "8bit".to_owned(),
            universe: 1,
            address: 1,
            group_id: Some(FixtureGroupId(1)),
            enabled: true,
        }];

        let snapshot = build_runtime_output_snapshot(&state).expect("snapshot");
        assert_eq!(snapshot.dmx_frames.len(), 1);
        assert_eq!(snapshot.dmx_frames[0].universe, 1);
        assert_eq!(snapshot.dmx_frames[0].slots[0], 206);
        assert!(snapshot.dmx_frames[0].slots[1] > 0);
        assert!(snapshot.dmx_frames[0].slots[2] > 0);
        assert!(snapshot.dmx_frames[0].slots[3] > 0);
    }

    #[test]
    fn artnet_packet_encodes_universe_and_length_deterministically() {
        let packet = build_artnet_dmx_packet(3, 9, &[1, 2, 3, 4]);
        assert_eq!(&packet[..8], b"Art-Net\0");
        assert_eq!(&packet[8..10], &[0x00, 0x50]);
        assert_eq!(packet[12], 9);
        assert_eq!(packet[14], 3);
        assert_eq!(packet[15], 0);
        assert_eq!(&packet[16..18], &[0x00, 0x04]);
        assert_eq!(&packet[18..22], &[1, 2, 3, 4]);
    }

    #[test]
    fn sacn_packet_encodes_universe_and_property_count_deterministically() {
        let packet = build_sacn_data_packet(7, 11, &[1, 2, 3, 4]);
        assert_eq!(&packet[..4], &[0x00, 0x10, 0x00, 0x00]);
        assert_eq!(&packet[4..16], b"ASC-E1.17\0\0\0");
        assert_eq!(packet[111], 11);
        assert_eq!(&packet[113..115], &7u16.to_be_bytes());
        assert_eq!(&packet[123..125], &5u16.to_be_bytes());
        assert_eq!(packet[125], 0);
        assert_eq!(&packet[126..130], &[1, 2, 3, 4]);
    }

    #[test]
    fn midi_feedback_packets_follow_learned_bindings_deterministically() {
        let mut state = StudioState::default();
        state.settings.midi.feedback_enabled = true;
        state.settings.midi.outputs = vec![crate::core::state::MidiPortDescriptor {
            id: "midi-out::0::apc40".to_owned(),
            name: "APC40".to_owned(),
            direction: MidiPortDirection::Output,
            profile_hint: None,
            detail: "APC40".to_owned(),
        }];
        state.settings.midi.selected_output = Some("midi-out::0::apc40".to_owned());
        state.settings.midi.bindings = vec![
            MidiBinding {
                id: 1,
                action: MidiAction::MasterIntensity,
                label: "Master".to_owned(),
                message: Some(MidiBindingMessage {
                    kind: MidiMessageKind::ControlChange,
                    channel: 1,
                    key: 7,
                }),
                hint: MidiControlHint::Continuous,
                learned: true,
                controller_profile: None,
            },
            MidiBinding {
                id: 2,
                action: MidiAction::TriggerChaseSlot(1),
                label: "Chase".to_owned(),
                message: Some(MidiBindingMessage {
                    kind: MidiMessageKind::Note,
                    channel: 1,
                    key: 16,
                }),
                hint: MidiControlHint::Button,
                learned: true,
                controller_profile: None,
            },
        ];
        state.master.speed = SpeedRatio::from_permille(800);

        let packets = build_midi_feedback_packets(&state);
        assert_eq!(packets.len(), 2);
        assert_eq!(packets[0].message.key, 7);
        assert!(packets[0].value > 0);
        assert_eq!(
            encode_midi_feedback_message(&packets[1]),
            vec![0x90, 16, 127]
        );
    }

    #[test]
    fn output_snapshot_respects_refresh_rate_cadence() {
        let mut state = StudioState::default();
        state.settings.dmx.output_enabled = true;
        state.settings.dmx.backend = DmxBackendKind::ArtNet;
        state.settings.dmx.refresh_rate_hz = 30;
        state.fixture_system.library.profiles = vec![rgb_profile()];
        state.fixture_system.library.patches = vec![FixturePatch {
            id: 1,
            profile_id: "demo-rgb".to_owned(),
            name: "Front Wash".to_owned(),
            mode_name: "8bit".to_owned(),
            universe: 1,
            address: 1,
            group_id: Some(FixtureGroupId(1)),
            enabled: true,
        }];
        state.output.sequence = 1;
        state.engine.clock.monotonic_ns = 16_666_667;

        assert!(build_runtime_output_snapshot(&state).is_none());

        state.engine.clock.monotonic_ns = 33_333_334;
        assert!(build_runtime_output_snapshot(&state).is_some());
    }

    #[test]
    fn output_monitor_snapshot_summarizes_routing_and_segments() {
        let mut state = StudioState::default();
        state.settings.dmx.backend = DmxBackendKind::ArtNet;
        state.settings.dmx.artnet_universe = 10;
        state.fixture_system.library.profiles = vec![rgb_profile()];
        state.fixture_system.library.patches = vec![FixturePatch {
            id: 1,
            profile_id: "demo-rgb".to_owned(),
            name: "Front Wash".to_owned(),
            mode_name: "8bit".to_owned(),
            universe: 2,
            address: 33,
            group_id: Some(FixtureGroupId(1)),
            enabled: true,
        }];

        let monitor = build_output_monitor_snapshot(&state);
        assert_eq!(monitor.universe_monitors.len(), 1);
        let universe = &monitor.universe_monitors[0];
        assert_eq!(universe.internal_universe, 2);
        assert_eq!(universe.routed_universe, Some(10));
        assert!(universe.destination.contains("Art-Net U10"));
        assert_eq!(universe.segment_levels.len(), 16);
        assert!(universe.active_slots > 0);
        assert_eq!(universe.patch_labels, vec!["Front Wash".to_owned()]);
    }

    fn rgb_profile() -> FixtureProfile {
        FixtureProfile {
            id: "demo-rgb".to_owned(),
            manufacturer: "Demo".to_owned(),
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
                fixture_channel("Dimmer", "Intensity", 0),
                fixture_channel("Red", "Color", 0),
                fixture_channel("Green", "Color", 0),
                fixture_channel("Blue", "Color", 0),
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

    fn fixture_channel(name: &str, group: &str, byte: u8) -> FixtureChannel {
        FixtureChannel {
            name: name.to_owned(),
            group: group.to_owned(),
            byte,
            default_value: 0,
            highlight_value: 255,
            capabilities: vec![FixtureCapability {
                start: 0,
                end: 255,
                label: name.to_owned(),
            }],
        }
    }
}
