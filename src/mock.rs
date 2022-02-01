use cosmwasm_std::BankMsg;
use cosmwasm_std::Coin;
use cosmwasm_std::CosmosMsg;
use cosmwasm_std::Response;
use provwasm_std::ProvenanceMsg;

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

pub fn send_msg(msg: &CosmosMsg<ProvenanceMsg>) -> (&String, &Vec<Coin>) {
    if let BankMsg::Send { to_address, amount } = bank_msg(msg) {
        (to_address, amount)
    } else {
        panic!("not a send bank message!")
    }
}
