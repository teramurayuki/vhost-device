// SPDX-License-Identifier: Apache-2.0 or BSD-3-Clause
// Copyright 2026 Panasonic Automotive Systems Co., Ltd.
// Author: Manos Pitsidianakis <manos.pitsidianakis@linaro.org>
//! An arguments type for the binary interface of this library.

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct RtcArgs {
    /// Location of vhost-user Unix domain socket.
    #[clap(short, long, value_name = "SOCKET")]
    pub socket_path: PathBuf,
    /// Don't offer alarm functionality for smeared UTC clocks. Turns
    /// `VIRTIO_RTC_F_ALARM` feature off.
    #[clap(long)]
    pub no_offer_alarm: bool,
}
