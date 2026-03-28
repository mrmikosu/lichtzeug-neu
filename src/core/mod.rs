mod automation;
mod editor;
mod engine;
mod engine_link;
mod event;
mod fixtures;
mod hardware;
mod history;
mod ids;
mod output;
mod project;
mod queue;
mod reducer;
mod show;
mod spec;
mod state;
mod time;
mod validation;

pub use automation::{
    clamp_automation_value, effective_clip_parameters, ensure_lane, evaluate_lane_value, lane,
    lane_mut, sort_lane_points,
};
pub use editor::{
    add_clip_editor_automation_point, advance_clip_editor, close_clip_editor,
    delete_clip_editor_automation_point, nudge_clip_editor_automation_point, open_clip_editor,
    select_clip_editor_automation_point, set_clip_editor_automation_mode,
    set_clip_editor_automation_target, set_clip_editor_automation_value, set_clip_editor_chase,
    set_clip_editor_cue, set_clip_editor_fx_depth, set_clip_editor_grid, set_clip_editor_intensity,
    set_clip_editor_speed, toggle_clip_editor_automation_lane,
};
pub use engine::{advance_engine_frame, enter_sync_phase, resume_after_sync, toggle_transport};
pub use engine_link::{
    DEFAULT_STAGELINQ_DISCOVERY_PORT, merge_engine_device, parse_engine_discovery_packet,
    select_followed_deck,
};
pub use event::{AppEvent, StateDiff, TimelineCursor, TimelineEvent, TimelineHit, TimelineZone};
pub use fixtures::{
    build_ofl_download_url, export_qxf_fixture, fixture_mode_channel_count,
    fixture_patch_channel_count, import_ofl_fixture, import_qxf_fixture,
};
pub use hardware::{
    controller_profile_bindings, controller_profile_from_name, decode_midi_bytes,
    is_trigger_message_active, midi_control_hint, midi_port_id, midi_value_permille,
    normalize_midi_binding_message, scan_dmx_interfaces, scan_hardware_inventory, scan_midi_ports,
};
pub use history::{
    apply_redo, apply_undo, begin_history_transaction, capture_history_snapshot,
    clear_pending_history, commit_history_transaction, record_history_entry,
    restore_history_snapshot,
};
pub use ids::{ChaseId, ClipId, CueId, FixtureGroupId, FxId, TrackId};
pub use output::{
    DmxUniverseFrame, MidiFeedbackMonitor, MidiFeedbackPacket, OutputDispatchFailure,
    OutputMonitorSnapshot, OutputUniverseMonitor, RuntimeOutputSnapshot,
    build_output_monitor_snapshot, build_runtime_output_snapshot, deliver_runtime_outputs,
};
pub use project::{
    ProjectFile, RecoveryRegistry, RecoverySlotFile, ReplayLogFile, VentureFile, VentureRegistry,
    delete_venture, ensure_venture_directory, export_project_json, export_replay_log_json,
    import_project_json, list_ventures, load_recovery_registry, load_venture,
    load_venture_registry, next_venture_name, rename_venture, replay_from_log_json,
    restore_recovery_slot, save_recovery_slot, save_venture, save_venture_as,
};
pub use queue::{
    EventHistoryEntry, EventQueueState, EventSystemPhase, ProcessedEvent, QueuedEvent,
    complete_current_event, enqueue_event, mark_event_dispatched, start_next_event,
};
pub use reducer::{dispatch, replay_events};
pub use show::{
    advance_show_frame, arm_cue, reverse_chase, select_fixture_group, select_fx, set_fx_depth,
    set_fx_phase_offset, set_fx_rate, set_fx_spread, set_fx_waveform, toggle_chase, toggle_fx,
    trigger_cue,
};
pub use spec::{
    ContractSpec, FsmSpec, FunctionContractSpec, FunctionSpec, MachineReadableSection, ModuleSpec,
    ValidationSpec, foundation_spec, foundation_spec_json,
};
pub use state::{
    AutomationInterpolation, AutomationLane, AutomationPoint, AutomationTarget, Chase,
    ChaseDirection, ChasePhase, ChaseStep, ChaseSystemState, Clip, ClipEditorPhase,
    ClipEditorState, ClipInlineParameterKind, ClipMarker, ClipPalette, ClipParameters, ClipPhase,
    ClipboardClip, ClipboardState, ContextMenuAction, ContextMenuState, ContextMenuTarget,
    ControllerProfileKind, CpuLoad, Cue, CuePhase, CueSystemState, CueVisualState, DmxBackendKind,
    DmxInterfaceDescriptor, DmxInterfaceKind, DmxSettingsState, EngineDeckFollowMode,
    EngineDeckPhase, EngineDeckTelemetry, EngineErrorState, EngineLinkMode, EngineLinkPhase,
    EngineLinkState, EngineMixerTelemetry, EnginePhase, EnginePrimeDevice, EngineResumeTarget,
    EngineServiceDescriptor, EngineState, EngineTelemetryFrame, FixtureCapability, FixtureChannel,
    FixtureGroup, FixtureGroupPatchSummary, FixtureLibraryPhase, FixtureLibraryState, FixtureMode,
    FixturePatch, FixturePhase, FixturePhysical, FixturePreviewNode, FixtureProfile,
    FixtureSourceInfo, FixtureSourceKind, FixtureSystemState, FixtureUniverseSummary, FxKind,
    FxLayer, FxPhase, FxSystemState, FxWaveform, HardwareDiscoveryPhase, HardwareInventorySnapshot,
    HistoryEntry, HistoryPhase, HistorySnapshot, HistoryState, HistoryTimelineSnapshot,
    HoverTarget, InputModifiersState, MidiAction, MidiBinding, MidiBindingMessage, MidiControlHint,
    MidiLearnPhase, MidiLearnState, MidiMessageKind, MidiPortDescriptor, MidiPortDirection,
    MidiRuntimeMessage, MidiSettingsState, OutputDeliveryPhase, OutputDispatchReport,
    OutputRuntimeState, PendingHistoryEntry, PerformanceState, RecoverySlotDescriptor,
    RenderRevisionState, ReplayLogState, SelectionState, SettingsState, SettingsTab, SnapGuide,
    SnapPhase, SnapResolution, SnapState, StateLifecycle, StatusBarState, StudioState,
    TIMELINE_CLIP_HEIGHT_PX, TIMELINE_CLIP_TOP_INSET_PX, TIMELINE_HEADER_HEIGHT_PX,
    TIMELINE_TRACK_GAP_PX, TIMELINE_TRACK_HEIGHT_PX, TimelineInteraction, TimelinePhase,
    TimelineState, TimelineViewport, Track, TransportState, VentureDescriptor, VenturePhase,
    VentureState,
};
pub use time::{
    BAR_BEATS, BeatTime, IntensityLevel, MonotonicClock, PPQ, RgbaColor, SpeedRatio, TempoBpm,
    ZoomFactor,
};
pub use validation::{
    ValidationIssue, ValidationIssueKind, ValidationReport, recover_state, validate_state,
};
