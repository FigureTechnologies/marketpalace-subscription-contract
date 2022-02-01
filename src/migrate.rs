use crate::error::ContractError;
use crate::msg::MigrateMsg;
use crate::version::CONTRACT_NAME;
use crate::version::CONTRACT_VERSION;
use cosmwasm_std::entry_point;
use cosmwasm_std::DepsMut;
use cosmwasm_std::Env;
use cosmwasm_std::Response;
use cw2::set_contract_version;
use provwasm_std::ProvenanceMsg;

#[entry_point]
pub fn migrate(
    deps: DepsMut,
    _: Env,
    _: MigrateMsg,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(Response::default())
}
