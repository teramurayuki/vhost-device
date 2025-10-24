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

fn main() {
    env_logger::init();

    let mut args = WifiArgs::parse();

    let mut listener = if let Some(fd) = args.socket_fd.take() {
        // SAFETY: user has assured us this is safe.
        unsafe { UnixListener::from_raw_fd(fd) }.into()
    } else if let Some(path) = args.socket_path.take() {
        Listener::new(path, true).unwrap()
    } else {
        unreachable!()
    };

    let config = WifiConfiguration::from(args);
    loop {
        let (socket, event_receiver) = nl80211::Nl80211Socket::new().unwrap();

        let wiphys = socket.list_wiphys().unwrap();
        log::info!(
            "Found {} wiphy{}",
            wiphys.len(),
            if wiphys.len() == 1 { "" } else { "s" }
        );
        for (idx, wiphy) in wiphys {
            if wiphy.name != config.phy {
                log::debug!("Ignoring wiphy#{} named {:?}", idx, wiphy.name);
                continue;
            }
            let ifaces = socket.list_ifaces(&wiphy).unwrap();
            log::info!("Cleaning up {} interfaces for wiphy#{}", ifaces.len(), idx);
            for (_, iface) in ifaces {
                socket.del_iface(&wiphy, iface.index).unwrap();
            }
        }
        if let Err(err) = start_backend(&mut listener, config.clone(), (socket, event_receiver)) {
            log::error!("{err}");
            exit(1);
        }
    }
}
