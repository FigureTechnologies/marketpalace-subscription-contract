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
use crate::msg::{CapitalCalls, HandleMsg, QueryMsg, Terms, Transactions};
use crate::state::{config, config_read, Distribution, Redemption, Status, Withdrawal};

pub type ContractResponse = Result<Response<ProvenanceMsg>, ContractError>;

// And declare a custom Error variant for the ones where you will want to make use of it
#[entry_point]
pub fn execute(
    deps: DepsMut<ProvenanceQuery>,
    env: Env,
    info: MessageInfo,
    msg: HandleMsg,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    match msg {
        HandleMsg::Recover { lp } => try_recover(deps, info, lp),
        HandleMsg::Accept {} => try_accept(deps, info),
        HandleMsg::ClaimInvestment { amount } => try_claim_investment(deps, env, info, amount),
        HandleMsg::ClaimRedemption {
            asset,
            capital,
            to,
            memo,
        } => try_claim_redemption(deps, env, info, asset, capital, to, memo),
        HandleMsg::ClaimDistribution { amount, to, memo } => {
            try_claim_distribution(deps, env, info, amount, to, memo)
        }
        HandleMsg::IssueWithdrawal { to, amount } => {
            try_issue_withdrawal(deps, env, info, to, amount)
        }
    }
}

pub fn try_recover(
    deps: DepsMut<ProvenanceQuery>,
    info: MessageInfo,
    lp: Addr,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let state = config_read(deps.storage).load()?;

    if info.sender != state.recovery_admin {
        return contract_error("only admin can recover subscription");
    }

    config(deps.storage).update(|mut state| -> Result<_, ContractError> {
        state.lp = lp;
        Ok(state)
    })?;

    Ok(Response::default())
}

pub fn try_accept(
    deps: DepsMut<ProvenanceQuery>,
    info: MessageInfo,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let state = config_read(deps.storage).load()?;

    if state.status != Status::Draft {
        return contract_error("subscription is not in draft status");
    }

    if info.sender != state.raise {
        return contract_error("only the raise contract can accept");
    }

    let commitment = match info.funds.first() {
        Some(commitment) => commitment,
        None => return contract_error("commitment required"),
    };

    if commitment.amount.u128() < state.capital_to_shares(state.min_commitment).into() {
        return contract_error("commitment less than minimum commitment");
    }

    if commitment.amount.u128() > state.capital_to_shares(state.max_commitment).into() {
        return contract_error("commitment more than maximum commitment");
    }

    config(deps.storage).update(|mut state| -> Result<_, ContractError> {
        state.status = Status::Accepted;
        Ok(state)
    })?;

    Ok(Response::default())
}

pub fn try_claim_investment(
    deps: DepsMut<ProvenanceQuery>,
    env: Env,
    info: MessageInfo,
    amount: u64,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let mut state = config(deps.storage).load()?;

    if state.status != Status::Accepted {
        return contract_error("subscription is not accepted");
    }

    if info.sender != state.lp {
        return contract_error("only the lp can claim investment");
    }

    state.sequence += 1;
    state
        .closed_capital_calls
        .insert(crate::state::CapitalCall {
            sequence: state.sequence,
            amount,
            days_of_notice: None,
        });

    config(deps.storage).save(&state)?;

    Ok(Response::new()
        .add_attribute(
            format!("{}.capital_call.sequence", env.contract.address),
            format!("{}", state.sequence),
        )
        .add_message(
            wasm_execute(
                state.raise.clone(),
                &RaiseExecuteMsg::ClaimInvestment { amount },
                vec![
                    coin(
                        state.capital_to_shares(amount).into(),
                        format!("{}.commitment", state.raise),
                    ),
                    coin(amount as u128, state.capital_denom),
                ],
            )
            .unwrap(),
        ))
}

pub fn try_claim_redemption(
    deps: DepsMut<ProvenanceQuery>,
    env: Env,
    info: MessageInfo,
    asset: u64,
    capital: u64,
    to: Option<Addr>,
    memo: Option<String>,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let mut state = config(deps.storage).load()?;

    if state.status != Status::Accepted {
        return contract_error("subscription is not accepted");
    }

    if info.sender != state.lp {
        return contract_error("only the lp can claim a redemption");
    }

    state.sequence += 1;
    state.redemptions.insert(Redemption {
        sequence: state.sequence,
        asset,
        capital,
    });
    config(deps.storage).save(&state)?;

    Ok(Response::new()
        .add_attribute(
            format!("{}.redemption.sequence", env.contract.address),
            format!("{}", state.sequence),
        )
        .add_message(
            wasm_execute(
                state.raise.clone(),
                &RaiseExecuteMsg::ClaimRedemption {
                    asset,
                    capital,
                    to: to.unwrap_or(env.contract.address),
                    memo,
                },
                coins(asset as u128, format!("{}.investment", state.raise)),
            )
            .unwrap(),
        ))
}

pub fn try_claim_distribution(
    deps: DepsMut<ProvenanceQuery>,
    env: Env,
    info: MessageInfo,
    amount: u64,
    to: Option<Addr>,
    memo: Option<String>,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let mut state = config(deps.storage).load()?;

    if state.status != Status::Accepted {
        return contract_error("subscription has not been accepted");
    }

    if info.sender != state.lp {
        return contract_error("only the lp can claim a distribution");
    }

    state.sequence += 1;
    state.distributions.insert(Distribution {
        sequence: state.sequence,
        amount,
    });
    config(deps.storage).save(&state)?;

    Ok(Response::new()
        .add_attribute(
            format!("{}.distribution.sequence", env.contract.address),
            format!("{}", state.sequence),
        )
        .add_message(
            wasm_execute(
                state.raise,
                &RaiseExecuteMsg::ClaimDistribution {
                    amount,
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
    env: Env,
    info: MessageInfo,
    to: Addr,
    amount: u64,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let state = config_read(deps.storage).load()?;

    if state.status != Status::Accepted {
        return contract_error("subscription has not been accepted");
    }

    if info.sender != state.lp {
        return contract_error("only the lp can withdraw");
    }

    let send = BankMsg::Send {
        to_address: to.to_string(),
        amount: coins(amount as u128, state.capital_denom),
    };

    config(deps.storage).update(|mut state| -> Result<_, ContractError> {
        state.sequence += 1;
        state.withdrawals.insert(Withdrawal {
            sequence: state.sequence,
            to,
            amount,
        });
        Ok(state)
    })?;

    let state = config_read(deps.storage).load()?;

    Ok(Response::new().add_message(send).add_attribute(
        format!("{}.withdrawal.sequence", env.contract.address),
        format!("{}", state.sequence),
    ))
}

#[entry_point]
pub fn query(deps: Deps<ProvenanceQuery>, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    let state = config_read(deps.storage).load()?;

    match msg {
        QueryMsg::GetTerms {} => to_binary(&Terms {
            lp: state.lp,
            raise: state.raise,
            capital_denom: state.capital_denom,
            min_commitment: state.min_commitment,
            max_commitment: state.max_commitment,
        }),
        QueryMsg::GetStatus {} => to_binary(&state.status),
        QueryMsg::GetTransactions {} => to_binary(&Transactions {
            capital_calls: CapitalCalls {
                active: state.active_capital_call,
                closed: state.closed_capital_calls,
                cancelled: state.cancelled_capital_calls,
            },
            redemptions: state.redemptions,
            distributions: state.distributions,
            withdrawals: state.withdrawals,
        }),
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
    fn accept() {
        let mut deps = default_deps(None);

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("raise_1", &coins(200, "investment_raise")),
            HandleMsg::Accept {},
        )
        .unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetStatus {}).unwrap();
        let status: Status = from_binary(&res).unwrap();
        assert_eq!(Status::Accepted, status);
    }

    #[test]
    fn accept_bad_actor() {
        let res = execute(
            default_deps(None).as_mut(),
            mock_env(),
            mock_info("bad_actor", &coins(200, "investment_raise")),
            HandleMsg::Accept {},
        );
        assert!(res.is_err());
    }

    #[test]
    fn claim_investment() {
        let mut deps = default_deps(Some(|state| {
            state.status = Status::Accepted;
        }));

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &vec![]),
            HandleMsg::ClaimInvestment { amount: 10_000 },
        )
        .unwrap();

        // verify exec message sent
        assert_eq!(1, res.messages.len());
        let (contract_addr, msg, funds) = execute_args::<RaiseExecuteMsg>(msg_at_index(&res, 0));
        assert_eq!("raise_1", contract_addr);
        assert_eq!(RaiseExecuteMsg::ClaimInvestment { amount: 10_000 }, msg);
        let commitment_coin = funds.get(0).unwrap();
        assert_eq!(100, commitment_coin.amount.u128());
        assert_eq!("raise_1.commitment", commitment_coin.denom);
        let capital_coin = funds.get(1).unwrap();
        assert_eq!(10_000, capital_coin.amount.u128());
        assert_eq!("stable_coin", capital_coin.denom);

        // verify attributes
        assert_eq!(1, res.attributes.len());
        let attribute = res.attributes.get(0).unwrap();
        assert_eq!("cosmos2contract.capital_call.sequence", attribute.key);
        assert_eq!("1", attribute.value);
    }

    #[test]
    fn claim_redemption() {
        let mut deps = default_deps(Some(|state| {
            state.status = Status::Accepted;
        }));
        load_markers(&mut deps.querier);

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &vec![]),
            HandleMsg::ClaimRedemption {
                asset: 5_000,
                capital: 2_500,
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
                asset: 5000,
                capital: 2_500,
                to: Addr::unchecked("cosmos2contract"),
                memo: None,
            },
            msg
        );
        assert_eq!(5_000, funds.first().unwrap().amount.u128());

        // verify attributes
        assert_eq!(1, res.attributes.len());
        let attribute = res.attributes.get(0).unwrap();
        assert_eq!("cosmos2contract.redemption.sequence", attribute.key);
        assert_eq!("1", attribute.value);
    }

    #[test]
    fn claim_redemption_bad_actor() {
        let mut deps = default_deps(Some(|state| {
            state.status = Status::Accepted;
        }));
        load_markers(&mut deps.querier);

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("bad_actor", &vec![]),
            HandleMsg::ClaimRedemption {
                asset: 5_000,
                capital: 2_500,
                to: None,
                memo: None,
            },
        );

        assert!(res.is_err());
    }

    #[test]
    fn claim_distribution() {
        let mut deps = default_deps(Some(|state| {
            state.status = Status::Accepted;
        }));

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &coins(5_000, "stable_coin")),
            HandleMsg::ClaimDistribution {
                amount: 5_000,
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
                amount: 5_000,
                to: Addr::unchecked("cosmos2contract"),
                memo: None
            },
            msg
        );

        // verify attributes
        assert_eq!(1, res.attributes.len());
        let attribute = res.attributes.get(0).unwrap();
        assert_eq!("cosmos2contract.distribution.sequence", attribute.key);
        assert_eq!("1", attribute.value);
    }

    #[test]
    fn claim_distribution_bad_actor() {
        let mut deps = default_deps(Some(|state| {
            state.status = Status::Accepted;
        }));

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("bad_actor", &coins(5_000, "stable_coin")),
            HandleMsg::ClaimDistribution {
                amount: 5_000,
                to: None,
                memo: None,
            },
        );

        assert!(res.is_err());
    }

    #[test]
    fn withdraw() {
        let mut deps = default_deps(Some(|state| {
            state.status = Status::Accepted;
        }));
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
        let mut deps = default_deps(Some(|state| {
            state.status = Status::Accepted;
        }));
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
