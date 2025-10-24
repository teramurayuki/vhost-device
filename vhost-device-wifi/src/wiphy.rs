// SPDX-License-Identifier: Apache-2.0 or BSD-3-Clause
// Copyright 2026 Panasonic Automotive Systems Co., Ltd.
// Author: Manos Pitsidianakis <manos.pitsidianakis@linaro.org>

use std::{
    os::fd::{OwnedFd, *},
    path::{Path, PathBuf},
    str::FromStr,
};

use nix::sys::socket::*;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
/// Errors related to low level helpers
pub enum Error {
    #[error("Could not open /sys/class/ieee80211/{0} wiphy: {2}")]
    Open(String, PathBuf, std::io::Error),
    #[error("Socket error: {0}")]
    Socket(#[from] nix::errno::Errno),
}

#[derive(Debug)]
pub struct Wiphy {
    name: String,
    index: u32,
    socket: OwnedFd,
}

const AF_NETLINK: libc::c_int = 16;
const NETLINK_GENERIC: libc::c_int = 16;

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
#[repr(transparent)]
pub struct NetlinkAddr(libc::sockaddr_nl);

impl NetlinkAddr {
    /// Construct a new socket address from its port ID and multicast groups
    /// mask.
    pub fn new(pid: i32, groups: u32) -> NetlinkAddr {
        let mut addr: libc::sockaddr_nl = unsafe { core::mem::zeroed() };
        addr.nl_family = AF_NETLINK as sa_family_t;
        addr.nl_pid = pid as u32;
        addr.nl_groups = groups;

        Self(addr)
    }

    /// Return the socket's port ID.
    pub const fn pid(&self) -> u32 {
        self.0.nl_pid
    }

    /// Return the socket's multicast groups mask
    pub const fn groups(&self) -> u32 {
        self.0.nl_groups
    }

    const fn as_ptr(&self) -> *const libc::sockaddr {
        self as *const Self as *const libc::sockaddr
    }

    const fn len(&self) -> libc::socklen_t {
        core::mem::size_of_val(self) as libc::socklen_t
    }

    fn bind(self, fd: RawFd) -> nix::Result<()> {
        let res = unsafe { libc::bind(fd, self.as_ptr(), self.len()) };

        nix::errno::Errno::result(res).map(drop)
    }
}

fn open_socket() -> nix::Result<OwnedFd> {
    let mut ty = SockType::Raw as libc::c_int;
    ty |= libc::SOCK_CLOEXEC;

    let res = unsafe { libc::socket(AF_NETLINK, ty, NETLINK_GENERIC) };

    match res {
        -1 => Err(nix::errno::Errno::last()),
        fd => {
            // Safe because libc::socket returned success
            unsafe { Ok(OwnedFd::from_raw_fd(fd)) }
        }
    }
}

#[derive(Clone)]
struct NetlinkExtAck;

impl nix::sys::socket::SetSockOpt for NetlinkExtAck {
    type Val = libc::c_int;

    fn set<F: std::os::unix::io::AsFd>(&self, fd: &F, val: &Self::Val) -> nix::Result<()> {
        use std::os::fd::AsRawFd;

        let level = libc::SOL_NETLINK;
        let flag = libc::NETLINK_EXT_ACK;
        let res = unsafe {
            libc::setsockopt(
                fd.as_fd().as_raw_fd(),
                level,
                flag,
                val as *const _ as *const _,
                core::mem::size_of_val(val) as libc::socklen_t,
            )
        };
        nix::errno::Errno::result(res).map(drop)
    }
}

impl Wiphy {
    pub fn new(name: String) -> Result<Self, Error> {
        let path = Path::new("/sys/class/ieee80211/").join(&name);
        let index = std::fs::read_to_string(path.join("index"))
            .map_err(|err| Error::Open(name.clone(), path, err))?;
        let index = u32::from_str(index.trim()).unwrap();
        // socket(AF_NETLINK, SOCK_RAW|SOCK_CLOEXEC, NETLINK_GENERIC) =
        // 3<NETLINK:[63209]>
        let sock = open_socket()?;
        // setsockopt(3<NETLINK:[63209]>, SOL_SOCKET, SO_SNDBUF, [32768], 4) = 0
        setsockopt(&sock, sockopt::SndBuf, &32768)?;
        // setsockopt(3<NETLINK:[63209]>, SOL_SOCKET, SO_RCVBUF, [32768], 4) = 0
        setsockopt(&sock, sockopt::RcvBuf, &32768)?;
        // bind(3<NETLINK:[63209]>, {sa_family=AF_NETLINK, nl_pid=-1170180326,
        // nl_groups=00000000}, 12) = 0
        let pid = nix::unistd::Pid::this();
        NetlinkAddr::new(pid.as_raw(), 0).bind(sock.as_raw_fd())?;
        //setsockopt(&sock, sockopt::RcvBuf, 32768)?;
        // getsockname(3<NETLINK:[63209]>, {sa_family=AF_NETLINK, nl_pid=-1170180326,
        // nl_groups=00000000}, [12]) = 0 setsockopt(3<NETLINK:[63209]>,
        // SOL_SOCKET, SO_SNDBUF, [8192], 4) = 0
        setsockopt(&sock, sockopt::SndBuf, &8192)?;
        // setsockopt(3<NETLINK:[63209]>, SOL_SOCKET, SO_RCVBUF, [8192], 4) = 0
        setsockopt(&sock, sockopt::RcvBuf, &8192)?;
        // setsockopt(3<NETLINK:[63209]>, SOL_NETLINK, NETLINK_EXT_ACK, [1], 4) = 0
        setsockopt(&sock, NetlinkExtAck, &1)?;
        Ok(Self {
            name,
            index,
            socket: sock,
        })
    }
}
