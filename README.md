# Echoserver

A simple TCP server utility written in rust.

All packets are sent in format `{id}::{metadata}::{content}`,
where metadata is `{playerCount}` and content is copied from the packet sent by the client.

This project was originally create to provide a simple server utility the Hollow Knight: Silksong mod SilklessCoop.

## Usage

`echoserver PORT [--mirror]`

Flags:
--mirror: Enable sending packets back to the original sender.

## Building from source

Run: `cargo build --release`

(optionally specify your target architecture using `--target <arch><sub>-<vendor>-<sys>-<abi>`)
