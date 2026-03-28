use crate::core::{AppEvent, StudioState, dispatch};
use crate::ui;
use iced::application::Appearance;
use iced::event;
use iced::keyboard::{self, Key, Modifiers, key::Named};
use iced::{Element, Event, Size, Subscription, Task, Theme, time, window};
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
        .run_with(|| (LumaSwitch::default(), Task::none()))
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

fn update(app: &mut LumaSwitch, message: AppEvent) {
    dispatch(&mut app.state, message);
}

fn subscription(_app: &LumaSwitch) -> Subscription<AppEvent> {
    Subscription::batch([
        time::every(FRAME_INTERVAL).map(|_| AppEvent::Tick),
        event::listen_with(runtime_event_to_app_event),
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
