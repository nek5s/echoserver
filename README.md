# Echoserver

A simple TCP server utility written in rust.

This project was originally create to provide a simple server utility the Hollow Knight: Silksong mod [SilklessCoop](https://www.nexusmods.com/hollowknightsilksong/mods/73).

Host this server with 30% off using affiliate code SILKLESS:

[![Nodecraft banner](./Media/nodecraft.jpg)](https://nodecraft.com/r/silkless)

## Installation

Download the file fitting your operating system from the [Releases](https://github.com/nek5s/echoserver/releases) section.

## Usage

- Double-click the executable file
- You should now see a message like `INFO:: listening on port 45565 with the following configuration:`
- The server is now ready to start accepting incoming connections.

To start the server with mirroring disabled, see [Advanced Usage](#advanced-usage).

## Advanced Usage

Command-line usage: `echoserver [port] [--no-mirror] [--debug] [--max-players=x] [--max-rate=x]`

### Parameters:

port: the network port to run the server on.

--max-players: the maximum amount of players that can connect to the server at once.

--max-rate: the maximum amount of messages each player can send per second.

### Flags:

--no-mirror: Disable sending packets back to the original sender.

--debug: Enable additional printing for debugging.

## Packet structure

All packets must have the total packet size in bytes prepended as a 32bit integer.

## Building from source

Run: `cargo build --release`

(optionally specify your target architecture using `--target <arch><sub>-<vendor>-<sys>-<abi>`)

## 📜 License

This software is licensed under the Creative Commons Attribution-NonCommercial 4.0 License.

**Commercial use is prohibited** without a separate license.

👉 To inquire about commercial licensing, contact: nek5s.dev@gmail.com
