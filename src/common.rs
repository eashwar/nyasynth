use biquad::ToHertz;
use derive_more::{Add, From, Sub};
use ordered_float::{FloatIsNan, NotNan};
use serde::{Deserialize, Serialize};

use crate::{
    ease::{lerp, Easing},
    sound_gen::EnvelopeType,
};

pub type SampleTime = usize;

/// A sample rate in Hz/seconds. Must be a positive value.
#[derive(Debug, Clone, Copy)]
pub struct SampleRate(pub f32);

impl SampleRate {
    pub fn new(rate: f32) -> Option<SampleRate> {
        if rate <= 0.0 {
            None
        } else {
            Some(SampleRate(rate))
        }
    }

    pub fn get(&self) -> f32 {
        self.0
    }

    pub fn to_seconds(&self, samples: SampleTime) -> Seconds {
        let seconds = samples as f32 / self.get();
        Seconds::new(seconds).unwrap()
    }

    pub fn hz(&self) -> biquad::Hertz<f32> {
        self.get().hz()
    }
}

impl From<f32> for SampleRate {
    fn from(value: f32) -> Self {
        SampleRate::new(value).unwrap()
    }
}

/// A wrapper struct representing a duration of seconds. This struct implements [std::ops::Div], so
/// it's possible to divide a [Seconds] by another [Seconds] and get an [f32].
#[derive(Debug, Clone, Copy, Add, Sub, From, PartialEq, Eq, PartialOrd, Ord)]
pub struct Seconds(pub NotNan<f32>);

impl Seconds {
    pub const ZERO: Seconds = Seconds(unsafe { NotNan::new_unchecked(0.0) });

    /// Construct a new [Seconds]. `seconds` must not be NaN.
    pub fn new(seconds: f32) -> Result<Seconds, FloatIsNan> {
        let value = NotNan::new(seconds)?;
        Ok(Seconds(value))
    }

    /// Get the number of seconds as an [f32].
    pub fn get(&self) -> f32 {
        self.0.into()
    }
}

impl std::ops::Div for Seconds {
    type Output = f32;

    fn div(self, rhs: Self) -> Self::Output {
        self.get() / rhs.get()
    }
}

impl From<Seconds> for f32 {
    fn from(value: Seconds) -> Self {
        value.get()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Add, Sub)]
/// A struct representing Decibels. This struct can be used with [Easing] and [EnvelopeType]. Note
/// that the specific implementation for EnvelopeType has decibels lerped in amplitude space for the
/// attack of a note, and then lerped in dB space for the rest of the note. This is to ensure that
/// sharp attacks remain sharp. Additionally, note that decibel values below `NEG_INF_DB_THRESHOLD`
/// are treated as zero. This is done so that it is possible to lerp from negative infinity decibels
/// (that is, an amplitude of zero) to positive amounts in a reasonable fashion (in particular, this
/// means extremely quiet sounds will instead become silence).
pub struct Decibel(f32);

impl Decibel {
    /// The threshold for which Decibel values below it will be treated as negative
    /// infinity dB.
    pub const NEG_INF_DB_THRESHOLD: f32 = -70.0;

    pub const fn from_db(db: f32) -> Decibel {
        Decibel(db)
    }

    pub const fn neg_inf_db() -> Decibel {
        Decibel::from_db(Decibel::NEG_INF_DB_THRESHOLD)
    }

    pub const fn zero_db() -> Decibel {
        Decibel::from_db(0.0)
    }

    pub fn from_amp(amp: f32) -> Decibel {
        Decibel::from_db(f32::log10(amp) * 10.0)
    }

    // Linearly interpolate in amplitude space.
    pub fn lerp_amp(start: Decibel, end: Decibel, t: f32) -> Decibel {
        let amp = lerp(start.get_amp(), end.get_amp(), t);
        Decibel::from_amp(amp)
    }

    // Linearly interpolate in Decibel space.
    pub fn lerp_db(start: f32, end: f32, t: f32) -> Decibel {
        let db = lerp(start, end, t);
        Decibel::from_db(db)
    }

    // Linearly interpolate in Decibel space, but values of t below 0.125 will
    // lerp from `start` to `Decibel::zero()`. This function is meant for use
    // with user-facing parameter knobs.
    pub const fn ease_db(start: f32, end: f32) -> Easing<Decibel> {
        Easing::SplitLinear {
            start: Decibel::neg_inf_db(),
            mid: Decibel::from_db(start),
            end: Decibel::from_db(end),
            split_at: 0.125,
        }
    }

    /// Get the peak amplitude a signal played at the given Decibel amount would produce.
    pub fn get_amp(&self) -> f32 {
        if self.get_db() <= Decibel::NEG_INF_DB_THRESHOLD {
            0.0
        } else {
            10.0f32.powf(self.get_db() / 10.0)
        }
    }

    // Get the decibel value as an f32.
    pub fn get_db(&self) -> f32 {
        self.0
    }
}

impl EnvelopeType for Decibel {
    fn lerp_attack(start: Self, end: Self, t: f32) -> Self {
        // Lerp in amplitude space during the attack phase. This is useful
        // long attacks usually need linear amplitude ramp ups.
        Decibel::lerp_amp(start, end, t)
    }
    fn lerp_decay(start: Self, end: Self, t: f32) -> Self {
        Decibel::lerp_db(start.get_db(), end.get_db(), t)
    }
    fn lerp_release(start: Self, end: Self, t: f32) -> Self {
        Decibel::lerp_db(start.get_db(), end.get_db(), t)
    }
    fn lerp_retrigger(start: Self, end: Self, t: f32) -> Self {
        Decibel::lerp_amp(start, end, t)
    }
    fn one() -> Self {
        Decibel::zero_db()
    }
    fn zero() -> Self {
        Decibel::neg_inf_db()
    }
}

impl std::ops::Mul<f32> for Decibel {
    type Output = Decibel;

    fn mul(self, rhs: f32) -> Self::Output {
        Decibel::from_db(self.0 * rhs)
    }
}

impl std::ops::Div<Decibel> for Decibel {
    type Output = f32;
    fn div(self, rhs: Decibel) -> Self::Output {
        self.get_db() / rhs.get_db()
    }
}
