//! Maschine MK2 button identifiers.
//!
//! The 48 buttons are packed one-bit-each into 6 bytes of the input report.
//! The exact bit position of each named button (the enum discriminant below)
//! follows the layout used by the `cabl` library. The handful actually used by
//! the groovebox (transport + edit buttons) are wired up in `app.rs`; the rest
//! are defined so the mapping is complete and easy to correct against hardware.
//!
//! NOTE (VERIFY): the precise discriminant-to-physical-button assignment should
//! be confirmed on a real unit. They are centralized here so a fix is one edit.

use core::convert::TryFrom;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Button {
    // Row above the left display
    F1 = 0,
    F2 = 1,
    F3 = 2,
    Control = 3,
    Nav = 4,
    NavLeft = 5,
    NavRight = 6,
    MainMenu = 7,
    // Row above the right display
    Browse = 8,
    Sampling = 9,
    BrowseLeft = 10,
    BrowseRight = 11,
    All = 12,
    AutoWrite = 13,
    // Group buttons A..H live in the group LED report, not here.
    // Pad-mode column
    Scene = 14,
    Pattern = 15,
    PadMode = 16,
    View = 17,
    Duplicate = 18,
    Select = 19,
    Solo = 20,
    Mute = 21,
    // Transport
    Restart = 22,
    StepLeft = 23,
    StepRight = 24,
    Grid = 25,
    Play = 26,
    Rec = 27,
    Erase = 28,
    Shift = 29,
    // Master section
    Volume = 30,
    Swing = 31,
    Tempo = 32,
    Master = 33,
    Group = 34,
    // Edit section
    Note = 35,
    Repeat = 36,
    Snap = 37,
    Quantize = 38,
    Clear = 39,
    Copy = 40,
    Paste = 41,
    Enter = 42,
    // Spare / unmapped
    Aux43 = 43,
    Aux44 = 44,
    Aux45 = 45,
    Aux46 = 46,
    Aux47 = 47,
}

impl Button {
    /// Bit index (0..47) of this button inside the 6-byte button field.
    pub fn bit_index(self) -> usize {
        self as usize
    }

    /// Human-readable name for UI / logging.
    pub fn name(self) -> &'static str {
        use Button::*;
        match self {
            F1 => "F1",
            F2 => "F2",
            F3 => "F3",
            Control => "CONTROL",
            Nav => "NAV",
            NavLeft => "NAV<",
            NavRight => "NAV>",
            MainMenu => "MENU",
            Browse => "BROWSE",
            Sampling => "SAMPLING",
            BrowseLeft => "BROWSE<",
            BrowseRight => "BROWSE>",
            All => "ALL",
            AutoWrite => "AUTO",
            Scene => "SCENE",
            Pattern => "PATTERN",
            PadMode => "PADMODE",
            View => "VIEW",
            Duplicate => "DUPLICATE",
            Select => "SELECT",
            Solo => "SOLO",
            Mute => "MUTE",
            Restart => "RESTART",
            StepLeft => "STEP<",
            StepRight => "STEP>",
            Grid => "GRID",
            Play => "PLAY",
            Rec => "REC",
            Erase => "ERASE",
            Shift => "SHIFT",
            Volume => "VOLUME",
            Swing => "SWING",
            Tempo => "TEMPO",
            Master => "MASTER",
            Group => "GROUP",
            Note => "NOTE",
            Repeat => "REPEAT",
            Snap => "SNAP",
            Quantize => "QUANTIZE",
            Clear => "CLEAR",
            Copy => "COPY",
            Paste => "PASTE",
            Enter => "ENTER",
            Aux43 => "AUX43",
            Aux44 => "AUX44",
            Aux45 => "AUX45",
            Aux46 => "AUX46",
            Aux47 => "AUX47",
        }
    }
}

impl TryFrom<usize> for Button {
    type Error = ();
    fn try_from(value: usize) -> Result<Self, Self::Error> {
        if value < super::device::NUM_BUTTONS {
            // Safe: enum is repr(u8) with contiguous discriminants 0..48.
            Ok(unsafe { core::mem::transmute::<u8, Button>(value as u8) })
        } else {
            Err(())
        }
    }
}
