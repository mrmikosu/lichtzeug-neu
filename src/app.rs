use crate::core::{
    AppEvent, FixtureProfile, MidiLearnPhase, MidiPortDirection, StudioState,
    build_ofl_download_url, build_runtime_output_snapshot, decode_midi_bytes,
    deliver_runtime_outputs, dispatch, export_qxf_fixture, import_ofl_fixture, import_qxf_fixture,
    midi_port_id, parse_engine_discovery_packet, scan_hardware_inventory,
};
use crate::ui;
use iced::application::Appearance;
use iced::event;
use iced::futures::{SinkExt, StreamExt};
use iced::keyboard::{self, Key, Modifiers, key::Named};
use iced::stream;
use iced::{Element, Event, Size, Subscription, Task, Theme, time, window};
use std::fs;
use std::net::UdpSocket;
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

const FRAME_INTERVAL: Duration = Duration::from_millis(16);

#[derive(Debug, Clone)]
pub struct LumaSwitch {
    pub state: StudioState,
}

impl Default for LumaSwitch {
    fn default() -> Self {
        let mut app = Self {
            state: StudioState::default(),
        };
        dispatch(&mut app.state, AppEvent::RefreshVentures);
        app
    }
}

pub fn run() -> iced::Result {
    iced::application(title, update, view)
        .theme(|_| Theme::Dark)
        .style(|_, _| Appearance {
            background_color: crate::ui::theme::app_background(),
            text_color: crate::ui::theme::text_primary(),
        })
        .subscription(subscription)
        .antialiasing(true)
        .window_size(Size::new(1680.0, 960.0))
        .centered()
        .run_with(|| {
            (
                LumaSwitch::default(),
                Task::done(AppEvent::RefreshHardwareInventory),
            )
        })
}

fn title(app: &LumaSwitch) -> String {
    format!(
        "Luma Switch Studio  |  {}{}  |  {}  |  {:.1} BPM",
        app.state.venture_summary(),
        if app.state.venture.dirty { " *" } else { "" },
        app.state.engine.transport.position_label(),
        app.state.engine.transport.bpm.as_f32()
    )
}

fn update(app: &mut LumaSwitch, message: AppEvent) -> Task<AppEvent> {
    match message.clone() {
        AppEvent::RequestImportFixtureFromOfl => {
            if !app.state.can_import_fixture_from_ofl() {
                dispatch(
                    &mut app.state,
                    AppEvent::FixtureIoFailed(
                        "OFL-Import benötigt Hersteller- und Fixture-Key".to_owned(),
                    ),
                );
                return Task::none();
            }

            let manufacturer_key = app
                .state
                .fixture_system
                .library
                .ofl_manufacturer_key
                .clone();
            let fixture_key = app.state.fixture_system.library.ofl_fixture_key.clone();
            dispatch(&mut app.state, message);
            Task::perform(
                fetch_ofl_fixture(manufacturer_key, fixture_key),
                |result| match result {
                    Ok(profile) => AppEvent::ApplyImportedFixtureProfile(profile),
                    Err(error) => AppEvent::FixtureIoFailed(error),
                },
            )
        }
        AppEvent::RequestImportFixtureFromQxfPath => {
            if !app.state.can_import_fixture_from_qxf() {
                dispatch(
                    &mut app.state,
                    AppEvent::FixtureIoFailed("QXF-Importpfad fehlt".to_owned()),
                );
                return Task::none();
            }

            let path = app.state.fixture_system.library.qxf_import_path.clone();
            dispatch(&mut app.state, message);
            Task::perform(import_qxf_profile_from_path(path), |result| match result {
                Ok(profile) => AppEvent::ApplyImportedFixtureProfile(profile),
                Err(error) => AppEvent::FixtureIoFailed(error),
            })
        }
        AppEvent::RequestExportSelectedFixtureAsQxf => {
            let Some(profile) = app.state.selected_fixture_profile().cloned() else {
                dispatch(
                    &mut app.state,
                    AppEvent::FixtureIoFailed("Kein Fixture-Profil selektiert".to_owned()),
                );
                return Task::none();
            };
            let path = app.state.fixture_system.library.qxf_export_path.clone();
            if path.trim().is_empty() {
                dispatch(
                    &mut app.state,
                    AppEvent::FixtureIoFailed("QXF-Exportpfad fehlt".to_owned()),
                );
                return Task::none();
            }

            dispatch(&mut app.state, message);
            Task::perform(
                export_profile_to_qxf(profile, path),
                |result| match result {
                    Ok(path) => AppEvent::CompleteFixtureQxfExport(path),
                    Err(error) => AppEvent::FixtureIoFailed(error),
                },
            )
        }
        AppEvent::RefreshHardwareInventory => {
            dispatch(&mut app.state, message);
            Task::perform(async { scan_hardware_inventory() }, |result| match result {
                Ok(snapshot) => AppEvent::ApplyHardwareInventory(snapshot),
                Err(error) => AppEvent::HardwareInventoryFailed(error),
            })
        }
        AppEvent::Tick => {
            dispatch(&mut app.state, AppEvent::Tick);
            let Some(snapshot) = build_runtime_output_snapshot(&app.state) else {
                return Task::none();
            };
            let sequence = snapshot.sequence;
            dispatch(
                &mut app.state,
                AppEvent::BeginRuntimeOutputDispatch(sequence),
            );
            Task::perform(
                async move { deliver_runtime_outputs(snapshot) },
                |result| match result {
                    Ok(report) => AppEvent::CompleteRuntimeOutputDispatch(report),
                    Err(error) => {
                        AppEvent::RuntimeOutputDispatchFailed(error.sequence, error.detail)
                    }
                },
            )
        }
        AppEvent::CompleteRuntimeOutputDispatch(report) => {
            dispatch(
                &mut app.state,
                AppEvent::CompleteRuntimeOutputDispatch(report),
            );
            Task::none()
        }
        AppEvent::RuntimeOutputDispatchFailed(sequence, detail) => {
            dispatch(
                &mut app.state,
                AppEvent::RuntimeOutputDispatchFailed(sequence, detail),
            );
            Task::none()
        }
        AppEvent::ReceiveMidiRuntimeMessage(message) => {
            let routed = if app.state.settings.midi.learn.phase == MidiLearnPhase::Idle {
                AppEvent::ReceiveMidiRuntimeMessage(message)
            } else {
                AppEvent::CompleteMidiLearn(message)
            };
            dispatch(&mut app.state, routed);
            Task::none()
        }
        _ => {
            dispatch(&mut app.state, message);
            Task::none()
        }
    }
}

fn subscription(app: &LumaSwitch) -> Subscription<AppEvent> {
    Subscription::batch([
        time::every(FRAME_INTERVAL).map(|_| AppEvent::Tick),
        event::listen_with(runtime_event_to_app_event),
        midi_input_subscription(app),
        engine_link_subscription(app),
    ])
}

fn view(app: &LumaSwitch) -> Element<'_, AppEvent> {
    ui::view(&app.state)
}

fn runtime_event_to_app_event(
    event: Event,
    _status: iced::event::Status,
    _window: window::Id,
) -> Option<AppEvent> {
    match event {
        Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
            Some(AppEvent::SetInputModifiers(modifiers_state(modifiers)))
        }
        Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
            key_pressed_to_app_event(key, modifiers)
        }
        _ => None,
    }
}

fn key_pressed_to_app_event(key: Key, modifiers: Modifiers) -> Option<AppEvent> {
    let key = key.as_ref();

    if modifiers.command() {
        return match key {
            Key::Character("z") | Key::Character("Z") if modifiers.shift() => Some(AppEvent::Redo),
            Key::Character("z") | Key::Character("Z") => Some(AppEvent::Undo),
            Key::Character("s") | Key::Character("S") if modifiers.shift() => {
                Some(AppEvent::SaveCurrentVentureAs)
            }
            Key::Character("s") | Key::Character("S") => Some(AppEvent::SaveCurrentVenture),
            Key::Character("o") | Key::Character("O") if modifiers.shift() => {
                Some(AppEvent::RestoreSelectedRecoverySlot)
            }
            Key::Character("o") | Key::Character("O") => Some(AppEvent::LoadSelectedVenture),
            Key::Character("n") | Key::Character("N") => Some(AppEvent::CreateNewVenture),
            Key::Character("r") | Key::Character("R") if modifiers.shift() => {
                Some(AppEvent::RenameSelectedVenture)
            }
            Key::Character("d") | Key::Character("D") => Some(AppEvent::DuplicateSelectedClips),
            Key::Character("c") | Key::Character("C") => Some(AppEvent::CopySelectedClips),
            Key::Character("x") | Key::Character("X") => Some(AppEvent::CutSelectedClips),
            Key::Character("v") | Key::Character("V") => Some(AppEvent::PasteClipboardAtPlayhead),
            Key::Named(Named::Delete) | Key::Named(Named::Backspace) => {
                Some(AppEvent::DeleteSelectedVenture)
            }
            _ => None,
        };
    }

    match key {
        Key::Named(Named::Delete) | Key::Named(Named::Backspace) => {
            Some(AppEvent::DeleteSelectedClips)
        }
        Key::Named(Named::ArrowLeft) => Some(AppEvent::NudgeSelectedClipsLeft),
        Key::Named(Named::ArrowRight) => Some(AppEvent::NudgeSelectedClipsRight),
        Key::Named(Named::Escape) => Some(AppEvent::CloseContextMenu),
        _ => None,
    }
}

fn modifiers_state(modifiers: Modifiers) -> crate::core::InputModifiersState {
    crate::core::InputModifiersState {
        shift: modifiers.shift(),
        alt: modifiers.alt(),
        command: modifiers.command(),
    }
}

fn midi_input_subscription(app: &LumaSwitch) -> Subscription<AppEvent> {
    let Some(selected_input) = app.state.settings.midi.selected_input.clone() else {
        return Subscription::none();
    };

    Subscription::run_with_id(
        ("midi-input", selected_input.clone()),
        stream::channel(128, move |mut output| async move {
            let mut midi_in = match midir::MidiInput::new("Luma Switch MIDI Input") {
                Ok(midi_in) => midi_in,
                Err(error) => {
                    let _ = output
                        .send(AppEvent::HardwareInventoryFailed(format!(
                            "MIDI-Input konnte nicht initialisiert werden: {}",
                            error
                        )))
                        .await;
                    return;
                }
            };
            midi_in.ignore(midir::Ignore::None);

            let ports = midi_in.ports();
            let selected_port = ports.iter().enumerate().find_map(|(index, port)| {
                let name = midi_in.port_name(port).ok()?;
                (midi_port_id(MidiPortDirection::Input, index, &name) == selected_input)
                    .then_some(port.clone())
            });

            let Some(port) = selected_port else {
                let _ = output
                    .send(AppEvent::HardwareInventoryFailed(
                        "Gewaehlter MIDI-Input ist nicht mehr verfügbar".to_owned(),
                    ))
                    .await;
                return;
            };

            let (sender, mut receiver) = iced::futures::channel::mpsc::unbounded();
            let connection = match midi_in.connect(
                &port,
                "luma-switch-midi-listener",
                move |timestamp_micros, bytes, _| {
                    if let Some(message) = decode_midi_bytes(timestamp_micros, bytes) {
                        let _ = sender.unbounded_send(message);
                    }
                },
                (),
            ) {
                Ok(connection) => connection,
                Err(error) => {
                    let _ = output
                        .send(AppEvent::HardwareInventoryFailed(format!(
                            "MIDI-Input konnte nicht verbunden werden: {}",
                            error
                        )))
                        .await;
                    return;
                }
            };

            let _connection = connection;
            while let Some(message) = receiver.next().await {
                let _ = output
                    .send(AppEvent::ReceiveMidiRuntimeMessage(message))
                    .await;
            }
        }),
    )
}

fn engine_link_subscription(app: &LumaSwitch) -> Subscription<AppEvent> {
    if !app.state.should_subscribe_engine_link() {
        return Subscription::none();
    }

    let discovery_port = app.state.settings.engine_link.discovery_port;
    Subscription::run_with_id(
        ("engine-link", discovery_port),
        stream::channel(128, move |mut output| async move {
            let _ = output.send(AppEvent::RefreshEngineLinkDiscovery).await;

            let socket = match UdpSocket::bind(("0.0.0.0", discovery_port)) {
                Ok(socket) => socket,
                Err(error) => {
                    let _ = output
                        .send(AppEvent::EngineLinkDiscoveryFailed(format!(
                            "StageLinq-Discovery-Port {} konnte nicht gebunden werden: {}",
                            discovery_port, error
                        )))
                        .await;
                    return;
                }
            };

            if let Err(error) = socket.set_read_timeout(Some(Duration::from_millis(400))) {
                let _ = output
                    .send(AppEvent::EngineLinkDiscoveryFailed(format!(
                        "StageLinq-Discovery-Timeout konnte nicht gesetzt werden: {}",
                        error
                    )))
                    .await;
                return;
            }

            let running = Arc::new(AtomicBool::new(true));
            let thread_running = Arc::clone(&running);
            let (sender, mut receiver) = iced::futures::channel::mpsc::unbounded();

            std::thread::spawn(move || {
                let mut buffer = [0_u8; 4096];
                while thread_running.load(Ordering::Relaxed) {
                    match socket.recv_from(&mut buffer) {
                        Ok((len, source)) => {
                            if let Some(device) =
                                parse_engine_discovery_packet(&buffer[..len], source)
                                && sender
                                    .unbounded_send(AppEvent::ApplyEngineLinkDiscoveryDevice(
                                        device,
                                    ))
                                    .is_err()
                            {
                                break;
                            }
                        }
                        Err(error)
                            if matches!(
                                error.kind(),
                                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                            ) => {}
                        Err(error) => {
                            let _ = sender.unbounded_send(AppEvent::EngineLinkDiscoveryFailed(
                                format!("StageLinq-Discovery-Fehler: {}", error),
                            ));
                            break;
                        }
                    }
                }
            });

            while let Some(event) = receiver.next().await {
                let _ = output.send(event).await;
            }

            running.store(false, Ordering::Relaxed);
        }),
    )
}

async fn fetch_ofl_fixture(
    manufacturer_key: String,
    fixture_key: String,
) -> Result<FixtureProfile, String> {
    let url = build_ofl_download_url(&manufacturer_key, &fixture_key);
    let response = reqwest::get(&url)
        .await
        .map_err(|err| format!("OFL-Download fehlgeschlagen: {}", err))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|err| format!("OFL-Antwort konnte nicht gelesen werden: {}", err))?;

    if !status.is_success() {
        return Err(format!("OFL-Download {} lieferte HTTP {}", url, status));
    }

    import_ofl_fixture(&body, Some(&manufacturer_key), Some(&fixture_key))
}

async fn import_qxf_profile_from_path(path: String) -> Result<FixtureProfile, String> {
    let xml = fs::read_to_string(&path)
        .map_err(|err| format!("QXF-Datei {} konnte nicht gelesen werden: {}", path, err))?;
    import_qxf_fixture(&xml, Some(&path))
}

async fn export_profile_to_qxf(profile: FixtureProfile, path: String) -> Result<String, String> {
    let xml = export_qxf_fixture(&profile)?;
    if let Some(parent) = Path::new(&path).parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "QXF-Zielverzeichnis {} konnte nicht angelegt werden: {}",
                parent.display(),
                err
            )
        })?;
    }
    fs::write(&path, xml).map_err(|err| {
        format!(
            "QXF-Datei {} konnte nicht geschrieben werden: {}",
            path, err
        )
    })?;
    Ok(path)
}
