# About
Rddns is a DynDNS client written in Rust.
Its main use case it to update multiple DynDNS records at once when an update is triggered by another DynDNS client that
can only update one DynDNS entry at a time.
Updates can be triggered by HTTP calls to a HTTP server embedded in rddns or by executing rddns in update mode.

# Features
* Trigger DDNS updates under different conditions.
  * "single-shot" updates on execution
  * HTTP requests to an embedded HTTP server
  * periodically (think of an embedded cron job).
* Different sources for IP addresses
  * IP addresses can be passed as command line or HTTP parameter
  * IP addresses can be read from network interfaces
  * static IP addresses
  * Multiple IP addresses can be combined to new ones.
    E.g. Combine a dynamically assigned IPv6 subnet with the static IPv6 host parts of all devices in the subnet and update DynDNS entries for all of them.
* Update multiple external DDNS updates at once by calling HTTP URLs.

# Usage
Rddns is started by passing a configuration file as parameter.
To update all IP address configurations once call rddns in update mode.

    rddns -c /path/to/config.toml update

To keep rddns running and waiting for conditions that should trigger an DDNS update run.

    rddns -c /path/to/config.toml trigger

Which events should trigger an update must be specified in the configuration file.

The configuration file contains the DynDNS entries that should be updated as well as all other configurable options.
It is described in the exemplary configuration file [example_config.toml](example_config.toml).

# Install
Currently no official pre compiled executables are available.
They have to be created by compiling the source manually.

There is however a Docker image available at [Docker Hub](https://hub.docker.com/r/sirabien/rddns).

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
