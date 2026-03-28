use crate::core::history::{capture_history_snapshot, restore_history_snapshot};
use crate::core::state::{
    HistorySnapshot, RecoverySlotDescriptor, ReplayLogState, StudioState, TransportState,
    VentureDescriptor,
};
use crate::core::validation::{recover_state, validate_state};
use crate::core::{AppEvent, replay_events};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const PROJECT_FILE_VERSION: u16 = 1;
const REPLAY_LOG_FILE_VERSION: u16 = 1;
const VENTURE_FILE_VERSION: u16 = 1;
const RECOVERY_FILE_VERSION: u16 = 1;
const VENTURE_FILE_SUFFIX: &str = ".venture.json";
const RECOVERY_FILE_SUFFIX: &str = ".recovery.json";
const RECOVERY_DIRECTORY_NAME: &str = ".recovery";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectFile {
    pub version: u16,
    pub transport: TransportState,
    pub snapshot: HistorySnapshot,
    pub replay_log: ReplayLogState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayLogFile {
    pub version: u16,
    pub events: Vec<AppEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VentureFile {
    pub version: u16,
    pub venture_id: String,
    pub venture_name: String,
    pub project: ProjectFile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct VentureRegistry {
    pub ventures: Vec<VentureDescriptor>,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoverySlotFile {
    pub version: u16,
    pub slot_id: String,
    pub label: String,
    pub source_venture_id: Option<String>,
    pub source_venture_name: Option<String>,
    pub project: ProjectFile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RecoveryRegistry {
    pub slots: Vec<RecoverySlotDescriptor>,
    pub issues: Vec<String>,
}

pub fn export_project_json(state: &StudioState) -> String {
    serde_json::to_string_pretty(&ProjectFile {
        version: PROJECT_FILE_VERSION,
        transport: state.engine.transport.clone(),
        snapshot: capture_history_snapshot(state),
        replay_log: state.replay_log.clone(),
    })
    .expect("serialize project")
}

pub fn import_project_json(json: &str) -> Result<StudioState, String> {
    let file: ProjectFile = serde_json::from_str(json).map_err(|err| err.to_string())?;
    let mut state = StudioState::default();
    state.engine.transport = file.transport;
    restore_history_snapshot(&mut state, &file.snapshot);
    state.replay_log = file.replay_log;
    state.history = Default::default();
    state.event_queue = Default::default();
    state.status.hint = "Projekt geladen".to_owned();

    let report = validate_state(&state);
    if report.valid {
        return Ok(state);
    }

    let recovered = recover_state(&mut state, &report);
    if recovered.valid {
        Ok(state)
    } else {
        Err(recovered
            .issues
            .first()
            .map(|issue| issue.detail.clone())
            .unwrap_or_else(|| "Projektimport fehlgeschlagen".to_owned()))
    }
}

pub fn export_replay_log_json(state: &StudioState) -> String {
    serde_json::to_string_pretty(&ReplayLogFile {
        version: REPLAY_LOG_FILE_VERSION,
        events: state.replay_log.events.clone(),
    })
    .expect("serialize replay log")
}

pub fn replay_from_log_json(json: &str) -> Result<StudioState, String> {
    let file: ReplayLogFile = serde_json::from_str(json).map_err(|err| err.to_string())?;
    Ok(replay_events(&file.events))
}

pub fn ensure_venture_directory<P: AsRef<Path>>(directory: P) -> Result<PathBuf, String> {
    let path = directory.as_ref();
    fs::create_dir_all(path).map_err(|err| {
        format!(
            "Venture-Verzeichnis {} konnte nicht angelegt werden: {}",
            path.display(),
            err
        )
    })?;
    Ok(path.to_path_buf())
}

pub fn load_venture_registry<P: AsRef<Path>>(directory: P) -> Result<VentureRegistry, String> {
    let directory = ensure_venture_directory(directory)?;
    let mut ventures = Vec::new();
    let mut issues = Vec::new();
    let mut paths = fs::read_dir(&directory)
        .map_err(|err| {
            format!(
                "Venture-Verzeichnis {} konnte nicht gelesen werden: {}",
                directory.display(),
                err
            )
        })?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(|filename| filename.ends_with(VENTURE_FILE_SUFFIX))
                    .unwrap_or(false)
        })
        .collect::<Vec<_>>();

    paths.sort_by(|left, right| {
        left.file_name()
            .and_then(|name| name.to_str())
            .cmp(&right.file_name().and_then(|name| name.to_str()))
    });

    for path in paths {
        match descriptor_from_path(&path) {
            Ok(descriptor) => ventures.push(descriptor),
            Err(error) => issues.push(error),
        }
    }

    ventures.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.id.cmp(&right.id))
    });
    issues.sort();

    Ok(VentureRegistry { ventures, issues })
}

pub fn load_recovery_registry<P: AsRef<Path>>(directory: P) -> Result<RecoveryRegistry, String> {
    let recovery_directory = ensure_recovery_directory(directory)?;
    let mut slots = Vec::new();
    let mut issues = Vec::new();
    let mut paths = fs::read_dir(&recovery_directory)
        .map_err(|err| {
            format!(
                "Recovery-Verzeichnis {} konnte nicht gelesen werden: {}",
                recovery_directory.display(),
                err
            )
        })?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(|filename| filename.ends_with(RECOVERY_FILE_SUFFIX))
                    .unwrap_or(false)
        })
        .collect::<Vec<_>>();

    paths.sort_by(|left, right| {
        left.file_name()
            .and_then(|name| name.to_str())
            .cmp(&right.file_name().and_then(|name| name.to_str()))
    });

    for path in paths {
        match recovery_descriptor_from_path(&path) {
            Ok(slot) => slots.push(slot),
            Err(error) => issues.push(error),
        }
    }

    slots.sort_by(|left, right| {
        left.id
            .cmp(&right.id)
            .then_with(|| left.label.to_lowercase().cmp(&right.label.to_lowercase()))
    });
    issues.sort();

    Ok(RecoveryRegistry { slots, issues })
}

pub fn list_ventures<P: AsRef<Path>>(directory: P) -> Result<Vec<VentureDescriptor>, String> {
    Ok(load_venture_registry(directory)?.ventures)
}

pub fn next_venture_name(existing: &[VentureDescriptor]) -> String {
    for index in 1..=999 {
        let candidate = format!("Venture {:02}", index);
        if existing
            .iter()
            .all(|venture| !venture.name.eq_ignore_ascii_case(&candidate))
        {
            return candidate;
        }
    }

    format!("Venture {}", existing.len().saturating_add(1))
}

pub fn save_venture<P: AsRef<Path>>(
    state: &StudioState,
    directory: P,
    selected_id: Option<&str>,
    draft_name: &str,
) -> Result<VentureDescriptor, String> {
    save_venture_internal(state, directory, selected_id, draft_name, false)
}

pub fn save_venture_as<P: AsRef<Path>>(
    state: &StudioState,
    directory: P,
    draft_name: &str,
) -> Result<VentureDescriptor, String> {
    save_venture_internal(state, directory, None, draft_name, true)
}

pub fn rename_venture<P: AsRef<Path>>(
    state: &StudioState,
    directory: P,
    selected_id: &str,
    draft_name: &str,
) -> Result<VentureDescriptor, String> {
    save_venture_internal(state, directory, Some(selected_id), draft_name, false)
}

fn save_venture_internal<P: AsRef<Path>>(
    state: &StudioState,
    directory: P,
    selected_id: Option<&str>,
    draft_name: &str,
    force_new_id: bool,
) -> Result<VentureDescriptor, String> {
    let directory = ensure_venture_directory(directory)?;
    let existing = load_venture_registry(&directory)?.ventures;
    let venture_name = normalized_venture_name(draft_name);

    if venture_name.is_empty() {
        return Err("Venture-Name darf nicht leer sein.".to_owned());
    }

    let venture_id = if force_new_id {
        unique_venture_id(&existing, &slugify_venture_name(&venture_name))
    } else {
        selected_id
            .filter(|id| existing.iter().any(|venture| venture.id == **id))
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| unique_venture_id(&existing, &slugify_venture_name(&venture_name)))
    };
    let filename = venture_filename(&venture_id);
    let file = VentureFile {
        version: VENTURE_FILE_VERSION,
        venture_id: venture_id.clone(),
        venture_name,
        project: ProjectFile {
            version: PROJECT_FILE_VERSION,
            transport: state.engine.transport.clone(),
            snapshot: capture_history_snapshot(state),
            replay_log: state.replay_log.clone(),
        },
    };
    let json = serde_json::to_vec_pretty(&file).map_err(|err| err.to_string())?;
    let path = directory.join(filename);
    fs::write(&path, json).map_err(|err| {
        format!(
            "Venture-Datei {} konnte nicht geschrieben werden: {}",
            path.display(),
            err
        )
    })?;

    descriptor_from_path(&path)
}

pub fn load_venture<P: AsRef<Path>>(
    directory: P,
    venture_id: &str,
) -> Result<(StudioState, VentureDescriptor), String> {
    let directory = ensure_venture_directory(directory)?;
    let descriptor = load_venture_registry(&directory)?
        .ventures
        .into_iter()
        .find(|venture| venture.id == venture_id)
        .ok_or_else(|| format!("Venture {} existiert nicht.", venture_id))?;
    let path = directory.join(&descriptor.filename);
    let json = fs::read_to_string(&path).map_err(|err| {
        format!(
            "Venture-Datei {} konnte nicht gelesen werden: {}",
            path.display(),
            err
        )
    })?;
    let file: VentureFile = serde_json::from_str(&json).map_err(|err| err.to_string())?;
    let project_json = serde_json::to_string(&file.project).map_err(|err| err.to_string())?;
    let state = import_project_json(&project_json)?;

    Ok((state, descriptor))
}

pub fn delete_venture<P: AsRef<Path>>(directory: P, venture_id: &str) -> Result<(), String> {
    let directory = ensure_venture_directory(directory)?;
    let descriptor = load_venture_registry(&directory)?
        .ventures
        .into_iter()
        .find(|venture| venture.id == venture_id)
        .ok_or_else(|| format!("Venture {} existiert nicht.", venture_id))?;
    let path = directory.join(descriptor.filename);
    fs::remove_file(&path).map_err(|err| {
        format!(
            "Venture-Datei {} konnte nicht gelöscht werden: {}",
            path.display(),
            err
        )
    })
}

pub fn save_recovery_slot<P: AsRef<Path>>(
    state: &StudioState,
    directory: P,
    label: &str,
    capacity: usize,
) -> Result<RecoverySlotDescriptor, String> {
    let venture_directory = ensure_venture_directory(directory)?;
    let recovery_directory = ensure_recovery_directory(&venture_directory)?;
    let existing = load_recovery_registry(&venture_directory)?.slots;
    let slot_sequence = next_recovery_sequence(&existing);
    let slot_id = format!("recovery-{:06}", slot_sequence);
    let normalized_label = normalized_venture_name(label);
    let label = if normalized_label.is_empty() {
        "Autosave".to_owned()
    } else {
        normalized_label
    };
    let filename = format!(
        "{}-{}{}",
        slot_id,
        slugify_venture_name(&label),
        RECOVERY_FILE_SUFFIX
    );
    let file = RecoverySlotFile {
        version: RECOVERY_FILE_VERSION,
        slot_id: slot_id.clone(),
        label,
        source_venture_id: state.venture.selected.clone(),
        source_venture_name: state.selected_venture().map(|venture| venture.name.clone()),
        project: ProjectFile {
            version: PROJECT_FILE_VERSION,
            transport: state.engine.transport.clone(),
            snapshot: capture_history_snapshot(state),
            replay_log: state.replay_log.clone(),
        },
    };
    let json = serde_json::to_vec_pretty(&file).map_err(|err| err.to_string())?;
    let path = recovery_directory.join(filename);
    fs::write(&path, json).map_err(|err| {
        format!(
            "Recovery-Slot {} konnte nicht geschrieben werden: {}",
            path.display(),
            err
        )
    })?;

    let descriptor = recovery_descriptor_from_path(&path)?;
    prune_recovery_slots(&recovery_directory, capacity)?;
    Ok(descriptor)
}

pub fn restore_recovery_slot<P: AsRef<Path>>(
    directory: P,
    slot_id: &str,
) -> Result<(StudioState, RecoverySlotDescriptor), String> {
    let venture_directory = ensure_venture_directory(directory)?;
    let recovery_directory = ensure_recovery_directory(&venture_directory)?;
    let descriptor = load_recovery_registry(&venture_directory)?
        .slots
        .into_iter()
        .find(|slot| slot.id == slot_id)
        .ok_or_else(|| format!("Recovery-Slot {} existiert nicht.", slot_id))?;
    let path = recovery_directory.join(&descriptor.filename);
    let json = fs::read_to_string(&path).map_err(|err| {
        format!(
            "Recovery-Slot {} konnte nicht gelesen werden: {}",
            path.display(),
            err
        )
    })?;
    let file: RecoverySlotFile = serde_json::from_str(&json).map_err(|err| err.to_string())?;
    let project_json = serde_json::to_string(&file.project).map_err(|err| err.to_string())?;
    let state = import_project_json(&project_json)?;

    Ok((state, descriptor))
}

fn descriptor_from_path(path: &Path) -> Result<VentureDescriptor, String> {
    let json = fs::read_to_string(path).map_err(|err| {
        format!(
            "Venture-Datei {} konnte nicht gelesen werden: {}",
            path.display(),
            err
        )
    })?;
    let file: VentureFile = serde_json::from_str(&json).map_err(|err| {
        format!(
            "Venture-Datei {} ist beschädigt oder inkompatibel: {}",
            path.display(),
            err
        )
    })?;
    let metadata = fs::metadata(path).map_err(|err| {
        format!(
            "Venture-Metadaten {} konnten nicht gelesen werden: {}",
            path.display(),
            err
        )
    })?;

    Ok(VentureDescriptor {
        id: file.venture_id,
        name: file.venture_name,
        filename: path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned(),
        updated_at_unix_ms: metadata_modified_unix_ms(&metadata),
    })
}

fn recovery_descriptor_from_path(path: &Path) -> Result<RecoverySlotDescriptor, String> {
    let json = fs::read_to_string(path).map_err(|err| {
        format!(
            "Recovery-Slot {} konnte nicht gelesen werden: {}",
            path.display(),
            err
        )
    })?;
    let file: RecoverySlotFile = serde_json::from_str(&json).map_err(|err| {
        format!(
            "Recovery-Slot {} ist beschädigt oder inkompatibel: {}",
            path.display(),
            err
        )
    })?;
    let metadata = fs::metadata(path).map_err(|err| {
        format!(
            "Recovery-Metadaten {} konnten nicht gelesen werden: {}",
            path.display(),
            err
        )
    })?;

    Ok(RecoverySlotDescriptor {
        id: file.slot_id,
        label: file.label,
        filename: path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned(),
        source_venture_id: file.source_venture_id,
        updated_at_unix_ms: metadata_modified_unix_ms(&metadata),
    })
}

fn metadata_modified_unix_ms(metadata: &fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn venture_filename(venture_id: &str) -> String {
    format!("{}{}", venture_id, VENTURE_FILE_SUFFIX)
}

fn ensure_recovery_directory<P: AsRef<Path>>(directory: P) -> Result<PathBuf, String> {
    let venture_directory = ensure_venture_directory(directory)?;
    let recovery_directory = venture_directory.join(RECOVERY_DIRECTORY_NAME);
    fs::create_dir_all(&recovery_directory).map_err(|err| {
        format!(
            "Recovery-Verzeichnis {} konnte nicht angelegt werden: {}",
            recovery_directory.display(),
            err
        )
    })?;
    Ok(recovery_directory)
}

fn normalized_venture_name(name: &str) -> String {
    name.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn slugify_venture_name(name: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for character in name.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            slug.push(character);
            last_was_dash = false;
        } else if !slug.is_empty() && !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    if slug.is_empty() {
        "venture".to_owned()
    } else {
        slug
    }
}

fn unique_venture_id(existing: &[VentureDescriptor], base_slug: &str) -> String {
    for index in 1..=u16::MAX {
        let candidate = if index == 1 {
            base_slug.to_owned()
        } else {
            format!("{}-{:02}", base_slug, index)
        };
        if existing.iter().all(|venture| venture.id != candidate) {
            return candidate;
        }
    }

    format!("{}-overflow", base_slug)
}

fn next_recovery_sequence(existing: &[RecoverySlotDescriptor]) -> u64 {
    existing
        .iter()
        .filter_map(|slot| slot.id.strip_prefix("recovery-"))
        .filter_map(|tail| tail.parse::<u64>().ok())
        .max()
        .unwrap_or(0)
        .saturating_add(1)
}

fn prune_recovery_slots(recovery_directory: &Path, capacity: usize) -> Result<(), String> {
    let mut paths = fs::read_dir(recovery_directory)
        .map_err(|err| {
            format!(
                "Recovery-Verzeichnis {} konnte nicht gelesen werden: {}",
                recovery_directory.display(),
                err
            )
        })?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(|filename| filename.ends_with(RECOVERY_FILE_SUFFIX))
                    .unwrap_or(false)
        })
        .collect::<Vec<_>>();

    paths.sort_by(|left, right| {
        left.file_name()
            .and_then(|name| name.to_str())
            .cmp(&right.file_name().and_then(|name| name.to_str()))
    });

    if paths.len() <= capacity {
        return Ok(());
    }

    let overflow = paths.len() - capacity;
    for path in paths.into_iter().take(overflow) {
        fs::remove_file(&path).map_err(|err| {
            format!(
                "Recovery-Slot {} konnte nicht entfernt werden: {}",
                path.display(),
                err
            )
        })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{AppEvent, ClipId, CueId};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn project_roundtrip_restores_authoring_state() {
        let state = replay_events(&[
            AppEvent::TriggerCue(CueId(2)),
            AppEvent::DuplicateSelectedClips,
        ]);
        let json = export_project_json(&state);
        let restored = import_project_json(&json).expect("import project");

        assert_eq!(
            restored.clip(ClipId(102)).expect("clip exists").start,
            state.clip(ClipId(102)).expect("clip exists").start
        );
        assert_eq!(
            restored.timeline.selected_clips,
            state.timeline.selected_clips
        );
    }

    #[test]
    fn replay_log_roundtrip_replays_deterministically() {
        let state = replay_events(&[AppEvent::DeleteSelectedClips, AppEvent::Undo]);
        let json = export_replay_log_json(&state);
        let replayed = replay_from_log_json(&json).expect("replay log");

        assert_eq!(
            serde_json::to_string(&state).expect("serialize left"),
            serde_json::to_string(&replayed).expect("serialize right")
        );
    }

    #[test]
    fn save_and_load_venture_roundtrip_restores_state() {
        let directory = temp_venture_dir("roundtrip");
        let state = replay_events(&[
            AppEvent::TriggerCue(CueId(2)),
            AppEvent::DuplicateSelectedClips,
            AppEvent::SetMasterIntensity(915),
        ]);

        let saved = save_venture(&state, &directory, None, "Main Stage")
            .expect("save venture to temp directory");
        let (loaded, descriptor) =
            load_venture(&directory, &saved.id).expect("load venture from temp directory");

        assert_eq!(descriptor.name, "Main Stage");
        assert_eq!(
            loaded.clip(ClipId(102)).expect("clip exists").start,
            state.clip(ClipId(102)).expect("clip exists").start
        );
        assert_eq!(loaded.master.intensity, state.master.intensity);

        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn list_ventures_sorts_by_name_deterministically() {
        let directory = temp_venture_dir("sorting");
        let state = StudioState::default();

        save_venture(&state, &directory, None, "Beta").expect("save beta");
        save_venture(&state, &directory, None, "Alpha").expect("save alpha");

        let ventures = list_ventures(&directory).expect("list ventures");

        assert_eq!(ventures.len(), 2);
        assert_eq!(ventures[0].name, "Alpha");
        assert_eq!(ventures[1].name, "Beta");

        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn save_as_creates_distinct_venture_id() {
        let directory = temp_venture_dir("save-as");
        let state = StudioState::default();

        let original = save_venture(&state, &directory, None, "Festival").expect("save original");
        let copied = save_venture_as(&state, &directory, "Festival").expect("save as copy");

        assert_ne!(original.id, copied.id);
        assert_eq!(copied.name, "Festival");
        assert_eq!(list_ventures(&directory).expect("ventures").len(), 2);

        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn delete_venture_removes_persisted_file() {
        let directory = temp_venture_dir("delete");
        let state = StudioState::default();

        let saved = save_venture(&state, &directory, None, "Disposable").expect("save venture");
        delete_venture(&directory, &saved.id).expect("delete venture");

        assert!(list_ventures(&directory).expect("ventures").is_empty());

        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn registry_skips_invalid_venture_files_and_reports_issues() {
        let directory = temp_venture_dir("registry-issues");
        let state = StudioState::default();

        save_venture(&state, &directory, None, "Valid").expect("save valid venture");
        ensure_venture_directory(&directory).expect("ensure directory exists");
        fs::write(
            directory.join("broken.venture.json"),
            br#"{ "version": 1, "venture_id": "broken" "#,
        )
        .expect("write invalid venture");

        let registry = load_venture_registry(&directory).expect("scan registry");

        assert_eq!(registry.ventures.len(), 1);
        assert_eq!(registry.ventures[0].name, "Valid");
        assert_eq!(registry.issues.len(), 1);
        assert!(
            registry.issues[0].contains("beschädigt") || registry.issues[0].contains("beschadigt")
        );

        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn save_and_restore_recovery_slot_roundtrip_restores_state() {
        let directory = temp_venture_dir("recovery-roundtrip");
        let state = replay_events(&[
            AppEvent::DuplicateSelectedClips,
            AppEvent::SetMasterIntensity(905),
        ]);

        let saved = save_recovery_slot(&state, &directory, "Autosave Duplicate", 8)
            .expect("save recovery slot");
        let (restored, descriptor) =
            restore_recovery_slot(&directory, &saved.id).expect("restore recovery slot");

        assert_eq!(descriptor.label, "Autosave Duplicate");
        assert_eq!(restored.master.intensity, state.master.intensity);
        assert_eq!(
            serde_json::to_string(&restored.timeline.tracks).expect("serialize restored"),
            serde_json::to_string(&state.timeline.tracks).expect("serialize source")
        );

        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn recovery_slot_pruning_keeps_latest_capacity() {
        let directory = temp_venture_dir("recovery-capacity");
        let state = StudioState::default();

        save_recovery_slot(&state, &directory, "One", 2).expect("slot one");
        save_recovery_slot(&state, &directory, "Two", 2).expect("slot two");
        save_recovery_slot(&state, &directory, "Three", 2).expect("slot three");

        let registry = load_recovery_registry(&directory).expect("load recovery registry");

        assert_eq!(registry.slots.len(), 2);
        assert_eq!(registry.slots[0].label, "Two");
        assert_eq!(registry.slots[1].label, "Three");

        let _ = fs::remove_dir_all(directory);
    }

    fn temp_venture_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "luma-switch-venture-{}-{}",
            label,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("unix time")
                .as_nanos()
        ))
    }
}
