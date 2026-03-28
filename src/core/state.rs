use crate::core::event::{AppEvent, StateDiff};
use crate::core::fixtures::fixture_mode_channel_count;
use crate::core::ids::{ChaseId, ClipId, CueId, FixtureGroupId, FxId, TrackId};
use crate::core::queue::EventQueueState;
use crate::core::time::{
    BAR_BEATS, BeatTime, IntensityLevel, MonotonicClock, PPQ, RgbaColor, SpeedRatio, TempoBpm,
    ZoomFactor,
};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const MIN_CLIP_DURATION: BeatTime = BeatTime::from_fraction(1, 4);
pub const TIMELINE_HEADER_HEIGHT_PX: i32 = 38;
pub const TIMELINE_TRACK_HEIGHT_PX: i32 = 84;
pub const TIMELINE_TRACK_GAP_PX: i32 = 12;
pub const TIMELINE_CLIP_HEIGHT_PX: i32 = 52;
pub const TIMELINE_CLIP_TOP_INSET_PX: i32 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StateLifecycle {
    Initializing,
    Valid,
    Updating,
    Invalid,
    Recovered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnginePhase {
    Stopped,
    Running,
    Paused,
    Syncing,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineResumeTarget {
    Stopped,
    Running,
    Paused,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineErrorState {
    pub code: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineState {
    pub phase: EnginePhase,
    pub resume_target: EngineResumeTarget,
    pub clock: MonotonicClock,
    pub transport: TransportState,
    pub error: Option<EngineErrorState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportState {
    pub bpm: TempoBpm,
    pub playhead: BeatTime,
    pub song_length: BeatTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MasterState {
    pub intensity: IntensityLevel,
    pub speed: SpeedRatio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimelinePhase {
    Idle,
    Dragging,
    Zooming,
    Snapping,
    Rendering,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineState {
    pub phase: TimelinePhase,
    pub viewport: TimelineViewport,
    pub snap: SnapState,
    pub tracks: Vec<Track>,
    pub selection: SelectionState,
    pub selected_clips: Vec<ClipId>,
    pub hover: Option<HoverTarget>,
    pub interaction: TimelineInteraction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineViewport {
    pub zoom: ZoomFactor,
    pub scroll: BeatTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapPhase {
    Free,
    Snapping,
    Locked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapState {
    pub phase: SnapPhase,
    pub enabled: bool,
    pub resolution: SnapResolution,
    pub guide: Option<SnapGuide>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapResolution {
    Beat,
    HalfBeat,
    QuarterBeat,
    EighthBeat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapGuide {
    pub beat: BeatTime,
    pub track: Option<TrackId>,
    pub strength_permille: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    pub id: TrackId,
    pub name: String,
    pub color: RgbaColor,
    pub muted: bool,
    pub solo: bool,
    pub clips: Vec<Clip>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClipPhase {
    Inactive,
    Active,
    Triggered,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clip {
    pub id: ClipId,
    pub title: String,
    pub phase: ClipPhase,
    pub start: BeatTime,
    pub duration: BeatTime,
    pub params: ClipParameters,
    pub automation: Vec<AutomationLane>,
    pub palette: ClipPalette,
    pub markers: Vec<ClipMarker>,
    pub linked_cue: Option<CueId>,
    pub cue_state: CueVisualState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipParameters {
    pub intensity: IntensityLevel,
    pub speed: SpeedRatio,
    pub fx_depth: IntensityLevel,
    pub bpm_grid: SnapResolution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClipInlineParameterKind {
    Intensity,
    Speed,
    FxDepth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AutomationTarget {
    Intensity,
    Speed,
    FxDepth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutomationInterpolation {
    Step,
    Linear,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationPoint {
    pub offset: BeatTime,
    pub value: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationLane {
    pub target: AutomationTarget,
    pub interpolation: AutomationInterpolation,
    pub enabled: bool,
    pub points: Vec<AutomationPoint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipMarker {
    pub label: String,
    pub offset: BeatTime,
    pub color: RgbaColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipPalette {
    pub base: RgbaColor,
    pub highlight: RgbaColor,
    pub edge: RgbaColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CueVisualState {
    Active,
    Ready,
    Inactive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CuePhase {
    Stored,
    Armed,
    Triggered,
    Fading,
    Active,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cue {
    pub id: CueId,
    pub name: String,
    pub phase: CuePhase,
    pub linked_clip: Option<ClipId>,
    pub color: RgbaColor,
    pub fade_duration: BeatTime,
    pub elapsed: BeatTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CueSystemState {
    pub selected: Option<CueId>,
    pub active: Option<CueId>,
    pub cues: Vec<Cue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChasePhase {
    Idle,
    Playing,
    Looping,
    Reversing,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChaseDirection {
    Forward,
    Reverse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChaseStep {
    pub label: String,
    pub cue_id: Option<CueId>,
    pub duration: BeatTime,
    pub color: RgbaColor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chase {
    pub id: ChaseId,
    pub name: String,
    pub phase: ChasePhase,
    pub direction: ChaseDirection,
    pub current_step: usize,
    pub progress: BeatTime,
    pub loop_enabled: bool,
    pub linked_clip: Option<ClipId>,
    pub steps: Vec<ChaseStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChaseSystemState {
    pub selected: Option<ChaseId>,
    pub selected_step: Option<usize>,
    pub chases: Vec<Chase>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FxPhase {
    Idle,
    Processing,
    Applied,
    Composed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FxKind {
    Color,
    Intensity,
    Position,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FxWaveform {
    Sine,
    Triangle,
    Saw,
    Pulse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FxLayer {
    pub id: FxId,
    pub name: String,
    pub phase: FxPhase,
    pub kind: FxKind,
    pub linked_clip: Option<ClipId>,
    pub enabled: bool,
    pub depth_permille: u16,
    pub rate: SpeedRatio,
    pub spread_permille: u16,
    pub phase_offset_permille: u16,
    pub waveform: FxWaveform,
    pub bpm_sync: SnapResolution,
    pub output_level: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FxSystemState {
    pub selected: Option<FxId>,
    pub layers: Vec<FxLayer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FixturePhase {
    Uninitialized,
    Mapped,
    Active,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureGroup {
    pub id: FixtureGroupId,
    pub name: String,
    pub phase: FixturePhase,
    pub fixture_count: u16,
    pub online: u16,
    pub linked_cue: Option<CueId>,
    pub linked_fx: Option<FxId>,
    pub accent: RgbaColor,
    pub output_level: u16,
    pub preview_nodes: Vec<FixturePreviewNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixturePreviewNode {
    pub label: String,
    pub x_permille: u16,
    pub y_permille: u16,
    pub z_permille: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FixtureLibraryPhase {
    Idle,
    Importing,
    Exporting,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FixtureSourceKind {
    Demo,
    OpenFixtureLibrary,
    Qxf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureSourceInfo {
    pub kind: FixtureSourceKind,
    pub manufacturer_key: Option<String>,
    pub fixture_key: Option<String>,
    pub source_path: Option<String>,
    pub ofl_url: Option<String>,
    pub creator_name: Option<String>,
    pub creator_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixturePhysical {
    pub dimensions_mm: Option<[u16; 3]>,
    pub weight_grams: Option<u32>,
    pub power_watts: Option<u16>,
    pub dmx_connector: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureCapability {
    pub start: u16,
    pub end: u16,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureChannel {
    pub name: String,
    pub group: String,
    pub byte: u8,
    pub default_value: u16,
    pub highlight_value: u16,
    pub capabilities: Vec<FixtureCapability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureMode {
    pub name: String,
    pub short_name: Option<String>,
    pub channels: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureProfile {
    pub id: String,
    pub manufacturer: String,
    pub model: String,
    pub short_name: String,
    pub categories: Vec<String>,
    pub physical: Option<FixturePhysical>,
    pub channels: Vec<FixtureChannel>,
    pub modes: Vec<FixtureMode>,
    pub source: FixtureSourceInfo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixturePatch {
    pub id: u32,
    pub profile_id: String,
    pub name: String,
    pub mode_name: String,
    pub universe: u16,
    pub address: u16,
    pub group_id: Option<FixtureGroupId>,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureLibraryState {
    pub phase: FixtureLibraryPhase,
    pub selected_profile: Option<String>,
    pub selected_patch: Option<u32>,
    pub profiles: Vec<FixtureProfile>,
    pub patches: Vec<FixturePatch>,
    pub ofl_manufacturer_key: String,
    pub ofl_fixture_key: String,
    pub qxf_import_path: String,
    pub qxf_export_path: String,
    pub last_summary: Option<String>,
    pub last_error: Option<String>,
}

impl Default for FixtureLibraryState {
    fn default() -> Self {
        Self {
            phase: FixtureLibraryPhase::Idle,
            selected_profile: None,
            selected_patch: None,
            profiles: Vec::new(),
            patches: Vec::new(),
            ofl_manufacturer_key: String::new(),
            ofl_fixture_key: String::new(),
            qxf_import_path: String::new(),
            qxf_export_path: String::new(),
            last_summary: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureUniverseSummary {
    pub universe: u16,
    pub patch_count: usize,
    pub enabled_patch_count: usize,
    pub footprint_channels: u16,
    pub occupied_channels: u16,
    pub highest_address: u16,
    pub conflicting_patch_ids: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureGroupPatchSummary {
    pub group_id: FixtureGroupId,
    pub patch_count: usize,
    pub enabled_patch_count: usize,
    pub footprint_channels: u16,
    pub occupied_channels: u16,
    pub universes: Vec<u16>,
    pub conflicting_patch_ids: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureSystemState {
    pub selected: Option<FixtureGroupId>,
    pub groups: Vec<FixtureGroup>,
    pub library: FixtureLibraryState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClipEditorPhase {
    Closed,
    Open,
    Adjusting,
    Previewing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipEditorState {
    pub phase: ClipEditorPhase,
    pub clip_id: Option<ClipId>,
    pub automation_target: AutomationTarget,
    pub selected_automation_point: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectionState {
    None,
    Clip(ClipId),
    Track(TrackId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HoverTarget {
    Playhead,
    ClipBody(ClipId),
    ClipStartHandle(ClipId),
    ClipEndHandle(ClipId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimelineInteraction {
    Idle,
    PendingBoxSelection {
        origin_track: Option<TrackId>,
        origin_beat: BeatTime,
        origin_x_px: i32,
        origin_y_px: i32,
        current_x_px: i32,
        current_y_px: i32,
    },
    BoxSelecting {
        origin_x_px: i32,
        origin_y_px: i32,
        current_x_px: i32,
        current_y_px: i32,
    },
    PendingClipDrag {
        clip_id: ClipId,
        origin_track: TrackId,
        origin_start: BeatTime,
        pointer_origin: BeatTime,
        pointer_origin_x_px: i32,
        pointer_origin_y_px: i32,
    },
    DragClip {
        clip_id: ClipId,
        origin_track: TrackId,
        origin_start: BeatTime,
        pointer_origin: BeatTime,
    },
    PendingResizeClipStart {
        clip_id: ClipId,
        origin_start: BeatTime,
        origin_duration: BeatTime,
        pointer_origin: BeatTime,
        pointer_origin_x_px: i32,
        pointer_origin_y_px: i32,
    },
    ResizeClipStart {
        clip_id: ClipId,
        origin_start: BeatTime,
        origin_duration: BeatTime,
        pointer_origin: BeatTime,
    },
    PendingResizeClipEnd {
        clip_id: ClipId,
        origin_start: BeatTime,
        origin_duration: BeatTime,
        pointer_origin: BeatTime,
        pointer_origin_x_px: i32,
        pointer_origin_y_px: i32,
    },
    ResizeClipEnd {
        clip_id: ClipId,
        origin_start: BeatTime,
        origin_duration: BeatTime,
        pointer_origin: BeatTime,
    },
    AdjustClipParameter {
        clip_id: ClipId,
        parameter: ClipInlineParameterKind,
    },
    ScrubPlayhead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuLoad(pub u16);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerformanceState {
    pub frame_index: u64,
    pub fps: u16,
    pub cpu_load: CpuLoad,
    pub frame_budget_ms: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusBarState {
    pub hint: String,
    pub last_diffs: Vec<StateDiff>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct InputModifiersState {
    pub shift: bool,
    pub alt: bool,
    pub command: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextMenuTarget {
    Clip(ClipId),
    Track(TrackId),
    Timeline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextMenuAction {
    Duplicate,
    Split,
    Delete,
    Copy,
    Cut,
    Paste,
    NudgeLeft,
    NudgeRight,
    SelectAllOnTrack,
    TrimToPlayhead,
    Close,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ContextMenuState {
    pub open: bool,
    pub target: Option<ContextMenuTarget>,
    pub x_px: i32,
    pub y_px: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardClip {
    pub track_id: TrackId,
    pub relative_start: BeatTime,
    pub clip: Clip,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardState {
    pub clips: Vec<ClipboardClip>,
    pub span: BeatTime,
    pub from_cut: bool,
    pub version: u64,
    pub last_paste_anchor: Option<BeatTime>,
    pub next_paste_index: u16,
}

impl Default for ClipboardState {
    fn default() -> Self {
        Self {
            clips: Vec::new(),
            span: BeatTime::ZERO,
            from_cut: false,
            version: 0,
            last_paste_anchor: None,
            next_paste_index: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayLogState {
    pub events: Vec<AppEvent>,
    pub capacity: usize,
}

impl Default for ReplayLogState {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            capacity: 512,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HistoryPhase {
    Idle,
    Tracking,
    UndoApplied,
    RedoApplied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryTimelineSnapshot {
    pub viewport: TimelineViewport,
    pub snap_enabled: bool,
    pub snap_resolution: SnapResolution,
    pub tracks: Vec<Track>,
    pub selection: SelectionState,
    pub selected_clips: Vec<ClipId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistorySnapshot {
    pub master: MasterState,
    pub timeline: HistoryTimelineSnapshot,
    pub clip_editor: ClipEditorState,
    pub cue_system: CueSystemState,
    pub chase_system: ChaseSystemState,
    pub fx_system: FxSystemState,
    pub fixture_system: FixtureSystemState,
    pub settings: SettingsState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingHistoryEntry {
    pub label: String,
    pub before: HistorySnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub label: String,
    pub trigger: AppEvent,
    pub before: HistorySnapshot,
    pub after: HistorySnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryState {
    pub phase: HistoryPhase,
    pub pending: Option<PendingHistoryEntry>,
    pub undo_stack: Vec<HistoryEntry>,
    pub redo_stack: Vec<HistoryEntry>,
    pub capacity: usize,
}

impl Default for HistoryState {
    fn default() -> Self {
        Self {
            phase: HistoryPhase::Idle,
            pending: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            capacity: 64,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VenturePhase {
    Idle,
    Saving,
    Loading,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VentureDescriptor {
    pub id: String,
    pub name: String,
    pub filename: String,
    pub updated_at_unix_ms: u64,
}

impl fmt::Display for VentureDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoverySlotDescriptor {
    pub id: String,
    pub label: String,
    pub filename: String,
    pub source_venture_id: Option<String>,
    pub updated_at_unix_ms: u64,
}

impl fmt::Display for RecoverySlotDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct VentureAuthoringSignature {
    pub master: MasterState,
    pub tracks: Vec<Track>,
    pub cue_system: CueSystemState,
    pub chase_system: ChaseSystemState,
    pub fx_system: FxSystemState,
    pub fixture_system: FixtureSystemState,
    pub settings: SettingsState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VentureState {
    pub phase: VenturePhase,
    pub directory: String,
    pub draft_name: String,
    pub selected: Option<String>,
    pub ventures: Vec<VentureDescriptor>,
    pub recovery_slots: Vec<RecoverySlotDescriptor>,
    pub registry_issues: Vec<String>,
    pub recovery_issues: Vec<String>,
    pub selected_recovery: Option<String>,
    pub dirty: bool,
    pub autosave_enabled: bool,
    pub recovery_capacity: usize,
    pub saved_fingerprint: String,
    pub last_autosave: Option<String>,
    pub last_saved: Option<String>,
    pub last_error: Option<String>,
}

impl Default for VentureState {
    fn default() -> Self {
        Self {
            phase: VenturePhase::Idle,
            directory: "ventures".to_owned(),
            draft_name: "Venture 01".to_owned(),
            selected: None,
            ventures: Vec::new(),
            recovery_slots: Vec::new(),
            registry_issues: Vec::new(),
            recovery_issues: Vec::new(),
            selected_recovery: None,
            dirty: false,
            autosave_enabled: true,
            recovery_capacity: 8,
            saved_fingerprint: String::new(),
            last_autosave: None,
            last_saved: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SettingsTab {
    General,
    Dmx,
    Midi,
    Controllers,
    Engine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HardwareDiscoveryPhase {
    Idle,
    Refreshing,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DmxBackendKind {
    Disabled,
    EnttecOpenDmx,
    ArtNet,
    Sacn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputDeliveryPhase {
    Idle,
    Dispatching,
    Delivered,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputDispatchReport {
    pub sequence: u64,
    pub dmx_backend: DmxBackendKind,
    pub dmx_frame_count: u16,
    pub midi_message_count: u16,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputRuntimeState {
    pub phase: OutputDeliveryPhase,
    pub sequence: u64,
    pub last_backend: DmxBackendKind,
    pub last_dmx_frame_count: u16,
    pub last_midi_message_count: u16,
    pub last_summary: Option<String>,
    pub last_error: Option<String>,
}

impl Default for OutputRuntimeState {
    fn default() -> Self {
        Self {
            phase: OutputDeliveryPhase::Idle,
            sequence: 0,
            last_backend: DmxBackendKind::Disabled,
            last_dmx_frame_count: 0,
            last_midi_message_count: 0,
            last_summary: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DmxInterfaceKind {
    EnttecOpenDmxCompatible,
    UsbSerial,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DmxInterfaceDescriptor {
    pub id: String,
    pub name: String,
    pub kind: DmxInterfaceKind,
    pub port_name: String,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial_number: Option<String>,
    pub detail: String,
    pub universe_capacity: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MidiPortDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControllerProfileKind {
    Apc40Mk2,
    DenonPrime2,
    BehringerCmdDc1,
    BehringerCmdLc1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiPortDescriptor {
    pub id: String,
    pub name: String,
    pub direction: MidiPortDirection,
    pub profile_hint: Option<ControllerProfileKind>,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MidiMessageKind {
    Note,
    ControlChange,
    PitchBend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MidiControlHint {
    Button,
    Continuous,
    Any,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiBindingMessage {
    pub kind: MidiMessageKind,
    pub channel: u8,
    pub key: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiRuntimeMessage {
    pub timestamp_micros: u64,
    pub kind: MidiMessageKind,
    pub channel: u8,
    pub key: u8,
    pub value: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MidiAction {
    TransportToggle,
    MasterIntensity,
    MasterSpeed,
    TimelineZoom,
    TriggerCueSlot(u8),
    TriggerChaseSlot(u8),
    FocusFixtureGroupSlot(u8),
    FxDepthSlot(u8),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiBinding {
    pub id: u32,
    pub action: MidiAction,
    pub label: String,
    pub message: Option<MidiBindingMessage>,
    pub hint: MidiControlHint,
    pub learned: bool,
    pub controller_profile: Option<ControllerProfileKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MidiLearnPhase {
    Idle,
    Listening,
    GuidedAutomap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiLearnState {
    pub phase: MidiLearnPhase,
    pub target_binding: Option<u32>,
    pub capture_queue: Vec<u32>,
    pub expected_hint: MidiControlHint,
    pub last_captured: Option<MidiBindingMessage>,
}

impl Default for MidiLearnState {
    fn default() -> Self {
        Self {
            phase: MidiLearnPhase::Idle,
            target_binding: None,
            capture_queue: Vec::new(),
            expected_hint: MidiControlHint::Any,
            last_captured: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DmxSettingsState {
    pub phase: HardwareDiscoveryPhase,
    pub backend: DmxBackendKind,
    pub output_enabled: bool,
    pub auto_connect: bool,
    pub blackout_on_stop: bool,
    pub selected_interface: Option<String>,
    pub interfaces: Vec<DmxInterfaceDescriptor>,
    pub artnet_target: String,
    pub artnet_universe: u16,
    pub sacn_target: String,
    pub sacn_universe: u16,
    pub refresh_rate_hz: u16,
    pub enttec_break_us: u16,
    pub enttec_mark_after_break_us: u16,
    pub last_summary: Option<String>,
    pub last_error: Option<String>,
}

impl Default for DmxSettingsState {
    fn default() -> Self {
        Self {
            phase: HardwareDiscoveryPhase::Idle,
            backend: DmxBackendKind::Disabled,
            output_enabled: false,
            auto_connect: true,
            blackout_on_stop: true,
            selected_interface: None,
            interfaces: Vec::new(),
            artnet_target: "255.255.255.255:6454".to_owned(),
            artnet_universe: 1,
            sacn_target: "239.255.0.1:5568".to_owned(),
            sacn_universe: 1,
            refresh_rate_hz: 30,
            enttec_break_us: 176,
            enttec_mark_after_break_us: 16,
            last_summary: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiSettingsState {
    pub phase: HardwareDiscoveryPhase,
    pub selected_input: Option<String>,
    pub selected_output: Option<String>,
    pub inputs: Vec<MidiPortDescriptor>,
    pub outputs: Vec<MidiPortDescriptor>,
    pub feedback_enabled: bool,
    pub bindings: Vec<MidiBinding>,
    pub learn: MidiLearnState,
    pub detected_controller: Option<ControllerProfileKind>,
    pub last_message: Option<MidiRuntimeMessage>,
    pub last_summary: Option<String>,
    pub last_error: Option<String>,
}

impl Default for MidiSettingsState {
    fn default() -> Self {
        Self {
            phase: HardwareDiscoveryPhase::Idle,
            selected_input: None,
            selected_output: None,
            inputs: Vec::new(),
            outputs: Vec::new(),
            feedback_enabled: true,
            bindings: Vec::new(),
            learn: MidiLearnState::default(),
            detected_controller: None,
            last_message: None,
            last_summary: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineLinkMode {
    Disabled,
    StageLinqExperimental,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineLinkPhase {
    Disabled,
    Idle,
    Discovering,
    DeviceSelected,
    Monitoring,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineDeckFollowMode {
    Disabled,
    Deck1,
    Deck2,
    MasterDeck,
    AnyPlayingDeck,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineDeckPhase {
    Idle,
    Paused,
    Playing,
    Cueing,
    Syncing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineServiceDescriptor {
    pub name: String,
    pub port: u16,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnginePrimeDevice {
    pub id: String,
    pub name: String,
    pub address: String,
    pub software_name: String,
    pub software_version: String,
    pub announce_port: u16,
    pub service_port: Option<u16>,
    pub token_hint: Option<String>,
    pub services: Vec<EngineServiceDescriptor>,
    pub detail: String,
    pub last_seen_frame: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineDeckTelemetry {
    pub deck_index: u8,
    pub track_name: String,
    pub artist_name: String,
    pub bpm: TempoBpm,
    pub beat: BeatTime,
    pub phase: EngineDeckPhase,
    pub is_master: bool,
    pub is_synced: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineMixerTelemetry {
    pub crossfader: IntensityLevel,
    pub channel_faders: Vec<IntensityLevel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineTelemetryFrame {
    pub device_id: String,
    pub decks: Vec<EngineDeckTelemetry>,
    pub mixer: EngineMixerTelemetry,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineLinkState {
    pub mode: EngineLinkMode,
    pub phase: EngineLinkPhase,
    pub enabled: bool,
    pub auto_connect: bool,
    pub adopt_transport: bool,
    pub follow_mode: EngineDeckFollowMode,
    pub discovery_port: u16,
    pub selected_device: Option<String>,
    pub devices: Vec<EnginePrimeDevice>,
    pub telemetry: Option<EngineTelemetryFrame>,
    pub last_summary: Option<String>,
    pub last_error: Option<String>,
}

impl Default for EngineLinkState {
    fn default() -> Self {
        Self {
            mode: EngineLinkMode::StageLinqExperimental,
            phase: EngineLinkPhase::Disabled,
            enabled: false,
            auto_connect: true,
            adopt_transport: false,
            follow_mode: EngineDeckFollowMode::MasterDeck,
            discovery_port: 51_337,
            selected_device: None,
            devices: Vec::new(),
            telemetry: None,
            last_summary: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsState {
    pub selected_tab: SettingsTab,
    pub show_fps_overlay: bool,
    pub show_cpu_overlay: bool,
    pub smooth_playhead: bool,
    pub follow_playhead: bool,
    pub dmx: DmxSettingsState,
    pub midi: MidiSettingsState,
    pub engine_link: EngineLinkState,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            selected_tab: SettingsTab::General,
            show_fps_overlay: true,
            show_cpu_overlay: true,
            smooth_playhead: true,
            follow_playhead: true,
            dmx: DmxSettingsState::default(),
            midi: MidiSettingsState::default(),
            engine_link: EngineLinkState::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HardwareInventorySnapshot {
    pub dmx_interfaces: Vec<DmxInterfaceDescriptor>,
    pub midi_inputs: Vec<MidiPortDescriptor>,
    pub midi_outputs: Vec<MidiPortDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RenderRevisionState {
    pub grid: u64,
    pub clips: u64,
    pub overlay: u64,
    pub chrome: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudioState {
    pub lifecycle: StateLifecycle,
    pub engine: EngineState,
    pub master: MasterState,
    pub timeline: TimelineState,
    pub clip_editor: ClipEditorState,
    pub cue_system: CueSystemState,
    pub chase_system: ChaseSystemState,
    pub fx_system: FxSystemState,
    pub fixture_system: FixtureSystemState,
    pub input_modifiers: InputModifiersState,
    pub context_menu: ContextMenuState,
    pub clipboard: ClipboardState,
    pub replay_log: ReplayLogState,
    pub event_queue: EventQueueState,
    pub performance: PerformanceState,
    pub status: StatusBarState,
    pub venture: VentureState,
    pub settings: SettingsState,
    pub output: OutputRuntimeState,
    pub history: HistoryState,
    pub revisions: RenderRevisionState,
}

impl Default for StudioState {
    fn default() -> Self {
        let mut state = Self {
            lifecycle: StateLifecycle::Valid,
            engine: EngineState {
                phase: EnginePhase::Running,
                resume_target: EngineResumeTarget::Running,
                clock: MonotonicClock::default(),
                transport: TransportState {
                    bpm: TempoBpm::from_whole_bpm(128),
                    playhead: BeatTime::from_beats(6),
                    song_length: BeatTime::from_beats(160),
                },
                error: None,
            },
            master: MasterState {
                intensity: IntensityLevel::from_permille(860),
                speed: SpeedRatio::from_permille(680),
            },
            timeline: TimelineState {
                phase: TimelinePhase::Idle,
                viewport: TimelineViewport {
                    zoom: ZoomFactor::from_permille(1180),
                    scroll: BeatTime::ZERO,
                },
                snap: SnapState {
                    phase: SnapPhase::Free,
                    enabled: true,
                    resolution: SnapResolution::QuarterBeat,
                    guide: Some(SnapGuide {
                        beat: BeatTime::from_beats(6),
                        track: None,
                        strength_permille: 450,
                    }),
                },
                tracks: demo_tracks(),
                selection: SelectionState::Clip(ClipId(102)),
                selected_clips: vec![ClipId(102)],
                hover: None,
                interaction: TimelineInteraction::Idle,
            },
            clip_editor: ClipEditorState {
                phase: ClipEditorPhase::Closed,
                clip_id: None,
                automation_target: AutomationTarget::Intensity,
                selected_automation_point: None,
            },
            cue_system: CueSystemState {
                selected: Some(CueId(1)),
                active: Some(CueId(1)),
                cues: demo_cues(),
            },
            chase_system: ChaseSystemState {
                selected: Some(ChaseId(1)),
                selected_step: Some(0),
                chases: demo_chases(),
            },
            fx_system: FxSystemState {
                selected: Some(FxId(1)),
                layers: demo_fx_layers(),
            },
            fixture_system: FixtureSystemState {
                selected: Some(FixtureGroupId(1)),
                groups: demo_fixture_groups(),
                library: FixtureLibraryState::default(),
            },
            input_modifiers: InputModifiersState::default(),
            context_menu: ContextMenuState::default(),
            clipboard: ClipboardState::default(),
            replay_log: ReplayLogState::default(),
            event_queue: EventQueueState::default(),
            performance: PerformanceState {
                frame_index: 0,
                fps: 60,
                cpu_load: CpuLoad(18),
                frame_budget_ms: 16,
            },
            status: StatusBarState {
                hint: "Timeline bereit".to_owned(),
                last_diffs: vec![StateDiff::Engine, StateDiff::TimelineViewport],
            },
            venture: VentureState::default(),
            settings: SettingsState::default(),
            output: OutputRuntimeState::default(),
            history: HistoryState::default(),
            revisions: RenderRevisionState {
                grid: 1,
                clips: 1,
                overlay: 1,
                chrome: 1,
            },
        };
        state.venture.saved_fingerprint = state.authoring_fingerprint();
        state
    }
}

impl StudioState {
    pub fn clip(&self, clip_id: ClipId) -> Option<&Clip> {
        self.timeline
            .tracks
            .iter()
            .flat_map(|track| track.clips.iter())
            .find(|clip| clip.id == clip_id)
    }

    pub fn track(&self, track_id: TrackId) -> Option<&Track> {
        self.timeline
            .tracks
            .iter()
            .find(|track| track.id == track_id)
    }

    pub fn cue(&self, cue_id: CueId) -> Option<&Cue> {
        self.cue_system.cues.iter().find(|cue| cue.id == cue_id)
    }

    pub fn chase(&self, chase_id: ChaseId) -> Option<&Chase> {
        self.chase_system
            .chases
            .iter()
            .find(|chase| chase.id == chase_id)
    }

    pub fn fx_layer(&self, fx_id: FxId) -> Option<&FxLayer> {
        self.fx_system.layers.iter().find(|layer| layer.id == fx_id)
    }

    pub fn fixture_group(&self, fixture_group_id: FixtureGroupId) -> Option<&FixtureGroup> {
        self.fixture_system
            .groups
            .iter()
            .find(|group| group.id == fixture_group_id)
    }

    pub fn clip_location(&self, clip_id: ClipId) -> Option<(usize, usize)> {
        self.timeline
            .tracks
            .iter()
            .enumerate()
            .find_map(|(track_index, track)| {
                track
                    .clips
                    .iter()
                    .position(|clip| clip.id == clip_id)
                    .map(|clip_index| (track_index, clip_index))
            })
    }

    pub fn track_index(&self, track_id: TrackId) -> Option<usize> {
        self.timeline
            .tracks
            .iter()
            .position(|track| track.id == track_id)
    }

    pub fn next_clip_id(&self) -> ClipId {
        let next = self
            .timeline
            .tracks
            .iter()
            .flat_map(|track| track.clips.iter())
            .map(|clip| clip.id.0)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        ClipId(next)
    }

    pub fn next_cue_id(&self) -> CueId {
        let next = self
            .cue_system
            .cues
            .iter()
            .map(|cue| cue.id.0)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        CueId(next)
    }

    pub fn next_chase_id(&self) -> ChaseId {
        let next = self
            .chase_system
            .chases
            .iter()
            .map(|chase| chase.id.0)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        ChaseId(next)
    }

    pub fn selected_clip(&self) -> Option<&Clip> {
        (self.timeline.selected_clips.len() == 1)
            .then(|| self.timeline.selected_clips.first().copied())
            .flatten()
            .and_then(|clip_id| self.clip(clip_id))
    }

    pub fn editor_clip(&self) -> Option<&Clip> {
        self.clip_editor
            .clip_id
            .and_then(|clip_id| self.clip(clip_id))
    }

    pub fn primary_selected_clip_id(&self) -> Option<ClipId> {
        match self.timeline.selection {
            SelectionState::Clip(clip_id) if self.timeline.selected_clips.contains(&clip_id) => {
                Some(clip_id)
            }
            SelectionState::Clip(_) | SelectionState::Track(_) | SelectionState::None => None,
        }
    }

    pub fn selected_clip_ids(&self) -> &[ClipId] {
        &self.timeline.selected_clips
    }

    pub fn selected_clip_count(&self) -> usize {
        self.timeline.selected_clips.len()
    }

    pub fn has_multi_clip_selection(&self) -> bool {
        self.selected_clip_count() > 1
    }

    pub fn can_duplicate_selected_clips(&self) -> bool {
        let Some((min_start, max_end)) = self.selected_clip_time_span() else {
            return false;
        };

        let span = max_end.saturating_sub(min_start);
        max_end.saturating_add(span) <= self.engine.transport.song_length
    }

    pub fn can_delete_selected_clips(&self) -> bool {
        !self.timeline.selected_clips.is_empty()
    }

    pub fn can_copy_selected_clips(&self) -> bool {
        !self.timeline.selected_clips.is_empty()
    }

    pub fn can_paste_clipboard(&self) -> bool {
        !self.clipboard.clips.is_empty()
    }

    pub fn can_split_selected_clips_at_playhead(&self) -> bool {
        let playhead = self.engine.transport.playhead;

        self.timeline
            .selected_clips
            .iter()
            .filter_map(|clip_id| self.clip(*clip_id))
            .any(|clip| {
                let clip_end = clip.start.saturating_add(clip.duration);
                if playhead <= clip.start || playhead >= clip_end {
                    return false;
                }

                let left = playhead.saturating_sub(clip.start);
                let right = clip_end.saturating_sub(playhead);
                left >= MIN_CLIP_DURATION && right >= MIN_CLIP_DURATION
            })
    }

    pub fn is_clip_selected(&self, clip_id: ClipId) -> bool {
        self.timeline.selected_clips.contains(&clip_id)
    }

    pub fn selected_cue(&self) -> Option<&Cue> {
        self.cue_system.selected.and_then(|cue_id| self.cue(cue_id))
    }

    pub fn selected_chase(&self) -> Option<&Chase> {
        self.chase_system
            .selected
            .and_then(|chase_id| self.chase(chase_id))
    }

    pub fn selected_chase_step_index(&self) -> Option<usize> {
        let selected = self.chase_system.selected_step?;
        let chase = self.selected_chase()?;
        (selected < chase.steps.len()).then_some(selected)
    }

    pub fn selected_chase_step(&self) -> Option<&ChaseStep> {
        let chase = self.selected_chase()?;
        let index = self.selected_chase_step_index()?;
        chase.steps.get(index)
    }

    pub fn can_delete_selected_cue(&self) -> bool {
        self.selected_cue().is_some()
    }

    pub fn can_delete_selected_chase(&self) -> bool {
        self.selected_chase().is_some()
    }

    pub fn can_add_selected_chase_step(&self) -> bool {
        self.selected_chase().is_some()
    }

    pub fn can_delete_selected_chase_step(&self) -> bool {
        self.selected_chase_step_index().is_some()
            && self
                .selected_chase()
                .map(|chase| chase.steps.len() > 1)
                .unwrap_or(false)
    }

    pub fn can_move_selected_chase_step_left(&self) -> bool {
        self.selected_chase_step_index()
            .is_some_and(|index| index > 0)
    }

    pub fn can_move_selected_chase_step_right(&self) -> bool {
        self.selected_chase()
            .zip(self.selected_chase_step_index())
            .is_some_and(|(chase, index)| index + 1 < chase.steps.len())
    }

    pub fn selected_fx(&self) -> Option<&FxLayer> {
        self.fx_system
            .selected
            .and_then(|fx_id| self.fx_layer(fx_id))
    }

    pub fn selected_fixture_group(&self) -> Option<&FixtureGroup> {
        self.fixture_system
            .selected
            .and_then(|group_id| self.fixture_group(group_id))
    }

    pub fn fixture_profile(&self, profile_id: &str) -> Option<&FixtureProfile> {
        self.fixture_system
            .library
            .profiles
            .iter()
            .find(|profile| profile.id == profile_id)
    }

    pub fn selected_fixture_profile(&self) -> Option<&FixtureProfile> {
        self.fixture_system
            .library
            .selected_profile
            .as_deref()
            .and_then(|profile_id| self.fixture_profile(profile_id))
    }

    pub fn fixture_patch(&self, patch_id: u32) -> Option<&FixturePatch> {
        self.fixture_system
            .library
            .patches
            .iter()
            .find(|patch| patch.id == patch_id)
    }

    pub fn selected_fixture_patch(&self) -> Option<&FixturePatch> {
        self.fixture_system
            .library
            .selected_patch
            .and_then(|patch_id| self.fixture_patch(patch_id))
    }

    pub fn fixture_patch_channel_count(&self, patch: &FixturePatch) -> Option<u16> {
        let profile = self.fixture_profile(&patch.profile_id)?;
        let count = fixture_mode_channel_count(profile, &patch.mode_name);
        u16::try_from(count).ok()
    }

    pub fn fixture_patch_end_address(&self, patch: &FixturePatch) -> Option<u16> {
        self.fixture_patch_range(patch).map(|(_, end)| end)
    }

    pub fn fixture_patch_conflicts(&self, patch_id: u32) -> Vec<u32> {
        let Some(patch) = self.fixture_patch(patch_id) else {
            return Vec::new();
        };
        let Some((start, end)) = self.fixture_patch_range(patch) else {
            return Vec::new();
        };

        let mut conflicts = self
            .fixture_system
            .library
            .patches
            .iter()
            .filter(|other| other.id != patch.id && other.universe == patch.universe)
            .filter_map(|other| {
                let (other_start, other_end) = self.fixture_patch_range(other)?;
                ranges_overlap(start, end, other_start, other_end).then_some(other.id)
            })
            .collect::<Vec<_>>();
        conflicts.sort_unstable();
        conflicts
    }

    pub fn fixture_patches_for_group(&self, group_id: FixtureGroupId) -> Vec<&FixturePatch> {
        let mut patches = self
            .fixture_system
            .library
            .patches
            .iter()
            .filter(|patch| patch.group_id == Some(group_id))
            .collect::<Vec<_>>();
        patches.sort_by_key(|patch| (patch.universe, patch.address, patch.id));
        patches
    }

    pub fn fixture_group_patch_summary(
        &self,
        group_id: FixtureGroupId,
    ) -> FixtureGroupPatchSummary {
        let patches = self.fixture_patches_for_group(group_id);
        let mut universes = patches
            .iter()
            .map(|patch| patch.universe)
            .collect::<Vec<_>>();
        universes.sort_unstable();
        universes.dedup();

        let mut conflicting_patch_ids = patches
            .iter()
            .flat_map(|patch| {
                let mut ids = self.fixture_patch_conflicts(patch.id);
                if !ids.is_empty() {
                    ids.push(patch.id);
                }
                ids
            })
            .collect::<Vec<_>>();
        conflicting_patch_ids.sort_unstable();
        conflicting_patch_ids.dedup();

        let footprint_channels = patches
            .iter()
            .filter_map(|patch| self.fixture_patch_channel_count(patch))
            .sum::<u16>();

        let occupied_channels = patches
            .iter()
            .filter_map(|patch| {
                self.fixture_patch_range(patch)
                    .map(|(start, end)| (patch.universe, start, end))
            })
            .flat_map(|(universe, start, end)| {
                (start..=end).map(move |channel| (universe, channel))
            })
            .fold(Vec::<(u16, u16)>::new(), |mut acc, slot| {
                if !acc.contains(&slot) {
                    acc.push(slot);
                }
                acc
            })
            .len() as u16;

        FixtureGroupPatchSummary {
            group_id,
            patch_count: patches.len(),
            enabled_patch_count: patches.iter().filter(|patch| patch.enabled).count(),
            footprint_channels,
            occupied_channels,
            universes,
            conflicting_patch_ids,
        }
    }

    pub fn fixture_universe_summaries(&self) -> Vec<FixtureUniverseSummary> {
        let mut universes = self
            .fixture_system
            .library
            .patches
            .iter()
            .map(|patch| patch.universe)
            .collect::<Vec<_>>();
        universes.sort_unstable();
        universes.dedup();

        universes
            .into_iter()
            .map(|universe| {
                let patches = self
                    .fixture_system
                    .library
                    .patches
                    .iter()
                    .filter(|patch| patch.universe == universe)
                    .collect::<Vec<_>>();

                let mut occupied = [false; 512];
                let mut highest_address = 0u16;
                let mut conflicting_patch_ids = Vec::new();
                let mut footprint_channels = 0u16;

                for patch in &patches {
                    if let Some(count) = self.fixture_patch_channel_count(patch) {
                        footprint_channels = footprint_channels.saturating_add(count);
                    }

                    if let Some((start, end)) = self.fixture_patch_range(patch) {
                        highest_address = highest_address.max(end.min(512));
                        for channel in start..=end.min(512) {
                            occupied[(channel - 1) as usize] = true;
                        }
                    }

                    let conflicts = self.fixture_patch_conflicts(patch.id);
                    if !conflicts.is_empty() {
                        conflicting_patch_ids.push(patch.id);
                        conflicting_patch_ids.extend(conflicts);
                    }
                }

                conflicting_patch_ids.sort_unstable();
                conflicting_patch_ids.dedup();

                FixtureUniverseSummary {
                    universe,
                    patch_count: patches.len(),
                    enabled_patch_count: patches.iter().filter(|patch| patch.enabled).count(),
                    footprint_channels,
                    occupied_channels: occupied.iter().filter(|occupied| **occupied).count() as u16,
                    highest_address,
                    conflicting_patch_ids,
                }
            })
            .collect()
    }

    pub fn next_fixture_patch_placement(
        &self,
        profile: &FixtureProfile,
        mode_name: &str,
    ) -> (u16, u16) {
        let footprint = fixture_mode_channel_count(profile, mode_name);
        let footprint = u16::try_from(footprint).unwrap_or(0);

        if footprint == 0 {
            return (1, 1);
        }

        for universe in 1..=64 {
            let mut spans = self
                .fixture_system
                .library
                .patches
                .iter()
                .filter(|patch| patch.universe == universe)
                .filter_map(|patch| self.fixture_patch_range(patch))
                .collect::<Vec<_>>();
            spans.sort_unstable_by_key(|(start, end)| (*start, *end));

            let mut next_address = 1u16;
            for (start, end) in spans {
                let required_end = next_address.saturating_add(footprint.saturating_sub(1));
                if required_end < start {
                    return (universe, next_address);
                }
                next_address = next_address.max(end.saturating_add(1));
            }

            let required_end = next_address.saturating_add(footprint.saturating_sub(1));
            if next_address >= 1 && required_end <= 512 {
                return (universe, next_address);
            }
        }

        (64, 1)
    }

    pub fn next_fixture_patch_id(&self) -> u32 {
        self.fixture_system
            .library
            .patches
            .iter()
            .map(|patch| patch.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
    }

    pub fn can_import_fixture_from_ofl(&self) -> bool {
        !self
            .fixture_system
            .library
            .ofl_manufacturer_key
            .trim()
            .is_empty()
            && !self
                .fixture_system
                .library
                .ofl_fixture_key
                .trim()
                .is_empty()
    }

    pub fn can_import_fixture_from_qxf(&self) -> bool {
        !self
            .fixture_system
            .library
            .qxf_import_path
            .trim()
            .is_empty()
    }

    pub fn can_export_selected_fixture_profile(&self) -> bool {
        self.selected_fixture_profile().is_some()
            && !self
                .fixture_system
                .library
                .qxf_export_path
                .trim()
                .is_empty()
    }

    pub fn can_delete_selected_fixture_profile(&self) -> bool {
        self.selected_fixture_profile().is_some()
    }

    pub fn can_create_fixture_patch(&self) -> bool {
        self.selected_fixture_profile()
            .is_some_and(|profile| !profile.modes.is_empty())
    }

    pub fn can_delete_selected_fixture_patch(&self) -> bool {
        self.selected_fixture_patch().is_some()
    }

    pub fn selected_dmx_interface(&self) -> Option<&DmxInterfaceDescriptor> {
        let selected = self.settings.dmx.selected_interface.as_deref()?;
        self.settings
            .dmx
            .interfaces
            .iter()
            .find(|interface| interface.id == selected)
    }

    pub fn selected_midi_input(&self) -> Option<&MidiPortDescriptor> {
        let selected = self.settings.midi.selected_input.as_deref()?;
        self.settings
            .midi
            .inputs
            .iter()
            .find(|port| port.id == selected)
    }

    pub fn selected_midi_output(&self) -> Option<&MidiPortDescriptor> {
        let selected = self.settings.midi.selected_output.as_deref()?;
        self.settings
            .midi
            .outputs
            .iter()
            .find(|port| port.id == selected)
    }

    pub fn selected_engine_device(&self) -> Option<&EnginePrimeDevice> {
        let selected = self.settings.engine_link.selected_device.as_deref()?;
        self.settings
            .engine_link
            .devices
            .iter()
            .find(|device| device.id == selected)
    }

    pub fn next_midi_binding_id(&self) -> u32 {
        self.settings
            .midi
            .bindings
            .iter()
            .map(|binding| binding.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
    }

    pub fn midi_binding(&self, binding_id: u32) -> Option<&MidiBinding> {
        self.settings
            .midi
            .bindings
            .iter()
            .find(|binding| binding.id == binding_id)
    }

    pub fn selected_controller_profile(&self) -> Option<ControllerProfileKind> {
        self.settings.midi.detected_controller.or_else(|| {
            self.selected_midi_input()
                .and_then(|port| port.profile_hint)
        })
    }

    pub fn can_apply_controller_automap(&self) -> bool {
        self.selected_midi_input().is_some() && self.selected_controller_profile().is_some()
    }

    pub fn can_refresh_hardware_inventory(&self) -> bool {
        !matches!(self.settings.dmx.phase, HardwareDiscoveryPhase::Refreshing)
            && !matches!(self.settings.midi.phase, HardwareDiscoveryPhase::Refreshing)
    }

    pub fn can_refresh_engine_link_discovery(&self) -> bool {
        self.settings.engine_link.enabled
            && !matches!(
                self.settings.engine_link.phase,
                EngineLinkPhase::Discovering
            )
    }

    pub fn should_subscribe_engine_link(&self) -> bool {
        self.settings.engine_link.enabled
            && !matches!(self.settings.engine_link.mode, EngineLinkMode::Disabled)
    }

    pub fn should_dispatch_runtime_outputs(&self) -> bool {
        let dmx_enabled = self.settings.dmx.output_enabled
            && !matches!(self.settings.dmx.backend, DmxBackendKind::Disabled);
        let midi_feedback_enabled = self.settings.midi.feedback_enabled
            && self.selected_midi_output().is_some()
            && self
                .settings
                .midi
                .bindings
                .iter()
                .any(|binding| binding.message.is_some());

        (dmx_enabled || midi_feedback_enabled)
            && self.output.phase != OutputDeliveryPhase::Dispatching
    }

    fn fixture_patch_range(&self, patch: &FixturePatch) -> Option<(u16, u16)> {
        if !(1..=512).contains(&patch.address) {
            return None;
        }
        let count = self.fixture_patch_channel_count(patch)?;
        (count > 0).then_some((patch.address, patch.address.saturating_add(count - 1)))
    }

    pub fn selected_summary(&self) -> String {
        if self.timeline.selected_clips.len() > 1 {
            return format!("Clips: {} selektiert", self.timeline.selected_clips.len());
        }

        match self.timeline.selection {
            SelectionState::Clip(clip_id) => self
                .clip(clip_id)
                .map(|clip| format!("Clip: {}", clip.title))
                .unwrap_or_else(|| "Clip".to_owned()),
            SelectionState::Track(track_id) => self
                .track(track_id)
                .map(|track| format!("Track: {}", track.name))
                .unwrap_or_else(|| "Track".to_owned()),
            SelectionState::None => "Keine Auswahl".to_owned(),
        }
    }

    pub fn selected_venture(&self) -> Option<&VentureDescriptor> {
        let selected = self.venture.selected.as_deref()?;
        self.venture
            .ventures
            .iter()
            .find(|venture| venture.id == selected)
    }

    pub fn selected_recovery_slot(&self) -> Option<&RecoverySlotDescriptor> {
        let selected = self.venture.selected_recovery.as_deref()?;
        self.venture
            .recovery_slots
            .iter()
            .find(|slot| slot.id == selected)
    }

    pub fn can_save_venture(&self) -> bool {
        !self.venture.draft_name.trim().is_empty()
    }

    pub fn can_save_venture_as(&self) -> bool {
        !self.venture.draft_name.trim().is_empty()
    }

    pub fn can_load_selected_venture(&self) -> bool {
        self.selected_venture().is_some()
    }

    pub fn can_rename_selected_venture(&self) -> bool {
        self.selected_venture().is_some() && !self.venture.draft_name.trim().is_empty()
    }

    pub fn can_delete_selected_venture(&self) -> bool {
        self.selected_venture().is_some()
    }

    pub fn can_restore_selected_recovery(&self) -> bool {
        self.selected_recovery_slot().is_some()
    }

    pub fn venture_summary(&self) -> String {
        match self.selected_venture() {
            Some(venture) => format!("Venture: {}", venture.name),
            None if !self.venture.draft_name.trim().is_empty() => {
                format!("Venture Draft: {}", self.venture.draft_name.trim())
            }
            None => "Kein Venture geladen".to_owned(),
        }
    }

    pub fn venture_issue_summary(&self) -> String {
        if self.venture.registry_issues.is_empty() {
            "Keine Registry-Warnungen".to_owned()
        } else {
            format!(
                "{} Registry-Warnung(en)",
                self.venture.registry_issues.len()
            )
        }
    }

    pub fn recovery_issue_summary(&self) -> String {
        if self.venture.recovery_issues.is_empty() {
            "Keine Recovery-Warnungen".to_owned()
        } else {
            format!(
                "{} Recovery-Warnung(en)",
                self.venture.recovery_issues.len()
            )
        }
    }

    pub fn dirty_summary(&self) -> String {
        if self.venture.dirty {
            "Unsaved Changes".to_owned()
        } else {
            "Saved".to_owned()
        }
    }

    pub fn authoring_fingerprint(&self) -> String {
        serde_json::to_string(&VentureAuthoringSignature {
            master: self.master.clone(),
            tracks: self.timeline.tracks.clone(),
            cue_system: self.cue_system.clone(),
            chase_system: self.chase_system.clone(),
            fx_system: self.fx_system.clone(),
            fixture_system: self.fixture_system.clone(),
            settings: self.settings.clone(),
        })
        .expect("serialize authoring fingerprint")
    }

    pub fn diff_summary(&self) -> String {
        self.status
            .last_diffs
            .iter()
            .map(StateDiff::label)
            .collect::<Vec<_>>()
            .join("  |  ")
    }

    pub fn selected_clip_time_span(&self) -> Option<(BeatTime, BeatTime)> {
        let mut selected = self
            .timeline
            .selected_clips
            .iter()
            .filter_map(|clip_id| self.clip(*clip_id));

        let first = selected.next()?;
        let mut min_start = first.start;
        let mut max_end = first.start.saturating_add(first.duration);

        for clip in selected {
            min_start = min_start.min(clip.start);
            max_end = max_end.max(clip.start.saturating_add(clip.duration));
        }

        Some((min_start, max_end))
    }

    pub fn can_undo(&self) -> bool {
        !self.history.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.history.redo_stack.is_empty()
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.history
            .undo_stack
            .last()
            .map(|entry| entry.label.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.history
            .redo_stack
            .last()
            .map(|entry| entry.label.as_str())
    }

    pub fn context_track_id(&self) -> Option<TrackId> {
        match self.context_menu.target {
            Some(ContextMenuTarget::Clip(clip_id)) => self
                .timeline
                .tracks
                .iter()
                .find(|track| track.clips.iter().any(|clip| clip.id == clip_id))
                .map(|track| track.id),
            Some(ContextMenuTarget::Track(track_id)) => Some(track_id),
            Some(ContextMenuTarget::Timeline) | None => None,
        }
    }
}

fn ranges_overlap(start: u16, end: u16, other_start: u16, other_end: u16) -> bool {
    start <= other_end && other_start <= end
}

impl EngineState {
    pub fn is_running(&self) -> bool {
        self.phase == EnginePhase::Running
    }
}

impl CueVisualState {
    pub fn from_phase(phase: CuePhase) -> Self {
        match phase {
            CuePhase::Stored => Self::Inactive,
            CuePhase::Armed => Self::Ready,
            CuePhase::Triggered | CuePhase::Fading | CuePhase::Active => Self::Active,
        }
    }
}

impl TransportState {
    pub fn position_label(&self) -> String {
        let bars = (self.playhead.ticks() / (PPQ * BAR_BEATS)).saturating_add(1);
        let beat_in_bar = ((self.playhead.ticks() / PPQ) % BAR_BEATS).saturating_add(1);
        let sixteenth = ((self.playhead.ticks() % PPQ) / (PPQ / 4)).saturating_add(1);
        format!("{bars:02}.{beat_in_bar}.{sixteenth}")
    }
}

impl SnapResolution {
    pub fn step(self) -> BeatTime {
        match self {
            Self::Beat => BeatTime::from_beats(1),
            Self::HalfBeat => BeatTime::from_fraction(1, 2),
            Self::QuarterBeat => BeatTime::from_fraction(1, 4),
            Self::EighthBeat => BeatTime::from_fraction(1, 8),
        }
    }
}

impl fmt::Display for FxWaveform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Sine => "Sine",
            Self::Triangle => "Triangle",
            Self::Saw => "Saw",
            Self::Pulse => "Pulse",
        };
        f.write_str(label)
    }
}

impl fmt::Display for AutomationTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Intensity => "Intensity",
            Self::Speed => "Speed",
            Self::FxDepth => "FX Depth",
        };
        f.write_str(label)
    }
}

impl fmt::Display for AutomationInterpolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Step => "Step",
            Self::Linear => "Linear",
        };
        f.write_str(label)
    }
}

impl fmt::Display for SnapResolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Beat => "1 Beat",
            Self::HalfBeat => "1/2 Beat",
            Self::QuarterBeat => "1/4 Beat",
            Self::EighthBeat => "1/8 Beat",
        };
        f.write_str(label)
    }
}

impl TimelineInteraction {
    pub fn active_clip(self) -> Option<ClipId> {
        match self {
            Self::PendingBoxSelection { .. } | Self::BoxSelecting { .. } => None,
            Self::PendingClipDrag { clip_id, .. }
            | Self::DragClip { clip_id, .. }
            | Self::PendingResizeClipStart { clip_id, .. }
            | Self::ResizeClipStart { clip_id, .. }
            | Self::PendingResizeClipEnd { clip_id, .. }
            | Self::ResizeClipEnd { clip_id, .. }
            | Self::AdjustClipParameter { clip_id, .. } => Some(clip_id),
            Self::Idle | Self::ScrubPlayhead => None,
        }
    }

    pub fn captures_pointer(self) -> bool {
        self != Self::Idle
    }
}

fn demo_tracks() -> Vec<Track> {
    vec![
        Track {
            id: TrackId(1),
            name: "Main FX".to_owned(),
            color: RgbaColor::rgb(43, 182, 171),
            muted: false,
            solo: false,
            clips: vec![
                Clip {
                    id: ClipId(101),
                    title: "Opener Wash".to_owned(),
                    phase: ClipPhase::Inactive,
                    start: BeatTime::from_beats(0),
                    duration: BeatTime::from_beats(8),
                    params: demo_clip_params(780, 760, 640, SnapResolution::QuarterBeat),
                    automation: demo_clip_automation(BeatTime::from_beats(8), 780, 760, 640),
                    palette: ClipPalette {
                        base: RgbaColor::rgb(22, 132, 140),
                        highlight: RgbaColor::rgb(67, 228, 211),
                        edge: RgbaColor::rgb(153, 255, 241),
                    },
                    markers: vec![
                        ClipMarker {
                            label: "BPM".to_owned(),
                            offset: BeatTime::from_beats(1),
                            color: RgbaColor::rgb(255, 222, 117),
                        },
                        ClipMarker {
                            label: "RGB".to_owned(),
                            offset: BeatTime::from_beats(4),
                            color: RgbaColor::rgb(255, 141, 88),
                        },
                    ],
                    linked_cue: Some(CueId(2)),
                    cue_state: CueVisualState::Ready,
                },
                Clip {
                    id: ClipId(102),
                    title: "Drop Sweep".to_owned(),
                    phase: ClipPhase::Inactive,
                    start: BeatTime::from_beats(8),
                    duration: BeatTime::from_beats(8),
                    params: demo_clip_params(910, 920, 860, SnapResolution::EighthBeat),
                    automation: demo_clip_automation(BeatTime::from_beats(8), 910, 920, 860),
                    palette: ClipPalette {
                        base: RgbaColor::rgb(191, 81, 32),
                        highlight: RgbaColor::rgb(255, 177, 117),
                        edge: RgbaColor::rgb(255, 232, 173),
                    },
                    markers: vec![
                        ClipMarker {
                            label: "Cue".to_owned(),
                            offset: BeatTime::from_fraction(1, 2),
                            color: RgbaColor::rgb(255, 241, 177),
                        },
                        ClipMarker {
                            label: "Color".to_owned(),
                            offset: BeatTime::from_fraction(9, 2),
                            color: RgbaColor::rgb(255, 103, 92),
                        },
                        ClipMarker {
                            label: "Tilt".to_owned(),
                            offset: BeatTime::from_fraction(27, 4),
                            color: RgbaColor::rgb(103, 168, 255),
                        },
                    ],
                    linked_cue: Some(CueId(1)),
                    cue_state: CueVisualState::Active,
                },
            ],
        },
        Track {
            id: TrackId(2),
            name: "Strobes".to_owned(),
            color: RgbaColor::rgb(246, 95, 95),
            muted: false,
            solo: true,
            clips: vec![
                Clip {
                    id: ClipId(201),
                    title: "Flash Chase".to_owned(),
                    phase: ClipPhase::Inactive,
                    start: BeatTime::from_beats(4),
                    duration: BeatTime::from_beats(4),
                    params: demo_clip_params(860, 1120, 720, SnapResolution::EighthBeat),
                    automation: demo_clip_automation(BeatTime::from_beats(4), 860, 1120, 720),
                    palette: ClipPalette {
                        base: RgbaColor::rgb(135, 34, 50),
                        highlight: RgbaColor::rgb(255, 112, 126),
                        edge: RgbaColor::rgb(255, 186, 192),
                    },
                    markers: vec![
                        ClipMarker {
                            label: "1/8".to_owned(),
                            offset: BeatTime::from_beats(1),
                            color: RgbaColor::rgb(255, 255, 255),
                        },
                        ClipMarker {
                            label: "Loop".to_owned(),
                            offset: BeatTime::from_beats(3),
                            color: RgbaColor::rgb(255, 208, 112),
                        },
                    ],
                    linked_cue: Some(CueId(3)),
                    cue_state: CueVisualState::Ready,
                },
                Clip {
                    id: ClipId(202),
                    title: "Break Snap".to_owned(),
                    phase: ClipPhase::Inactive,
                    start: BeatTime::from_beats(12),
                    duration: BeatTime::from_fraction(7, 2),
                    params: demo_clip_params(620, 580, 500, SnapResolution::QuarterBeat),
                    automation: demo_clip_automation(BeatTime::from_fraction(7, 2), 620, 580, 500),
                    palette: ClipPalette {
                        base: RgbaColor::rgb(110, 23, 34),
                        highlight: RgbaColor::rgb(223, 72, 96),
                        edge: RgbaColor::rgb(255, 150, 163),
                    },
                    markers: vec![ClipMarker {
                        label: "Hit".to_owned(),
                        offset: BeatTime::from_fraction(7, 4),
                        color: RgbaColor::rgb(255, 223, 143),
                    }],
                    linked_cue: Some(CueId(4)),
                    cue_state: CueVisualState::Inactive,
                },
            ],
        },
        Track {
            id: TrackId(3),
            name: "Color Sweep".to_owned(),
            color: RgbaColor::rgb(98, 146, 255),
            muted: false,
            solo: false,
            clips: vec![Clip {
                id: ClipId(301),
                title: "Prism Morph".to_owned(),
                phase: ClipPhase::Inactive,
                start: BeatTime::from_beats(2),
                duration: BeatTime::from_beats(10),
                params: demo_clip_params(740, 700, 930, SnapResolution::HalfBeat),
                automation: demo_clip_automation(BeatTime::from_beats(10), 740, 700, 930),
                palette: ClipPalette {
                    base: RgbaColor::rgb(31, 72, 160),
                    highlight: RgbaColor::rgb(104, 164, 255),
                    edge: RgbaColor::rgb(189, 221, 255),
                },
                markers: vec![
                    ClipMarker {
                        label: "Hue".to_owned(),
                        offset: BeatTime::from_beats(2),
                        color: RgbaColor::rgb(139, 255, 202),
                    },
                    ClipMarker {
                        label: "BPM".to_owned(),
                        offset: BeatTime::from_fraction(15, 2),
                        color: RgbaColor::rgb(255, 233, 150),
                    },
                ],
                linked_cue: Some(CueId(2)),
                cue_state: CueVisualState::Ready,
            }],
        },
        Track {
            id: TrackId(4),
            name: "Cues".to_owned(),
            color: RgbaColor::rgb(191, 155, 67),
            muted: false,
            solo: false,
            clips: vec![
                Clip {
                    id: ClipId(401),
                    title: "Build Cue".to_owned(),
                    phase: ClipPhase::Inactive,
                    start: BeatTime::from_beats(0),
                    duration: BeatTime::from_beats(2),
                    params: demo_clip_params(800, 660, 440, SnapResolution::QuarterBeat),
                    automation: demo_clip_automation(BeatTime::from_beats(2), 800, 660, 440),
                    palette: ClipPalette {
                        base: RgbaColor::rgb(94, 78, 24),
                        highlight: RgbaColor::rgb(238, 206, 104),
                        edge: RgbaColor::rgb(255, 234, 165),
                    },
                    markers: vec![ClipMarker {
                        label: "Cue A".to_owned(),
                        offset: BeatTime::from_fraction(1, 2),
                        color: RgbaColor::rgb(255, 244, 202),
                    }],
                    linked_cue: Some(CueId(1)),
                    cue_state: CueVisualState::Active,
                },
                Clip {
                    id: ClipId(402),
                    title: "Reverse Sweep".to_owned(),
                    phase: ClipPhase::Inactive,
                    start: BeatTime::from_beats(9),
                    duration: BeatTime::from_beats(5),
                    params: demo_clip_params(700, 1180, 610, SnapResolution::HalfBeat),
                    automation: demo_clip_automation(BeatTime::from_beats(5), 700, 1180, 610),
                    palette: ClipPalette {
                        base: RgbaColor::rgb(82, 62, 18),
                        highlight: RgbaColor::rgb(210, 179, 79),
                        edge: RgbaColor::rgb(255, 231, 170),
                    },
                    markers: vec![
                        ClipMarker {
                            label: "Cue B".to_owned(),
                            offset: BeatTime::from_fraction(3, 4),
                            color: RgbaColor::rgb(255, 243, 208),
                        },
                        ClipMarker {
                            label: "Rev".to_owned(),
                            offset: BeatTime::from_beats(4),
                            color: RgbaColor::rgb(153, 203, 255),
                        },
                    ],
                    linked_cue: Some(CueId(3)),
                    cue_state: CueVisualState::Ready,
                },
            ],
        },
    ]
}

fn demo_clip_params(
    intensity_permille: u16,
    speed_permille: u16,
    fx_depth_permille: u16,
    bpm_grid: SnapResolution,
) -> ClipParameters {
    ClipParameters {
        intensity: IntensityLevel::from_permille(intensity_permille),
        speed: SpeedRatio::from_permille(speed_permille),
        fx_depth: IntensityLevel::from_permille(fx_depth_permille),
        bpm_grid,
    }
}

fn demo_clip_automation(
    duration: BeatTime,
    intensity_permille: u16,
    speed_permille: u16,
    fx_depth_permille: u16,
) -> Vec<AutomationLane> {
    let quarter = BeatTime::from_ticks(duration.ticks() / 4);
    let half = BeatTime::from_ticks(duration.ticks() / 2);
    let tail = duration.saturating_sub(BeatTime::from_ticks((duration.ticks() / 10).max(1)));

    vec![
        AutomationLane {
            target: AutomationTarget::Intensity,
            interpolation: AutomationInterpolation::Linear,
            enabled: true,
            points: vec![
                AutomationPoint {
                    offset: BeatTime::ZERO,
                    value: intensity_permille,
                },
                AutomationPoint {
                    offset: half,
                    value: ((intensity_permille as u32 * 112) / 100).min(1000) as u16,
                },
                AutomationPoint {
                    offset: tail,
                    value: ((intensity_permille as u32 * 82) / 100) as u16,
                },
            ],
        },
        AutomationLane {
            target: AutomationTarget::Speed,
            interpolation: AutomationInterpolation::Step,
            enabled: true,
            points: vec![
                AutomationPoint {
                    offset: BeatTime::ZERO,
                    value: speed_permille,
                },
                AutomationPoint {
                    offset: quarter,
                    value: ((speed_permille as u32 * 118) / 100)
                        .clamp(SpeedRatio::MIN as u32, SpeedRatio::MAX as u32)
                        as u16,
                },
                AutomationPoint {
                    offset: tail,
                    value: ((speed_permille as u32 * 92) / 100)
                        .clamp(SpeedRatio::MIN as u32, SpeedRatio::MAX as u32)
                        as u16,
                },
            ],
        },
        AutomationLane {
            target: AutomationTarget::FxDepth,
            interpolation: AutomationInterpolation::Linear,
            enabled: true,
            points: vec![
                AutomationPoint {
                    offset: BeatTime::ZERO,
                    value: fx_depth_permille,
                },
                AutomationPoint {
                    offset: half,
                    value: ((fx_depth_permille as u32 * 125) / 100).min(1000) as u16,
                },
                AutomationPoint {
                    offset: tail,
                    value: ((fx_depth_permille as u32 * 76) / 100) as u16,
                },
            ],
        },
    ]
}

fn demo_cues() -> Vec<Cue> {
    vec![
        Cue {
            id: CueId(1),
            name: "Drop Cue".to_owned(),
            phase: CuePhase::Active,
            linked_clip: Some(ClipId(102)),
            color: RgbaColor::rgb(255, 196, 120),
            fade_duration: BeatTime::from_fraction(1, 2),
            elapsed: BeatTime::ZERO,
        },
        Cue {
            id: CueId(2),
            name: "Opener Cue".to_owned(),
            phase: CuePhase::Armed,
            linked_clip: Some(ClipId(101)),
            color: RgbaColor::rgb(117, 234, 214),
            fade_duration: BeatTime::from_fraction(1, 1),
            elapsed: BeatTime::ZERO,
        },
        Cue {
            id: CueId(3),
            name: "Strobe Cue".to_owned(),
            phase: CuePhase::Armed,
            linked_clip: Some(ClipId(201)),
            color: RgbaColor::rgb(255, 138, 153),
            fade_duration: BeatTime::from_fraction(1, 4),
            elapsed: BeatTime::ZERO,
        },
        Cue {
            id: CueId(4),
            name: "Break Cue".to_owned(),
            phase: CuePhase::Stored,
            linked_clip: Some(ClipId(202)),
            color: RgbaColor::rgb(255, 221, 159),
            fade_duration: BeatTime::from_fraction(3, 4),
            elapsed: BeatTime::ZERO,
        },
    ]
}

fn demo_chases() -> Vec<Chase> {
    vec![
        Chase {
            id: ChaseId(1),
            name: "Flash Chase".to_owned(),
            phase: ChasePhase::Playing,
            direction: ChaseDirection::Forward,
            current_step: 0,
            progress: BeatTime::ZERO,
            loop_enabled: true,
            linked_clip: Some(ClipId(201)),
            steps: vec![
                ChaseStep {
                    label: "L".to_owned(),
                    cue_id: Some(CueId(3)),
                    duration: BeatTime::from_fraction(1, 2),
                    color: RgbaColor::rgb(255, 238, 214),
                },
                ChaseStep {
                    label: "C".to_owned(),
                    cue_id: Some(CueId(1)),
                    duration: BeatTime::from_fraction(1, 2),
                    color: RgbaColor::rgb(255, 170, 170),
                },
                ChaseStep {
                    label: "R".to_owned(),
                    cue_id: Some(CueId(3)),
                    duration: BeatTime::from_fraction(1, 2),
                    color: RgbaColor::rgb(255, 244, 214),
                },
            ],
        },
        Chase {
            id: ChaseId(2),
            name: "Reverse Sweep".to_owned(),
            phase: ChasePhase::Stopped,
            direction: ChaseDirection::Reverse,
            current_step: 2,
            progress: BeatTime::ZERO,
            loop_enabled: true,
            linked_clip: Some(ClipId(402)),
            steps: vec![
                ChaseStep {
                    label: "Back".to_owned(),
                    cue_id: Some(CueId(4)),
                    duration: BeatTime::from_fraction(3, 4),
                    color: RgbaColor::rgb(204, 218, 255),
                },
                ChaseStep {
                    label: "Sweep".to_owned(),
                    cue_id: Some(CueId(2)),
                    duration: BeatTime::from_fraction(3, 4),
                    color: RgbaColor::rgb(255, 232, 191),
                },
                ChaseStep {
                    label: "Hit".to_owned(),
                    cue_id: Some(CueId(1)),
                    duration: BeatTime::from_fraction(3, 4),
                    color: RgbaColor::rgb(255, 196, 133),
                },
            ],
        },
    ]
}

fn demo_fx_layers() -> Vec<FxLayer> {
    vec![
        FxLayer {
            id: FxId(1),
            name: "Color Morph".to_owned(),
            phase: FxPhase::Composed,
            kind: FxKind::Color,
            linked_clip: Some(ClipId(102)),
            enabled: true,
            depth_permille: 860,
            rate: SpeedRatio::from_permille(1180),
            spread_permille: 720,
            phase_offset_permille: 140,
            waveform: FxWaveform::Sine,
            bpm_sync: SnapResolution::QuarterBeat,
            output_level: 780,
        },
        FxLayer {
            id: FxId(2),
            name: "Intensity Pulse".to_owned(),
            phase: FxPhase::Applied,
            kind: FxKind::Intensity,
            linked_clip: Some(ClipId(101)),
            enabled: true,
            depth_permille: 640,
            rate: SpeedRatio::from_permille(860),
            spread_permille: 520,
            phase_offset_permille: 420,
            waveform: FxWaveform::Pulse,
            bpm_sync: SnapResolution::EighthBeat,
            output_level: 522,
        },
        FxLayer {
            id: FxId(3),
            name: "Tilt Sweep".to_owned(),
            phase: FxPhase::Idle,
            kind: FxKind::Position,
            linked_clip: Some(ClipId(301)),
            enabled: false,
            depth_permille: 480,
            rate: SpeedRatio::from_permille(740),
            spread_permille: 680,
            phase_offset_permille: 260,
            waveform: FxWaveform::Triangle,
            bpm_sync: SnapResolution::HalfBeat,
            output_level: 0,
        },
    ]
}

fn demo_fixture_groups() -> Vec<FixtureGroup> {
    vec![
        FixtureGroup {
            id: FixtureGroupId(1),
            name: "Heads".to_owned(),
            phase: FixturePhase::Active,
            fixture_count: 16,
            online: 16,
            linked_cue: Some(CueId(1)),
            linked_fx: Some(FxId(1)),
            accent: RgbaColor::rgb(117, 234, 214),
            output_level: 810,
            preview_nodes: preview_nodes(
                &["L1", "L2", "R1", "R2"],
                &[
                    (180, 160, 720),
                    (330, 260, 810),
                    (680, 220, 760),
                    (830, 120, 690),
                ],
            ),
        },
        FixtureGroup {
            id: FixtureGroupId(2),
            name: "Wash".to_owned(),
            phase: FixturePhase::Mapped,
            fixture_count: 8,
            online: 8,
            linked_cue: Some(CueId(2)),
            linked_fx: Some(FxId(2)),
            accent: RgbaColor::rgb(104, 164, 255),
            output_level: 540,
            preview_nodes: preview_nodes(
                &["W1", "W2", "W3"],
                &[(240, 620, 460), (500, 700, 420), (760, 610, 480)],
            ),
        },
        FixtureGroup {
            id: FixtureGroupId(3),
            name: "Strobes".to_owned(),
            phase: FixturePhase::Mapped,
            fixture_count: 4,
            online: 4,
            linked_cue: Some(CueId(3)),
            linked_fx: None,
            accent: RgbaColor::rgb(255, 196, 120),
            output_level: 360,
            preview_nodes: preview_nodes(&["S1", "S2"], &[(400, 420, 360), (620, 420, 360)]),
        },
    ]
}

fn preview_nodes(labels: &[&str], coords: &[(u16, u16, u16)]) -> Vec<FixturePreviewNode> {
    labels
        .iter()
        .zip(coords.iter())
        .map(|(label, (x, y, z))| FixturePreviewNode {
            label: (*label).to_owned(),
            x_permille: *x,
            y_permille: *y,
            z_permille: *z,
        })
        .collect()
}
