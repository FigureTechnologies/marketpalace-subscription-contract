use std::convert::TryInto;
use std::num::TryFromIntError;

use crate::error::contract_error;
use crate::raise_msg::RaiseExecuteMsg;
use cosmwasm_std::{coin, wasm_execute};
use cosmwasm_std::{
    coins, entry_point, to_binary, Addr, BankMsg, Binary, Deps, DepsMut, Env, MessageInfo,
    Response, StdResult,
};
use provwasm_std::ProvenanceMsg;
use provwasm_std::ProvenanceQuery;

use crate::error::ContractError;
use crate::msg::{HandleMsg, QueryMsg};
use crate::state::{config, config_read};

pub type ContractResponse = Result<Response<ProvenanceMsg>, ContractError>;

// And declare a custom Error variant for the ones where you will want to make use of it
#[entry_point]
pub fn execute(
    deps: DepsMut<ProvenanceQuery>,
    env: Env,
    info: MessageInfo,
    msg: HandleMsg,
) -> ContractResponse {
    match msg {
        HandleMsg::Recover { lp } => try_recover(deps, info, lp),
        HandleMsg::CloseRemainingCommitment {} => try_close_remaining_commitment(deps, env, info),
        HandleMsg::AcceptCommitmentUpdate { forfeit_commitment } => {
            try_accept_commitment_update(deps, info, forfeit_commitment)
        }
        HandleMsg::ClaimInvestment {} => try_claim_investment(deps, env, info),
        HandleMsg::ClaimRedemption { asset, to, memo } => {
            try_claim_redemption(deps, env, info, asset, to, memo)
        }
        HandleMsg::ClaimDistribution { to, memo } => {
            try_claim_distribution(deps, env, info, to, memo)
        }
        HandleMsg::IssueWithdrawal { to, amount } => try_issue_withdrawal(deps, info, to, amount),
    }
}

pub fn try_recover(
    deps: DepsMut<ProvenanceQuery>,
    info: MessageInfo,
    lp: Addr,
) -> ContractResponse {
    let mut state = config_read(deps.storage).load()?;

    if info.sender != state.recovery_admin {
        return contract_error("only admin can recover subscription");
    }

    state.lp = lp;
    config(deps.storage).save(&state)?;

    Ok(Response::default())
}

pub fn try_close_remaining_commitment(
    deps: DepsMut<ProvenanceQuery>,
    env: Env,
    info: MessageInfo,
) -> ContractResponse {
    let state = config_read(deps.storage).load()?;

    if info.sender != state.lp {
        return contract_error("only the lp can close remaining commitment");
    }

    let remaining_commitment = deps
        .querier
        .query_balance(env.contract.address, state.commitment_denom.clone())?;

    Ok(Response::new().add_message(wasm_execute(
        state.raise.clone(),
        &RaiseExecuteMsg::CloseRemainingCommitment {},
        coins(remaining_commitment.amount.into(), state.commitment_denom),
    )?))
}

pub fn try_accept_commitment_update(
    deps: DepsMut<ProvenanceQuery>,
    info: MessageInfo,
    forfeit_commitment: Option<u64>,
) -> ContractResponse {
    let state = config(deps.storage).load()?;

    if info.sender != state.lp {
        return contract_error("only the lp can accept commitment update");
    }

    Ok(Response::new().add_message(wasm_execute(
        state.raise.clone(),
        &RaiseExecuteMsg::AcceptCommitmentUpdate {},
        match forfeit_commitment {
            Some(amount) => coins(amount.into(), state.commitment_denom),
            None => vec![],
        },
    )?))
}

pub fn try_claim_investment(
    deps: DepsMut<ProvenanceQuery>,
    env: Env,
    info: MessageInfo,
) -> ContractResponse {
    let state = config(deps.storage).load()?;

    if info.sender != state.lp {
        return contract_error("only the lp can claim investment");
    }

    let fund = info.funds.first().ok_or("no funds found")?;
    let amount: u64 = fund
        .amount
        .u128()
        .try_into()
        .map_err(|err: TryFromIntError| format!("{}", err))?;

    let mut funds = vec![
        coin(amount.into(), state.capital_denom.clone()),
        coin(
            state.capital_to_shares(amount).into(),
            state.commitment_denom.clone(),
        ),
    ];
    funds.sort_by_key(|coin| coin.denom.clone());

    let remaining_commitment = deps
        .querier
        .query_balance(env.contract.address, state.commitment_denom)?;

    let response = if remaining_commitment.amount.u128() == 0 {
        Response::new().add_message(wasm_execute(
            state.raise.clone(),
            &RaiseExecuteMsg::AcceptCommitmentUpdate {},
            vec![],
        )?)
    } else {
        Response::new()
    };

    Ok(response.add_message(wasm_execute(
        state.raise,
        &RaiseExecuteMsg::ClaimInvestment {},
        funds,
    )?))
}

pub fn try_claim_redemption(
    deps: DepsMut<ProvenanceQuery>,
    env: Env,
    info: MessageInfo,
    asset: u64,
    to: Option<Addr>,
    memo: Option<String>,
) -> ContractResponse {
    let state = config(deps.storage).load()?;

    if info.sender != state.lp {
        return contract_error("only the lp can claim a redemption");
    }

    Ok(Response::new().add_message(wasm_execute(
        state.raise.clone(),
        &RaiseExecuteMsg::ClaimRedemption {
            to: to.unwrap_or(env.contract.address),
            memo,
        },
        coins(asset as u128, format!("{}.investment", state.raise)),
    )?))
}

pub fn try_claim_distribution(
    deps: DepsMut<ProvenanceQuery>,
    env: Env,
    info: MessageInfo,
    to: Option<Addr>,
    memo: Option<String>,
) -> ContractResponse {
    let state = config(deps.storage).load()?;

    if info.sender != state.lp {
        return contract_error("only the lp can claim a distribution");
    }

    Ok(Response::new().add_message(
        wasm_execute(
            state.raise,
            &RaiseExecuteMsg::ClaimDistribution {
                to: to.unwrap_or(env.contract.address),
                memo,
            },
            vec![],
        )
        .unwrap(),
    ))
}

pub fn try_issue_withdrawal(
    deps: DepsMut<ProvenanceQuery>,
    info: MessageInfo,
    to: Addr,
    amount: u64,
) -> ContractResponse {
    let state = config(deps.storage).load()?;

    if info.sender != state.lp {
        return contract_error("only the lp can withdraw");
    }

    Ok(Response::new().add_message(BankMsg::Send {
        to_address: to.to_string(),
        amount: coins(amount.into(), state.capital_denom),
    }))
}

#[entry_point]
pub fn query(deps: Deps<ProvenanceQuery>, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetState {} => to_binary(&config_read(deps.storage).load()?),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::execute_args;
    use crate::mock::msg_at_index;
    use crate::mock::send_msg;
    use crate::state::State;
    use cosmwasm_std::testing::MockApi;
    use cosmwasm_std::testing::MockStorage;
    use cosmwasm_std::testing::MOCK_CONTRACT_ADDR;
    use cosmwasm_std::testing::{mock_env, mock_info};
    use cosmwasm_std::OwnedDeps;
    use cosmwasm_std::{coins, from_binary, Addr};
    use provwasm_mocks::{mock_dependencies, must_read_binary_file, ProvenanceMockQuerier};
    use provwasm_std::Marker;

    fn load_markers(querier: &mut ProvenanceMockQuerier) {
        let bin = must_read_binary_file("testdata/marker.json");
        let expected_marker: Marker = from_binary(&bin).unwrap();
        querier.with_markers(vec![expected_marker.clone()]);
    }

    pub fn default_deps(
        update_state: Option<fn(&mut State)>,
    ) -> OwnedDeps<MockStorage, MockApi, ProvenanceMockQuerier, ProvenanceQuery> {
        let mut deps = mock_dependencies(&[]);

        let mut state = State::test_default();
        if let Some(update) = update_state {
            update(&mut state);
        }
        config(&mut deps.storage).save(&state).unwrap();

        deps
    }

    #[test]
    fn recover() {
        execute(
            default_deps(None).as_mut(),
            mock_env(),
            mock_info("admin", &vec![]),
            HandleMsg::Recover {
                lp: Addr::unchecked("lp_2"),
            },
        )
        .unwrap();
    }

    #[test]
    fn bad_actor_recover_fail() {
        let res = execute(
            default_deps(None).as_mut(),
            mock_env(),
            mock_info("bad_actor", &vec![]),
            HandleMsg::Recover {
                lp: Addr::unchecked("bad_actor"),
            },
        );
        assert_eq!(true, res.is_err());
    }

    #[test]
    fn close_remaining_commitment() {
        let mut deps = default_deps(None);
        deps.querier
            .base
            .update_balance(MOCK_CONTRACT_ADDR, coins(100, "raise_1.commitment"));

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &vec![]),
            HandleMsg::CloseRemainingCommitment {},
        )
        .unwrap();

        // verify exec message sent
        assert_eq!(1, res.messages.len());
        let (contract_addr, msg, funds) = execute_args::<RaiseExecuteMsg>(msg_at_index(&res, 0));
        assert_eq!("raise_1", contract_addr);
        assert_eq!(RaiseExecuteMsg::CloseRemainingCommitment {}, msg);
        let commitment_coin = funds.get(0).unwrap();
        assert_eq!(100, commitment_coin.amount.u128());
        assert_eq!("raise_1.commitment", commitment_coin.denom);
    }

    #[test]
    fn close_remaining_commitment_bad_actor() {
        let mut deps = default_deps(None);
        deps.querier
            .base
            .update_balance(MOCK_CONTRACT_ADDR, coins(100, "raise_1.commitment"));

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("bad_actor", &vec![]),
            HandleMsg::CloseRemainingCommitment {},
        );

        assert!(res.is_err());
    }

    #[test]
    fn accept_commitment_update_increase() {
        let mut deps = default_deps(None);

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &vec![]),
            HandleMsg::AcceptCommitmentUpdate {
                forfeit_commitment: None,
            },
        )
        .unwrap();

        // verify exec message sent
        assert_eq!(1, res.messages.len());
        let (contract_addr, msg, funds) = execute_args::<RaiseExecuteMsg>(msg_at_index(&res, 0));
        assert_eq!("raise_1", contract_addr);
        assert_eq!(RaiseExecuteMsg::AcceptCommitmentUpdate {}, msg);
        assert!(funds.is_empty());
    }

    #[test]
    fn accept_commitment_update_decrease() {
        let mut deps = default_deps(None);

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &vec![]),
            HandleMsg::AcceptCommitmentUpdate {
                forfeit_commitment: Some(1_000),
            },
        )
        .unwrap();

        // verify exec message sent
        assert_eq!(1, res.messages.len());
        let (contract_addr, msg, funds) = execute_args::<RaiseExecuteMsg>(msg_at_index(&res, 0));
        assert_eq!("raise_1", contract_addr);
        assert_eq!(RaiseExecuteMsg::AcceptCommitmentUpdate {}, msg);
        let commitment_coin = funds.get(0).unwrap();
        assert_eq!(1_000, commitment_coin.amount.u128());
        assert_eq!("raise_1.commitment", commitment_coin.denom);
    }

    #[test]
    fn accept_commitment_bad_actor() {
        let mut deps = default_deps(None);

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("bad_actor", &vec![]),
            HandleMsg::AcceptCommitmentUpdate {
                forfeit_commitment: None,
            },
        );

        assert!(res.is_err());
    }

    #[test]
    fn claim_investment() {
        let mut deps = default_deps(None);
        deps.querier
            .base
            .update_balance(MOCK_CONTRACT_ADDR, coins(100, "raise_1.commitment"));

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &coins(10_000, "stable_coin")),
            HandleMsg::ClaimInvestment {},
        )
        .unwrap();

        // verify exec message sent
        assert_eq!(1, res.messages.len());
        let (contract_addr, msg, funds) = execute_args::<RaiseExecuteMsg>(msg_at_index(&res, 0));
        assert_eq!("raise_1", contract_addr);
        assert_eq!(RaiseExecuteMsg::ClaimInvestment {}, msg);
        let commitment_coin = funds.get(0).unwrap();
        assert_eq!(100, commitment_coin.amount.u128());
        assert_eq!("raise_1.commitment", commitment_coin.denom);
        let capital_coin = funds.get(1).unwrap();
        assert_eq!(10_000, capital_coin.amount.u128());
        assert_eq!("stable_coin", capital_coin.denom);
    }

    #[test]
    fn claim_investment_outstanding_commitment_update() {
        let mut deps = default_deps(None);
        deps.querier
            .base
            .update_balance(MOCK_CONTRACT_ADDR, coins(0, "raise_1.commitment"));

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &coins(10_000, "stable_coin")),
            HandleMsg::ClaimInvestment {},
        )
        .unwrap();

        // verify messages sent
        assert_eq!(2, res.messages.len());

        // verify accept commitment update message
        let (contract_addr, msg, _funds) = execute_args::<RaiseExecuteMsg>(msg_at_index(&res, 0));
        assert_eq!("raise_1", contract_addr);
        assert_eq!(RaiseExecuteMsg::AcceptCommitmentUpdate {}, msg);

        // verify claim investment message
        let (contract_addr, msg, funds) = execute_args::<RaiseExecuteMsg>(msg_at_index(&res, 1));
        assert_eq!("raise_1", contract_addr);
        assert_eq!(RaiseExecuteMsg::ClaimInvestment {}, msg);
        let commitment_coin = funds.get(0).unwrap();
        assert_eq!(100, commitment_coin.amount.u128());
        assert_eq!("raise_1.commitment", commitment_coin.denom);
        let capital_coin = funds.get(1).unwrap();
        assert_eq!(10_000, capital_coin.amount.u128());
        assert_eq!("stable_coin", capital_coin.denom);
    }

    #[test]
    fn claim_investment_bad_actor() {
        let mut deps = default_deps(None);

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("bad_actor", &vec![]),
            HandleMsg::ClaimInvestment {},
        );

        assert!(res.is_err());
    }

    #[test]
    fn claim_redemption() {
        let mut deps = default_deps(None);
        load_markers(&mut deps.querier);

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &vec![]),
            HandleMsg::ClaimRedemption {
                asset: 5_000,
                to: None,
                memo: None,
            },
        )
        .unwrap();

        // verify exec message sent
        assert_eq!(1, res.messages.len());
        let (contract_addr, msg, funds) = execute_args::<RaiseExecuteMsg>(msg_at_index(&res, 0));
        assert_eq!("raise_1", contract_addr);
        assert_eq!(
            RaiseExecuteMsg::ClaimRedemption {
                to: Addr::unchecked(MOCK_CONTRACT_ADDR),
                memo: None,
            },
            msg
        );
        assert_eq!(5_000, funds.first().unwrap().amount.u128());
    }

    #[test]
    fn claim_redemption_bad_actor() {
        let mut deps = default_deps(None);
        load_markers(&mut deps.querier);

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("bad_actor", &vec![]),
            HandleMsg::ClaimRedemption {
                asset: 5_000,
                to: None,
                memo: None,
            },
        );

        assert!(res.is_err());
    }

    #[test]
    fn claim_distribution() {
        let mut deps = default_deps(None);

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &coins(5_000, "stable_coin")),
            HandleMsg::ClaimDistribution {
                to: None,
                memo: None,
            },
        )
        .unwrap();

        // verify exec message sent
        assert_eq!(1, res.messages.len());
        let (contract_addr, msg, _funds) = execute_args::<RaiseExecuteMsg>(msg_at_index(&res, 0));
        assert_eq!("raise_1", contract_addr);
        assert_eq!(
            RaiseExecuteMsg::ClaimDistribution {
                to: Addr::unchecked(MOCK_CONTRACT_ADDR),
                memo: None
            },
            msg
        );
    }

    #[test]
    fn claim_distribution_bad_actor() {
        let mut deps = default_deps(None);

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("bad_actor", &coins(5_000, "stable_coin")),
            HandleMsg::ClaimDistribution {
                to: None,
                memo: None,
            },
        );

        assert!(res.is_err());
    }

    #[test]
    fn withdraw() {
        let mut deps = default_deps(None);

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &vec![]),
            HandleMsg::IssueWithdrawal {
                to: Addr::unchecked("lp_side_account"),
                amount: 10_000,
            },
        )
        .unwrap();

        // verify send message sent
        assert_eq!(1, res.messages.len());
        let (to_address, coins) = send_msg(msg_at_index(&res, 0));
        assert_eq!("lp_side_account", to_address);
        assert_eq!(10_000, coins.first().unwrap().amount.u128());
    }

    #[test]
    fn withdraw_bad_actor() {
        let mut deps = default_deps(None);

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("bad_actor", &vec![]),
            HandleMsg::IssueWithdrawal {
                to: Addr::unchecked("lp_side_account"),
                amount: 10_000,
            },
        );
        assert!(res.is_err());
    }
}
