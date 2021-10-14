# Marketpalace Subscription Contract
Manage subscription investments and commitments (capital calls).

### build
1. make
2. make optimize

### store contract on chain
    provenanced -t tx wasm store ./artifacts/marketpalace_subscription_contract.wasm \
      --home $NODE \
      --from validator \
      --chain-id $CHAIN \
      --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
      --yes
