# Luma Switch Studio

**Lichtzeug-neu** – professionelle DMX Show-Control-Anwendung in Rust/iced.

## Module

### `src/core/`
| Modul | Beschreibung |
|---|---|
| `state.rs` | Gesamter App-State (StudioState) |
| `reducer.rs` | Event-Reducer / State-Dispatch |
| `spec.rs` | Fixture-Spec & Capability-System |
| `validation.rs` | State-Validierung |
| `output.rs` | DMX/MIDI Output-Dispatcher |
| `show.rs` | Show-Daten & Serialisierung |
| `project.rs` | Projekt-Management (Ventures) |
| `fixtures.rs` | Fixture-Profile & Patching |
| `hardware.rs` | Hardware-Discovery (DMX/MIDI) |
| `engine_link.rs` | StageLinq / Denon Engine OS Link |
| `engine.rs` | Transport-Engine |
| `editor.rs` | Clip-Editor-Logik |
| `automation.rs` | Automation-Lanes |
| `event.rs` | AppEvent-Definitionen |
| `history.rs` | Undo/Redo-History |
| `ids.rs` | Typed IDs |
| `queue.rs` | Event-Queue |
| `time.rs` | BeatTime / TempoBpm / PPQ |

### `src/ui/`
| Modul | Beschreibung |
|---|---|
| `mod.rs` | Haupt-UI-Layout |
| `timeline.rs` | Timeline-Canvas |
| `fixture_view.rs` | Fixture-Patch-View |
| `theme.rs` | Theme-System |

## Build

```bash
cargo build --release
cargo run
```

## Status
- Letzter Sync: 2026-03-28
- Branch: main
