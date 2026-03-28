use crate::core::event::AppEvent;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventSystemPhase {
    Idle,
    EventQueued,
    Processing,
    Dispatched,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueuedEvent {
    pub sequence: u64,
    pub event: AppEvent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessedEvent {
    pub sequence: u64,
    pub event: AppEvent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventHistoryEntry {
    pub sequence: u64,
    pub phase: EventSystemPhase,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventQueueState {
    pub phase: EventSystemPhase,
    pub next_sequence: u64,
    pub queue: VecDeque<QueuedEvent>,
    pub processing: Option<QueuedEvent>,
    pub completed: Vec<ProcessedEvent>,
    pub history: Vec<EventHistoryEntry>,
}

impl Default for EventQueueState {
    fn default() -> Self {
        Self {
            phase: EventSystemPhase::Idle,
            next_sequence: 1,
            queue: VecDeque::new(),
            processing: None,
            completed: Vec::new(),
            history: Vec::new(),
        }
    }
}

pub fn enqueue_event(state: &mut EventQueueState, event: AppEvent) -> u64 {
    let sequence = state.next_sequence;
    state.next_sequence = state.next_sequence.saturating_add(1);
    state.queue.push_back(QueuedEvent { sequence, event });
    state.phase = EventSystemPhase::EventQueued;
    state.history.push(EventHistoryEntry {
        sequence,
        phase: EventSystemPhase::EventQueued,
    });
    sequence
}

pub fn start_next_event(state: &mut EventQueueState) -> Option<QueuedEvent> {
    let next = state.queue.pop_front()?;
    state.phase = EventSystemPhase::Processing;
    state.processing = Some(next.clone());
    state.history.push(EventHistoryEntry {
        sequence: next.sequence,
        phase: EventSystemPhase::Processing,
    });
    Some(next)
}

pub fn mark_event_dispatched(state: &mut EventQueueState) {
    if let Some(current) = &state.processing {
        state.phase = EventSystemPhase::Dispatched;
        state.history.push(EventHistoryEntry {
            sequence: current.sequence,
            phase: EventSystemPhase::Dispatched,
        });
    }
}

pub fn complete_current_event(state: &mut EventQueueState) {
    if let Some(current) = state.processing.take() {
        state.phase = EventSystemPhase::Completed;
        state.history.push(EventHistoryEntry {
            sequence: current.sequence,
            phase: EventSystemPhase::Completed,
        });
        state.completed.push(ProcessedEvent {
            sequence: current.sequence,
            event: current.event,
        });

        if state.completed.len() > 128 {
            let overflow = state.completed.len() - 128;
            state.completed.drain(0..overflow);
        }
        if state.history.len() > 512 {
            let overflow = state.history.len() - 512;
            state.history.drain(0..overflow);
        }
    }

    if state.queue.is_empty() {
        state.phase = EventSystemPhase::Idle;
    } else {
        state.phase = EventSystemPhase::EventQueued;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::AppEvent;

    #[test]
    fn queue_preserves_event_order() {
        let mut queue = EventQueueState::default();
        enqueue_event(&mut queue, AppEvent::Tick);
        enqueue_event(&mut queue, AppEvent::Tick);

        let first = start_next_event(&mut queue).expect("first event");
        mark_event_dispatched(&mut queue);
        complete_current_event(&mut queue);

        let second = start_next_event(&mut queue).expect("second event");

        assert!(first.sequence < second.sequence);
    }
}
