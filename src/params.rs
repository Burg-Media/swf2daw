//! Parameters of the sound options: a strip per bus and the master rack. All of them are
//! normal plugin parameters, so the DAW can automate them.

use crate::dsp::{EQ_FREQS, Rack, StripEq};
use nih_plug::prelude::*;

/// Names of the buses, also the names of the plugin's extra outputs.
pub const BUS_NAMES: [&str; crate::audio::BUSES] = ["Pulse 1", "Pulse 2", "Wave", "Noise", "Music", "SFX"];

fn db_param(name: String) -> FloatParam {
    FloatParam::new(name, 0.0, FloatRange::Linear { min: -12.0, max: 12.0 })
        .with_step_size(0.1)
        .with_unit(" dB")
        .with_value_to_string(formatters::v2s_f32_rounded(1))
}

fn amount_param(name: String, default: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min: 0.0, max: 1.0 })
        .with_unit("%")
        .with_value_to_string(formatters::v2s_f32_percentage(0))
        .with_string_to_value(formatters::s2v_f32_percentage())
}

#[derive(Params)]
pub struct StripParams {
    #[id = "level"]
    pub level: FloatParam,
    #[id = "pan"]
    pub pan: FloatParam,
    #[id = "low"]
    pub low: FloatParam,
    #[id = "mid"]
    pub mid: FloatParam,
    #[id = "high"]
    pub high: FloatParam,
    #[id = "mute"]
    pub mute: BoolParam,
    #[id = "solo"]
    pub solo: BoolParam,
}

impl StripParams {
    pub fn new(bus: &str) -> Self {
        Self {
            level: FloatParam::new(
                format!("{bus} Level"),
                1.0,
                FloatRange::Skewed {
                    min: 0.0,
                    max: util::db_to_gain(6.0),
                    factor: FloatRange::gain_skew_factor(-60.0, 6.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(30.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(1))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),
            pan: FloatParam::new(format!("{bus} Pan"), 0.0, FloatRange::Linear { min: -1.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(30.0))
                .with_value_to_string(formatters::v2s_f32_panning())
                .with_string_to_value(formatters::s2v_f32_panning()),
            low: db_param(format!("{bus} Low")),
            mid: db_param(format!("{bus} Mid")),
            high: db_param(format!("{bus} High")),
            mute: BoolParam::new(format!("{bus} Mute"), false),
            solo: BoolParam::new(format!("{bus} Solo"), false),
        }
    }

    pub fn eq(&self) -> StripEq {
        StripEq { low: self.low.value(), mid: self.mid.value(), high: self.high.value() }
    }
}

#[derive(Params)]
pub struct EqBand {
    #[id = "gain"]
    pub gain: FloatParam,
}

/// One effect of the rack: an on switch and one knob.
#[derive(Params)]
pub struct FxParams {
    #[id = "on"]
    pub on: BoolParam,
    #[id = "amount"]
    pub amount: FloatParam,
}

impl FxParams {
    fn new(name: &str, default: f32) -> Self {
        Self {
            on: BoolParam::new(name.to_string(), false),
            amount: amount_param(format!("{name} Amount"), default),
        }
    }

    fn get(&self) -> Option<f32> {
        self.on.value().then(|| self.amount.value())
    }
}

#[derive(Params)]
pub struct RackParams {
    #[nested(array, group = "EQ")]
    pub eq: [EqBand; 10],
    #[id = "mega"]
    pub mega: BoolParam,

    #[id = "filter"]
    pub filter: BoolParam,
    #[id = "cutoff"]
    pub cutoff: FloatParam,
    #[id = "reso"]
    pub reso: FloatParam,

    #[nested(id_prefix = "crush", group = "Bitcrush")]
    pub crush: FxParams,
    #[nested(id_prefix = "wobble", group = "Tape Wobble")]
    pub wobble: FxParams,
    #[nested(id_prefix = "ring", group = "Ring Mod")]
    pub ring: FxParams,
    #[nested(id_prefix = "gate", group = "Trance Gate")]
    pub gate: FxParams,
    #[nested(id_prefix = "echo", group = "Echo")]
    pub echo: FxParams,
    #[nested(id_prefix = "space", group = "Space")]
    pub space: FxParams,
}

pub const EQ_LABELS: [&str; 10] = ["31", "62", "125", "250", "500", "1k", "2k", "4k", "8k", "16k"];

impl Default for RackParams {
    fn default() -> Self {
        Self {
            eq: std::array::from_fn(|i| EqBand { gain: db_param(format!("EQ {} Hz", EQ_LABELS[i])) }),
            mega: BoolParam::new("Mega Bass", false),
            filter: BoolParam::new("Filter", false),
            cutoff: FloatParam::new(
                "Filter Cutoff",
                2000.0,
                FloatRange::Skewed { min: 80.0, max: 20000.0, factor: FloatRange::skew_factor(-2.0) },
            )
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_hz_then_khz(1))
            .with_string_to_value(formatters::s2v_f32_hz_then_khz()),
            reso: FloatParam::new("Filter Resonance", 0.7, FloatRange::Skewed { min: 0.5, max: 16.0, factor: 0.5 })
                .with_value_to_string(formatters::v2s_f32_rounded(1)),
            crush: FxParams::new("Bitcrush", 0.55),
            wobble: FxParams::new("Tape Wobble", 0.4),
            ring: FxParams::new("Ring Mod", 0.35),
            gate: FxParams::new("Trance Gate", 0.7),
            echo: FxParams::new("Echo", 0.35),
            space: FxParams::new("Space", 0.45),
        }
    }
}

impl RackParams {
    pub fn get(&self) -> Rack {
        debug_assert_eq!(EQ_FREQS.len(), self.eq.len());
        Rack {
            eq: std::array::from_fn(|i| self.eq[i].gain.value()),
            mega: self.mega.value(),
            filter: self.filter.value().then(|| (self.cutoff.value(), self.reso.value())),
            crush: self.crush.get(),
            wobble: self.wobble.get(),
            ring: self.ring.get(),
            gate: self.gate.get(),
            echo: self.echo.get(),
            space: self.space.get(),
        }
    }
}
