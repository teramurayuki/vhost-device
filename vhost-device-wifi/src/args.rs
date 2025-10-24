// SPDX-License-Identifier: Apache-2.0 or BSD-3-Clause
// Copyright 2026 Panasonic Automotive Systems Co., Ltd.
// Author: Manos Pitsidianakis <manos.pitsidianakis@linaro.org>
//! An arguments type for the binary interface of this library.

use std::{os::fd::RawFd, path::PathBuf};

use clap::{ArgGroup, Parser};

#[derive(Parser, Clone, Debug)]
#[clap(
    author,
    version,
    about,
    long_about = None,
    group(ArgGroup::new("socket group").required(true).args(&["socket_path", "socket_fd"])),
)]
pub struct WifiArgs {
    /// Location of vhost-user Unix domain socket.
    #[clap(short, long, value_name = "SOCKET")]
    pub socket_path: Option<PathBuf>,
    /// vhost-user Unix domain socket FD.
    #[clap(long, value_name = "FD")]
    pub socket_fd: Option<RawFd>,
    #[clap(value_name = "WIPHY")]
    pub phy: String,
}
