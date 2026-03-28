use iced::Color;
use serde::{Deserialize, Serialize};

pub const PPQ: u32 = 960;
pub const BAR_BEATS: u32 = 4;
pub const DEFAULT_FRAME_INTERVAL_NS: u64 = 16_666_667;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub struct BeatTime(u32);

impl BeatTime {
    pub const ZERO: Self = Self(0);

    pub const fn from_ticks(ticks: u32) -> Self {
        Self(ticks)
    }

    pub const fn ticks(self) -> u32 {
        self.0
    }

    pub const fn from_beats(beats: u32) -> Self {
        Self(beats * PPQ)
    }

    pub const fn from_fraction(numerator: u32, denominator: u32) -> Self {
        Self((numerator * PPQ) / denominator)
    }

    pub fn from_beats_f32(beats: f32) -> Self {
        let ticks = (beats.max(0.0) * PPQ as f32).round() as u32;
        Self(ticks)
    }

    pub fn as_beats_f32(self) -> f32 {
        self.0 as f32 / PPQ as f32
    }

    pub fn saturating_add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }

    pub fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }

    pub fn min(self, other: Self) -> Self {
        Self(self.0.min(other.0))
    }

    pub fn max(self, other: Self) -> Self {
        Self(self.0.max(other.0))
    }

    pub fn clamp(self, min: Self, max: Self) -> Self {
        Self(self.0.clamp(min.0, max.0))
    }

    pub fn quantize(self, step: Self) -> Self {
        if step.0 == 0 {
            return self;
        }

        let half = step.0 / 2;
        let snapped = ((self.0 + half) / step.0) * step.0;
        Self(snapped)
    }

    pub fn wrapping_add(self, delta: Self, modulus: Self) -> Self {
        if modulus.0 == 0 {
            return self.saturating_add(delta);
        }

        Self((self.0 + delta.0) % modulus.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TempoBpm(u32);

impl TempoBpm {
    pub const fn from_centi_bpm(value: u32) -> Self {
        Self(value)
    }

    pub const fn from_whole_bpm(value: u32) -> Self {
        Self(value * 100)
    }

    pub fn as_f32(self) -> f32 {
        self.0 as f32 / 100.0
    }

    pub const fn centi_bpm(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IntensityLevel(u16);

impl IntensityLevel {
    pub const MIN: u16 = 0;
    pub const MAX: u16 = 1000;

    pub fn from_permille(value: u16) -> Self {
        Self(value.clamp(Self::MIN, Self::MAX))
    }

    pub fn from_f32(value: f32) -> Self {
        let permille = (value.clamp(0.0, 1.0) * 1000.0).round() as u16;
        Self::from_permille(permille)
    }

    pub fn as_f32(self) -> f32 {
        self.0 as f32 / 1000.0
    }

    pub const fn permille(self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpeedRatio(u16);

impl SpeedRatio {
    pub const MIN: u16 = 200;
    pub const MAX: u16 = 1500;

    pub fn from_permille(value: u16) -> Self {
        Self(value.clamp(Self::MIN, Self::MAX))
    }

    pub fn from_f32(value: f32) -> Self {
        let permille = (value.clamp(0.2, 1.5) * 1000.0).round() as u16;
        Self::from_permille(permille)
    }

    pub fn as_f32(self) -> f32 {
        self.0 as f32 / 1000.0
    }

    pub const fn permille(self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ZoomFactor(u16);

impl ZoomFactor {
    pub const MIN: u16 = 450;
    pub const MAX: u16 = 2400;

    pub fn from_permille(value: u16) -> Self {
        Self(value.clamp(Self::MIN, Self::MAX))
    }

    pub fn from_f32(value: f32) -> Self {
        let permille = (value.clamp(0.45, 2.4) * 1000.0).round() as u16;
        Self::from_permille(permille)
    }

    pub fn as_f32(self) -> f32 {
        self.0 as f32 / 1000.0
    }

    pub const fn permille(self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RgbaColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl RgbaColor {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn to_iced(self) -> Color {
        Color::from_rgba8(self.r, self.g, self.b, self.a as f32 / 255.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonotonicClock {
    pub frame_index: u64,
    pub monotonic_ns: u64,
    pub frame_interval_ns: u64,
    pub beat_carry: u128,
}

impl MonotonicClock {
    pub const fn new(frame_interval_ns: u64) -> Self {
        Self {
            frame_index: 0,
            monotonic_ns: 0,
            frame_interval_ns,
            beat_carry: 0,
        }
    }

    pub fn advance_frame(&mut self) {
        self.frame_index = self.frame_index.saturating_add(1);
        self.monotonic_ns = self.monotonic_ns.saturating_add(self.frame_interval_ns);
    }
}

impl Default for TempoBpm {
    fn default() -> Self {
        Self::from_whole_bpm(128)
    }
}

impl Default for IntensityLevel {
    fn default() -> Self {
        Self::from_permille(1000)
    }
}

impl Default for SpeedRatio {
    fn default() -> Self {
        Self::from_permille(1000)
    }
}

impl Default for ZoomFactor {
    fn default() -> Self {
        Self::from_permille(1000)
    }
}

impl Default for MonotonicClock {
    fn default() -> Self {
        Self::new(DEFAULT_FRAME_INTERVAL_NS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beat_time_quantizes_to_expected_grid() {
        let raw = BeatTime::from_fraction(37, 8);
        let snapped = raw.quantize(BeatTime::from_fraction(1, 4));
        assert_eq!(snapped, BeatTime::from_fraction(19, 4));
    }

    #[test]
    fn parameter_wrappers_clamp_deterministically() {
        assert_eq!(IntensityLevel::from_f32(1.4).permille(), 1000);
        assert_eq!(SpeedRatio::from_f32(0.1).permille(), 200);
        assert_eq!(ZoomFactor::from_f32(3.0).permille(), 2400);
    }

    #[test]
    fn monotonic_clock_advances_without_wall_time() {
        let mut clock = MonotonicClock::new(DEFAULT_FRAME_INTERVAL_NS);
        clock.advance_frame();
        clock.advance_frame();

        assert_eq!(clock.frame_index, 2);
        assert_eq!(clock.monotonic_ns, DEFAULT_FRAME_INTERVAL_NS * 2);
    }
}
