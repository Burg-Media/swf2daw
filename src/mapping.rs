//! What MIDI notes and CCs do in the game.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Everything a MIDI source can be bound to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Target {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    Start,
    Select,
    Space,
    KeyC,
    MouseX,
    MouseY,
    Click,
}

impl Target {
    pub const ALL: [Target; 13] = [
        Target::Up,
        Target::Down,
        Target::Left,
        Target::Right,
        Target::A,
        Target::B,
        Target::Start,
        Target::Select,
        Target::Space,
        Target::KeyC,
        Target::MouseX,
        Target::MouseY,
        Target::Click,
    ];

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap()
    }

    /// Label on the mapper button.
    pub fn label(self) -> &'static str {
        match self {
            Target::Up => "UP",
            Target::Down => "DOWN",
            Target::Left => "LEFT",
            Target::Right => "RIGHT",
            Target::A => "A / X",
            Target::B => "B / Z",
            Target::Start => "START",
            Target::Select => "SELECT",
            Target::Space => "SPACE",
            Target::KeyC => "C",
            Target::MouseX => "MOUSE X",
            Target::MouseY => "MOUSE Y",
            Target::Click => "CLICK",
        }
    }

    /// Bit in gb2swf's gbPad mask (order of Main.BUTTONS: right, left, up, down, a, b, select, start).
    pub fn gb_bit(self) -> Option<u8> {
        Some(match self {
            Target::Right => 0,
            Target::Left => 1,
            Target::Up => 2,
            Target::Down => 3,
            Target::A => 4,
            Target::B => 5,
            Target::Select => 6,
            Target::Start => 7,
            _ => return None,
        })
    }

    /// Continuous targets take the CC value, the rest are buttons.
    pub fn is_axis(self) -> bool {
        matches!(self, Target::MouseX | Target::MouseY)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Source {
    Note(u8),
    Cc(u8),
}

impl Source {
    /// "C4", "D#4" (middle C = 60 = C4) or "CC 1".
    pub fn name(self) -> String {
        match self {
            Source::Note(n) => {
                const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
                format!("{}{}", NAMES[(n % 12) as usize], n as i32 / 12 - 1)
            }
            Source::Cc(c) => format!("CC {c}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapping {
    pub binds: BTreeMap<Source, Target>,
}

impl Default for Mapping {
    /// Arrow cluster on the white keys around middle C (D# sits above D like the up arrow),
    /// buttons to the right of it, mod wheel and CC 2 as a mouse, sustain pedal clicks.
    fn default() -> Self {
        let binds = [
            (Source::Note(60), Target::Left),
            (Source::Note(62), Target::Down),
            (Source::Note(63), Target::Up),
            (Source::Note(64), Target::Right),
            (Source::Note(65), Target::Select),
            (Source::Note(67), Target::Start),
            (Source::Note(69), Target::B),
            (Source::Note(71), Target::A),
            (Source::Note(72), Target::Space),
            (Source::Note(74), Target::KeyC),
            (Source::Cc(1), Target::MouseX),
            (Source::Cc(2), Target::MouseY),
            (Source::Cc(64), Target::Click),
        ]
        .into_iter()
        .collect();
        Self { binds }
    }
}

impl Mapping {
    pub fn get(&self, src: Source) -> Option<Target> {
        self.binds.get(&src).copied()
    }

    /// Binds `src` to `target`, replacing whatever was bound to that target before.
    pub fn learn(&mut self, src: Source, target: Target) {
        self.binds.retain(|_, t| *t != target);
        self.binds.insert(src, target);
    }

    /// First source bound to a target, for the label on the mapper.
    pub fn source_of(&self, target: Target) -> Option<Source> {
        self.binds.iter().find(|(_, t)| **t == target).map(|(s, _)| *s)
    }
}
