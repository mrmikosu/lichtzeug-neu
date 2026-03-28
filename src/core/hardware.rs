use crate::core::state::{
    ControllerProfileKind, DmxInterfaceDescriptor, DmxInterfaceKind, HardwareInventorySnapshot,
    MidiAction, MidiBinding, MidiBindingMessage, MidiControlHint, MidiMessageKind,
    MidiPortDescriptor, MidiPortDirection, MidiRuntimeMessage,
};
use midir::{Ignore, MidiInput, MidiOutput};
use serialport::SerialPortType;

pub fn scan_hardware_inventory() -> Result<HardwareInventorySnapshot, String> {
    Ok(HardwareInventorySnapshot {
        dmx_interfaces: scan_dmx_interfaces()?,
        midi_inputs: scan_midi_ports(MidiPortDirection::Input)?,
        midi_outputs: scan_midi_ports(MidiPortDirection::Output)?,
    })
}

pub fn scan_dmx_interfaces() -> Result<Vec<DmxInterfaceDescriptor>, String> {
    let mut interfaces = serialport::available_ports()
        .map_err(|error| format!("DMX-Interfaces konnten nicht gelesen werden: {}", error))?
        .into_iter()
        .map(|port| {
            let (manufacturer, product, serial_number, detail_suffix, kind) = match &port.port_type
            {
                SerialPortType::UsbPort(info) => {
                    let manufacturer = info.manufacturer.clone();
                    let product = info.product.clone();
                    let serial_number = info.serial_number.clone();
                    let kind = classify_dmx_interface(
                        &port.port_name,
                        manufacturer.as_deref(),
                        product.as_deref(),
                    );
                    (
                        manufacturer.clone(),
                        product.clone(),
                        serial_number,
                        format!(
                            "{} / {}",
                            product.clone().unwrap_or_else(|| "USB serial".to_owned()),
                            manufacturer
                                .clone()
                                .unwrap_or_else(|| "unknown vendor".to_owned())
                        ),
                        kind,
                    )
                }
                SerialPortType::BluetoothPort => (
                    None,
                    Some("Bluetooth serial".to_owned()),
                    None,
                    "Bluetooth serial".to_owned(),
                    DmxInterfaceKind::Unknown,
                ),
                SerialPortType::PciPort => (
                    None,
                    Some("PCI serial".to_owned()),
                    None,
                    "PCI serial".to_owned(),
                    DmxInterfaceKind::UsbSerial,
                ),
                SerialPortType::Unknown => (
                    None,
                    Some("Serial".to_owned()),
                    None,
                    "Serial".to_owned(),
                    DmxInterfaceKind::Unknown,
                ),
            };

            DmxInterfaceDescriptor {
                id: format!("dmx::{}", sanitize_port_id(&port.port_name)),
                name: product.clone().unwrap_or_else(|| port.port_name.clone()),
                kind,
                port_name: port.port_name.clone(),
                manufacturer,
                product,
                serial_number,
                detail: format!("{} @ {}", detail_suffix, port.port_name),
                universe_capacity: 1,
            }
        })
        .collect::<Vec<_>>();

    interfaces.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
    Ok(interfaces)
}

pub fn scan_midi_ports(direction: MidiPortDirection) -> Result<Vec<MidiPortDescriptor>, String> {
    let mut ports = match direction {
        MidiPortDirection::Input => {
            let mut midi_in =
                MidiInput::new("Luma Switch MIDI Scan").map_err(|error| error.to_string())?;
            midi_in.ignore(Ignore::None);
            let available = midi_in.ports();
            available
                .iter()
                .enumerate()
                .map(|(index, port)| {
                    let name = midi_in
                        .port_name(port)
                        .unwrap_or_else(|_| format!("MIDI Input {}", index + 1));
                    build_midi_port_descriptor(direction, index, &name)
                })
                .collect::<Vec<_>>()
        }
        MidiPortDirection::Output => {
            let midi_out =
                MidiOutput::new("Luma Switch MIDI Scan").map_err(|error| error.to_string())?;
            let available = midi_out.ports();
            available
                .iter()
                .enumerate()
                .map(|(index, port)| {
                    let name = midi_out
                        .port_name(port)
                        .unwrap_or_else(|_| format!("MIDI Output {}", index + 1));
                    build_midi_port_descriptor(direction, index, &name)
                })
                .collect::<Vec<_>>()
        }
    };

    ports.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
    Ok(ports)
}

pub fn midi_port_id(direction: MidiPortDirection, index: usize, name: &str) -> String {
    let prefix = match direction {
        MidiPortDirection::Input => "midi-in",
        MidiPortDirection::Output => "midi-out",
    };
    format!("{}::{}::{}", prefix, index, sanitize_port_id(name))
}

pub fn decode_midi_bytes(timestamp_micros: u64, bytes: &[u8]) -> Option<MidiRuntimeMessage> {
    let status = *bytes.first()?;
    let channel = (status & 0x0f).saturating_add(1);
    let data1 = *bytes.get(1).unwrap_or(&0);
    let data2 = *bytes.get(2).unwrap_or(&0);

    match status & 0xf0 {
        0x80 => Some(MidiRuntimeMessage {
            timestamp_micros,
            kind: MidiMessageKind::Note,
            channel,
            key: data1,
            value: 0,
        }),
        0x90 => Some(MidiRuntimeMessage {
            timestamp_micros,
            kind: MidiMessageKind::Note,
            channel,
            key: data1,
            value: data2 as u16,
        }),
        0xb0 => Some(MidiRuntimeMessage {
            timestamp_micros,
            kind: MidiMessageKind::ControlChange,
            channel,
            key: data1,
            value: data2 as u16,
        }),
        0xe0 => Some(MidiRuntimeMessage {
            timestamp_micros,
            kind: MidiMessageKind::PitchBend,
            channel,
            key: 0,
            value: ((data2 as u16) << 7) | data1 as u16,
        }),
        _ => None,
    }
}

pub fn normalize_midi_binding_message(message: &MidiRuntimeMessage) -> Option<MidiBindingMessage> {
    match message.kind {
        MidiMessageKind::Note | MidiMessageKind::ControlChange | MidiMessageKind::PitchBend => {
            Some(MidiBindingMessage {
                kind: message.kind,
                channel: message.channel,
                key: message.key,
            })
        }
    }
}

pub fn midi_control_hint(message: &MidiRuntimeMessage) -> MidiControlHint {
    match message.kind {
        MidiMessageKind::Note => MidiControlHint::Button,
        MidiMessageKind::ControlChange | MidiMessageKind::PitchBend => MidiControlHint::Continuous,
    }
}

pub fn is_trigger_message_active(message: &MidiRuntimeMessage) -> bool {
    match message.kind {
        MidiMessageKind::Note => message.value > 0,
        MidiMessageKind::ControlChange => message.value >= 64,
        MidiMessageKind::PitchBend => message.value > 8192,
    }
}

pub fn midi_value_permille(message: &MidiRuntimeMessage) -> u16 {
    match message.kind {
        MidiMessageKind::PitchBend => ((message.value as u32 * 1000) / 16_383) as u16,
        MidiMessageKind::Note | MidiMessageKind::ControlChange => {
            ((message.value.min(127) as u32 * 1000) / 127) as u16
        }
    }
}

pub fn controller_profile_from_name(name: &str) -> Option<ControllerProfileKind> {
    let normalized = name.to_ascii_lowercase();
    if normalized.contains("apc40") {
        Some(ControllerProfileKind::Apc40Mk2)
    } else if normalized.contains("prime 2")
        || normalized.contains("prime2")
        || normalized.contains("denon dj prime 2")
    {
        Some(ControllerProfileKind::DenonPrime2)
    } else if normalized.contains("cmd dc-1") || normalized.contains("cmd dc1") {
        Some(ControllerProfileKind::BehringerCmdDc1)
    } else if normalized.contains("cmd lc-1") || normalized.contains("cmd lc1") {
        Some(ControllerProfileKind::BehringerCmdLc1)
    } else {
        None
    }
}

pub fn controller_profile_bindings(
    profile: ControllerProfileKind,
    start_id: u32,
) -> Vec<MidiBinding> {
    automap_blueprint(profile)
        .into_iter()
        .enumerate()
        .map(|(index, (label, action, hint))| MidiBinding {
            id: start_id.saturating_add(index as u32),
            action,
            label: label.to_owned(),
            message: None,
            hint,
            learned: false,
            controller_profile: Some(profile),
        })
        .collect()
}

fn build_midi_port_descriptor(
    direction: MidiPortDirection,
    index: usize,
    name: &str,
) -> MidiPortDescriptor {
    let profile_hint = controller_profile_from_name(name);
    MidiPortDescriptor {
        id: midi_port_id(direction, index, name),
        name: name.to_owned(),
        direction,
        profile_hint,
        detail: match profile_hint {
            Some(profile) => format!("{} ({})", name, controller_profile_label(profile)),
            None => name.to_owned(),
        },
    }
}

fn classify_dmx_interface(
    port_name: &str,
    manufacturer: Option<&str>,
    product: Option<&str>,
) -> DmxInterfaceKind {
    let joined = format!(
        "{} {} {}",
        port_name,
        manufacturer.unwrap_or_default(),
        product.unwrap_or_default()
    )
    .to_ascii_lowercase();

    if joined.contains("enttec")
        || (joined.contains("ftdi") && joined.contains("dmx"))
        || joined.contains("open dmx")
    {
        DmxInterfaceKind::EnttecOpenDmxCompatible
    } else if joined.contains("usb") || joined.contains("serial") || joined.contains("tty") {
        DmxInterfaceKind::UsbSerial
    } else {
        DmxInterfaceKind::Unknown
    }
}

fn sanitize_port_id(value: &str) -> String {
    let mut sanitized = String::new();
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            sanitized.push(character.to_ascii_lowercase());
        } else if !sanitized.ends_with('-') {
            sanitized.push('-');
        }
    }
    sanitized.trim_matches('-').to_owned()
}

fn controller_profile_label(profile: ControllerProfileKind) -> &'static str {
    match profile {
        ControllerProfileKind::Apc40Mk2 => "APC40 mkII",
        ControllerProfileKind::DenonPrime2 => "Denon Prime 2",
        ControllerProfileKind::BehringerCmdDc1 => "CMD DC-1",
        ControllerProfileKind::BehringerCmdLc1 => "CMD LC-1",
    }
}

fn automap_blueprint(profile: ControllerProfileKind) -> Vec<(String, MidiAction, MidiControlHint)> {
    match profile {
        ControllerProfileKind::Apc40Mk2 => {
            let mut bindings = vec![
                (
                    "Master Fader".to_owned(),
                    MidiAction::MasterIntensity,
                    MidiControlHint::Continuous,
                ),
                (
                    "Crossfader".to_owned(),
                    MidiAction::MasterSpeed,
                    MidiControlHint::Continuous,
                ),
                (
                    "Device Knob 1".to_owned(),
                    MidiAction::FxDepthSlot(1),
                    MidiControlHint::Continuous,
                ),
                (
                    "Device Knob 2".to_owned(),
                    MidiAction::FxDepthSlot(2),
                    MidiControlHint::Continuous,
                ),
                (
                    "Device Knob 3".to_owned(),
                    MidiAction::FxDepthSlot(3),
                    MidiControlHint::Continuous,
                ),
                (
                    "Device Knob 4".to_owned(),
                    MidiAction::FxDepthSlot(4),
                    MidiControlHint::Continuous,
                ),
                (
                    "Device Knob 5".to_owned(),
                    MidiAction::FxDepthSlot(5),
                    MidiControlHint::Continuous,
                ),
                (
                    "Device Knob 6".to_owned(),
                    MidiAction::FxDepthSlot(6),
                    MidiControlHint::Continuous,
                ),
                (
                    "Device Knob 7".to_owned(),
                    MidiAction::FxDepthSlot(7),
                    MidiControlHint::Continuous,
                ),
                (
                    "Device Knob 8".to_owned(),
                    MidiAction::FxDepthSlot(8),
                    MidiControlHint::Continuous,
                ),
                (
                    "Transport".to_owned(),
                    MidiAction::TransportToggle,
                    MidiControlHint::Button,
                ),
            ];
            for slot in 1..=40 {
                bindings.push((
                    format!("Clip Grid {}", slot),
                    MidiAction::TriggerCueSlot(slot),
                    MidiControlHint::Button,
                ));
            }
            bindings
        }
        ControllerProfileKind::DenonPrime2 => {
            let mut bindings = vec![
                (
                    "Deck 1 Sweep FX".to_owned(),
                    MidiAction::MasterIntensity,
                    MidiControlHint::Continuous,
                ),
                (
                    "Deck 2 Sweep FX".to_owned(),
                    MidiAction::MasterSpeed,
                    MidiControlHint::Continuous,
                ),
                (
                    "Deck 1 Filter".to_owned(),
                    MidiAction::FxDepthSlot(1),
                    MidiControlHint::Continuous,
                ),
                (
                    "Deck 2 Filter".to_owned(),
                    MidiAction::FxDepthSlot(2),
                    MidiControlHint::Continuous,
                ),
                (
                    "FX Select 1".to_owned(),
                    MidiAction::FxDepthSlot(3),
                    MidiControlHint::Continuous,
                ),
                (
                    "FX Select 2".to_owned(),
                    MidiAction::FxDepthSlot(4),
                    MidiControlHint::Continuous,
                ),
                (
                    "View Encoder".to_owned(),
                    MidiAction::TimelineZoom,
                    MidiControlHint::Continuous,
                ),
                (
                    "Deck 1 Play".to_owned(),
                    MidiAction::TransportToggle,
                    MidiControlHint::Button,
                ),
                (
                    "Deck 2 Play".to_owned(),
                    MidiAction::TransportToggle,
                    MidiControlHint::Button,
                ),
                (
                    "Deck 1 Load".to_owned(),
                    MidiAction::FocusFixtureGroupSlot(1),
                    MidiControlHint::Button,
                ),
                (
                    "Deck 2 Load".to_owned(),
                    MidiAction::FocusFixtureGroupSlot(2),
                    MidiControlHint::Button,
                ),
                (
                    "Deck 1 Censor".to_owned(),
                    MidiAction::FocusFixtureGroupSlot(3),
                    MidiControlHint::Button,
                ),
                (
                    "Deck 2 Censor".to_owned(),
                    MidiAction::FocusFixtureGroupSlot(4),
                    MidiControlHint::Button,
                ),
            ];
            for slot in 1..=8 {
                bindings.push((
                    format!("Deck 1 Performance Pad {}", slot),
                    MidiAction::TriggerCueSlot(slot),
                    MidiControlHint::Button,
                ));
            }
            for slot in 1..=8 {
                bindings.push((
                    format!("Deck 2 Performance Pad {}", slot),
                    MidiAction::TriggerCueSlot(slot + 8),
                    MidiControlHint::Button,
                ));
            }
            for slot in 1..=4 {
                bindings.push((
                    format!("Deck 1 Pad Mode {}", slot),
                    MidiAction::TriggerChaseSlot(slot),
                    MidiControlHint::Button,
                ));
            }
            for slot in 1..=4 {
                bindings.push((
                    format!("Deck 2 Pad Mode {}", slot),
                    MidiAction::TriggerChaseSlot(slot + 4),
                    MidiControlHint::Button,
                ));
            }
            bindings
        }
        ControllerProfileKind::BehringerCmdDc1 => {
            let mut bindings = vec![
                (
                    "Jog / Zoom".to_owned(),
                    MidiAction::TimelineZoom,
                    MidiControlHint::Continuous,
                ),
                (
                    "Encoder 1".to_owned(),
                    MidiAction::FxDepthSlot(1),
                    MidiControlHint::Continuous,
                ),
                (
                    "Encoder 2".to_owned(),
                    MidiAction::FxDepthSlot(2),
                    MidiControlHint::Continuous,
                ),
                (
                    "Encoder 3".to_owned(),
                    MidiAction::FxDepthSlot(3),
                    MidiControlHint::Continuous,
                ),
                (
                    "Encoder 4".to_owned(),
                    MidiAction::FxDepthSlot(4),
                    MidiControlHint::Continuous,
                ),
                (
                    "Encoder 5".to_owned(),
                    MidiAction::FxDepthSlot(5),
                    MidiControlHint::Continuous,
                ),
                (
                    "Encoder 6".to_owned(),
                    MidiAction::FxDepthSlot(6),
                    MidiControlHint::Continuous,
                ),
                (
                    "Encoder 7".to_owned(),
                    MidiAction::FxDepthSlot(7),
                    MidiControlHint::Continuous,
                ),
                (
                    "Encoder 8".to_owned(),
                    MidiAction::FxDepthSlot(8),
                    MidiControlHint::Continuous,
                ),
                (
                    "Shift / Transport".to_owned(),
                    MidiAction::TransportToggle,
                    MidiControlHint::Button,
                ),
            ];
            for slot in 1..=16 {
                bindings.push((
                    format!("Pad {}", slot),
                    MidiAction::TriggerCueSlot(slot),
                    MidiControlHint::Button,
                ));
            }
            for slot in 1..=8 {
                bindings.push((
                    format!("FX Button {}", slot),
                    MidiAction::TriggerChaseSlot(slot),
                    MidiControlHint::Button,
                ));
            }
            bindings
        }
        ControllerProfileKind::BehringerCmdLc1 => {
            let mut bindings = vec![
                (
                    "Encoder 1".to_owned(),
                    MidiAction::FxDepthSlot(1),
                    MidiControlHint::Continuous,
                ),
                (
                    "Encoder 2".to_owned(),
                    MidiAction::FxDepthSlot(2),
                    MidiControlHint::Continuous,
                ),
                (
                    "Encoder 3".to_owned(),
                    MidiAction::FxDepthSlot(3),
                    MidiControlHint::Continuous,
                ),
                (
                    "Encoder 4".to_owned(),
                    MidiAction::FxDepthSlot(4),
                    MidiControlHint::Continuous,
                ),
                (
                    "Encoder 5".to_owned(),
                    MidiAction::FxDepthSlot(5),
                    MidiControlHint::Continuous,
                ),
                (
                    "Encoder 6".to_owned(),
                    MidiAction::FxDepthSlot(6),
                    MidiControlHint::Continuous,
                ),
                (
                    "Encoder 7".to_owned(),
                    MidiAction::FxDepthSlot(7),
                    MidiControlHint::Continuous,
                ),
                (
                    "Encoder 8".to_owned(),
                    MidiAction::FxDepthSlot(8),
                    MidiControlHint::Continuous,
                ),
                (
                    "Transport".to_owned(),
                    MidiAction::TransportToggle,
                    MidiControlHint::Button,
                ),
                (
                    "Master Macro".to_owned(),
                    MidiAction::MasterIntensity,
                    MidiControlHint::Continuous,
                ),
            ];
            for slot in 1..=32 {
                bindings.push((
                    format!("Grid {}", slot),
                    MidiAction::TriggerCueSlot(slot),
                    MidiControlHint::Button,
                ));
            }
            bindings
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controller_profile_detects_denon_prime_2_from_port_name() {
        assert_eq!(
            controller_profile_from_name("Denon DJ PRIME 2 MIDI"),
            Some(ControllerProfileKind::DenonPrime2)
        );
        assert_eq!(
            controller_profile_from_name("PRIME2 Control Surface"),
            Some(ControllerProfileKind::DenonPrime2)
        );
    }

    #[test]
    fn denon_prime_2_automap_blueprint_is_stable() {
        let bindings = controller_profile_bindings(ControllerProfileKind::DenonPrime2, 1);
        assert_eq!(bindings.len(), 37);
        assert_eq!(bindings[0].label, "Deck 1 Sweep FX");
        assert_eq!(bindings[6].label, "View Encoder");
        assert_eq!(bindings[13].label, "Deck 1 Performance Pad 1");
        assert_eq!(
            bindings
                .iter()
                .find(|binding| binding.label == "Deck 2 Performance Pad 8")
                .map(|binding| binding.action),
            Some(MidiAction::TriggerCueSlot(16))
        );
        assert_eq!(
            bindings
                .iter()
                .find(|binding| binding.label == "Deck 2 Pad Mode 4")
                .map(|binding| binding.action),
            Some(MidiAction::TriggerChaseSlot(8))
        );
    }
}
