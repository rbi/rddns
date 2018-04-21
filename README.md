# About
Rddns is a DynDNS client written in Rust.
Its main use case it to update multiple DynDNS records at once when an update is triggered by another DynDNS client that
can only update one DynDNS entry at a time.
Updates can be triggered by HTTP calls to a HTTP server embedded in rddns or by executing rddns in update mode.

# Key Features
* Trigger DynDNS updates with HTTP calls to rddns.
  * It is also possible to trigger updates just once when executing rddns.
* Update multiple dynamic DNS entries with different IP addresses with a single call.
* Combine different IP addresses to new ones used for updating DynDNS entries.
  * E.g. Combine a dynamically assigned IPv6 subnet with the static IPv6 host parts of all devices in the subnet and
    update DynDNS entries for all of them.

# Usage
Rddns is started by passing a configuration file as parameter.
To update all IP address configurations once call rddns in update mode.

    rddns -c /path/to/config.toml update

To start the embedded HTTP server that listens for updates start rddns in server mode.

    rddns -c /path/to/config.toml server

After starting the rddns HTTP server listens for requests.
The port it is listening on will be printed to the console.
On each incoming request an update will be triggered.

The configuration file contains the DynDNS entries that should be updated as well as all other configurable options.
It is described in the exemplary configuration file [example_config.toml](example_config.toml).

# Install
Currently no official pre compiled executables are available.
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