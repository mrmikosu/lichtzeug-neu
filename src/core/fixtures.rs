use crate::core::state::{
    FixtureCapability, FixtureChannel, FixtureMode, FixturePatch, FixturePhysical, FixtureProfile,
    FixtureSourceInfo, FixtureSourceKind,
};
use roxmltree::{Document, Node};
use serde_json::Value;
use std::fmt::Write;

const OFL_BASE_URL: &str = "https://open-fixture-library.org";

pub fn build_ofl_download_url(manufacturer_key: &str, fixture_key: &str) -> String {
    format!(
        "{}/{}/{}.ofl",
        OFL_BASE_URL,
        normalize_key(manufacturer_key),
        normalize_key(fixture_key)
    )
}

pub fn import_ofl_fixture(
    json: &str,
    manufacturer_key: Option<&str>,
    fixture_key: Option<&str>,
) -> Result<FixtureProfile, String> {
    let root: Value = serde_json::from_str(json).map_err(|err| err.to_string())?;
    let manufacturer = manufacturer_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(title_case_slug)
        .unwrap_or_else(|| "Open Fixture Library".to_owned());
    let model = required_str(&root, "name")?.to_owned();
    let short_name = root
        .get("shortName")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(model.as_str())
        .to_owned();
    let categories = root
        .get("categories")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|values| !values.is_empty())
        .ok_or_else(|| "OFL fixture without categories".to_owned())?;
    let channels = parse_ofl_channels(&root)?;
    let modes = parse_ofl_modes(&root)?;
    let manufacturer_key = manufacturer_key
        .map(normalize_key)
        .filter(|value| !value.is_empty());
    let fixture_key = fixture_key
        .map(normalize_key)
        .filter(|value| !value.is_empty());

    Ok(FixtureProfile {
        id: fixture_profile_id(&manufacturer, &model),
        manufacturer,
        model,
        short_name,
        categories,
        physical: parse_ofl_physical(&root),
        channels,
        modes,
        source: FixtureSourceInfo {
            kind: FixtureSourceKind::OpenFixtureLibrary,
            manufacturer_key: manufacturer_key.clone(),
            fixture_key: fixture_key.clone(),
            source_path: None,
            ofl_url: manufacturer_key
                .zip(fixture_key)
                .map(|(manufacturer, fixture)| build_ofl_download_url(&manufacturer, &fixture)),
            creator_name: root
                .get("meta")
                .and_then(|meta| meta.get("authors"))
                .and_then(Value::as_array)
                .and_then(|authors| authors.first())
                .and_then(Value::as_str)
                .map(str::to_owned),
            creator_version: root
                .get("$schema")
                .and_then(Value::as_str)
                .map(str::to_owned),
        },
    })
}

pub fn import_qxf_fixture(xml: &str, source_path: Option<&str>) -> Result<FixtureProfile, String> {
    let sanitized = strip_xml_doctype(xml);
    let doc = Document::parse(&sanitized).map_err(|err| err.to_string())?;
    let root = doc.root_element();

    if root.tag_name().name() != "FixtureDefinition" {
        return Err("QXF root node must be FixtureDefinition".to_owned());
    }

    let manufacturer =
        child_text(root, "Manufacturer").ok_or_else(|| "QXF missing Manufacturer".to_owned())?;
    let model = child_text(root, "Model").ok_or_else(|| "QXF missing Model".to_owned())?;
    let qxf_type = child_text(root, "Type").unwrap_or_else(|| "Other".to_owned());

    let channels = root
        .children()
        .filter(|node| node.is_element() && node.tag_name().name() == "Channel")
        .map(parse_qxf_channel)
        .collect::<Result<Vec<_>, _>>()?;

    let modes = root
        .children()
        .filter(|node| node.is_element() && node.tag_name().name() == "Mode")
        .map(parse_qxf_mode)
        .collect::<Result<Vec<_>, _>>()?;

    if modes.is_empty() {
        return Err("QXF fixture without modes".to_owned());
    }

    let creator = root
        .children()
        .find(|node| node.is_element() && node.tag_name().name() == "Creator");

    Ok(FixtureProfile {
        id: fixture_profile_id(&manufacturer, &model),
        manufacturer,
        model: model.clone(),
        short_name: model,
        categories: qxf_categories(&qxf_type),
        physical: None,
        channels,
        modes,
        source: FixtureSourceInfo {
            kind: FixtureSourceKind::Qxf,
            manufacturer_key: None,
            fixture_key: None,
            source_path: source_path.map(str::to_owned),
            ofl_url: None,
            creator_name: creator.and_then(|node| child_text(node, "Name")),
            creator_version: creator.and_then(|node| child_text(node, "Version")),
        },
    })
}

pub fn export_qxf_fixture(profile: &FixtureProfile) -> Result<String, String> {
    if profile.channels.is_empty() {
        return Err("Fixture profile without channels cannot be exported as QXF".to_owned());
    }

    if profile.modes.is_empty() {
        return Err("Fixture profile without modes cannot be exported as QXF".to_owned());
    }

    let mut xml = String::new();
    xml.push_str("<!DOCTYPE FixtureDefinition>\n");
    xml.push_str("<FixtureDefinition xmlns=\"http://www.qlcplus.org/FixtureDefinition\">\n");
    xml.push_str("  <Creator>\n");
    xml.push_str("    <Name>Luma Switch Studio</Name>\n");
    xml.push_str("    <Version>0.1.0</Version>\n");
    xml.push_str("    <Author>Codex</Author>\n");
    xml.push_str("  </Creator>\n");
    write_tag(&mut xml, 1, "Manufacturer", &profile.manufacturer);
    write_tag(&mut xml, 1, "Model", &profile.model);
    write_tag(&mut xml, 1, "Type", &qxf_type_from_profile(profile));

    for channel in &profile.channels {
        writeln!(
            xml,
            "  <Channel Name=\"{}\">",
            escape_xml_attr(&channel.name)
        )
        .expect("write qxf channel");
        writeln!(
            xml,
            "    <Group Byte=\"{}\">{}</Group>",
            channel.byte,
            escape_xml_text(&channel.group)
        )
        .expect("write qxf group");
        write_tag(&mut xml, 2, "Default", &channel.default_value.to_string());
        write_tag(
            &mut xml,
            2,
            "Highlight",
            &channel.highlight_value.to_string(),
        );
        for capability in &channel.capabilities {
            writeln!(
                xml,
                "    <Capability Min=\"{}\" Max=\"{}\">{}</Capability>",
                capability.start,
                capability.end,
                escape_xml_text(&capability.label)
            )
            .expect("write qxf capability");
        }
        xml.push_str("  </Channel>\n");
    }

    for mode in &profile.modes {
        writeln!(xml, "  <Mode Name=\"{}\">", escape_xml_attr(&mode.name)).expect("write qxf mode");

        for (index, channel_name) in mode.channels.iter().enumerate() {
            writeln!(
                xml,
                "    <Channel Number=\"{}\">{}</Channel>",
                index,
                escape_xml_text(channel_name)
            )
            .expect("write qxf mode channel");
        }

        xml.push_str("  </Mode>\n");
    }

    xml.push_str("</FixtureDefinition>\n");
    Ok(xml)
}

fn strip_xml_doctype(xml: &str) -> String {
    let Some(start) = xml.find("<!DOCTYPE") else {
        return xml.to_owned();
    };

    let bytes = xml.as_bytes();
    let mut index = start;
    let mut bracket_depth = 0usize;
    let mut quoted = None;

    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(active_quote) = quoted {
            if byte == active_quote {
                quoted = None;
            }
        } else {
            match byte {
                b'"' | b'\'' => quoted = Some(byte),
                b'[' => bracket_depth = bracket_depth.saturating_add(1),
                b']' => bracket_depth = bracket_depth.saturating_sub(1),
                b'>' if bracket_depth == 0 => {
                    let mut sanitized = String::with_capacity(xml.len());
                    sanitized.push_str(&xml[..start]);
                    sanitized.push_str(&xml[index + 1..]);
                    return sanitized;
                }
                _ => {}
            }
        }
        index += 1;
    }

    xml.to_owned()
}

pub fn fixture_mode_channel_count(profile: &FixtureProfile, mode_name: &str) -> usize {
    profile
        .modes
        .iter()
        .find(|mode| mode.name == mode_name)
        .map(|mode| mode.channels.len())
        .unwrap_or(0)
}

pub fn fixture_patch_channel_count(profile: &FixtureProfile, patch: &FixturePatch) -> usize {
    fixture_mode_channel_count(profile, &patch.mode_name)
}

fn parse_ofl_channels(root: &Value) -> Result<Vec<FixtureChannel>, String> {
    let Some(channels) = root.get("availableChannels").and_then(Value::as_object) else {
        return Err("OFL fixture without availableChannels".to_owned());
    };

    let mut entries = channels.iter().collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(right.0));

    let parsed = entries
        .into_iter()
        .map(|(key, value)| {
            let group = channel_group_from_ofl(value).unwrap_or_else(|| "Generic".to_owned());
            let capabilities = parse_ofl_capabilities(value);
            FixtureChannel {
                name: key.clone(),
                group,
                byte: infer_channel_byte(key),
                default_value: value
                    .get("defaultValue")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as u16,
                highlight_value: value
                    .get("highlightValue")
                    .and_then(Value::as_u64)
                    .unwrap_or(255) as u16,
                capabilities,
            }
        })
        .collect::<Vec<_>>();

    Ok(parsed)
}

fn parse_ofl_modes(root: &Value) -> Result<Vec<FixtureMode>, String> {
    let Some(modes) = root.get("modes").and_then(Value::as_array) else {
        return Err("OFL fixture without modes".to_owned());
    };

    let parsed = modes
        .iter()
        .map(|mode| {
            let name = mode
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "OFL mode without name".to_owned())?
                .to_owned();
            let short_name = mode
                .get("shortName")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned);
            let channels = mode
                .get("channels")
                .and_then(Value::as_array)
                .ok_or_else(|| format!("OFL mode {name} without channels"))?
                .iter()
                .flat_map(flatten_ofl_mode_channel)
                .collect::<Vec<_>>();

            if channels.is_empty() {
                return Err(format!("OFL mode {name} contains no usable channels"));
            }

            Ok(FixtureMode {
                name,
                short_name,
                channels,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(parsed)
}

fn parse_ofl_physical(root: &Value) -> Option<FixturePhysical> {
    let physical = root.get("physical")?;

    let dimensions = physical
        .get("dimensions")
        .and_then(Value::as_array)
        .and_then(|values| {
            (values.len() == 3).then(|| {
                [
                    values[0].as_u64().unwrap_or(0) as u16,
                    values[1].as_u64().unwrap_or(0) as u16,
                    values[2].as_u64().unwrap_or(0) as u16,
                ]
            })
        });

    let weight_kg = physical.get("weight").and_then(Value::as_f64);
    let power_watts = physical.get("power").and_then(Value::as_f64);
    let dmx_connector = physical
        .get("DMXconnector")
        .and_then(Value::as_str)
        .map(str::to_owned);

    (dimensions.is_some()
        || weight_kg.is_some()
        || power_watts.is_some()
        || dmx_connector.is_some())
    .then(|| FixturePhysical {
        dimensions_mm: dimensions,
        weight_grams: weight_kg.map(|value| (value * 1000.0).round() as u32),
        power_watts: power_watts.map(|value| value.round() as u16),
        dmx_connector,
    })
}

fn parse_ofl_capabilities(channel: &Value) -> Vec<FixtureCapability> {
    if let Some(capabilities) = channel.get("capabilities").and_then(Value::as_array) {
        return capabilities
            .iter()
            .filter_map(|capability| {
                let range = capability.get("dmxRange").and_then(Value::as_array)?;
                if range.len() != 2 {
                    return None;
                }

                Some(FixtureCapability {
                    start: range[0].as_u64().unwrap_or(0) as u16,
                    end: range[1].as_u64().unwrap_or(255) as u16,
                    label: capability_label(capability),
                })
            })
            .collect();
    }

    channel
        .get("capability")
        .map(|capability| {
            vec![FixtureCapability {
                start: 0,
                end: 255,
                label: capability_label(capability),
            }]
        })
        .unwrap_or_else(|| {
            vec![FixtureCapability {
                start: 0,
                end: 255,
                label: "Raw".to_owned(),
            }]
        })
}

fn channel_group_from_ofl(channel: &Value) -> Option<String> {
    let capability = channel.get("capability").or_else(|| {
        channel
            .get("capabilities")
            .and_then(Value::as_array)
            .and_then(|caps| caps.first())
    })?;

    let kind = capability.get("type").and_then(Value::as_str)?;
    Some(
        match kind {
            "ColorIntensity" | "ColorPreset" | "ColorTemperature" => "Colour",
            "Intensity" | "ShutterStrobe" => "Intensity",
            "Pan" => "Pan",
            "Tilt" => "Tilt",
            "Gobo" | "GoboRotation" | "GoboShake" => "Gobo",
            "Prism" | "PrismRotation" => "Prism",
            "Speed" => "Speed",
            "Effect" | "EffectSpeed" => "Effect",
            "BeamAngle" | "Focus" | "Frost" | "Zoom" => "Beam",
            "Maintenance" => "Maintenance",
            other => other,
        }
        .to_owned(),
    )
}

fn capability_label(capability: &Value) -> String {
    if let Some(label) = capability.get("comment").and_then(Value::as_str) {
        return label.trim().to_owned();
    }

    if let Some(kind) = capability.get("type").and_then(Value::as_str) {
        if let Some(color) = capability.get("color").and_then(Value::as_str) {
            return format!("{kind} {color}");
        }

        if let Some(name) = capability.get("name").and_then(Value::as_str) {
            return format!("{kind} {name}");
        }

        return kind.to_owned();
    }

    "Capability".to_owned()
}

fn flatten_ofl_mode_channel(value: &Value) -> Vec<String> {
    match value {
        Value::String(channel) => vec![channel.clone()],
        Value::Null => vec!["Unused".to_owned()],
        Value::Object(object)
            if object.get("insert").and_then(Value::as_str) == Some("matrixChannels") =>
        {
            object
                .get("templateChannels")
                .and_then(Value::as_array)
                .map(|channels| {
                    channels
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect::<Vec<_>>()
                })
                .filter(|channels| !channels.is_empty())
                .unwrap_or_else(|| vec!["Matrix Channels".to_owned()])
        }
        _ => vec!["Unsupported Channel".to_owned()],
    }
}

fn parse_qxf_channel(node: Node<'_, '_>) -> Result<FixtureChannel, String> {
    let name = node
        .attribute("Name")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "QXF channel without Name attribute".to_owned())?
        .to_owned();
    let group_node = node
        .children()
        .find(|child| child.is_element() && child.tag_name().name() == "Group");
    let group = group_node
        .and_then(|group| text_content(group))
        .unwrap_or_else(|| "Generic".to_owned());
    let byte = group_node
        .and_then(|group| group.attribute("Byte"))
        .and_then(|value| value.parse::<u8>().ok())
        .unwrap_or_else(|| infer_channel_byte(&name));
    let default_value = node
        .children()
        .find(|child| child.is_element() && child.tag_name().name() == "Default")
        .and_then(text_content)
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(0);
    let highlight_value = node
        .children()
        .find(|child| child.is_element() && child.tag_name().name() == "Highlight")
        .and_then(text_content)
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(255);
    let capabilities = node
        .children()
        .filter(|child| child.is_element() && child.tag_name().name() == "Capability")
        .map(|capability| FixtureCapability {
            start: capability
                .attribute("Min")
                .and_then(|value| value.parse::<u16>().ok())
                .unwrap_or(0),
            end: capability
                .attribute("Max")
                .and_then(|value| value.parse::<u16>().ok())
                .unwrap_or(255),
            label: text_content(capability).unwrap_or_else(|| "Capability".to_owned()),
        })
        .collect::<Vec<_>>();

    Ok(FixtureChannel {
        name,
        group,
        byte,
        default_value,
        highlight_value,
        capabilities: if capabilities.is_empty() {
            vec![FixtureCapability {
                start: 0,
                end: 255,
                label: "Raw".to_owned(),
            }]
        } else {
            capabilities
        },
    })
}

fn parse_qxf_mode(node: Node<'_, '_>) -> Result<FixtureMode, String> {
    let name = node
        .attribute("Name")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "QXF mode without Name attribute".to_owned())?
        .to_owned();
    let channels = node
        .children()
        .filter(|child| child.is_element() && child.tag_name().name() == "Channel")
        .filter_map(text_content)
        .collect::<Vec<_>>();

    if channels.is_empty() {
        return Err(format!("QXF mode {name} without channel bindings"));
    }

    Ok(FixtureMode {
        name,
        short_name: None,
        channels,
    })
}

fn child_text(node: Node<'_, '_>, tag_name: &str) -> Option<String> {
    node.children()
        .find(|child| child.is_element() && child.tag_name().name() == tag_name)
        .and_then(text_content)
}

fn text_content(node: Node<'_, '_>) -> Option<String> {
    let text = node.text()?.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

fn qxf_categories(qxf_type: &str) -> Vec<String> {
    match qxf_type {
        "Moving Head" | "Headmover" => vec!["Moving Head".to_owned()],
        "Scanner" => vec!["Scanner".to_owned()],
        "Color Changer" | "Colour Changer" => vec!["Color Changer".to_owned()],
        "Dimmer" => vec!["Dimmer".to_owned()],
        "Laser" => vec!["Laser".to_owned()],
        "Strobe" => vec!["Strobe".to_owned()],
        "Smoke" | "Hazer" => vec!["Smoke".to_owned()],
        _ => vec!["Other".to_owned()],
    }
}

fn qxf_type_from_profile(profile: &FixtureProfile) -> String {
    if profile
        .categories
        .iter()
        .any(|category| category == "Moving Head")
    {
        return "Moving Head".to_owned();
    }
    if profile
        .categories
        .iter()
        .any(|category| category == "Scanner")
    {
        return "Scanner".to_owned();
    }
    if profile
        .categories
        .iter()
        .any(|category| category == "Dimmer")
    {
        return "Dimmer".to_owned();
    }
    if profile
        .categories
        .iter()
        .any(|category| category == "Laser")
    {
        return "Laser".to_owned();
    }
    if profile
        .categories
        .iter()
        .any(|category| category == "Strobe")
    {
        return "Strobe".to_owned();
    }
    if profile
        .categories
        .iter()
        .any(|category| category == "Color Changer")
    {
        return "Color Changer".to_owned();
    }

    "Effect".to_owned()
}

fn fixture_profile_id(manufacturer: &str, model: &str) -> String {
    slugify(&format!("{manufacturer}-{model}"))
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }

    slug.trim_matches('-').to_owned()
}

fn normalize_key(value: &str) -> String {
    slugify(value.trim())
}

fn title_case_slug(value: &str) -> String {
    value
        .split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn infer_channel_byte(name: &str) -> u8 {
    let lower = name.to_ascii_lowercase();
    if lower.contains("fine") || lower.contains("16-bit") || lower.contains("16 bit") {
        1
    } else {
        0
    }
}

fn required_str<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("missing string field {key}"))
}

fn write_tag(output: &mut String, indent_level: usize, tag: &str, value: &str) {
    let indent = "  ".repeat(indent_level);
    writeln!(output, "{indent}<{tag}>{}</{tag}>", escape_xml_text(value)).expect("write xml tag");
}

fn escape_xml_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_xml_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ofl_import_maps_basic_fixture() {
        let json = r#"{
          "$schema":"https://raw.githubusercontent.com/OpenLightingProject/open-fixture-library/master/schemas/fixture.json",
          "name":"Demo Spot 200",
          "shortName":"DS200",
          "categories":["Moving Head"],
          "meta":{"authors":["Tester"],"createDate":"2024-01-01","lastModifyDate":"2024-01-02"},
          "physical":{"dimensions":[300,420,250],"weight":8.2,"power":250,"DMXconnector":"5-pin"},
          "availableChannels":{
            "Dimmer":{"defaultValue":0,"highlightValue":255,"capability":{"type":"Intensity"}},
            "Pan":{"capability":{"type":"Pan"}},
            "Color":{"capabilities":[
              {"dmxRange":[0,15],"type":"ColorPreset","comment":"Open"},
              {"dmxRange":[16,31],"type":"ColorPreset","comment":"Red"}
            ]}
          },
          "modes":[
            {"name":"8-bit","channels":["Dimmer","Pan","Color"]}
          ]
        }"#;

        let profile = import_ofl_fixture(json, Some("demo-light"), Some("demo-spot-200"))
            .expect("import ofl");

        assert_eq!(profile.manufacturer, "Demo Light");
        assert_eq!(profile.model, "Demo Spot 200");
        assert_eq!(profile.channels.len(), 3);
        assert_eq!(profile.modes[0].channels, vec!["Dimmer", "Pan", "Color"]);
        assert_eq!(
            profile.source.ofl_url.as_deref(),
            Some("https://open-fixture-library.org/demo-light/demo-spot-200.ofl")
        );
    }

    #[test]
    fn qxf_import_and_export_roundtrip_core_fields() {
        let xml = r#"<!DOCTYPE FixtureDefinition>
<FixtureDefinition xmlns="http://www.qlcplus.org/FixtureDefinition">
  <Creator>
    <Name>QLC+</Name>
    <Version>4.12.2</Version>
    <Author>Tester</Author>
  </Creator>
  <Manufacturer>Acme</Manufacturer>
  <Model>Beam 10</Model>
  <Type>Moving Head</Type>
  <Channel Name="Dimmer">
    <Group Byte="0">Intensity</Group>
    <Default>0</Default>
    <Highlight>255</Highlight>
    <Capability Min="0" Max="255">Dimmer</Capability>
  </Channel>
  <Channel Name="Pan Fine">
    <Group Byte="1">Pan</Group>
    <Capability Min="0" Max="255">Pan fine</Capability>
  </Channel>
  <Mode Name="Standard">
    <Channel Number="0">Dimmer</Channel>
    <Channel Number="1">Pan Fine</Channel>
  </Mode>
</FixtureDefinition>
"#;

        let profile = import_qxf_fixture(xml, Some("/tmp/demo.qxf")).expect("import qxf");
        assert_eq!(profile.manufacturer, "Acme");
        assert_eq!(profile.channels[1].byte, 1);
        assert_eq!(profile.modes[0].name, "Standard");

        let exported = export_qxf_fixture(&profile).expect("export qxf");
        assert!(exported.contains("<Manufacturer>Acme</Manufacturer>"));
        assert!(exported.contains("<Mode Name=\"Standard\">"));
        assert!(exported.contains("<Channel Name=\"Pan Fine\">"));

        let roundtrip =
            import_qxf_fixture(&exported, Some("/tmp/roundtrip.qxf")).expect("import exported qxf");
        assert_eq!(roundtrip.manufacturer, profile.manufacturer);
        assert_eq!(roundtrip.model, profile.model);
        assert_eq!(roundtrip.modes[0].channels, profile.modes[0].channels);
    }
}
