# Marketpalace Subscription Contract
Manage subscription investments and commitments (capital calls).

### build
1. make
2. make optimize

### store contract on chain
    provenanced -t tx wasm store ./artifacts/marketpalace_subscription_contract.wasm \
      --source "https://github.com/FigureTechnologies/marketpalace-subscription-contract" \
      --builder "cosmwasm/rust-optimizer:0.11.3" \
      --from $(faucet) \
      --home $N0 \
      --chain-id chain-local \
      --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
      --yes
