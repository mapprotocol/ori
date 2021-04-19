// Copyright 2021 MAP Protocol Authors.
// This file is part of MAP Protocol.

// MAP Protocol is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// MAP Protocol is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with MAP Protocol.  If not, see <http://www.gnu.org/licenses/>.


use clap::{App, Arg};

// TODO: Add DOS prevention CLI params
pub fn cli_app<'a, 'b>() -> App<'a, 'b> {
    App::new("boot_node")
        .about("Start a special Map-tool process that only serves as a discv5 boot-node. This \
        process will *not* import blocks or perform most typical mainnet node functions. Instead, it \
        will simply run the discv5 service and assist nodes on the network to discover each other. \
        This is the recommended way to provide a network boot-node since it has a reduced attack \
        surface compared to a full Map node.")
        .settings(&[clap::AppSettings::ColoredHelp])
        .arg(
            Arg::with_name("enr-address")
                .value_name("IP-ADDRESS")
                .help("The external IP address/ DNS address to broadcast to other peers on how to reach this node. \
                If a DNS address is provided, the enr-address is set to the IP address it resolves to and \
                does not auto-update based on PONG responses in discovery.")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("port")
                .value_name("PORT")
                .help("The UDP port to listen on.")
                .default_value("11000")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("listen-address")
                .long("listen-address")
                .value_name("ADDRESS")
                .help("The address the bootnode will listen for UDP connections.")
                .default_value("0.0.0.0")
                .takes_value(true)
        )
        .arg(
            Arg::with_name("boot-nodes")
                .long("boot-nodes")
                .allow_hyphen_values(true)
                .value_name("ENR-LIST/Multiaddr")
                .help("One or more comma-delimited base64-encoded ENR's or multiaddr strings of peers to initially add to the local routing table")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("enr-udp-port")
                .long("enr-port")
                .value_name("PORT")
                .help("The UDP port of the boot node's ENR. This is the port that external peers will dial to reach this boot node. Set this only if the external port differs from the listening port.")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("enable-enr-auto-update")
                .short("x")
                .long("enable-enr-auto-update")
                .help("Discovery can automatically update the node's local ENR with an external IP address and port as seen by other peers on the network. \
                This enables this feature.")
        )
}
