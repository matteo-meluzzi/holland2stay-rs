#!/bin/bash

cross build --target aarch64-unknown-linux-gnu --release
scp target/aarch64-unknown-linux-gnu/release/holland2stay-rs matteo@192.168.0.116:~/.cargo/bin