// SPDX-License-Identifier: Apache-2.0 or BSD-3-Clause
// Copyright 2026 Panasonic Automotive Systems Co., Ltd.
// Author: Manos Pitsidianakis <manos.pitsidianakis@linaro.org>

use std::{
    os::unix::{net::UnixListener, prelude::*},
    process::exit,
};

use clap::Parser;
use vhost::vhost_user::Listener;
use vhost_device_wifi::{nl80211, start_backend, WifiConfiguration};

mod args;
use args::WifiArgs;

impl From<WifiArgs> for WifiConfiguration {
    fn from(args: WifiArgs) -> Self {
        WifiConfiguration { phy: args.phy }
    }
}

fn run_app(mut listener: Listener, args: WifiArgs) -> Result<(), Box<dyn std::error::Error>> {
    let config = WifiConfiguration::from(args);
    loop {
        let (socket, event_receiver) = nl80211::Nl80211Socket::new().unwrap();

        let mut wiphys = socket.list_wiphys().unwrap();
        log::info!(
            "Found {} wiphy{}",
            wiphys.len(),
            if wiphys.len() == 1 { "" } else { "s" }
        );
        let Some((_, wiphy)) = wiphys.extract_if(.., |_, w| w.name == config.phy).next() else {
            return Err(format!(
                "Could not find wiphy {}. Available wiphys are: {:?}",
                config.phy,
                wiphys.into_values().map(|w| w.name).collect::<Vec<_>>()
            )
            .into());
        };
        start_backend(&mut listener, wiphy, (socket, event_receiver))?;
    }
}

fn main() {
    env_logger::init();

    let mut args = WifiArgs::parse();

    let listener = if let Some(fd) = args.socket_fd.take() {
        // SAFETY: user has assured us this is safe.
        unsafe { UnixListener::from_raw_fd(fd) }.into()
    } else if let Some(path) = args.socket_path.take() {
        Listener::new(path, true).unwrap()
    } else {
        unreachable!()
    };

    if let Err(err) = run_app(listener, args) {
        log::error!("{err}");
        exit(1);
    }
}
