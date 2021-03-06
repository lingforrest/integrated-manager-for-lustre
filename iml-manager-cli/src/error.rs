// Copyright (c) 2020 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use iml_wire_types::Command;

#[derive(Debug)]
pub enum DurationParseError {
    NoUnit,
    InvalidUnit,
    InvalidValue,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum RunStratagemCommandResult {
    DurationOrderError,
    FilesystemRequired,
    FilesystemDoesNotExist,
    FilesystemUnavailable,
    InvalidArgument,
    PurgeDurationTooBig,
    ReportDurationTooBig,
    PurgeDurationTooSmall,
    ReportDurationTooSmall,
    Mdt0NotFound,
    Mdt0NotMounted,
    StratagemServerProfileNotInstalled,
    StratagemClientProfileNotInstalled,
    RequiredFieldsMissing,
    ServerError,
    UnknownError,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct RunStratagemValidationError {
    pub code: RunStratagemCommandResult,
    pub message: String,
}

impl std::fmt::Display for DurationParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            DurationParseError::NoUnit => write!(f, "No unit specified."),
            DurationParseError::InvalidUnit => {
                write!(f, "Invalid unit. Valid units include 'h' and 'd'.")
            }
            DurationParseError::InvalidValue => {
                write!(f, "Invalid value specified. Must be a valid integer.")
            }
        }
    }
}

impl std::error::Error for DurationParseError {}

#[derive(Debug)]
pub enum ImlManagerCliError {
    ApiError(String),
    ClientRequestError(iml_manager_client::ImlManagerClientError),
    CombineEasyError(combine::stream::easy::Errors<char, &'static str, usize>),
    DoesNotExist(&'static str),
    FailedCommandError(Vec<Command>),
    IntParseError(std::num::ParseIntError),
    IoError(std::io::Error),
    ParseDurationError(DurationParseError),
    ReqwestError(reqwest::Error),
    RunStratagemValidationError(RunStratagemValidationError),
    SerdeJsonError(serde_json::error::Error),
    TokioJoinError(tokio::task::JoinError),
    TokioTimerError(tokio::time::Error),
}

impl std::fmt::Display for ImlManagerCliError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            ImlManagerCliError::ApiError(ref err) => write!(f, "{}", err),
            ImlManagerCliError::ClientRequestError(ref err) => write!(f, "{}", err),
            ImlManagerCliError::CombineEasyError(ref err) => write!(f, "{}", err),
            ImlManagerCliError::DoesNotExist(ref err) => write!(f, "{} does not exist", err),
            ImlManagerCliError::FailedCommandError(ref xs) => {
                let failed_msg = xs.iter().fold(
                    String::from("The following commands have failed:\n"),
                    |acc, x| format!("{}{}\n", acc, x.message),
                );

                write!(f, "{}", failed_msg)
            }
            ImlManagerCliError::IntParseError(ref err) => write!(f, "{}", err),
            ImlManagerCliError::IoError(ref err) => write!(f, "{}", err),
            ImlManagerCliError::ParseDurationError(ref err) => write!(f, "{}", err),
            ImlManagerCliError::ReqwestError(ref err) => write!(f, "{}", err),
            ImlManagerCliError::RunStratagemValidationError(ref err) => write!(f, "{}", err),
            ImlManagerCliError::SerdeJsonError(ref err) => write!(f, "{}", err),
            ImlManagerCliError::TokioJoinError(ref err) => write!(f, "{}", err),
            ImlManagerCliError::TokioTimerError(ref err) => write!(f, "{}", err),
        }
    }
}

impl std::fmt::Display for RunStratagemValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ImlManagerCliError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match *self {
            ImlManagerCliError::ApiError(_) => None,
            ImlManagerCliError::ClientRequestError(ref err) => Some(err),
            ImlManagerCliError::CombineEasyError(ref err) => Some(err),
            ImlManagerCliError::DoesNotExist(_) => None,
            ImlManagerCliError::FailedCommandError(_) => None,
            ImlManagerCliError::IntParseError(ref err) => Some(err),
            ImlManagerCliError::IoError(ref err) => Some(err),
            ImlManagerCliError::ParseDurationError(ref err) => Some(err),
            ImlManagerCliError::ReqwestError(ref err) => Some(err),
            ImlManagerCliError::RunStratagemValidationError(ref err) => Some(err),
            ImlManagerCliError::SerdeJsonError(ref err) => Some(err),
            ImlManagerCliError::TokioJoinError(ref err) => Some(err),
            ImlManagerCliError::TokioTimerError(ref err) => Some(err),
        }
    }
}

impl std::error::Error for RunStratagemValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl From<std::num::ParseIntError> for ImlManagerCliError {
    fn from(err: std::num::ParseIntError) -> Self {
        ImlManagerCliError::IntParseError(err)
    }
}

impl From<DurationParseError> for ImlManagerCliError {
    fn from(err: DurationParseError) -> Self {
        ImlManagerCliError::ParseDurationError(err)
    }
}

impl From<tokio::time::Error> for ImlManagerCliError {
    fn from(err: tokio::time::Error) -> Self {
        ImlManagerCliError::TokioTimerError(err)
    }
}

impl From<tokio::task::JoinError> for ImlManagerCliError {
    fn from(err: tokio::task::JoinError) -> Self {
        ImlManagerCliError::TokioJoinError(err)
    }
}

impl From<iml_manager_client::ImlManagerClientError> for ImlManagerCliError {
    fn from(err: iml_manager_client::ImlManagerClientError) -> Self {
        ImlManagerCliError::ClientRequestError(err)
    }
}

impl From<RunStratagemValidationError> for ImlManagerCliError {
    fn from(err: RunStratagemValidationError) -> Self {
        ImlManagerCliError::RunStratagemValidationError(err)
    }
}

impl From<serde_json::error::Error> for ImlManagerCliError {
    fn from(err: serde_json::error::Error) -> Self {
        ImlManagerCliError::SerdeJsonError(err)
    }
}

impl From<std::io::Error> for ImlManagerCliError {
    fn from(err: std::io::Error) -> Self {
        ImlManagerCliError::IoError(err)
    }
}

impl From<combine::stream::easy::Errors<char, &str, usize>> for ImlManagerCliError {
    fn from(err: combine::stream::easy::Errors<char, &str, usize>) -> Self {
        ImlManagerCliError::CombineEasyError(err.map_range(|_| ""))
    }
}

impl From<Vec<Command>> for ImlManagerCliError {
    fn from(xs: Vec<Command>) -> Self {
        ImlManagerCliError::FailedCommandError(xs)
    }
}

impl From<reqwest::Error> for ImlManagerCliError {
    fn from(err: reqwest::Error) -> Self {
        ImlManagerCliError::ReqwestError(err)
    }
}
