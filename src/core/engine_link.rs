use crate::core::state::{
    EngineDeckFollowMode, EngineDeckPhase, EngineDeckTelemetry, EnginePrimeDevice,
    EngineServiceDescriptor, EngineTelemetryFrame,
};
use serde::Deserialize;
use std::net::SocketAddr;

pub const DEFAULT_STAGELINQ_DISCOVERY_PORT: u16 = 51_337;

#[derive(Debug, Deserialize)]
struct DiscoveryPacketJson {
    kind: Option<String>,
    device_name: Option<String>,
    name: Option<String>,
    software_name: Option<String>,
    software_version: Option<String>,
    announce_port: Option<u16>,
    service_port: Option<u16>,
    token_hint: Option<String>,
    services: Option<Vec<DiscoveryServiceJson>>,
}

#[derive(Debug, Deserialize)]
struct DiscoveryServiceJson {
    name: String,
    port: u16,
    detail: Option<String>,
}

pub fn parse_engine_discovery_packet(
    payload: &[u8],
    source: SocketAddr,
) -> Option<EnginePrimeDevice> {
    parse_json_discovery_packet(payload, source)
        .or_else(|| parse_text_discovery_packet(payload, source))
}

pub fn merge_engine_device(
    devices: &mut Vec<EnginePrimeDevice>,
    device: EnginePrimeDevice,
) -> bool {
    if let Some(existing) = devices.iter_mut().find(|entry| entry.id == device.id) {
        if *existing == device {
            return false;
        }
        *existing = device;
    } else {
        devices.push(device);
    }

    devices.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then(left.address.cmp(&right.address))
            .then(left.service_port.cmp(&right.service_port))
    });
    true
}

pub fn select_followed_deck(
    frame: &EngineTelemetryFrame,
    follow_mode: EngineDeckFollowMode,
) -> Option<&EngineDeckTelemetry> {
    match follow_mode {
        EngineDeckFollowMode::Disabled => None,
        EngineDeckFollowMode::Deck1 => frame.decks.iter().find(|deck| deck.deck_index == 1),
        EngineDeckFollowMode::Deck2 => frame.decks.iter().find(|deck| deck.deck_index == 2),
        EngineDeckFollowMode::MasterDeck => {
            frame.decks.iter().find(|deck| deck.is_master).or_else(|| {
                frame
                    .decks
                    .iter()
                    .find(|deck| deck.phase == EngineDeckPhase::Playing)
            })
        }
        EngineDeckFollowMode::AnyPlayingDeck => frame
            .decks
            .iter()
            .find(|deck| deck.phase == EngineDeckPhase::Playing)
            .or_else(|| frame.decks.first()),
    }
}

fn parse_json_discovery_packet(payload: &[u8], source: SocketAddr) -> Option<EnginePrimeDevice> {
    let packet: DiscoveryPacketJson = serde_json::from_slice(payload).ok()?;
    if packet
        .kind
        .as_deref()
        .is_some_and(|kind| !kind.to_ascii_lowercase().contains("stagelinq"))
    {
        return None;
    }

    let name = packet
        .device_name
        .or(packet.name)
        .unwrap_or_else(|| format!("Engine Device {}", source.ip()));
    let software_name = packet
        .software_name
        .unwrap_or_else(|| "Engine DJ".to_owned());
    let software_version = packet
        .software_version
        .unwrap_or_else(|| "unknown".to_owned());
    let announce_port = packet
        .announce_port
        .unwrap_or(DEFAULT_STAGELINQ_DISCOVERY_PORT);
    let service_port = packet.service_port.filter(|port| *port > 0);
    let mut services = packet
        .services
        .unwrap_or_default()
        .into_iter()
        .map(|service| EngineServiceDescriptor {
            name: service.name,
            port: service.port,
            detail: service
                .detail
                .unwrap_or_else(|| "StageLinq service".to_owned()),
        })
        .collect::<Vec<_>>();
    services.sort_by(|left, right| left.name.cmp(&right.name).then(left.port.cmp(&right.port)));

    let id = normalized_device_id(
        &name,
        &source.ip().to_string(),
        service_port.unwrap_or(announce_port),
    );
    Some(EnginePrimeDevice {
        id,
        name,
        address: source.ip().to_string(),
        software_name,
        software_version,
        announce_port,
        service_port,
        token_hint: packet.token_hint,
        services,
        detail: format!("{} {} @ {}", "StageLinq", announce_port, source.ip()),
        last_seen_frame: 0,
    })
}

fn parse_text_discovery_packet(payload: &[u8], source: SocketAddr) -> Option<EnginePrimeDevice> {
    let text = String::from_utf8_lossy(payload);
    let lower = text.to_ascii_lowercase();
    if !lower.contains("discoverer_howdy_") && !lower.contains("stagelinq") {
        return None;
    }

    let name =
        infer_device_name(&lower).unwrap_or_else(|| format!("Engine Device {}", source.ip()));
    let software_name = if lower.contains("engine") {
        "Engine DJ".to_owned()
    } else {
        "StageLinq".to_owned()
    };
    let software_version = extract_version(&text).unwrap_or_else(|| "unknown".to_owned());
    let announce_port = DEFAULT_STAGELINQ_DISCOVERY_PORT;
    let service_port =
        extract_port_candidate(&text).filter(|port| *port != DEFAULT_STAGELINQ_DISCOVERY_PORT);
    let token_hint = extract_hex_token(&text);
    let id = normalized_device_id(
        &name,
        &source.ip().to_string(),
        service_port.unwrap_or(announce_port),
    );

    Some(EnginePrimeDevice {
        id,
        name,
        address: source.ip().to_string(),
        software_name,
        software_version,
        announce_port,
        service_port,
        token_hint,
        services: Vec::new(),
        detail: summarize_text_payload(&text),
        last_seen_frame: 0,
    })
}

fn infer_device_name(lower: &str) -> Option<String> {
    [
        ("prime 2", "Denon Prime 2"),
        ("prime2", "Denon Prime 2"),
        ("prime 4", "Denon Prime 4"),
        ("prime4", "Denon Prime 4"),
        ("sc5000", "Denon SC5000"),
        ("sc6000", "Denon SC6000"),
        ("x1850", "Denon X1850"),
        ("x1800", "Denon X1800"),
    ]
    .into_iter()
    .find_map(|(needle, name)| lower.contains(needle).then_some(name.to_owned()))
}

fn normalized_device_id(name: &str, address: &str, port: u16) -> String {
    let mut id = String::new();
    for character in format!("{}-{}-{}", name, address, port).chars() {
        if character.is_ascii_alphanumeric() {
            id.push(character.to_ascii_lowercase());
        } else if !id.ends_with('-') {
            id.push('-');
        }
    }
    id.trim_matches('-').to_owned()
}

fn extract_port_candidate(text: &str) -> Option<u16> {
    let mut current = String::new();
    let mut candidates = Vec::new();

    for character in text.chars() {
        if character.is_ascii_digit() {
            current.push(character);
        } else if !current.is_empty() {
            if let Ok(value) = current.parse::<u16>()
                && (1_024..=65_535).contains(&value)
            {
                candidates.push(value);
            }
            current.clear();
        }
    }

    if let Ok(value) = current.parse::<u16>()
        && (1_024..=65_535).contains(&value)
    {
        candidates.push(value);
    }

    candidates
        .into_iter()
        .find(|candidate| *candidate != DEFAULT_STAGELINQ_DISCOVERY_PORT)
}

fn extract_version(text: &str) -> Option<String> {
    text.split(|character: char| !character.is_ascii_alphanumeric() && character != '.')
        .find(|segment| {
            segment
                .chars()
                .filter(|character| *character == '.')
                .count()
                >= 1
                && segment.chars().any(|character| character.is_ascii_digit())
        })
        .map(str::to_owned)
}

fn extract_hex_token(text: &str) -> Option<String> {
    text.split(|character: char| !character.is_ascii_hexdigit())
        .find(|segment| segment.len() >= 8)
        .map(|segment| segment.to_ascii_lowercase())
}

fn summarize_text_payload(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return "StageLinq discovery".to_owned();
    }

    let summary = trimmed.replace('\n', " ").replace('\r', " ");
    let mut compact = String::new();
    let mut previous_space = false;
    for character in summary.chars() {
        if character.is_whitespace() {
            if !previous_space {
                compact.push(' ');
            }
            previous_space = true;
        } else if character.is_ascii_graphic() || character == ' ' {
            compact.push(character);
            previous_space = false;
        }
    }
    compact.chars().take(96).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{BeatTime, EngineMixerTelemetry, IntensityLevel, TempoBpm};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    fn test_addr() -> SocketAddr {
        SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 44)),
            DEFAULT_STAGELINQ_DISCOVERY_PORT,
        )
    }

    #[test]
    fn parse_json_discovery_packet_is_stable() {
        let payload = br#"{
            "kind":"stagelinq_discovery",
            "device_name":"Denon Prime 2",
            "software_name":"Engine DJ",
            "software_version":"4.1.0",
            "announce_port":51337,
            "service_port":50010,
            "token_hint":"abcd1234",
            "services":[
                {"name":"BeatInfo","port":50020,"detail":"beat"},
                {"name":"StateMap","port":50030,"detail":"state"}
            ]
        }"#;

        let parsed = parse_engine_discovery_packet(payload, test_addr()).expect("parsed");
        assert_eq!(parsed.name, "Denon Prime 2");
        assert_eq!(parsed.service_port, Some(50010));
        assert_eq!(parsed.services.len(), 2);
        assert_eq!(parsed.services[0].name, "BeatInfo");
    }

    #[test]
    fn parse_text_discovery_packet_extracts_prime_name_and_port() {
        let payload = b"DISCOVERER_HOWDY_ prime 2 EngineDJ 4.0.1 port 50010 token DEADBEEF";
        let parsed = parse_engine_discovery_packet(payload, test_addr()).expect("parsed");

        assert_eq!(parsed.name, "Denon Prime 2");
        assert_eq!(parsed.software_version, "4.0.1");
        assert_eq!(parsed.service_port, Some(50010));
        assert_eq!(parsed.token_hint.as_deref(), Some("deadbeef"));
    }

    #[test]
    fn merge_engine_device_updates_in_place_deterministically() {
        let mut devices = vec![EnginePrimeDevice {
            id: "prime".to_owned(),
            name: "Prime".to_owned(),
            address: "192.168.1.2".to_owned(),
            software_name: "Engine DJ".to_owned(),
            software_version: "4.0.0".to_owned(),
            announce_port: DEFAULT_STAGELINQ_DISCOVERY_PORT,
            service_port: Some(50010),
            token_hint: None,
            services: Vec::new(),
            detail: "first".to_owned(),
            last_seen_frame: 1,
        }];

        let changed = merge_engine_device(
            &mut devices,
            EnginePrimeDevice {
                id: "prime".to_owned(),
                name: "Prime".to_owned(),
                address: "192.168.1.2".to_owned(),
                software_name: "Engine DJ".to_owned(),
                software_version: "4.1.0".to_owned(),
                announce_port: DEFAULT_STAGELINQ_DISCOVERY_PORT,
                service_port: Some(50010),
                token_hint: None,
                services: Vec::new(),
                detail: "second".to_owned(),
                last_seen_frame: 2,
            },
        );

        assert!(changed);
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].software_version, "4.1.0");
        assert_eq!(devices[0].last_seen_frame, 2);
    }

    #[test]
    fn select_followed_deck_prefers_master_then_playing() {
        let frame = EngineTelemetryFrame {
            device_id: "prime".to_owned(),
            decks: vec![
                EngineDeckTelemetry {
                    deck_index: 1,
                    track_name: "Intro".to_owned(),
                    artist_name: "Artist".to_owned(),
                    bpm: TempoBpm::from_whole_bpm(124),
                    beat: BeatTime::from_beats(8),
                    phase: EngineDeckPhase::Playing,
                    is_master: false,
                    is_synced: false,
                },
                EngineDeckTelemetry {
                    deck_index: 2,
                    track_name: "Drop".to_owned(),
                    artist_name: "Artist".to_owned(),
                    bpm: TempoBpm::from_whole_bpm(128),
                    beat: BeatTime::from_beats(16),
                    phase: EngineDeckPhase::Playing,
                    is_master: true,
                    is_synced: true,
                },
            ],
            mixer: EngineMixerTelemetry {
                crossfader: IntensityLevel::from_permille(500),
                channel_faders: vec![
                    IntensityLevel::from_permille(1000),
                    IntensityLevel::from_permille(750),
                ],
            },
            summary: "Prime session".to_owned(),
        };

        let followed =
            select_followed_deck(&frame, EngineDeckFollowMode::MasterDeck).expect("followed deck");
        assert_eq!(followed.deck_index, 2);
    }
}
