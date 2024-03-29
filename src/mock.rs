use cosmwasm_std::from_binary;
use cosmwasm_std::BankMsg;
use cosmwasm_std::Coin;
use cosmwasm_std::CosmosMsg;
use cosmwasm_std::Response;
use cosmwasm_std::WasmMsg;
use provwasm_mocks::{must_read_binary_file, ProvenanceMockQuerier};
use provwasm_std::{Marker, MarkerMsgParams, ProvenanceMsg, ProvenanceMsgParams};
use serde::de::DeserializeOwned;

pub fn msg_at_index(res: &Response<ProvenanceMsg>, i: usize) -> &CosmosMsg<ProvenanceMsg> {
    &res.messages.get(i).unwrap().msg
}

pub fn bank_msg(msg: &CosmosMsg<ProvenanceMsg>) -> &BankMsg {
    if let CosmosMsg::Bank(msg) = msg {
        msg
    } else {
        panic!("not a cosmos bank message!")
    }
}

pub fn marker_transfer_msg(msg: &CosmosMsg<ProvenanceMsg>) -> &MarkerMsgParams {
    if let CosmosMsg::Custom(msg) = msg {
        if let ProvenanceMsgParams::Marker(params) = &msg.params {
            if let MarkerMsgParams::TransferMarkerCoins {
                coin: _,
                to: _,
                from: _,
            } = params
            {
                params
            } else {
                panic!("not a marker transfer message!")
            }
        } else {
            panic!("not a marker message!")
        }
    } else {
        panic!("not a cosmos custom message!")
    }
}

pub fn send_msg(msg: &CosmosMsg<ProvenanceMsg>) -> (&String, &Vec<Coin>) {
    if let BankMsg::Send { to_address, amount } = bank_msg(msg) {
        (to_address, amount)
    } else {
        panic!("not a send bank message!")
    }
}

pub fn wasm_msg(msg: &CosmosMsg<ProvenanceMsg>) -> &WasmMsg {
    if let CosmosMsg::Wasm(msg) = msg {
        msg
    } else {
        panic!("not a cosmos wasm message")
    }
}

pub fn execute_args<T: DeserializeOwned>(
    msg: &CosmosMsg<ProvenanceMsg>,
) -> (&String, T, &Vec<Coin>) {
    if let WasmMsg::Execute {
        contract_addr,
        msg,
        funds,
    } = wasm_msg(msg)
    {
        (contract_addr, from_binary::<T>(msg).unwrap(), funds)
    } else {
        panic!("not a wasm execute message")
    }
}

pub fn load_markers(querier: &mut ProvenanceMockQuerier) {
    let get_marker = |name: &str| -> Marker {
        let bin = must_read_binary_file(&format!("testdata/{}_marker.json", name));
        from_binary(&bin).unwrap()
    };

    querier.with_markers(vec![
        get_marker("capital"),
        get_marker("restricted_capital"),
    ]);
}
