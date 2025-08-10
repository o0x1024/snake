#!/bin/bash

# Build the WASM plugin
cargo build --target wasm32-unknown-unknown --release

# Copy the WASM file to the plugin directory
cp target/wasm32-unknown-unknown/release/aurora_plugin_template.wasm ../vulnerability_scanner/plugin.wasm
cp target/wasm32-unknown-unknown/release/aurora_plugin_template.wasm ../password_cracker/plugin.wasm

echo "Plugin built and copied successfully!"