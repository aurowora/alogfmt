/*
    Copyright (C) 2023 Aurora McGinnis

    This Source Code Form is subject to the terms of the Mozilla Public
    License, v. 2.0. If a copy of the MPL was not distributed with this
    file, You can obtain one at http://mozilla.org/MPL/2.0/.

    util.rs: Utility functions
*/

/// Given a ASCII control character, space, or DEL character, return its corresponding unicode photo.
pub(crate) fn as_control_picture(ch: char) -> Option<char> {
    match ch {
        '\0' => Some('␀'),
        '\x01' => Some('␁'),
        '\x02' => Some('␂'),
        '\x03' => Some('␃'),
        '\x04' => Some('␄'),
        '\x05' => Some('␅'),
        '\x06' => Some('␆'),
        '\x07' => Some('␇'),
        '\x08' => Some('␈'),
        '\x09' => Some('␉'),
        '\x0A' => Some('␊'),
        '\x0B' => Some('␋'),
        '\x0C' => Some('␌'),
        '\x0D' => Some('␍'),
        '\x0E' => Some('␎'),
        '\x0F' => Some('␏'),
        '\x10' => Some('␐'),
        '\x11' => Some('␑'),
        '\x12' => Some('␒'),
        '\x13' => Some('␓'),
        '\x14' => Some('␔'),
        '\x15' => Some('␕'),
        '\x16' => Some('␖'),
        '\x17' => Some('␗'),
        '\x18' => Some('␘'),
        '\x19' => Some('␙'),
        '\x1A' => Some('␚'),
        '\x1B' => Some('␛'),
        '\x1C' => Some('␜'),
        '\x1D' => Some('␝'),
        '\x1E' => Some('␞'),
        '\x1F' => Some('␟'),
        '\x20' => Some('␠'),
        '\x7F' => Some('␡'),
        _ => None,
    }
}
