use std::convert::TryInto;

use crate::error::contract_error;
use crate::raise_msg::RaiseExecuteMsg;
use cosmwasm_std::{coin, wasm_execute};
use cosmwasm_std::{
    coins, entry_point, to_binary, BankMsg, Binary, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult,
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
    _env: Env,
    info: MessageInfo,
    msg: HandleMsg,
) -> ContractResponse {
    match msg {
        HandleMsg::Recover { lp } => {
            let mut state = config_read(deps.storage).load()?;

            if info.sender != state.recovery_admin {
                return contract_error("only admin can recover subscription");
            }

            state.lp = lp;
            config(deps.storage).save(&state)?;

            Ok(Response::default())
        }
        HandleMsg::CompleteAssetExchange { exchange, to, memo } => {
            let state = config(deps.storage).load()?;

            if info.sender != state.lp {
                return contract_error("only the lp complete asset exchange");
            }

            let mut funds = Vec::new();
            if let Some(investment) = exchange.investment {
                if investment < 0 {
                    funds.push(coin(investment.try_into()?, state.investment_denom.clone()));
                }
            }
            if let Some(commitment) = exchange.commitment {
                if commitment < 0 {
                    funds.push(coin(commitment.try_into()?, state.commitment_denom.clone()));
                }
            }
            if let Some(capital) = exchange.capital {
                if capital < 0 {
                    funds.push(coin(capital.try_into()?, state.capital_denom.clone()));
                }
            }

            Ok(Response::new().add_message(wasm_execute(
                state.raise,
                &RaiseExecuteMsg::CompleteAssetExchange { exchange, to, memo },
                funds,
            )?))
        }
        HandleMsg::IssueWithdrawal { to, amount } => {
            let state = config(deps.storage).load()?;

            if info.sender != state.lp {
                return contract_error("only the lp can withdraw");
            }

            Ok(Response::new().add_message(BankMsg::Send {
                to_address: to.to_string(),
                amount: coins(amount.into(), state.capital_denom),
            }))
        }
    }
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
