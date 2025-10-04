# Echoserver

A simple TCP server utility written in rust.

This project was originally create to provide a simple server utility the Hollow Knight: Silksong mod [SilklessCoop](https://www.nexusmods.com/hollowknightsilksong/mods/73).

Host this server with 30% off using affiliate code SILKLESS:

[![Nodecraft banner](./nodecraft.jpg)](https://nodecraft.com/r/silkless)

## Installation

Download the file fitting your operating system from the [Releases](https://github.com/nek5s/echoserver/releases) section.

## Usage

- Double-click the executable file
- You should now see a message like `INFO:: listening on port 45565 with the following configuration:`
- The server is now ready to start accepting incoming connections.

To start the server with mirroring disabled, see [Advanced Usage](#advanced-usage).

## Configuration

The following parameters can be set in a `config.yaml` file or via command line arguments

### Parameters:

|Parameter Name         |Config File Name   |Argument Name      |Description                                                        |Default Value  |
|-                      |-                  |-                  |-                                                                  |-              |
|Port                   |port               |--port=x           |Port the server will run on                                        |45565          |
|Mirror Mode            |mirror             |--no-mirror        |Toggle sending back player data to original sender (= ghost)       |true           |
|Max Player Count       |max_players        |--max_players=x    |Set the maximum amount of players that can connect at once         |10             |
|Max Data Rate          |max_rate           |--max_rate=x       |Set the maximum amount of bytes each player can send per second    |8000           |
|Enable Debug Printing  |debug_print        |--debug            |Enable debug printing, only really useful for mod testing          |false          |

## Packet structure

All packets must have the total packet size in bytes prepended as a 32bit integer.

## Building from source

Run: `cargo build --release`

(optionally specify your target architecture using `--target <arch><sub>-<vendor>-<sys>-<abi>`)

## 📜 License

This software is licensed under the Creative Commons Attribution-NonCommercial 4.0 License.

**Commercial use is prohibited** without a separate license.

👉 To inquire about commercial licensing, contact: nek5s.dev@gmail.com
