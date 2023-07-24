/*
    Copyright (C) 2023 Aurora McGinnis

    This Source Code Form is subject to the terms of the Mozilla Public
    License, v. 2.0. If a copy of the MPL was not distributed with this
    file, You can obtain one at http://mozilla.org/MPL/2.0/.

    error.rs: Provide a Result typedef and an Error type
*/

use serde::ser::Error as SerError;
use std::fmt::Display;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

/// Error type for logfmt serialization failures.
#[derive(Error, Debug)]
pub enum Error {
    #[error("cannot write an empty identifier")]
    EmptyIdentifier,
    #[error("error writing to buffer")]
    WriteError {
        #[from]
        source: std::io::Error,
    },
    #[error("error from Serialize implementation: {msg}")]
    SerializeError { msg: String },
}

impl SerError for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        Error::SerializeError {
            msg: msg.to_string(),
        }
    }
}
