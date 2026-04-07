#!/bin/bash
cargo build --features nova -p thales-cli
./target/debug/thales-cli help | grep "nova"
