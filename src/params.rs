use std::convert::TryFrom;

use crate::{
    ease::{DiscreteLinear, Easing},
    generate_raw_params, impl_display, impl_from_i32, impl_get_default, impl_get_ref,
    impl_into_i32, impl_new,
    presets::{FilterTypeDiscrim, I32Divable},
    sound_gen::{Decibel, FilterParams, NoteShape, NoteShapeDiscrim},
};

use derive_more::Display;
use serde::{Deserialize, Serialize};
use vst::{host::Host, plugin::PluginParameters, util::AtomicFloat};

// Low Pass, High Pass, Bandpass
// All Pass, Notch Filter
// Low Shelf, High Shelf, Peaking EQ
pub const FILTER_TYPE_VARIANT_COUNT: usize = 8;

// Default values for master volume
pub const DEFAULT_MASTER_VOL: f32 = 0.6875; // -3 dB

// Default values for individual oscillator volume
pub const DEFAULT_OSC_VOL: f32 = 0.5833; // -3 dB

// Min and maximum ranges for volume knobs.
pub const MASTER_VOL_MIN_DB: f32 = -36.0;
pub const MASTER_VOL_MAX_DB: f32 = 12.0;
pub const OSC_VOL_MIN_DB: f32 = -24.0;
pub const OSC_VOL_MAX_DB: f32 = 12.0;
pub const SUSTAIN_MIN_DB: f32 = -24.0;

// Default values for volume envelope
pub const DEFAULT_VOL_ATTACK: f32 = 0.1;
pub const DEFAULT_VOL_HOLD: f32 = 0.0;
pub const DEFAULT_VOL_DECAY: f32 = 0.5; // ~200 ms
pub const DEFAULT_VOL_SUSTAIN: f32 = 0.75;
pub const DEFAULT_VOL_RELEASE: f32 = 0.3;

// Default values for modulation envelopes
pub const DEFAULT_ATTACK: f32 = 0.00001;
pub const DEFAULT_HOLD: f32 = 0.2;
pub const DEFAULT_DECAY: f32 = 0.0;
pub const DEFAULT_SUSTAIN: f32 = 0.0;
pub const DEFAULT_RELEASE: f32 = 0.00001;
pub const DEFAULT_MULTIPLY: f32 = 0.5; // +0%

pub const IDENTITY: Easing<f32> = Easing::Linear {
    start: 0.0,
    end: 1.0,
};

pub const BIPOLAR: Easing<f32> = Easing::Linear {
    start: -1.0,
    end: 1.0,
};

pub const EASER: ParamEaser = ParamEaser {
    master_vol: Decibel::ease_db(MASTER_VOL_MIN_DB, MASTER_VOL_MAX_DB),
    osc_vol: Decibel::ease_db(OSC_VOL_MIN_DB, OSC_VOL_MAX_DB),
    vol_sustain: Decibel::ease_db(SUSTAIN_MIN_DB, 0.0),
    phase: IDENTITY,
    pan: BIPOLAR,
    warp: IDENTITY,
    shape: DiscreteLinear {
        values: [
            NoteShapeDiscrim::Sine,
            NoteShapeDiscrim::Skewtooth,
            NoteShapeDiscrim::Square,
            NoteShapeDiscrim::Noise,
        ],
    },
    fine_tune: Easing::Linear {
        start: -100.0,
        end: 100.0,
    },
    coarse_tune: Easing::SteppedLinear {
        start: I32Divable::new(-24),
        end: I32Divable::new(24),
        steps: 24 * 2 + 1,
    },
    env_attack: Easing::Exponential {
        start: 0.001,
        end: 2.0,
    },
    env_hold: Easing::Exponential {
        start: 0.0,
        end: 5.0,
    },
    env_decay: Easing::Exponential {
        start: 0.001,
        end: 5.0,
    },
    env_sustain: IDENTITY,
    env_release: Easing::Exponential {
        start: 0.001,
        end: 5.0,
    },
    env_multiply: BIPOLAR,
    vol_lfo_amp: Easing::Exponential {
        start: 0.0,
        end: 1.0,
    },
    pitch_lfo_amp: Easing::Exponential {
        start: 0.0,
        end: 0.1,
    },
    lfo_period: Easing::Exponential {
        start: 0.001,
        end: 10.0,
    },
    filter_type: DiscreteLinear {
        values: [
            FilterTypeDiscrim::LowPass,
            FilterTypeDiscrim::HighPass,
            FilterTypeDiscrim::PeakingEQ,
            FilterTypeDiscrim::LowShelf,
            FilterTypeDiscrim::HighShelf,
            FilterTypeDiscrim::BandPass,
            FilterTypeDiscrim::Notch,
            FilterTypeDiscrim::AllPass,
        ],
    },
    filter_db: Easing::Linear {
        start: -36.0,
        end: 36.0,
    },
    filter_freq: IDENTITY,
    filter_q: Easing::Linear {
        start: 0.01,
        end: 10.0,
    },
};

pub struct ParamEaser {
    master_vol: Easing<Decibel>,
    osc_vol: Easing<Decibel>,
    vol_sustain: Easing<Decibel>,
    pan: Easing<f32>,
    phase: Easing<f32>,
    pub shape: DiscreteLinear<NoteShapeDiscrim, { NoteShapeDiscrim::VARIANT_COUNT }>,
    pub warp: Easing<f32>,
    fine_tune: Easing<f32>,
    coarse_tune: Easing<I32Divable>,
    env_attack: Easing<f32>,
    env_hold: Easing<f32>,
    env_decay: Easing<f32>,
    env_sustain: Easing<f32>,
    env_release: Easing<f32>,
    env_multiply: Easing<f32>,
    vol_lfo_amp: Easing<f32>,
    pitch_lfo_amp: Easing<f32>,
    lfo_period: Easing<f32>,
    filter_type: DiscreteLinear<FilterTypeDiscrim, 8>,
    filter_freq: Easing<f32>,
    filter_q: Easing<f32>,
    filter_db: Easing<f32>,
}

pub struct Parameters {
    pub osc_1: OSCParams,
    pub master_vol: Decibel,
}

impl From<&RawParameters> for Parameters {
    fn from(params: &RawParameters) -> Self {
        Parameters {
            osc_1: OSCParams::from(&params.get_osc_1()),
            master_vol: EASER.master_vol.ease(params.master_vol.get()),
        }
    }
}

pub struct OSCParams {
    pub volume: Decibel,
    pub shape: NoteShape,
    // A value in [-1.0, 1.0] range. -1.0 means hard pan left. 1.0 means hard
    // pan right. 0.0 means center.
    pub pan: f32,
    // A normalized angle in [0.0, 1.0] range
    pub phase: f32,
    // In semi-tones
    pub coarse_tune: i32,
    // In cents
    pub fine_tune: f32,
    pub vol_adsr: VolEnvParams,
    pub vol_lfo: LFO,
    pub pitch_adsr: GeneralEnvParams,
    pub pitch_lfo: LFO,
    pub filter_params: FilterParams,
}

impl From<&RawOSC> for OSCParams {
    fn from(params: &RawOSC) -> Self {
        OSCParams {
            volume: EASER.osc_vol.ease(params.volume),
            shape: NoteShape::new(EASER.shape.ease(params.shape), EASER.warp.ease(params.warp)),
            pan: EASER.pan.ease(params.pan),
            phase: EASER.phase.ease(params.phase),
            // In semi-tones
            coarse_tune: EASER.coarse_tune.ease(params.coarse_tune).into(),
            // In [-1.0, 1.0] range
            fine_tune: EASER.fine_tune.ease(params.fine_tune),
            vol_adsr: VolEnvParams::from(&params.vol_adsr),
            vol_lfo: LFO {
                amplitude: EASER.vol_lfo_amp.ease(params.vol_lfo.amplitude),
                period: EASER.lfo_period.ease(params.vol_lfo.period),
            },
            pitch_adsr: GeneralEnvParams::from(&params.pitch_adsr),
            pitch_lfo: LFO {
                amplitude: EASER.pitch_lfo_amp.ease(params.pitch_lfo.amplitude),
                period: EASER.lfo_period.ease(params.pitch_lfo.period),
            },
            filter_params: FilterParams {
                filter: to_filter_type(params.filter_type, (params.filter_gain - 0.5) * 36.0),
                q_value: (params.filter_q * 10.0).max(0.01),
                freq: params.filter_freq,
            },
        }
    }
}
// A set of immutable envelope parameters. The envelope is defined as follows:
// - In the attack phase, the envelope value goes from the `zero` value to the
//   `max` value.
// - In the decay phase, the envelope value goes from the `max` value to the
//   `sustain` value.
// - In the sustain phase, the envelope value is constant at the `sustain` value.
// - In the release phase, the envelope value goes from the `sustain` value to
//   `zero` value.
// The envelope value is then scaled by the `multiply` value
pub trait EnvelopeParams<T> {
    // In seconds, how long attack phase is
    fn attack(&self) -> f32;
    // In seconds, how long hold phase is
    fn hold(&self) -> f32;
    // In seconds, how long decay phase is
    fn decay(&self) -> f32;
    // The value to go to during sustain phase
    fn sustain(&self) -> T;
    // In seconds, how long release phase is
    fn release(&self) -> f32;
    // In -1.0 to 1.0 range usually. Multiplied by the value given by the ADSR
    fn multiply(&self) -> f32 {
        1.0
    }
}

/// An ADSR envelope.
#[derive(Debug, Clone, Copy)]
pub struct VolEnvParams {
    pub attack: f32,
    pub hold: f32,
    pub decay: f32,
    pub sustain: Decibel,
    pub release: f32,
}

impl EnvelopeParams<Decibel> for VolEnvParams {
    fn attack(&self) -> f32 {
        self.attack
    }
    fn hold(&self) -> f32 {
        self.hold
    }
    fn decay(&self) -> f32 {
        self.decay
    }
    fn sustain(&self) -> Decibel {
        self.sustain
    }
    fn release(&self) -> f32 {
        self.release
    }
}

impl VolEnvParams {
    fn from(params: &RawEnvelope) -> Self {
        VolEnvParams {
            attack: EASER.env_attack.ease(params.attack),
            hold: EASER.env_hold.ease(params.hold),
            decay: EASER.env_decay.ease(params.decay),
            sustain: EASER.vol_sustain.ease(params.sustain),
            release: EASER.env_release.ease(params.release),
        }
    }
}

/// An ADSR envelope.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GeneralEnvParams {
    // In seconds
    pub attack: f32,
    // In seconds
    pub hold: f32,
    // In seconds
    pub decay: f32,
    // In percent (0.0 to 1.0)
    pub sustain: f32,
    // In seconds
    pub release: f32,
    // In percent (0.0 to 1.0)
    pub multiply: f32,
}

impl EnvelopeParams<f32> for GeneralEnvParams {
    fn attack(&self) -> f32 {
        self.attack
    }
    fn hold(&self) -> f32 {
        self.hold
    }
    fn decay(&self) -> f32 {
        self.decay
    }
    fn sustain(&self) -> f32 {
        self.sustain
    }
    fn release(&self) -> f32 {
        self.release
    }
    fn multiply(&self) -> f32 {
        self.multiply
    }
}

impl From<&RawEnvelope> for GeneralEnvParams {
    fn from(params: &RawEnvelope) -> Self {
        GeneralEnvParams {
            attack: EASER.env_attack.ease(params.attack),
            hold: EASER.env_hold.ease(params.hold),
            decay: EASER.env_decay.ease(params.decay),
            sustain: EASER.env_sustain.ease(params.sustain),
            release: EASER.env_release.ease(params.release),
            multiply: EASER.env_multiply.ease(params.multiply),
        }
    }
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct LFO {
    pub amplitude: f32,
    // In seconds
    pub period: f32,
}

impl RawParameters {
    /// Set a parameter, but do not send any GUI knob update. You usually want to
    /// call this if you are working in UI code since the knobs will already update
    /// themselves properly.
    pub fn set(&self, value: f32, parameter: ParameterType) {
        // These begin_edit/end_edit calls are needed so Ableton will notice
        // parameter changes in the "Configure" window.
        // TODO: investigate if I should send this only on mouseup/mousedown
        // TODO2: Ableton acts glitchy if I send self.host.update_display(), but
        // not doing it means I can't update the text on a parameter if a different
        // parameter would change its name...
        self.host.begin_edit(parameter.into());
        self.get_ref(parameter).set(value);
        self.host.end_edit(parameter.into());
    }

    /// Set a parameter, updating the GUI with it. This is usually only called
    /// when setting a parameter from the host side or internally, such as when
    /// loading a preset. (basically any time that we set a parameter not through
    /// a GUI Message/knob interaction).
    pub fn set_and_update_knob(&self, value: f32, parameter: ParameterType) {
        self.set(value, parameter);

        // Notify the GUI to update its view
        // TODO: Is it really okay to just ignore errors?
        let _ = self.sender.send((parameter, value));
    }

    pub fn get(&self, parameter: ParameterType) -> f32 {
        self.get_ref(parameter).get()
    }

    /// Returns a user-facing text output for the given parameter. This is broken
    /// into a tuple consisting of (`value`, `units`)
    fn get_strings(&self, parameter: ParameterType) -> (String, String) {
        use EnvelopeParam::*;
        use OSCParameterType::*;
        let params = Parameters::from(self);

        fn make_strings(value: f32, label: &str) -> (String, String) {
            (format!("{:.2}", value), label.to_string())
        }

        fn duration_strings(duration: f32) -> (String, String) {
            if duration < 1.0 {
                (format!("{:.1}", duration * 1000.0), " ms".to_string())
            } else {
                (format!("{:.2}", duration), " sec".to_string())
            }
        }

        fn make_pan(pan: f32) -> (String, String) {
            if pan < 0.0 {
                make_strings(-pan * 100.0, "% L")
            } else if pan > 0.0 {
                make_strings(pan * 100.0, "% R")
            } else {
                ("".to_string(), "% C".to_string())
            }
        }

        fn envelope_strings(envelope: GeneralEnvParams, param: EnvelopeParam) -> (String, String) {
            match param {
                Attack => duration_strings(envelope.attack),
                Hold => duration_strings(envelope.hold),
                Decay => duration_strings(envelope.decay),
                Sustain => make_strings(envelope.sustain * 100.0, "%"),
                Release => duration_strings(envelope.release),
                Multiply => make_strings(envelope.multiply * 100.0, "%"),
            }
        }

        fn vol_env_strings(envelope: VolEnvParams, param: EnvelopeParam) -> (String, String) {
            match param {
                Attack => duration_strings(envelope.attack),
                Hold => duration_strings(envelope.hold),
                Decay => duration_strings(envelope.decay),
                Sustain => volume_string(envelope.sustain),
                Release => duration_strings(envelope.release),
                Multiply => make_strings(-9999.0, "IMPOSSIBLE"),
            }
        }

        fn volume_string(decibel: Decibel) -> (String, String) {
            if decibel.get_db() <= crate::sound_gen::NEG_INF_DB_THRESHOLD {
                ("-inf".to_string(), "dB".to_string())
            } else if decibel.get_db() < 0.0 {
                (format!("{:.2}", decibel.get_db()), "dB".to_string())
            } else {
                (format!("+{:.2}", decibel.get_db()), "dB".to_string())
            }
        }

        match parameter {
            ParameterType::MasterVolume => volume_string(params.master_vol),
            ParameterType::OSC1(osc_param) => {
                let osc = match parameter {
                    ParameterType::OSC1(_) => &params.osc_1,
                    _ => unreachable!(),
                };
                match osc_param {
                    Volume => volume_string(osc.volume),
                    Pan => make_pan(osc.pan),
                    Phase => make_strings(osc.phase * 360.0, " deg"),
                    Shape => (format!("{:.2}", osc.shape), "".to_string()),
                    Warp => match osc.shape {
                        NoteShape::Skewtooth(warp) | NoteShape::Square(warp) => {
                            (format!("{:.2}", warp), "".to_string())
                        }
                        NoteShape::Sine | NoteShape::Noise => ("N/A".to_string(), "".to_string()),
                    },
                    CoarseTune => (format!("{}", osc.coarse_tune), " semis".to_string()),
                    FineTune => make_strings(osc.fine_tune, " cents"),
                    VolumeEnv(param) => vol_env_strings(osc.vol_adsr, param),
                    VolLFOAmplitude => make_strings(osc.vol_lfo.amplitude * 100.0, "%"),
                    VolLFOPeriod => duration_strings(osc.vol_lfo.period),
                    PitchEnv(param) => envelope_strings(osc.pitch_adsr, param),
                    PitchLFOAmplitude => make_strings(osc.pitch_lfo.amplitude * 100.0, "%"),
                    PitchLFOPeriod => duration_strings(osc.pitch_lfo.period),
                    FilterType => (biquad_to_string(osc.filter_params.filter), "".to_string()),
                    FilterFreq => make_strings(osc.filter_params.freq, " Hz"),
                    FilterQ => make_strings(osc.filter_params.q_value, ""),
                    FilterGain => match osc.filter_params.filter {
                        biquad::Type::LowShelf(db_gain)
                        | biquad::Type::HighShelf(db_gain)
                        | biquad::Type::PeakingEQ(db_gain) => make_strings(db_gain, " dB"),
                        _ => ("N/A".to_string(), "".to_string()),
                    },
                }
            }
        }
    }
}

impl PluginParameters for RawParameters {
    fn get_parameter_label(&self, index: i32) -> String {
        if let Ok(parameter) = ParameterType::try_from(index) {
            self.get_strings(parameter).1
        } else {
            "".to_string()
        }
    }

    fn get_parameter_text(&self, index: i32) -> String {
        if let Ok(parameter) = ParameterType::try_from(index) {
            self.get_strings(parameter).0
        } else {
            "".to_string()
        }
    }

    fn get_parameter_name(&self, index: i32) -> String {
        if let Ok(param) = ParameterType::try_from(index) {
            param.to_string()
        } else {
            "".to_string()
        }
    }

    fn get_parameter(&self, index: i32) -> f32 {
        if let Ok(parameter) = ParameterType::try_from(index) {
            self.get(parameter)
        } else {
            0.0
        }
    }

    fn set_parameter(&self, index: i32, value: f32) {
        if let Ok(parameter) = ParameterType::try_from(index) {
            // This is needed because some VST hosts, such as Ableton, echo a
            // parameter change back to the plugin. This causes issues such as
            // weird knob behavior where the knob "flickers" because the user tries
            // to change the knob value, but ableton keeps sending back old, echoed
            // values.
            #[allow(clippy::float_cmp)]
            if self.get(parameter) == value {
                return;
            }

            // We need to update the GUI since we set the parameter via the host side.
            self.set_and_update_knob(value, parameter);
        }
    }

    fn can_be_automated(&self, index: i32) -> bool {
        ParameterType::try_from(index).is_ok()
    }

    fn string_to_parameter(&self, _index: i32, _text: String) -> bool {
        false
    }
}

/// Oscillator specific parameters. These are normalized f32 which should be
/// baked by calling OSCParams::from()
pub struct RawOSC {
    pub volume: f32,
    pub shape: f32,
    pub pan: f32,
    pub phase: f32,
    pub warp: f32,
    pub coarse_tune: f32,
    pub fine_tune: f32,
    pub vol_adsr: RawEnvelope,
    pub vol_lfo: RawLFO,
    pub pitch_adsr: RawEnvelope,
    pub pitch_lfo: RawLFO,
    pub filter_type: f32,
    pub filter_freq: f32,
    pub filter_q: f32,
    pub filter_gain: f32,
}

// Convience struct, represents parameters that are part of an envelope
#[derive(Debug)]
pub struct RawEnvelope {
    pub attack: f32,
    pub hold: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
    pub multiply: f32,
}

impl RawEnvelope {
    pub fn get(&self, param: EnvelopeParam) -> f32 {
        match param {
            EnvelopeParam::Attack => self.attack,
            EnvelopeParam::Hold => self.hold,
            EnvelopeParam::Decay => self.decay,
            EnvelopeParam::Sustain => self.sustain,
            EnvelopeParam::Release => self.release,
            EnvelopeParam::Multiply => self.multiply,
        }
    }
}

pub struct RawLFO {
    period: f32,
    amplitude: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OSCParameterType {
    Volume,
    Phase,
    Pan,
    Shape,
    Warp,
    FineTune,
    CoarseTune,
    VolumeEnv(EnvelopeParam),
    VolLFOAmplitude,
    VolLFOPeriod,
    PitchEnv(EnvelopeParam),
    PitchLFOAmplitude,
    PitchLFOPeriod,
    FilterType,
    FilterFreq,
    FilterQ,
    FilterGain,
}

impl std::fmt::Display for OSCParameterType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use OSCParameterType::*;
        match self {
            Volume => write!(f, "Volume"),
            Phase => write!(f, "Phase"),
            Pan => write!(f, "Pan"),
            Shape => write!(f, "Shape"),
            Warp => write!(f, "Warp"),
            CoarseTune => write!(f, "Coarse Tune"),
            FineTune => write!(f, "Fine Tune"),
            VolumeEnv(param) => write!(f, "{} (Volume)", param),
            VolLFOAmplitude => write!(f, "LFO Amplitude (Volume)"),
            VolLFOPeriod => write!(f, "LFO Period (Volume)"),
            PitchEnv(param) => write!(f, "{} (Pitch)", param),
            PitchLFOAmplitude => write!(f, "LFO Amplitude (Pitch)"),
            PitchLFOPeriod => write!(f, "LFO Period (Pitch)"),
            FilterType => write!(f, "Filter Type"),
            FilterFreq => write!(f, "Filter Frequency"),
            FilterQ => write!(f, "Q-Factor"),
            FilterGain => write!(f, "Filter Gain"),
        }
    }
}

#[derive(Debug, Display, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeParam {
    Attack,
    Decay,
    Hold,
    Sustain,
    Release,
    Multiply,
}

impl EnvelopeParam {
    pub fn get_default(param: EnvelopeParam) -> f32 {
        match param {
            EnvelopeParam::Attack => DEFAULT_ATTACK,
            EnvelopeParam::Decay => DEFAULT_DECAY,
            EnvelopeParam::Hold => DEFAULT_HOLD,
            EnvelopeParam::Sustain => DEFAULT_SUSTAIN,
            EnvelopeParam::Release => DEFAULT_RELEASE,
            EnvelopeParam::Multiply => DEFAULT_MULTIPLY,
        }
    }
}

/// The type of parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterType {
    MasterVolume,
    OSC1(OSCParameterType),
}

impl From<OSCParameterType> for ParameterType {
    fn from(param: OSCParameterType) -> Self {
        ParameterType::OSC1(param)
    }
}

pub fn to_filter_type(x: f32, db_gain: f32) -> biquad::Type<f32> {
    let x = (x * 8.0) as i32;
    match x {
        0 => biquad::Type::LowPass,
        1 => biquad::Type::HighPass,
        2 => biquad::Type::PeakingEQ(db_gain),
        3 => biquad::Type::LowShelf(db_gain),
        4 => biquad::Type::HighShelf(db_gain),
        5 => biquad::Type::BandPass,
        6 => biquad::Type::Notch,
        _ => biquad::Type::AllPass,
    }
}

pub fn biquad_to_string(x: biquad::Type<f32>) -> String {
    match x {
        biquad::Type::SinglePoleLowPass => "Single Pole Low Pass".to_string(),
        biquad::Type::LowPass => "Low Pass".to_string(),
        biquad::Type::HighPass => "High Pass".to_string(),
        biquad::Type::BandPass => "Band Pass".to_string(),
        biquad::Type::AllPass => "All Pass".to_string(),
        biquad::Type::Notch => "Notch Filter".to_string(),
        biquad::Type::LowShelf(_) => "Low Shelf".to_string(),
        biquad::Type::HighShelf(_) => "High Shelf".to_string(),
        biquad::Type::PeakingEQ(_) => "Peaking EQ".to_string(),
    }
}

macro_rules! table {
    ($macro:ident) => {
        $macro! {
        //  RawParameter identifier, ParameterType identifier
            RawParameters,          ParameterType;
            //  dummy parameters, included only because EnvelopeParam has a multiply
            //  variant but this doesn't make sense for volume envelopes.
            ParameterType::OSC1(OSCParameterType::VolumeEnv(EnvelopeParam::Multiply)),      osc_1_vol_env_multiply,         "OSC 1 Volume Multiply",      -1,   1.0,    NONE,     NONE;
            ParameterType::OSC1(OSCParameterType::PitchEnv(EnvelopeParam::Sustain)),        osc_1_pitch_env_sustain,        "OSC 1 Pitch Sustain",        -3,   0.0,    NONE,     NONE;
            //  real parameters
            //  variant                                                                     field_name                      name                          idx   default                 easer_field         preset_field
            ParameterType::MasterVolume,                                                    master_vol,                     "Master Volume",              0,    DEFAULT_MASTER_VOL,     master_vol,         master_vol;
            ParameterType::OSC1(OSCParameterType::Volume),                                  osc_1_volume,                   "OSC 1 Volume",               1,    DEFAULT_OSC_VOL,        osc_vol,            osc_1.volume;
            ParameterType::OSC1(OSCParameterType::Phase),                                   osc_1_phase,                    "OSC 1 Phase",                2,    0.0,                    phase,              osc_1.phase;
            ParameterType::OSC1(OSCParameterType::Pan),                                     osc_1_pan,                      "OSC 1 Pan",                  3,    0.5,                    pan,                osc_1.pan;
            ParameterType::OSC1(OSCParameterType::Shape),                                   osc_1_shape,                    "OSC 1 Shape",                4,    0.0,                    shape,              osc_1.shape;
            ParameterType::OSC1(OSCParameterType::Warp),                                    osc_1_warp,                     "OSC 1 Warp",                 5,    0.5,                    warp,               osc_1.warp;
            ParameterType::OSC1(OSCParameterType::FineTune),                                osc_1_fine_tune,                "OSC 1 Fine Tune",            6,    0.5,                    fine_tune,          osc_1.fine_tune;
            ParameterType::OSC1(OSCParameterType::CoarseTune),                              osc_1_coarse_tune,              "OSC 1 Coarse Tune",          7,    0.5,                    coarse_tune,        osc_1.coarse_tune;
            ParameterType::OSC1(OSCParameterType::VolumeEnv(EnvelopeParam::Attack)),        osc_1_vol_env_attack,           "OSC 1 Volume Attack",        8,    DEFAULT_VOL_ATTACK,     env_attack,         osc_1.vol_attack;
            ParameterType::OSC1(OSCParameterType::VolumeEnv(EnvelopeParam::Hold)),          osc_1_vol_env_hold,             "OSC 1 Volume Hold",          9,    DEFAULT_VOL_HOLD,       env_hold,           osc_1.vol_hold;
            ParameterType::OSC1(OSCParameterType::VolumeEnv(EnvelopeParam::Decay)),         osc_1_vol_env_decay,            "OSC 1 Volume Decay",         10,   DEFAULT_VOL_DECAY,      env_decay,          osc_1.vol_decay;
            ParameterType::OSC1(OSCParameterType::VolumeEnv(EnvelopeParam::Sustain)),       osc_1_vol_env_sustain,          "OSC 1 Volume Sustain",       11,   DEFAULT_VOL_SUSTAIN,    vol_sustain,        osc_1.vol_sustain;
            ParameterType::OSC1(OSCParameterType::VolumeEnv(EnvelopeParam::Release)),       osc_1_vol_env_release,          "OSC 1 Volume Release",       12,   DEFAULT_VOL_RELEASE,    env_release,        osc_1.vol_release;
            ParameterType::OSC1(OSCParameterType::VolLFOAmplitude),                         osc_1_vol_lfo_amplitude,        "OSC 1 Vol LFO Amplitude",    13,   0.0,                    vol_lfo_amp,        osc_1.vol_lfo.amplitude;
            ParameterType::OSC1(OSCParameterType::VolLFOPeriod),                            osc_1_vol_lfo_period,           "OSC 1 Vol LFO Period",       14,   0.5,                    lfo_period,         osc_1.vol_lfo.period;
            ParameterType::OSC1(OSCParameterType::PitchEnv(EnvelopeParam::Attack)),         osc_1_pitch_env_attack,         "OSC 1 Pitch Attack",         15,   DEFAULT_ATTACK,         env_attack,         osc_1.pitch_attack;
            ParameterType::OSC1(OSCParameterType::PitchEnv(EnvelopeParam::Hold)),           osc_1_pitch_env_hold,           "OSC 1 Pitch Hold",           16,   DEFAULT_HOLD,           env_hold,           osc_1.pitch_hold;
            ParameterType::OSC1(OSCParameterType::PitchEnv(EnvelopeParam::Decay)),          osc_1_pitch_env_decay,          "OSC 1 Pitch Decay",          17,   DEFAULT_DECAY,          env_decay,          osc_1.pitch_decay;
            ParameterType::OSC1(OSCParameterType::PitchEnv(EnvelopeParam::Release)),        osc_1_pitch_env_release,        "OSC 1 Pitch Release",        18,   DEFAULT_RELEASE,        env_release,        osc_1.pitch_release;
            ParameterType::OSC1(OSCParameterType::PitchEnv(EnvelopeParam::Multiply)),       osc_1_pitch_env_multiply,       "OSC 1 Pitch Multiply",       19,   DEFAULT_MULTIPLY,       env_multiply,       osc_1.pitch_multiply;
            ParameterType::OSC1(OSCParameterType::PitchLFOAmplitude),                       osc_1_pitch_lfo_amplitude,      "OSC 1 Pitch LFO Amplitude",  20,   0.0,                    pitch_lfo_amp,      osc_1.pitch_lfo.amplitude;
            ParameterType::OSC1(OSCParameterType::PitchLFOPeriod),                          osc_1_pitch_lfo_period,         "OSC 1 Pitch LFO Period",     21,   0.5,                    lfo_period,         osc_1.pitch_lfo.period;
            ParameterType::OSC1(OSCParameterType::FilterType),                              osc_1_filter_type,              "OSC 1 Filter Type",          22,   0.0,                    filter_type,        osc_1.filter.filter_type;
            ParameterType::OSC1(OSCParameterType::FilterFreq),                              osc_1_filter_freq,              "OSC 1 Filter Freq",          23,   1.0,                    filter_freq,        osc_1.filter.freq;
            ParameterType::OSC1(OSCParameterType::FilterQ),                                 osc_1_filter_q,                 "OSC 1 Filter Q",             24,   0.1,                    filter_q,           osc_1.filter.q;
            ParameterType::OSC1(OSCParameterType::FilterGain),                              osc_1_filter_gain,              "OSC 1 Filter Gain",          25,   0.5,                    filter_db,          osc_1.filter.db_gain;
        }
    };
}

impl ParameterType {
    pub const COUNT: usize = 66;
}

impl RawParameters {
    pub fn get_osc_1(&self) -> RawOSC {
        RawOSC {
            volume: self.osc_1_volume.get(),
            phase: self.osc_1_phase.get(),
            pan: self.osc_1_pan.get(),
            shape: self.osc_1_shape.get(),
            warp: self.osc_1_warp.get(),
            fine_tune: self.osc_1_fine_tune.get(),
            coarse_tune: self.osc_1_coarse_tune.get(),
            vol_adsr: RawEnvelope {
                attack: self.osc_1_vol_env_attack.get(),
                hold: self.osc_1_vol_env_hold.get(),
                decay: self.osc_1_vol_env_decay.get(),
                sustain: self.osc_1_vol_env_sustain.get(),
                release: self.osc_1_vol_env_release.get(),
                multiply: self.osc_1_vol_env_multiply.get(),
            },
            vol_lfo: RawLFO {
                amplitude: self.osc_1_vol_lfo_amplitude.get(),
                period: self.osc_1_vol_lfo_period.get(),
            },
            pitch_adsr: RawEnvelope {
                attack: self.osc_1_pitch_env_attack.get(),
                hold: self.osc_1_pitch_env_hold.get(),
                decay: self.osc_1_pitch_env_decay.get(),
                sustain: self.osc_1_pitch_env_sustain.get(),
                release: self.osc_1_pitch_env_release.get(),
                multiply: self.osc_1_pitch_env_multiply.get(),
            },
            pitch_lfo: RawLFO {
                amplitude: self.osc_1_pitch_lfo_amplitude.get(),
                period: self.osc_1_pitch_lfo_period.get(),
            },
            filter_type: self.osc_1_filter_type.get(),
            filter_freq: self.osc_1_filter_freq.get(),
            filter_q: self.osc_1_filter_q.get(),
            filter_gain: self.osc_1_filter_gain.get(),
        }
    }
}

table! {generate_raw_params}
table! {impl_new}
table! {impl_get_ref}
table! {impl_get_default}
table! {impl_from_i32}
table! {impl_into_i32}
table! {impl_display}
