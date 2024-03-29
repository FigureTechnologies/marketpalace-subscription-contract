use std::num::TryFromIntError;

use cosmwasm_std::{Response, StdError};
use provwasm_std::ProvenanceMsg;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
}

impl From<&str> for ContractError {
    fn from(msg: &str) -> Self {
        ContractError::Std(StdError::generic_err(msg))
    }
}

impl From<String> for ContractError {
    fn from(msg: String) -> Self {
        ContractError::Std(StdError::generic_err(msg))
    }
}

impl From<TryFromIntError> for ContractError {
    fn from(msg: TryFromIntError) -> Self {
        ContractError::Std(StdError::generic_err(msg.to_string()))
    }
}

pub fn contract_error(err: &str) -> Result<Response<ProvenanceMsg>, ContractError> {
    Err(ContractError::Std(StdError::generic_err(err)))
}
