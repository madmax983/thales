sed -i 's/self.config.base_url.trim_end_matches('\''\/'\'')/self.config.base_url.trim_end_matches('\''\/'\'').trim_end_matches("\/v2")/g' crates/providers/alpaca/src/lib.rs
sed -i 's/"{}\/orders/"{}\/v2\/orders/g' crates/providers/alpaca/src/lib.rs
sed -i 's/"{}\/positions/"{}\/v2\/positions/g' crates/providers/alpaca/src/lib.rs
