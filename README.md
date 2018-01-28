# About
Rddns is a DynDNS client written in Rust.
Its main use case it to update multiple DynDNS records at once when an update is triggered by another DynDNS client that
can only update one DynDNS entry at a time.
Updates are triggered by HTTP calls to the HTTP server embedded in rddns.

# Status
Rddns is functional but lacks some basic functionality like proper error handling or configurability of the embedded
HTTP server.

# Usage
Rddns is started by passing a configuration file as parameter.

    rddns /path/to/config.toml

The configuration file contains the DynDNS entries that should be updated as well as all other configurable options.
It is described in the exemplary configuration file [example_config.toml](example_config.toml).

After starting the rddns HTTP server listens for requests.
The port it is listening on will be printed to the console.
On each incoming request an update will be triggered.

# Install
Currently no pre compiled executables are available.
They have to be created by compiling the source manually.

# Build
To build rddns Rust is required.
Rust install instructions can be found [on the offical Rust website](https://www.rust-lang.org/install.html) (it is
really easy).
After installing Rust the executable `cargo` will be available.
The release version of rddns can than be build with the following command.

    cargo build --release

This will create the executable `target/release/rddns`.

# License
rddns is released under the [GPLv3](LICENSE.md) license.