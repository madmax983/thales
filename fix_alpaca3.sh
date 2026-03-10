sed -i 's/let url = format!("{}/v2/account"/let url = format!("{}/account"/g' crates/providers/alpaca/src/lib.rs
sed -i 's/let url = format!("{}/orders"/let url = format!("{}/v2/orders"/g' crates/providers/alpaca/src/lib.rs
sed -i 's/format!("{}/orders?status=open"/format!("{}/v2/orders?status=open"/g' crates/providers/alpaca/src/lib.rs
sed -i 's/format!("{}/orders/{}"/format!("{}/v2/orders/{}"/g' crates/providers/alpaca/src/lib.rs
sed -i 's/let url = format!("{}/positions"/let url = format!("{}/v2/positions"/g' crates/providers/alpaca/src/lib.rs
sed -i 's/let url = format!("{}/account"/let url = format!("{}/v2/account"/g' crates/providers/alpaca/src/lib.rs
