use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FsmSpec {
    pub states: Vec<String>,
    pub transitions: Vec<TransitionSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionSpec {
    pub from: String,
    pub to: String,
    pub event: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractSpec {
    pub invariants: Vec<String>,
    pub pre_conditions: Vec<String>,
    pub post_conditions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionContractSpec {
    pub name: String,
    pub input_types: Vec<String>,
    pub output_type: String,
    pub contract: ContractSpec,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionSpec {
    pub module: String,
    pub functions: Vec<FunctionContractSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationSpec {
    pub required_checks: Vec<String>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleSpec {
    pub name: String,
    pub goal: String,
    pub fsm: FsmSpec,
    pub state_type: String,
    pub input_type: String,
    pub output_type: String,
    pub contracts: ContractSpec,
    pub tests: Vec<String>,
    pub implementation_files: Vec<String>,
    pub validation: ValidationSpec,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineReadableSection {
    pub architecture_goal: String,
    pub project_structure: Vec<String>,
    pub modules: Vec<ModuleSpec>,
    pub functions: Vec<FunctionSpec>,
}

pub fn foundation_spec() -> MachineReadableSection {
    MachineReadableSection {
        architecture_goal: "State -> Engine -> Event System -> UI -> Input -> State".to_owned(),
        project_structure: vec![
            "src/lib.rs".to_owned(),
            "src/app.rs".to_owned(),
            "src/core/mod.rs".to_owned(),
            "src/core/time.rs".to_owned(),
            "src/core/state.rs".to_owned(),
            "src/core/event.rs".to_owned(),
            "src/core/automation.rs".to_owned(),
            "src/core/history.rs".to_owned(),
            "src/core/project.rs".to_owned(),
            "src/core/queue.rs".to_owned(),
            "src/core/engine.rs".to_owned(),
            "src/core/reducer.rs".to_owned(),
            "src/core/editor.rs".to_owned(),
            "src/core/show.rs".to_owned(),
            "src/core/validation.rs".to_owned(),
            "src/core/spec.rs".to_owned(),
            "src/ui/mod.rs".to_owned(),
            "src/ui/fixture_view.rs".to_owned(),
            "src/ui/timeline.rs".to_owned(),
        ],
        modules: vec![
            module_state_system(),
            module_event_system(),
            module_history_system(),
            module_edit_commands(),
            module_automation_system(),
            module_clipboard_workflow(),
            module_venture_management(),
            module_persistence_replay(),
            module_engine(),
            module_timeline(),
            module_clip_editor(),
            module_cue_system(),
            module_chase_system(),
            module_fx_system(),
            module_fixture_system(),
            module_project_structure(),
        ],
        functions: vec![
            FunctionSpec {
                module: "StateSystem".to_owned(),
                functions: vec![FunctionContractSpec {
                    name: "validate_state".to_owned(),
                    input_types: vec!["&StudioState".to_owned()],
                    output_type: "ValidationReport".to_owned(),
                    contract: ContractSpec {
                        invariants: vec![
                            "ClipId und TrackId sind eindeutig.".to_owned(),
                            "Playhead, Scroll und Snap-Guide liegen innerhalb der Songlänge."
                                .to_owned(),
                        ],
                        pre_conditions: vec!["State ist vollständig initialisiert.".to_owned()],
                        post_conditions: vec![
                            "Alle festgestellten Invariantenverletzungen sind im Report enthalten."
                                .to_owned(),
                        ],
                    },
                }],
            },
            FunctionSpec {
                module: "EventSystem".to_owned(),
                functions: vec![FunctionContractSpec {
                    name: "enqueue_event".to_owned(),
                    input_types: vec!["&mut EventQueueState".to_owned(), "AppEvent".to_owned()],
                    output_type: "u64".to_owned(),
                    contract: ContractSpec {
                        invariants: vec!["Sequenzen steigen strikt monoton.".to_owned()],
                        pre_conditions: vec!["EventQueueState ist valide.".to_owned()],
                        post_conditions: vec![
                            "Das Event liegt genau einmal in der Queue.".to_owned(),
                            "Der Rückgabewert ist die vergebene Sequenznummer.".to_owned(),
                        ],
                    },
                }],
            },
            FunctionSpec {
                module: "HistorySystem".to_owned(),
                functions: vec![FunctionContractSpec {
                    name: "apply_undo".to_owned(),
                    input_types: vec!["&mut StudioState".to_owned()],
                    output_type: "Vec<StateDiff>".to_owned(),
                    contract: ContractSpec {
                        invariants: vec![
                            "Undo- und Redo-Stacks bleiben kapazitaetsbegrenzt und geordnet."
                                .to_owned(),
                            "Snapshots stellen nur typisierte, serialisierbare State-Teile wieder her."
                                .to_owned(),
                        ],
                        pre_conditions: vec![
                            "Es existiert mindestens ein Undo-Eintrag oder die Funktion ist no-op."
                                .to_owned(),
                        ],
                        post_conditions: vec![
                            "Der zuletzt aufgezeichnete Authoring-State ist deterministisch wiederhergestellt."
                                .to_owned(),
                            "Der Eintrag wurde in den Redo-Stack ueberfuehrt.".to_owned(),
                        ],
                    },
                }],
            },
            FunctionSpec {
                module: "EditCommands".to_owned(),
                functions: vec![FunctionContractSpec {
                    name: "duplicate_selected_clips".to_owned(),
                    input_types: vec!["&mut StudioState".to_owned()],
                    output_type: "Vec<StateDiff>".to_owned(),
                    contract: ContractSpec {
                        invariants: vec![
                            "Neue ClipIds sind eindeutig und monoton aus dem aktuellen Clip-Bestand abgeleitet."
                                .to_owned(),
                            "Mehrfachauswahl bleibt nach Edit-Commands deterministisch sortiert."
                                .to_owned(),
                        ],
                        pre_conditions: vec![
                            "Die selektierten Clips existieren im Timeline-State.".to_owned(),
                        ],
                        post_conditions: vec![
                            "Duplicate, Split und Delete verlassen den State referenzkonsistent."
                                .to_owned(),
                        ],
                    },
                }],
            },
            FunctionSpec {
                module: "AutomationSystem".to_owned(),
                functions: vec![FunctionContractSpec {
                    name: "evaluate_lane_value".to_owned(),
                    input_types: vec![
                        "&Clip".to_owned(),
                        "AutomationTarget".to_owned(),
                        "BeatTime".to_owned(),
                    ],
                    output_type: "Option<u16>".to_owned(),
                    contract: ContractSpec {
                        invariants: vec![
                            "Automation-Punkte bleiben nach Target-Ranges geklemmt.".to_owned(),
                            "Lineare und Step-Interpolation sind rein aus Lane-Daten ableitbar."
                                .to_owned(),
                        ],
                        pre_conditions: vec![
                            "Die Lane gehört deterministisch zu einem Clip oder ist None."
                                .to_owned(),
                        ],
                        post_conditions: vec![
                            "Gleiche Lane-Daten und gleicher Clip-Zeitpunkt erzeugen denselben Wert."
                                .to_owned(),
                        ],
                    },
                }],
            },
            FunctionSpec {
                module: "ClipboardWorkflow".to_owned(),
                functions: vec![FunctionContractSpec {
                    name: "paste_clipboard_at_playhead".to_owned(),
                    input_types: vec!["&mut StudioState".to_owned()],
                    output_type: "Vec<StateDiff>".to_owned(),
                    contract: ContractSpec {
                        invariants: vec![
                            "Clipboard-Einträge behalten ihre Track-Zuordnung und relativen Offsets."
                                .to_owned(),
                            "Wiederholtes Paste am gleichen Anchor nutzt einen deterministischen Offset-Index."
                                .to_owned(),
                        ],
                        pre_conditions: vec![
                            "Clipboard enthält typisierte Clips oder die Funktion ist no-op."
                                .to_owned(),
                        ],
                        post_conditions: vec![
                            "Eingefügte Clips liegen vollständig innerhalb der Songlänge."
                                .to_owned(),
                        ],
                    },
                }],
            },
            FunctionSpec {
                module: "VentureManagement".to_owned(),
                functions: vec![
                    FunctionContractSpec {
                        name: "save_venture".to_owned(),
                        input_types: vec![
                            "&StudioState".to_owned(),
                            "impl AsRef<Path>".to_owned(),
                            "Option<&str>".to_owned(),
                            "&str".to_owned(),
                        ],
                        output_type: "Result<VentureDescriptor, String>".to_owned(),
                        contract: ContractSpec {
                            invariants: vec![
                                "Jedes Venture wird genau einer JSON-Datei mit stabiler Venture-Id zugeordnet."
                                    .to_owned(),
                                "Refresh, Save und Load bleiben vom Authoring-Replay-Log getrennt."
                                    .to_owned(),
                            ],
                            pre_conditions: vec![
                                "Der Venture-Name ist nicht leer oder die Funktion liefert einen Fehler."
                                    .to_owned(),
                            ],
                            post_conditions: vec![
                                "Nach erfolgreichem Speichern ist das Venture über list_ventures deterministisch wieder auffindbar."
                                    .to_owned(),
                            ],
                        },
                    },
                    FunctionContractSpec {
                        name: "load_venture_registry".to_owned(),
                        input_types: vec!["impl AsRef<Path>".to_owned()],
                        output_type: "Result<VentureRegistry, String>".to_owned(),
                        contract: ContractSpec {
                            invariants: vec![
                                "Beschädigte Venture-Dateien blockieren die Registry nicht, sondern werden als Issues gesammelt."
                                    .to_owned(),
                                "Die Registry-Reihenfolge bleibt deterministisch sortiert.".to_owned(),
                            ],
                            pre_conditions: vec![
                                "Das Venture-Verzeichnis ist lesbar oder die Funktion liefert einen Fehler."
                                    .to_owned(),
                            ],
                            post_conditions: vec![
                                "Alle lesbaren Ventures erscheinen genau einmal in ventures, alle beschädigten Dateien genau einmal in issues."
                                    .to_owned(),
                            ],
                        },
                    },
                ],
            },
            FunctionSpec {
                module: "PersistenceReplay".to_owned(),
                functions: vec![FunctionContractSpec {
                    name: "export_project_json".to_owned(),
                    input_types: vec!["&StudioState".to_owned()],
                    output_type: "String".to_owned(),
                    contract: ContractSpec {
                        invariants: vec![
                            "Projektdatei enthält nur serialisierbare, deterministische State-Anteile."
                                .to_owned(),
                            "Replay-Log bleibt in derselben Reihenfolge erhalten.".to_owned(),
                        ],
                        pre_conditions: vec!["State ist validiert oder recoverable.".to_owned()],
                        post_conditions: vec![
                            "Exportiertes JSON roundtript über den Import ohne Mehrdeutigkeit."
                                .to_owned(),
                        ],
                    },
                }],
            },
            FunctionSpec {
                module: "Engine".to_owned(),
                functions: vec![FunctionContractSpec {
                    name: "advance_engine_frame".to_owned(),
                    input_types: vec!["&mut StudioState".to_owned()],
                    output_type: "Vec<StateDiff>".to_owned(),
                    contract: ContractSpec {
                        invariants: vec![
                            "Clock ist monoton.".to_owned(),
                            "Gleicher Ausgangs-State erzeugt gleichen Folge-State.".to_owned(),
                        ],
                        pre_conditions: vec!["State ist valide.".to_owned()],
                        post_conditions: vec![
                            "Frame-Zähler wurde erhöht.".to_owned(),
                            "Playhead folgt der BPM-basierten, zentralen Zeitbasis.".to_owned(),
                        ],
                    },
                }],
            },
            FunctionSpec {
                module: "Timeline".to_owned(),
                functions: vec![FunctionContractSpec {
                    name: "dispatch".to_owned(),
                    input_types: vec!["&mut StudioState".to_owned(), "AppEvent".to_owned()],
                    output_type: "()".to_owned(),
                    contract: ContractSpec {
                        invariants: vec![
                            "Timeline-Mutationen passieren nur über Events.".to_owned(),
                            "Snap ist deterministisch quantisiert.".to_owned(),
                        ],
                        pre_conditions: vec!["Event ist typisiert und serialisierbar.".to_owned()],
                        post_conditions: vec![
                            "Alle Queue-Events wurden in deterministischer Reihenfolge verarbeitet."
                                .to_owned(),
                        ],
                    },
                }],
            },
            FunctionSpec {
                module: "ClipEditor".to_owned(),
                functions: vec![FunctionContractSpec {
                    name: "set_clip_editor_fx_depth".to_owned(),
                    input_types: vec!["&mut StudioState".to_owned(), "u16".to_owned()],
                    output_type: "Vec<StateDiff>".to_owned(),
                    contract: ContractSpec {
                        invariants: vec![
                            "Editor kann nur einen existierenden Clip bearbeiten.".to_owned(),
                            "Clip-Parameter bleiben typisiert und geklemmt.".to_owned(),
                        ],
                        pre_conditions: vec!["ClipEditorState ist offen.".to_owned()],
                        post_conditions: vec![
                            "Der bearbeitete Clip wechselt in Previewing und erzeugt deterministische Diffs."
                                .to_owned(),
                        ],
                    },
                }],
            },
            FunctionSpec {
                module: "CueSystem".to_owned(),
                functions: vec![
                    FunctionContractSpec {
                        name: "trigger_cue".to_owned(),
                        input_types: vec!["&mut StudioState".to_owned(), "CueId".to_owned()],
                        output_type: "Vec<StateDiff>".to_owned(),
                        contract: ContractSpec {
                            invariants: vec![
                                "Maximal ein Cue ist Trigger-Quelle des aktuellen Aktiv-Zustands."
                                    .to_owned(),
                                "Clip-Cue-Markierungen werden aus Cue-Phasen abgeleitet.".to_owned(),
                            ],
                            pre_conditions: vec!["CueId existiert.".to_owned()],
                            post_conditions: vec![
                                "Ziel-Cue ist Triggered oder Active.".to_owned(),
                                "Vorher aktive Cues wechseln deterministisch in Fading.".to_owned(),
                            ],
                        },
                    },
                    FunctionContractSpec {
                        name: "delete_selected_cue".to_owned(),
                        input_types: vec!["&mut StudioState".to_owned()],
                        output_type: "Vec<StateDiff>".to_owned(),
                        contract: ContractSpec {
                            invariants: vec![
                                "Cue-Loeschungen bereinigen Clip-, Fixture- und Chase-Verweise deterministisch."
                                    .to_owned(),
                            ],
                            pre_conditions: vec!["Ein Cue ist selektiert.".to_owned()],
                            post_conditions: vec![
                                "Selektion springt deterministisch auf einen gueltigen Nachbar-Cue oder None."
                                    .to_owned(),
                            ],
                        },
                    },
                ],
            },
            FunctionSpec {
                module: "ChaseSystem".to_owned(),
                functions: vec![
                    FunctionContractSpec {
                        name: "toggle_chase".to_owned(),
                        input_types: vec!["&mut StudioState".to_owned(), "ChaseId".to_owned()],
                        output_type: "Vec<StateDiff>".to_owned(),
                        contract: ContractSpec {
                            invariants: vec![
                                "Step-Fortschritt bleibt innerhalb der Chase-Step-Liste.".to_owned(),
                                "Step-Wechsel sind durch BeatTime quantisiert.".to_owned(),
                            ],
                            pre_conditions: vec!["ChaseId existiert.".to_owned()],
                            post_conditions: vec![
                                "Die Chase wechselt deterministisch zwischen Playing/Reversing und Stopped."
                                    .to_owned(),
                            ],
                        },
                    },
                    FunctionContractSpec {
                        name: "set_selected_chase_step_duration".to_owned(),
                        input_types: vec!["&mut StudioState".to_owned(), "BeatTime".to_owned()],
                        output_type: "Vec<StateDiff>".to_owned(),
                        contract: ContractSpec {
                            invariants: vec![
                                "Chase-Step-Dauern bleiben >= MIN_CLIP_DURATION.".to_owned(),
                            ],
                            pre_conditions: vec!["Chase und Chase-Step sind selektiert.".to_owned()],
                            post_conditions: vec![
                                "Step-Dauer ist geklemmt und die Chase bleibt konsistent.".to_owned(),
                            ],
                        },
                    },
                ],
            },
            FunctionSpec {
                module: "FxSystem".to_owned(),
                functions: vec![FunctionContractSpec {
                    name: "set_fx_depth".to_owned(),
                    input_types: vec![
                        "&mut StudioState".to_owned(),
                        "FxId".to_owned(),
                        "u16".to_owned(),
                    ],
                    output_type: "Vec<StateDiff>".to_owned(),
                    contract: ContractSpec {
                        invariants: vec![
                            "Depth und Output bleiben in 0..=1000.".to_owned(),
                            "FX-Phasen folgen Clip- und Master-Zustand deterministisch.".to_owned(),
                        ],
                        pre_conditions: vec!["FxId existiert.".to_owned()],
                        post_conditions: vec![
                            "Der FX-Layer ist selektiert und sein Depth-Wert ist geklemmt."
                                .to_owned(),
                        ],
                    },
                }],
            },
            FunctionSpec {
                module: "FixtureSystem".to_owned(),
                functions: vec![FunctionContractSpec {
                    name: "select_fixture_group".to_owned(),
                    input_types: vec![
                        "&mut StudioState".to_owned(),
                        "FixtureGroupId".to_owned(),
                    ],
                    output_type: "Vec<StateDiff>".to_owned(),
                    contract: ContractSpec {
                        invariants: vec![
                            "Fixture-Ausgänge bleiben in 0..=1000.".to_owned(),
                            "Fixture-Status folgt Cue- und FX-Quellen deterministisch.".to_owned(),
                        ],
                        pre_conditions: vec!["FixtureGroupId existiert.".to_owned()],
                        post_conditions: vec![
                            "Die selektierte Fixture-Gruppe ist eindeutig gesetzt.".to_owned(),
                        ],
                    },
                }],
            },
        ],
    }
}

pub fn foundation_spec_json() -> String {
    serde_json::to_string_pretty(&foundation_spec()).expect("serialize foundation spec")
}

fn validation_spec() -> ValidationSpec {
    ValidationSpec {
        required_checks: vec![
            "Typkonsistenz".to_owned(),
            "Zustandskonsistenz".to_owned(),
            "Determinismus".to_owned(),
            "Referenzintegrität".to_owned(),
            "Timing-Konsistenz".to_owned(),
        ],
        status: "valid".to_owned(),
    }
}

fn module_project_structure() -> ModuleSpec {
    ModuleSpec {
        name: "ProjectStructure".to_owned(),
        goal: "Klare Trennung von State, Engine, Queue, Validierung und UI.".to_owned(),
        fsm: FsmSpec {
            states: vec!["Defined".to_owned(), "Integrated".to_owned()],
            transitions: vec![TransitionSpec {
                from: "Defined".to_owned(),
                to: "Integrated".to_owned(),
                event: "ModulesConnected".to_owned(),
            }],
        },
        state_type: "WorkspaceLayout".to_owned(),
        input_type: "ModuleGraph".to_owned(),
        output_type: "DeterministicProjectTopology".to_owned(),
        contracts: ContractSpec {
            invariants: vec!["Keine zyklischen Modulabhängigkeiten.".to_owned()],
            pre_conditions: vec!["Alle Kernmodule existieren.".to_owned()],
            post_conditions: vec!["Core-Module sind über src/lib.rs exportiert.".to_owned()],
        },
        tests: vec![
            "integration_project_structure_exports_core_modules".to_owned(),
            "machine_readable_spec_roundtrip".to_owned(),
        ],
        implementation_files: vec![
            "src/lib.rs".to_owned(),
            "src/core/mod.rs".to_owned(),
            "src/core/spec.rs".to_owned(),
        ],
        validation: validation_spec(),
    }
}

fn module_state_system() -> ModuleSpec {
    ModuleSpec {
        name: "StateSystem".to_owned(),
        goal: "Vollständig typisierter, serialisierbarer, deterministischer Anwendungszustand."
            .to_owned(),
        fsm: FsmSpec {
            states: vec![
                "Initializing".to_owned(),
                "Valid".to_owned(),
                "Updating".to_owned(),
                "Invalid".to_owned(),
                "Recovered".to_owned(),
            ],
            transitions: vec![
                transition("Initializing", "Valid", "BootstrapValidated"),
                transition("Valid", "Updating", "EventDequeued"),
                transition("Updating", "Valid", "ValidationPassed"),
                transition("Updating", "Invalid", "ValidationFailed"),
                transition("Invalid", "Recovered", "RecoveryApplied"),
                transition("Recovered", "Valid", "RevalidationPassed"),
            ],
        },
        state_type: "StudioState".to_owned(),
        input_type: "AppEvent".to_owned(),
        output_type: "ValidatedStudioState".to_owned(),
        contracts: ContractSpec {
            invariants: vec![
                "Keine zyklischen Referenzen.".to_owned(),
                "Alle IDs sind gültig.".to_owned(),
                "Monotone, zentrale Zeitbasis.".to_owned(),
                "Clip-Mehrfachauswahl bleibt eindeutig, referenzstabil und deterministisch sortiert."
                    .to_owned(),
            ],
            pre_conditions: vec!["Input-State ist vollständig typisiert.".to_owned()],
            post_conditions: vec!["Ausgabe-State erfüllt alle Kerninvarianten.".to_owned()],
        },
        tests: vec![
            "validation_detects_missing_selected_clip".to_owned(),
            "recovery_clears_invalid_selection".to_owned(),
            "integration_box_selection_replays_deterministically".to_owned(),
            "replay_produces_identical_state_snapshot".to_owned(),
        ],
        implementation_files: vec![
            "src/core/state.rs".to_owned(),
            "src/core/validation.rs".to_owned(),
        ],
        validation: validation_spec(),
    }
}

fn module_event_system() -> ModuleSpec {
    ModuleSpec {
        name: "EventSystem".to_owned(),
        goal: "Strikt typisierte Event-Queue mit deterministischer Reihenfolge.".to_owned(),
        fsm: FsmSpec {
            states: vec![
                "Idle".to_owned(),
                "EventQueued".to_owned(),
                "Processing".to_owned(),
                "Dispatched".to_owned(),
                "Completed".to_owned(),
            ],
            transitions: vec![
                transition("Idle", "EventQueued", "Enqueue"),
                transition("EventQueued", "Processing", "StartNext"),
                transition("Processing", "Dispatched", "Reduce"),
                transition("Dispatched", "Completed", "Complete"),
                transition("Completed", "Idle", "QueueEmpty"),
            ],
        },
        state_type: "EventQueueState".to_owned(),
        input_type: "AppEvent".to_owned(),
        output_type: "QueuedEvent".to_owned(),
        contracts: ContractSpec {
            invariants: vec!["Sequenznummern sind streng monoton.".to_owned()],
            pre_conditions: vec!["Event ist serialisierbar.".to_owned()],
            post_conditions: vec!["Queue-Reihenfolge ist reproduzierbar.".to_owned()],
        },
        tests: vec![
            "queue_preserves_event_order".to_owned(),
            "integration_queue_drives_reducer_without_reordering".to_owned(),
            "replay_produces_identical_state_snapshot".to_owned(),
        ],
        implementation_files: vec![
            "src/core/event.rs".to_owned(),
            "src/core/queue.rs".to_owned(),
            "src/core/reducer.rs".to_owned(),
        ],
        validation: validation_spec(),
    }
}

fn module_history_system() -> ModuleSpec {
    ModuleSpec {
        name: "HistorySystem".to_owned(),
        goal: "Deterministisches Undo/Redo ueber kapazitaetsbegrenzte, serialisierbare Snapshots."
            .to_owned(),
        fsm: FsmSpec {
            states: vec![
                "Idle".to_owned(),
                "Tracking".to_owned(),
                "UndoApplied".to_owned(),
                "RedoApplied".to_owned(),
            ],
            transitions: vec![
                transition("Idle", "Tracking", "BeginHistoryTransaction"),
                transition("Tracking", "Idle", "CommitHistoryTransaction"),
                transition("Tracking", "Idle", "ClearPendingHistory"),
                transition("Idle", "UndoApplied", "Undo"),
                transition("UndoApplied", "Idle", "HistoryCommitted"),
                transition("Idle", "RedoApplied", "Redo"),
                transition("RedoApplied", "Idle", "HistoryCommitted"),
            ],
        },
        state_type: "HistoryState".to_owned(),
        input_type: "AppEvent::Undo | AppEvent::Redo | TimelineEvent".to_owned(),
        output_type: "Vec<StateDiff>".to_owned(),
        contracts: ContractSpec {
            invariants: vec![
                "Undo- und Redo-Stacks bleiben streng zeitlich geordnet.".to_owned(),
                "Neue Authoring-Aenderungen leeren den Redo-Stack deterministisch.".to_owned(),
                "Pending-Transaktionen werden nur fuer echte Edit-Gesten gefuehrt.".to_owned(),
            ],
            pre_conditions: vec![
                "History-Snapshots referenzieren nur valide, serialisierbare Teilzustaende."
                    .to_owned(),
            ],
            post_conditions: vec![
                "Undo und Redo stellen denselben Authoring-State fuer dieselbe Eventfolge wieder her."
                    .to_owned(),
            ],
        },
        tests: vec![
            "record_history_entry_clears_redo_and_respects_capacity".to_owned(),
            "undo_restores_clip_drag_as_single_history_step".to_owned(),
            "redo_reapplies_clip_drag_after_undo".to_owned(),
            "selection_only_clip_click_does_not_create_history_entry".to_owned(),
            "new_history_change_clears_redo_stack".to_owned(),
            "integration_undo_redo_replays_deterministically".to_owned(),
        ],
        implementation_files: vec![
            "src/core/history.rs".to_owned(),
            "src/core/state.rs".to_owned(),
            "src/core/reducer.rs".to_owned(),
            "src/ui/mod.rs".to_owned(),
        ],
        validation: validation_spec(),
    }
}

fn module_edit_commands() -> ModuleSpec {
    ModuleSpec {
        name: "EditCommands".to_owned(),
        goal: "Deterministische Duplicate-, Split- und Delete-Workflows fuer ausgewaehlte Clips."
            .to_owned(),
        fsm: FsmSpec {
            states: vec![
                "Idle".to_owned(),
                "Duplicating".to_owned(),
                "Splitting".to_owned(),
                "Deleting".to_owned(),
                "Committed".to_owned(),
            ],
            transitions: vec![
                transition("Idle", "Duplicating", "DuplicateSelectedClips"),
                transition("Idle", "Splitting", "SplitSelectedClipsAtPlayhead"),
                transition("Idle", "Deleting", "DeleteSelectedClips"),
                transition("Duplicating", "Committed", "SelectionUpdated"),
                transition("Splitting", "Committed", "SegmentsCreated"),
                transition("Deleting", "Committed", "ReferencesCleared"),
                transition("Committed", "Idle", "RenderDiffCommitted"),
            ],
        },
        state_type: "TimelineState".to_owned(),
        input_type:
            "AppEvent::DuplicateSelectedClips | AppEvent::SplitSelectedClipsAtPlayhead | AppEvent::DeleteSelectedClips"
                .to_owned(),
        output_type: "Vec<StateDiff>".to_owned(),
        contracts: ContractSpec {
            invariants: vec![
                "Edit-Commands erzeugen keine doppelten ClipIds.".to_owned(),
                "Delete bereinigt Cue-, Chase- und FX-Referenzen auf entfernte Clips deterministisch."
                    .to_owned(),
                "Split erzeugt nur Segmente, die mindestens MIN_CLIP_DURATION lang sind."
                    .to_owned(),
            ],
            pre_conditions: vec![
                "Die Auswahl ist typisiert und referenziert existierende Clips oder fuehrt zu einem no-op."
                    .to_owned(),
            ],
            post_conditions: vec![
                "Alle Edit-Commands bleiben undo/redo-faehig und replay-deterministisch."
                    .to_owned(),
            ],
        },
        tests: vec![
            "duplicate_selected_clips_preserves_group_offset_and_selects_duplicates".to_owned(),
            "split_selected_clip_at_playhead_creates_two_segments".to_owned(),
            "delete_selected_clips_clears_reverse_links".to_owned(),
            "integration_duplicate_selected_clips_replays_deterministically".to_owned(),
            "integration_split_selected_clips_replays_deterministically".to_owned(),
            "integration_delete_selected_clips_replays_deterministically".to_owned(),
        ],
        implementation_files: vec![
            "src/core/event.rs".to_owned(),
            "src/core/reducer.rs".to_owned(),
            "src/core/state.rs".to_owned(),
            "src/ui/mod.rs".to_owned(),
        ],
        validation: validation_spec(),
    }
}

fn module_automation_system() -> ModuleSpec {
    ModuleSpec {
        name: "AutomationSystem".to_owned(),
        goal: "Deterministische Clip-Automation mit typisierten Lanes, Punkten und Interpolation."
            .to_owned(),
        fsm: FsmSpec {
            states: vec![
                "Idle".to_owned(),
                "LaneSelected".to_owned(),
                "PointEditing".to_owned(),
                "Previewing".to_owned(),
                "Committed".to_owned(),
            ],
            transitions: vec![
                transition("Idle", "LaneSelected", "SetClipEditorAutomationTarget"),
                transition(
                    "LaneSelected",
                    "PointEditing",
                    "SelectClipEditorAutomationPoint",
                ),
                transition(
                    "PointEditing",
                    "Previewing",
                    "SetClipEditorAutomationPointValue",
                ),
                transition(
                    "LaneSelected",
                    "Previewing",
                    "AddClipEditorAutomationPointAtPlayhead",
                ),
                transition("Previewing", "Committed", "ShowPreviewApplied"),
                transition("Committed", "Idle", "TickCommitted"),
            ],
        },
        state_type: "Vec<AutomationLane>".to_owned(),
        input_type: "AppEvent::SetClipEditorAutomation* | Tick".to_owned(),
        output_type: "Vec<StateDiff>".to_owned(),
        contracts: ContractSpec {
            invariants: vec![
                "Pro Clip existiert höchstens eine Lane pro AutomationTarget.".to_owned(),
                "Point-Offsets liegen innerhalb der Clip-Dauer.".to_owned(),
                "Point-Werte bleiben im gültigen Bereich des Targets.".to_owned(),
            ],
            pre_conditions: vec![
                "Der Clip-Editor referenziert einen existierenden Clip.".to_owned(),
            ],
            post_conditions: vec![
                "Playback und Preview lesen dieselben Lane-Daten deterministisch aus.".to_owned(),
            ],
        },
        tests: vec![
            "linear_automation_interpolates_deterministically".to_owned(),
            "effective_parameters_fallback_to_clip_defaults".to_owned(),
            "clip_editor_parameter_change_enters_previewing".to_owned(),
        ],
        implementation_files: vec![
            "src/core/automation.rs".to_owned(),
            "src/core/editor.rs".to_owned(),
            "src/core/show.rs".to_owned(),
            "src/ui/timeline.rs".to_owned(),
            "src/core/validation.rs".to_owned(),
        ],
        validation: validation_spec(),
    }
}

fn module_clipboard_workflow() -> ModuleSpec {
    ModuleSpec {
        name: "ClipboardWorkflow".to_owned(),
        goal: "Deterministische Copy-, Cut-, Paste-, Nudge- und Kontext-Edit-Kommandos."
            .to_owned(),
        fsm: FsmSpec {
            states: vec![
                "Empty".to_owned(),
                "Copied".to_owned(),
                "Cut".to_owned(),
                "Pasted".to_owned(),
                "ContextApplied".to_owned(),
            ],
            transitions: vec![
                transition("Empty", "Copied", "CopySelectedClips"),
                transition("Empty", "Cut", "CutSelectedClips"),
                transition("Copied", "Pasted", "PasteClipboardAtPlayhead"),
                transition("Cut", "Pasted", "PasteClipboardAtPlayhead"),
                transition("Pasted", "ContextApplied", "ApplyContextMenuAction"),
                transition("ContextApplied", "Copied", "CopySelectedClips"),
            ],
        },
        state_type: "ClipboardState".to_owned(),
        input_type:
            "AppEvent::CopySelectedClips | AppEvent::CutSelectedClips | AppEvent::PasteClipboardAtPlayhead | AppEvent::NudgeSelectedClips* | AppEvent::ApplyContextMenuAction"
                .to_owned(),
        output_type: "Vec<StateDiff>".to_owned(),
        contracts: ContractSpec {
            invariants: vec![
                "Clipboard-Einträge behalten relative Offsets zur Gruppenspanne.".to_owned(),
                "Paste-Serien am gleichen Playhead bleiben wiederholbar geordnet.".to_owned(),
                "Kontextaktionen bleiben Events und mutieren nie direkt aus der UI.".to_owned(),
            ],
            pre_conditions: vec![
                "Selektion oder Clipboard referenzieren existierende Tracks/Clips oder die Aktion ist no-op."
                    .to_owned(),
            ],
            post_conditions: vec![
                "Clipboard- und Timeline-State bleiben referenzkonsistent und replay-deterministisch."
                    .to_owned(),
            ],
        },
        tests: vec![
            "integration_clipboard_paste_replays_deterministically".to_owned(),
            "integration_context_menu_nudge_replays_deterministically".to_owned(),
            "integration_duplicate_selected_clips_replays_deterministically".to_owned(),
        ],
        implementation_files: vec![
            "src/core/event.rs".to_owned(),
            "src/core/reducer.rs".to_owned(),
            "src/core/state.rs".to_owned(),
            "src/ui/mod.rs".to_owned(),
            "src/ui/timeline.rs".to_owned(),
        ],
        validation: validation_spec(),
    }
}

fn module_persistence_replay() -> ModuleSpec {
    ModuleSpec {
        name: "PersistenceReplay".to_owned(),
        goal: "Serialisierbare Projekt-Snapshots und Event-Replays ohne Ambiguität.".to_owned(),
        fsm: FsmSpec {
            states: vec![
                "Idle".to_owned(),
                "Exporting".to_owned(),
                "Importing".to_owned(),
                "Replaying".to_owned(),
                "Validated".to_owned(),
            ],
            transitions: vec![
                transition("Idle", "Exporting", "ExportProjectJson"),
                transition("Exporting", "Idle", "ProjectSerialized"),
                transition("Idle", "Importing", "ImportProjectJson"),
                transition("Importing", "Validated", "ProjectRecoveredOrValid"),
                transition("Idle", "Replaying", "ReplayFromLogJson"),
                transition("Replaying", "Validated", "ReplayCompleted"),
            ],
        },
        state_type: "ProjectFile | ReplayLogFile".to_owned(),
        input_type: "&StudioState | &str".to_owned(),
        output_type: "String | StudioState".to_owned(),
        contracts: ContractSpec {
            invariants: vec![
                "Projekt- und Replay-Dateien sind vollständig parsebares JSON.".to_owned(),
                "Import nutzt dieselben Validation- und Recovery-Regeln wie der Live-State."
                    .to_owned(),
            ],
            pre_conditions: vec!["JSON entspricht dem typisierten Projektformat.".to_owned()],
            post_conditions: vec![
                "Roundtrip und Replay liefern denselben deterministischen Zielzustand.".to_owned(),
            ],
        },
        tests: vec![
            "project_roundtrip_restores_authoring_state".to_owned(),
            "replay_log_roundtrip_replays_deterministically".to_owned(),
            "integration_project_export_import_roundtrip_is_deterministic".to_owned(),
        ],
        implementation_files: vec![
            "src/core/project.rs".to_owned(),
            "src/core/history.rs".to_owned(),
            "src/core/validation.rs".to_owned(),
            "src/core/reducer.rs".to_owned(),
        ],
        validation: validation_spec(),
    }
}

fn module_venture_management() -> ModuleSpec {
    ModuleSpec {
        name: "VentureManagement".to_owned(),
        goal: "Übergeordnetes Laden, Speichern, Auswaehlen, Dirty-Tracking und Recovery von Ventures."
            .to_owned(),
        fsm: FsmSpec {
            states: vec![
                "Idle".to_owned(),
                "Refreshing".to_owned(),
                "Saving".to_owned(),
                "Loading".to_owned(),
                "Deleting".to_owned(),
                "Autosaving".to_owned(),
                "Error".to_owned(),
            ],
            transitions: vec![
                transition("Idle", "Refreshing", "RefreshVentures"),
                transition("Refreshing", "Idle", "RegistryLoaded"),
                transition("Idle", "Saving", "SaveCurrentVenture"),
                transition("Idle", "Saving", "SaveCurrentVentureAs"),
                transition("Idle", "Saving", "RenameSelectedVenture"),
                transition("Saving", "Idle", "VentureSaved"),
                transition("Idle", "Loading", "LoadSelectedVenture"),
                transition("Loading", "Idle", "VentureLoaded"),
                transition("Idle", "Deleting", "DeleteSelectedVenture"),
                transition("Deleting", "Idle", "VentureDeleted"),
                transition("Idle", "Autosaving", "AutosaveRecoverySlot"),
                transition("Autosaving", "Idle", "RecoverySaved"),
                transition("Idle", "Loading", "RestoreSelectedRecoverySlot"),
                transition("Idle", "Idle", "CreateNewVenture"),
                transition("Refreshing", "Error", "RegistryLoadFailed"),
                transition("Saving", "Error", "VentureSaveFailed"),
                transition("Loading", "Error", "VentureLoadFailed"),
                transition("Deleting", "Error", "VentureDeleteFailed"),
                transition("Autosaving", "Error", "RecoverySaveFailed"),
            ],
        },
        state_type: "VentureState".to_owned(),
        input_type:
            "AppEvent::RefreshVentures | AppEvent::SelectVenture | AppEvent::SelectRecoverySlot | AppEvent::SetVentureDraftName | AppEvent::SaveCurrentVenture | AppEvent::SaveCurrentVentureAs | AppEvent::RenameSelectedVenture | AppEvent::LoadSelectedVenture | AppEvent::DeleteSelectedVenture | AppEvent::RestoreSelectedRecoverySlot | AppEvent::AutosaveRecoverySlot | AppEvent::CreateNewVenture"
                .to_owned(),
        output_type: "Vec<StateDiff>".to_owned(),
        contracts: ContractSpec {
            invariants: vec![
                "Selektierte Venture-Ids referenzieren einen existierenden Registry-Eintrag oder sind None."
                    .to_owned(),
                "Selektierte Recovery-Slot-Ids referenzieren einen existierenden Recovery-Eintrag oder sind None."
                    .to_owned(),
                "Venture-Dateien liegen ausschließlich unter dem konfigurierten Venture-Verzeichnis."
                    .to_owned(),
                "Venture-Management-Events verändern das Authoring-Replay nicht.".to_owned(),
                "Beschädigte Venture-Dateien erscheinen nur als Registry-Issues und nicht als ladbare Ventures."
                    .to_owned(),
                "Dirty-State wird ausschließlich aus der stabilen Authoring-Signatur berechnet.".to_owned(),
            ],
            pre_conditions: vec![
                "Das Venture-Verzeichnis ist schreibbar oder die Operation liefert einen typisierten Fehler."
                    .to_owned(),
            ],
            post_conditions: vec![
                "Laden und Speichern aktualisieren die Venture-Registry deterministisch.".to_owned(),
                "CreateNewVenture setzt den Authoring-State zurück und behält die Registry bei."
                    .to_owned(),
                "DeleteSelectedVenture entfernt nur den persistierten Slot und lässt den aktuellen In-Memory-State als Draft bestehen."
                    .to_owned(),
                "AutosaveRecoverySlot schreibt Recovery-Slots nur für echte Authoring-Zustandsänderungen."
                    .to_owned(),
            ],
        },
        tests: vec![
            "save_and_load_venture_roundtrip_restores_state".to_owned(),
            "list_ventures_sorts_by_name_deterministically".to_owned(),
            "save_as_creates_distinct_venture_id".to_owned(),
            "delete_venture_removes_persisted_file".to_owned(),
            "registry_skips_invalid_venture_files_and_reports_issues".to_owned(),
            "save_and_restore_recovery_slot_roundtrip_restores_state".to_owned(),
            "recovery_slot_pruning_keeps_latest_capacity".to_owned(),
            "venture_save_as_and_delete_do_not_pollute_replay_log".to_owned(),
            "venture_rename_preserves_selected_id".to_owned(),
            "venture_dirty_state_and_recovery_restore_roundtrip".to_owned(),
            "integration_autosave_recovery_restore_is_deterministic".to_owned(),
            "integration_venture_save_load_roundtrip_is_deterministic".to_owned(),
        ],
        implementation_files: vec![
            "src/core/project.rs".to_owned(),
            "src/core/state.rs".to_owned(),
            "src/core/event.rs".to_owned(),
            "src/core/reducer.rs".to_owned(),
            "src/core/validation.rs".to_owned(),
            "src/ui/mod.rs".to_owned(),
            "src/app.rs".to_owned(),
        ],
        validation: validation_spec(),
    }
}

fn module_engine() -> ModuleSpec {
    ModuleSpec {
        name: "Engine".to_owned(),
        goal: "Monotone Clock, BPM-basierte Zeit und deterministische Ausführung.".to_owned(),
        fsm: FsmSpec {
            states: vec![
                "Stopped".to_owned(),
                "Running".to_owned(),
                "Paused".to_owned(),
                "Syncing".to_owned(),
                "Error".to_owned(),
            ],
            transitions: vec![
                transition("Stopped", "Running", "ToggleTransport"),
                transition("Running", "Paused", "ToggleTransport"),
                transition("Paused", "Running", "ToggleTransport"),
                transition("Running", "Syncing", "BeginScrub"),
                transition("Syncing", "Running", "EndScrub"),
                transition("Running", "Error", "ValidationFailure"),
            ],
        },
        state_type: "EngineState".to_owned(),
        input_type: "Tick".to_owned(),
        output_type: "StateDiff::Engine".to_owned(),
        contracts: ContractSpec {
            invariants: vec![
                "Clock ist monoton.".to_owned(),
                "Playhead bleibt innerhalb der Songlänge.".to_owned(),
            ],
            pre_conditions: vec!["frame_interval_ns > 0".to_owned()],
            post_conditions: vec![
                "Frame-Zähler und Playhead sind deterministisch fortgeschrieben.".to_owned(),
            ],
        },
        tests: vec![
            "engine_advances_playhead_deterministically".to_owned(),
            "simulation_engine_tick_updates_clip_phases".to_owned(),
            "replay_produces_identical_state_snapshot".to_owned(),
        ],
        implementation_files: vec![
            "src/core/time.rs".to_owned(),
            "src/core/engine.rs".to_owned(),
        ],
        validation: validation_spec(),
    }
}

fn module_timeline() -> ModuleSpec {
    ModuleSpec {
        name: "Timeline".to_owned(),
        goal:
            "Deterministische Positionierung, Input-Fidelity, Snap-Logik und diff-basiertes Rendering."
                .to_owned(),
        fsm: FsmSpec {
            states: vec![
                "Idle".to_owned(),
                "Dragging".to_owned(),
                "Zooming".to_owned(),
                "Snapping".to_owned(),
                "Rendering".to_owned(),
            ],
            transitions: vec![
                transition("Idle", "Dragging", "PointerPressedClip"),
                transition("Idle", "Zooming", "MouseWheel"),
                transition("Dragging", "Snapping", "SnapGuideAcquired"),
                transition("Snapping", "Rendering", "StateDiffCommitted"),
                transition("Rendering", "Idle", "FramePresented"),
            ],
        },
        state_type: "TimelineState".to_owned(),
        input_type: "TimelineEvent".to_owned(),
        output_type: "Vec<StateDiff>".to_owned(),
        contracts: ContractSpec {
            invariants: vec![
                "Position = f(Time, Zoom)".to_owned(),
                "Snap verwendet quantisierte BeatTime-Werte.".to_owned(),
                "FX- und Cue-Overlays werden ausschließlich aus Clip- und Show-State abgeleitet."
                    .to_owned(),
                "Canvas-Hotspots dispatchen Cue-, Chase- und FX-Aktionen ausschließlich über Events."
                    .to_owned(),
                "Inline-Parametergriffe bleiben auf typisierte Clip-Parameter abgebildet."
                    .to_owned(),
                "Drag/Resize mutieren erst nach deterministischer Hysterese.".to_owned(),
                "Wheel-Zoom hält den Beat unter dem Mausanker stabil.".to_owned(),
                "Box-Selection erzeugt eine deterministisch sortierte Clip-Menge.".to_owned(),
            ],
            pre_conditions: vec!["TimelineCursor ist typisiert.".to_owned()],
            post_conditions: vec![
                "Clip-Mutationen bleiben im Songbereich.".to_owned(),
                "Render-Revisions werden nur bei echten Diffs erhöht.".to_owned(),
                "Snap-Locks bleiben stabil, bis die Release-Schwelle überschritten wird."
                    .to_owned(),
                "Leerklick auf Track-Hintergrund und Marquee-Selection bleiben unterscheidbar."
                    .to_owned(),
            ],
        },
        tests: vec![
            "clip_drag_snaps_to_quarter_grid".to_owned(),
            "resize_clip_end_respects_min_duration".to_owned(),
            "clip_cue_hotspot_triggers_linked_cue_without_dragging".to_owned(),
            "clip_chase_hotspot_toggles_linked_chase".to_owned(),
            "clip_fx_hotspot_focuses_fx_from_canvas".to_owned(),
            "inline_parameter_drag_updates_clip_intensity_deterministically".to_owned(),
            "pending_clip_drag_requires_hysteresis_before_moving".to_owned(),
            "box_selection_selects_multiple_clips_deterministically".to_owned(),
            "track_click_without_box_drag_preserves_track_selection_behavior".to_owned(),
            "locked_snap_guide_holds_until_release_threshold".to_owned(),
            "scroll_zoom_keeps_anchor_beat_stable".to_owned(),
            "scenario_drag_zoom_scrub_keeps_state_valid".to_owned(),
            "clip_hotspots_are_generated_for_linked_entities".to_owned(),
            "clip_param_handles_are_generated_for_inline_editing".to_owned(),
            "cursor_info_anywhere_supports_dragging_outside_canvas_bounds".to_owned(),
            "box_selection_rect_is_available_during_marquee_selection".to_owned(),
            "integration_timeline_hotspots_dispatch_show_actions".to_owned(),
            "integration_inline_parameter_drag_replays_deterministically".to_owned(),
            "integration_box_selection_replays_deterministically".to_owned(),
            "integration_small_pointer_jitter_does_not_move_clip".to_owned(),
            "integration_zoom_anchor_replays_deterministically".to_owned(),
            "replay_produces_identical_state_snapshot".to_owned(),
            "waveform_preview_sample_is_deterministic_and_bounded".to_owned(),
        ],
        implementation_files: vec![
            "src/core/reducer.rs".to_owned(),
            "src/ui/timeline.rs".to_owned(),
        ],
        validation: validation_spec(),
    }
}

fn module_clip_editor() -> ModuleSpec {
    ModuleSpec {
        name: "ClipEditor".to_owned(),
        goal:
            "Deterministischer Clip-Editor mit parametrisierter Live-Vorschau im Timeline-Bereich."
                .to_owned(),
        fsm: FsmSpec {
            states: vec![
                "Closed".to_owned(),
                "Open".to_owned(),
                "Adjusting".to_owned(),
                "Previewing".to_owned(),
            ],
            transitions: vec![
                transition("Closed", "Open", "OpenClipEditor"),
                transition("Open", "Adjusting", "SetClipEditorParam"),
                transition("Adjusting", "Previewing", "PreviewCommitted"),
                transition("Previewing", "Open", "TickCommitted"),
                transition("Open", "Closed", "CloseClipEditor"),
            ],
        },
        state_type: "ClipEditorState".to_owned(),
        input_type: "AppEvent::OpenClipEditor | AppEvent::SetClipEditor* | AppEvent::TriggerCue | AppEvent::ToggleChase".to_owned(),
        output_type: "Vec<StateDiff>".to_owned(),
        contracts: ContractSpec {
            invariants: vec![
                "Der Editor referenziert nur existierende Clips.".to_owned(),
                "Clip-Parameter bleiben typisiert und innerhalb ihrer Ranges.".to_owned(),
                "Direkte Cue- und Chase-Aktionen im Timeline-Bereich bleiben event-getrieben."
                    .to_owned(),
            ],
            pre_conditions: vec!["Ein Clip ist selektiert oder explizit angegeben.".to_owned()],
            post_conditions: vec![
                "Clip-Vorschau und Clip-State bleiben deterministisch synchron.".to_owned(),
            ],
        },
        tests: vec![
            "opening_clip_editor_selects_clip".to_owned(),
            "clip_editor_parameter_change_enters_previewing".to_owned(),
            "clip_editor_can_relink_chase".to_owned(),
            "clip_editor_overlay_and_replay_are_deterministic".to_owned(),
        ],
        implementation_files: vec![
            "src/core/editor.rs".to_owned(),
            "src/core/state.rs".to_owned(),
            "src/core/reducer.rs".to_owned(),
            "src/ui/timeline.rs".to_owned(),
        ],
        validation: validation_spec(),
    }
}

fn module_cue_system() -> ModuleSpec {
    ModuleSpec {
        name: "CueSystem".to_owned(),
        goal:
            "Triggerbare und editierbare Cues mit deterministischem Armed-, Triggered-, Fading- und Active-Zyklus."
                .to_owned(),
        fsm: FsmSpec {
            states: vec![
                "Stored".to_owned(),
                "Armed".to_owned(),
                "Triggered".to_owned(),
                "Fading".to_owned(),
                "Active".to_owned(),
            ],
            transitions: vec![
                transition("Stored", "Armed", "ArmCue"),
                transition("Armed", "Triggered", "TriggerCue"),
                transition("Triggered", "Active", "TickCommitted"),
                transition("Active", "Fading", "OtherCueTriggered"),
                transition("Fading", "Stored", "FadeElapsed"),
            ],
        },
        state_type: "CueSystemState".to_owned(),
        input_type:
            "AppEvent::SelectCue | AppEvent::CreateCue | AppEvent::DeleteSelectedCue | AppEvent::SetSelectedCue* | AppEvent::ArmCue | AppEvent::TriggerCue | Tick"
                .to_owned(),
        output_type: "Vec<StateDiff>".to_owned(),
        contracts: ContractSpec {
            invariants: vec![
                "Cue-Verweise auf Clips bleiben gültig.".to_owned(),
                "Clip-Cue-Markierungen folgen den Cue-Phasen ohne Seiteneffekte.".to_owned(),
                "Cue-Loeschungen bereinigen Chase- und Fixture-Verweise deterministisch."
                    .to_owned(),
            ],
            pre_conditions: vec![
                "CueId ist fuer Selektions-/Trigger-Pfade gueltig.".to_owned(),
                "Bei DeleteSelectedCue existiert eine Auswahl.".to_owned(),
            ],
            post_conditions: vec![
                "Ziel-Cue ist deterministisch selektiert.".to_owned(),
                "Aktive Cue-Zustände sind nach Trigger/Re-Fade konsistent.".to_owned(),
            ],
        },
        tests: vec![
            "trigger_cue_moves_previous_active_to_fading".to_owned(),
            "delete_selected_cue_clears_clip_fixture_and_chase_links".to_owned(),
            "integration_cue_trigger_updates_fixture_and_clip_views".to_owned(),
            "cue_and_chase_authoring_replay_is_deterministic".to_owned(),
            "replay_produces_identical_show_state_snapshot".to_owned(),
        ],
        implementation_files: vec![
            "src/core/state.rs".to_owned(),
            "src/core/show.rs".to_owned(),
            "src/core/validation.rs".to_owned(),
        ],
        validation: validation_spec(),
    }
}

fn module_chase_system() -> ModuleSpec {
    ModuleSpec {
        name: "ChaseSystem".to_owned(),
        goal:
            "Step-basierte und editierbare Chases mit deterministischer Vorwärts-, Loop- und Reverse-Logik."
                .to_owned(),
        fsm: FsmSpec {
            states: vec![
                "Idle".to_owned(),
                "Playing".to_owned(),
                "Looping".to_owned(),
                "Reversing".to_owned(),
                "Stopped".to_owned(),
            ],
            transitions: vec![
                transition("Idle", "Playing", "ToggleChase"),
                transition("Stopped", "Playing", "ToggleChase"),
                transition("Playing", "Looping", "LastStepWrapped"),
                transition("Playing", "Stopped", "LastStepCompleted"),
                transition("Playing", "Reversing", "ReverseChase"),
                transition("Reversing", "Stopped", "FirstStepCompleted"),
            ],
        },
        state_type: "ChaseSystemState".to_owned(),
        input_type:
            "AppEvent::SelectChase | AppEvent::CreateChase | AppEvent::DeleteSelectedChase | AppEvent::SetSelectedChase* | AppEvent::SelectChaseStep | AppEvent::AddSelectedChaseStep | AppEvent::DeleteSelectedChaseStep | AppEvent::MoveSelectedChaseStep* | AppEvent::ToggleChase | AppEvent::ReverseChase | Tick"
                .to_owned(),
        output_type: "Vec<StateDiff>".to_owned(),
        contracts: ContractSpec {
            invariants: vec![
                "current_step bleibt innerhalb der Step-Liste.".to_owned(),
                "Step-Wechsel triggern verknüpfte Cues in fester Reihenfolge.".to_owned(),
                "Chase-Step-Dauern bleiben >= MIN_CLIP_DURATION.".to_owned(),
            ],
            pre_conditions: vec![
                "ChaseId ist fuer Selektions-/Playback-Pfade gueltig.".to_owned(),
                "Bei Step-Edits ist ein Chase-Step selektiert.".to_owned(),
            ],
            post_conditions: vec![
                "Die Chase-Phase entspricht der Richtung und Loop-Situation.".to_owned(),
            ],
        },
        tests: vec![
            "chase_advances_and_triggers_linked_cue".to_owned(),
            "create_chase_and_edit_steps_updates_selection_deterministically".to_owned(),
            "set_selected_chase_step_cue_updates_selected_cue_and_clamps_duration".to_owned(),
            "simulation_chase_step_progress_is_deterministic".to_owned(),
            "cue_and_chase_authoring_replay_is_deterministic".to_owned(),
            "replay_produces_identical_show_state_snapshot".to_owned(),
        ],
        implementation_files: vec![
            "src/core/state.rs".to_owned(),
            "src/core/show.rs".to_owned(),
            "src/core/validation.rs".to_owned(),
        ],
        validation: validation_spec(),
    }
}

fn module_fx_system() -> ModuleSpec {
    ModuleSpec {
        name: "FxSystem".to_owned(),
        goal: "Deterministische FX-Layer mit klaren Processing-, Applied- und Composed-Übergängen."
            .to_owned(),
        fsm: FsmSpec {
            states: vec![
                "Idle".to_owned(),
                "Processing".to_owned(),
                "Applied".to_owned(),
                "Composed".to_owned(),
            ],
            transitions: vec![
                transition("Idle", "Processing", "ToggleFxOn"),
                transition("Processing", "Applied", "LinkedClipActive"),
                transition("Applied", "Composed", "MultipleLayersActive"),
                transition("Applied", "Idle", "ToggleFxOff"),
                transition("Composed", "Idle", "ToggleFxOff"),
            ],
        },
        state_type: "FxSystemState".to_owned(),
        input_type:
            "AppEvent::ToggleFx | AppEvent::SetFxDepth | AppEvent::SetFxRate | AppEvent::SetFxSpread | AppEvent::SetFxPhaseOffset | AppEvent::SetFxWaveform | Tick"
                .to_owned(),
        output_type: "Vec<StateDiff>".to_owned(),
        contracts: ContractSpec {
            invariants: vec![
                "Depth und Output bleiben normiert.".to_owned(),
                "Spread und Phase Offset bleiben in 0..=1000.".to_owned(),
                "FX-Zustand wird nur aus State und Events abgeleitet.".to_owned(),
            ],
            pre_conditions: vec!["FxId ist gültig.".to_owned()],
            post_conditions: vec![
                "Enabled, Phase und Output sind deterministisch aktualisiert.".to_owned(),
            ],
        },
        tests: vec![
            "fx_output_follows_master_intensity_for_active_clip".to_owned(),
            "fx_waveform_settings_change_modulated_output_deterministically".to_owned(),
            "integration_fx_depth_event_is_clamped_and_replayed".to_owned(),
            "integration_fx_waveform_and_fixture_preview_replay_is_deterministic".to_owned(),
            "replay_produces_identical_show_state_snapshot".to_owned(),
        ],
        implementation_files: vec![
            "src/core/state.rs".to_owned(),
            "src/core/show.rs".to_owned(),
            "src/core/validation.rs".to_owned(),
            "src/ui/mod.rs".to_owned(),
        ],
        validation: validation_spec(),
    }
}

fn module_fixture_system() -> ModuleSpec {
    ModuleSpec {
        name: "FixtureSystem".to_owned(),
        goal: "Deterministisch abgeleitete Fixture-Gruppenstatus aus Cue- und FX-Quellen."
            .to_owned(),
        fsm: FsmSpec {
            states: vec![
                "Uninitialized".to_owned(),
                "Mapped".to_owned(),
                "Active".to_owned(),
                "Error".to_owned(),
            ],
            transitions: vec![
                transition("Uninitialized", "Mapped", "FixtureMapped"),
                transition("Mapped", "Active", "CueOrFxActive"),
                transition("Active", "Mapped", "CueAndFxIdle"),
                transition("Mapped", "Error", "FixtureOffline"),
                transition("Error", "Mapped", "RecoveryApplied"),
            ],
        },
        state_type: "FixtureSystemState".to_owned(),
        input_type: "AppEvent::SelectFixtureGroup | Tick".to_owned(),
        output_type: "Vec<StateDiff>".to_owned(),
        contracts: ContractSpec {
            invariants: vec![
                "online <= fixture_count".to_owned(),
                "Fixture-Ausgang bleibt normiert.".to_owned(),
                "Preview-Nodes liegen in einem normierten 2.5D-Raum.".to_owned(),
                "Canvas-Hit-Testing selektiert deterministisch genau eine Fixture-Gruppe."
                    .to_owned(),
            ],
            pre_conditions: vec!["FixtureGroupId ist gültig.".to_owned()],
            post_conditions: vec![
                "Fixture-Phase entspricht deterministisch der Source-Situation.".to_owned(),
            ],
        },
        tests: vec![
            "fixture_group_becomes_active_from_linked_sources".to_owned(),
            "scenario_fixture_selection_stays_valid_after_show_updates".to_owned(),
            "integration_fx_waveform_and_fixture_preview_replay_is_deterministic".to_owned(),
            "replay_produces_identical_show_state_snapshot".to_owned(),
            "group_hit_test_selects_fixture_from_projected_node_space".to_owned(),
        ],
        implementation_files: vec![
            "src/core/state.rs".to_owned(),
            "src/core/show.rs".to_owned(),
            "src/core/validation.rs".to_owned(),
            "src/ui/mod.rs".to_owned(),
            "src/ui/fixture_view.rs".to_owned(),
        ],
        validation: validation_spec(),
    }
}

fn transition(from: &str, to: &str, event: &str) -> TransitionSpec {
    TransitionSpec {
        from: from.to_owned(),
        to: to.to_owned(),
        event: event.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_readable_spec_roundtrips() {
        let json = foundation_spec_json();
        let parsed: MachineReadableSection =
            serde_json::from_str(&json).expect("spec parses as json");
        assert_eq!(parsed.modules.len(), 16);
    }
}
