// Copyright (c) 2020 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use futures::channel::oneshot;
use iml_job_scheduler_rpc::ImlJobSchedulerRpcError;
use iml_rabbit::{self, ImlRabbitError};
use warp::reject;

#[derive(Debug)]
pub enum ImlApiError {
    ImlJobSchedulerRpcError(ImlJobSchedulerRpcError),
    ImlRabbitError(ImlRabbitError),
    OneshotCanceled(oneshot::Canceled),
    SerdeJsonError(serde_json::error::Error),
}

impl reject::Reject for ImlApiError {}

impl std::fmt::Display for ImlApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            ImlApiError::ImlJobSchedulerRpcError(ref err) => write!(f, "{}", err),
            ImlApiError::ImlRabbitError(ref err) => write!(f, "{}", err),
            ImlApiError::OneshotCanceled(ref err) => write!(f, "{}", err),
            ImlApiError::SerdeJsonError(ref err) => write!(f, "{}", err),
        }
    }
}

impl std::error::Error for ImlApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match *self {
            ImlApiError::ImlJobSchedulerRpcError(ref err) => Some(err),
            ImlApiError::ImlRabbitError(ref err) => Some(err),
            ImlApiError::OneshotCanceled(ref err) => Some(err),
            ImlApiError::SerdeJsonError(ref err) => Some(err),
        }
    }
}

impl From<ImlRabbitError> for ImlApiError {
    fn from(err: ImlRabbitError) -> Self {
        ImlApiError::ImlRabbitError(err)
    }
}

impl From<serde_json::error::Error> for ImlApiError {
    fn from(err: serde_json::error::Error) -> Self {
        ImlApiError::SerdeJsonError(err)
    }
}

impl From<oneshot::Canceled> for ImlApiError {
    fn from(err: oneshot::Canceled) -> Self {
        ImlApiError::OneshotCanceled(err)
    }
}

impl From<ImlJobSchedulerRpcError> for ImlApiError {
    fn from(err: ImlJobSchedulerRpcError) -> Self {
        ImlApiError::ImlJobSchedulerRpcError(err)
    }
}

impl From<ImlApiError> for warp::Rejection {
    fn from(err: ImlApiError) -> Self {
        warp::reject::custom(err)
    }
}
