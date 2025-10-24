// SPDX-License-Identifier: Apache-2.0 or BSD-3-Clause

use std::process::exit;

use clap::Parser;
use vhost_device_rtc::{start_backend, RtcConfiguration};

mod args;
use args::RtcArgs;

impl From<RtcArgs> for RtcConfiguration {
    fn from(args: RtcArgs) -> Self {
        RtcConfiguration {
            socket_path: args.socket_path,
            offer_alarm: !args.no_offer_alarm,
        }
    }
}

fn main() {
    env_logger::init();

    let config = RtcConfiguration::from(RtcArgs::parse());
    if let Err(err) = start_backend(config) {
        log::error!("{err}");
        exit(1);
    }
}
