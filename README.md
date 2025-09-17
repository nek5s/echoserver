# Echoserver

A simple TCP server utility written in rust.

This project was originally create to provide a simple server utility the Hollow Knight: Silksong mod [SilklessCoop](https://www.nexusmods.com/hollowknightsilksong/mods/73).

## Installation

Download the file fitting your operating system from the [Releases](https://github.com/nek5s/echoserver/releases) section.

## Usage

- Double-click the executable file
- You should now see a message like `INFO:: listening on port 45565 with the following configuration:`
- The server is now ready to start accepting incoming connections.

Note: this is intended for local testing of the mod, so it will send back your own data (you will see your own ghost)

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

All packets must follow this schema:

- Size of packet (in bytes) stored in the first 4 bytes
- Key of packet (= packet type) stored in the 5th byte
  - Key=1 for join, followed by 64 bytes of id string and 64 bytes of version string
  - Key=2 for leave
- Content of packet stored in the remaining bytes (size - 5)

The server will keep track of connections using generated ids, that can be replaced by sending a Key=1 packet.

The server will share all received packets to all other connections (including the original sender if mirror is enabled).

The server will also broadcast a Key=2 packet once a connection closes.

## Building from source

Run: `cargo build --release`

(optionally specify your target architecture using `--target <arch><sub>-<vendor>-<sys>-<abi>`)

## 📜 License

This software is licensed under the Creative Commons Attribution-NonCommercial 4.0 License.

**Commercial use is prohibited** without a separate license.

👉 To inquire about commercial licensing, contact: nek5s.dev@gmail.com
