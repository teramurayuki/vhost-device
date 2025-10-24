// SPDX-License-Identifier: Apache-2.0 or BSD-3-Clause
// Copyright 2026 Panasonic Automotive Systems Co., Ltd.
// Author: Manos Pitsidianakis <manos.pitsidianakis@linaro.org>

use std::{
    collections::{BTreeMap, VecDeque},
    os::fd::{OwnedFd, *},
    sync::{Arc, Mutex},
};

use nix::sys::socket::MsgFlags;
use nix::{
    errno::Errno,
    ioctl_read_bad, ioctl_write_ptr_bad,
    sys::{eventfd::EventFd, socket::*},
};
use thiserror::Error as ThisError;
use vhost_user_backend::{VringRwLock, VringT};
use virtio_queue::DescriptorChain;
use vm_memory::{GuestMemoryLoadGuard, GuestMemoryMmap};

ioctl_write_ptr_bad!(siocsifflags, libc::SIOCSIFFLAGS, libc::ifreq);
ioctl_read_bad!(siocgifname, libc::SIOCGIFNAME, libc::ifreq);
ioctl_read_bad!(siocgifhwaddr, libc::SIOCGIFHWADDR, libc::ifreq);
ioctl_read_bad!(siocgiwmode, libc::SIOCGIWMODE, libc::iwreq);
ioctl_write_ptr_bad!(siocsiwmode, libc::SIOCSIWMODE, libc::iwreq);

#[derive(Debug, ThisError)]
/// Errors related to initializing `AF_PACKET` socket
pub enum InitError {
    #[error("socket(AF_PACKET, SOCK_RAW, htons(ETH_P_ALL)) returned error: {0}")]
    Socket(#[from] nix::errno::Errno),
    #[error("ioctl(<socket>, {0}, {{..}}) returned error: {1}")]
    Ioctl(&'static str, nix::errno::Errno),
    #[error("bind(<socket>, {{sa_family=AF_PACKET, sll_protocol=htons(ETH_P_ALL), sll_ifindex=[..], sll_hatype=0, sll_pkttype=PACKET_HOST, sll_halen=0}}, 20) returned error: {0}")]
    Bind(nix::errno::Errno),
    #[error("setsockopt(<socket>, SOL_PACKET, PACKET_ADD_MEMBERSHIP, {{mr_ifindex=[..], mr_type=PACKET_MR_PROMISC, mr_alen=0, mr_address=[..]}}, 16) returned error: {0}")]
    SetSockOptPacketAddMembership(nix::errno::Errno),
    #[error("setsockopt(<socket>, SOL_PACKET, PACKET_AUXDATA, 1, 4) returned error: {0}")]
    SetSockOptPacketAuxData(nix::errno::Errno),
}

#[derive(Debug, ThisError)]
/// Errors related to IO socket
pub enum Error {
    #[error("Could not initialize AF_PACKET I/O socket for interface {0}: {1}")]
    Init(String, InitError),
    #[error("Interface {0} socket write error: {0}")]
    Write(String, nix::errno::Errno),
}

type MacAddress = [libc::c_char; 14];

#[derive(Debug)]
pub struct IfSocket {
    name: String,
    #[expect(dead_code)]
    ifname: [u8; 16],
    #[expect(dead_code)]
    index: u32,
    #[expect(dead_code)]
    mac: MacAddress,
    pub socket: OwnedFd,
}

#[derive(Copy, Clone)]
struct PacketAddMembership;

impl nix::sys::socket::SetSockOpt for PacketAddMembership {
    type Val = (libc::c_int, MacAddress);

    fn set<F: std::os::unix::io::AsFd>(&self, fd: &F, val: &Self::Val) -> nix::Result<()> {
        use std::os::fd::AsRawFd;

        let level = libc::SOL_PACKET;
        let flag = libc::PACKET_ADD_MEMBERSHIP;
        // SAFETY: packet_mreq is plain-old-data
        let mut mreq: libc::packet_mreq = unsafe { core::mem::zeroed() };
        mreq.mr_ifindex = val.0;
        mreq.mr_type = libc::PACKET_MR_PROMISC as u16;
        mreq.mr_alen = 0;
        mreq.mr_address.copy_from_slice(&val.1[..8]);
        // SAFETY: Passing valid data
        let res = unsafe {
            libc::setsockopt(
                fd.as_fd().as_raw_fd(),
                level,
                flag,
                (&raw const mreq).cast(),
                core::mem::size_of::<libc::packet_mreq>() as libc::socklen_t,
            )
        };
        nix::errno::Errno::result(res).map(drop)
    }
}

#[derive(Copy, Clone)]
struct PacketAuxdata;

impl nix::sys::socket::SetSockOpt for PacketAuxdata {
    type Val = bool;

    fn set<F: std::os::unix::io::AsFd>(&self, fd: &F, val: &Self::Val) -> nix::Result<()> {
        use std::os::fd::AsRawFd;

        let level = libc::SOL_PACKET;
        let flag = libc::PACKET_AUXDATA;
        let val: libc::c_int = if *val { 1 } else { 0 };
        // SAFETY: Passing valid data
        let res = unsafe {
            libc::setsockopt(
                fd.as_fd().as_raw_fd(),
                level,
                flag,
                (&raw const val).cast(),
                core::mem::size_of::<libc::c_int>() as libc::socklen_t,
            )
        };
        nix::errno::Errno::result(res).map(drop)
    }
}

impl IfSocket {
    pub fn new(name: String, index: u32) -> Result<Self, Error> {
        let sock: OwnedFd = {
            let mut ty = SockType::Raw as libc::c_int;
            ty |= libc::SOCK_CLOEXEC;

            // SAFETY: this is safe to call
            let res = unsafe { libc::socket(libc::AF_PACKET, ty, libc::ETH_P_ALL.to_be()) };

            match res {
                -1 => {
                    return Err(Error::Init(
                        name,
                        InitError::Socket(nix::errno::Errno::last()),
                    ))
                }
                fd => {
                    // SAFETY: safe because libc::socket returned success
                    unsafe { OwnedFd::from_raw_fd(fd) }
                }
            }
        };
        let mut ifreq: libc::ifreq =

        // SAFETY: libc::ifreq is plain-old-data
            unsafe { core::mem::zeroed() };
        ifreq.ifr_ifru.ifru_ifindex = index.cast_signed();
        let ifname = {
            // SAFETY: this ioctl is safe to call
            unsafe { siocgifname(sock.as_raw_fd(), &raw mut ifreq) }
                .map_err(|err| Error::Init(name.clone(), InitError::Ioctl("SIOCGIFNAME", err)))?;
            ifreq.ifr_name
        };

        {
            ifreq.ifr_ifru.ifru_flags = (libc::IFF_UP
                | libc::IFF_BROADCAST
                | libc::IFF_NOTRAILERS
                | libc::IFF_RUNNING
                | libc::IFF_PROMISC
                | libc::IFF_ALLMULTI) as libc::c_short;
            // SAFETY: this ioctl is safe to call
            unsafe { siocsifflags(sock.as_raw_fd(), &raw mut ifreq) }
                .map_err(|err| Error::Init(name.clone(), InitError::Ioctl("SIOCSIFFLAGS", err)))?;
        }
        let mac = {
            // SAFETY: this ioctl is safe to call
            unsafe { siocgifhwaddr(sock.as_raw_fd(), &raw mut ifreq) }
                .map_err(|err| Error::Init(name.clone(), InitError::Ioctl("SIOCGIFHWADDR", err)))?;
            // SAFETY: SIOCGIFHWADDR ioctl sets this union variant
            let hwaddr: libc::sockaddr = unsafe { ifreq.ifr_ifru.ifru_hwaddr };
            assert_eq!(hwaddr.sa_family, libc::ARPHRD_ETHER);
            hwaddr.sa_data
        };
        {
            // SAFETY: libc::sockaddr_ll is plain-old-data
            let mut addr: libc::sockaddr_ll = unsafe { core::mem::zeroed() };
            addr.sll_family = libc::AF_PACKET as u16;
            addr.sll_protocol = (libc::ETH_P_ALL as u16).to_be();
            addr.sll_ifindex = index.cast_signed();
            addr.sll_hatype = 0;
            const IEEE80211_ADDR_LEN: u8 = 6;
            addr.sll_halen = IEEE80211_ADDR_LEN;
            addr.sll_pkttype = libc::PACKET_HOST;
            // SAFETY: `addr` is of correct type and correctly initialized
            let res = unsafe {
                libc::bind(
                    sock.as_raw_fd(),
                    (&raw const addr).cast(),
                    core::mem::size_of::<libc::sockaddr_ll>() as u32,
                )
            };

            nix::errno::Errno::result(res)
                .map(drop)
                .map_err(|err| Error::Init(name.clone(), InitError::Bind(err)))?;
        }
        // {
        //     // SAFETY: libc::iwreq is plain-old-data
        //     let mut iwreq: libc::iwreq = unsafe { core::mem::zeroed() };
        //     iwreq.ifr_ifrn.ifrn_name = ifname;
        //     // SAFETY: this ioctl is safe to call
        //     unsafe { siocgiwmode(sock.as_raw_fd(), &raw mut iwreq) }
        //         .map_err(|err| Error::Init(name.clone(), InitError::Ioctl("SIOCGIWMODE", err)))?;
        //     // SAFETY: SIOCGIWMODE sets this union variant.
        //     let mode = unsafe { iwreq.u.mode };
        //     if mode != libc::IW_MODE_MONITOR as u32 {
        //         iwreq.u.mode = libc::IW_MODE_MONITOR as u32;
        //         // SAFETY: this ioctl is safe to call
        //         unsafe { siocsiwmode(sock.as_raw_fd(), &raw mut iwreq) }.map_err(|err| {
        //             Error::Init(name.clone(), InitError::Ioctl("SIOCSIWMODE", err))
        //         })?;
        //     }
        // }
        setsockopt(&sock, PacketAddMembership, &(index.cast_signed(), mac)).map_err(|err| {
            Error::Init(name.clone(), InitError::SetSockOptPacketAddMembership(err))
        })?;
        setsockopt(&sock, PacketAuxdata, &true)
            .map_err(|err| Error::Init(name.clone(), InitError::SetSockOptPacketAuxData(err)))?;
        Ok(Self {
            name,
            ifname,
            index,
            mac,
            socket: sock,
        })
    }

    pub fn write_all(&self, buf: &[u8]) -> Result<(), Error> {
        let iov = [std::io::IoSlice::new(buf)];
        let wrote = nix::sys::socket::sendmsg::<()>(
            self.socket.as_fd().as_raw_fd(),
            &iov,
            &[],
            MsgFlags::empty(),
            None,
        )
        .map_err(|err| Error::Write(self.name.clone(), err))?;
        assert_eq!(wrote, buf.len());

        Ok(())
    }
}

pub struct Buffer {
    pub desc_chain: DescriptorChain<GuestMemoryLoadGuard<GuestMemoryMmap<()>>>,
    pub vring: VringRwLock,
}

fn feed_rxq(buffer: Buffer, iface: u32, data: &[u8]) {
    let mem = buffer.desc_chain.memory();
    let mut writer = buffer.desc_chain.clone().writer(mem).unwrap();
    writer
        .write_obj(crate::virtio_wifi::VirtioWifiHdr {
            code: crate::virtio_wifi::VIRTIO_WIFI_S_OK.into(),
            index: iface.into(),
        })
        .unwrap();
    use std::io::Write;
    writer.write_all(data).unwrap();

    let used_len = writer.bytes_written();

    buffer
        .vring
        .add_used(buffer.desc_chain.head_index(), used_len as u32)
        .unwrap();
    buffer.vring.signal_used_queue().unwrap();
}

fn feed_eventq(buffer: Buffer, event: Vec<u8>) {
    let mem = buffer.desc_chain.memory();
    let mut writer = buffer.desc_chain.clone().writer(mem).unwrap();
    use std::io::Write;
    let len = event.len();
    log::debug!("event bytes len = {len}");
    writer.write_all(&event).unwrap();

    let used_len = writer.bytes_written();

    buffer
        .vring
        .add_used(buffer.desc_chain.head_index(), used_len as u32)
        .unwrap();
    buffer.vring.signal_used_queue().unwrap();
}

#[derive(Clone)]
pub struct IoWorkerData {
    pub ifsockets: Arc<Mutex<BTreeMap<u32, IfSocket>>>,
    rx_buffers: Arc<Mutex<Vec<Buffer>>>,
    new_rx_buffer_event: Arc<EventFd>,
    event_buffers: Arc<Mutex<Vec<Buffer>>>,
    new_event_buffer_event: Arc<EventFd>,
    socket_change_event: Arc<EventFd>,
    status_event: Arc<EventFd>,
}

impl Default for IoWorkerData {
    fn default() -> Self {
        Self::new()
    }
}

#[repr(u64)]
pub enum StatusCommand {
    // Must start at non-zero, because eventfd is a counter and needs a non-zero value to be
    // added.
    Start = 1,
    Stop = 2,
    Exit = 3,
}

impl IoWorkerData {
    pub fn new() -> Self {
        Self {
            ifsockets: Arc::new(Mutex::new(BTreeMap::new())),
            rx_buffers: Arc::new(Mutex::new(vec![])),
            new_rx_buffer_event: Arc::new(EventFd::new().unwrap()),
            event_buffers: Arc::new(Mutex::new(vec![])),
            new_event_buffer_event: Arc::new(EventFd::new().unwrap()),
            socket_change_event: Arc::new(EventFd::new().unwrap()),
            status_event: Arc::new(EventFd::new().unwrap()),
        }
    }

    #[inline]
    pub fn add_rx_buffer(
        &self,
        desc_chains: virtio_queue::AvailIter<'_, GuestMemoryLoadGuard<GuestMemoryMmap<()>>>,
        vring: VringRwLock,
    ) {
        let mut desc_chains = desc_chains.peekable();
        if desc_chains.peek().is_none() {
            return;
        }
        let mut buffers_lck = self.rx_buffers.lock().unwrap();
        // let mut i = 0;

        for desc_chain in desc_chains {
            // i += 1;
            buffers_lck.push(Buffer {
                desc_chain,
                vring: vring.clone(),
            });
        }

        self.signal_add_rx_buffer();
        // CHATTY: log::debug!("Received {} rxq buffer(s)", i);
    }

    #[inline]
    pub fn signal_add_rx_buffer(&self) {
        self.new_rx_buffer_event.write(42).unwrap();
    }

    #[inline]
    pub fn add_event_buffer(
        &self,
        desc_chains: virtio_queue::AvailIter<'_, GuestMemoryLoadGuard<GuestMemoryMmap<()>>>,
        vring: VringRwLock,
    ) {
        let mut desc_chains = desc_chains.peekable();
        if desc_chains.peek().is_none() {
            return;
        }
        let mut buffers_lck = self.event_buffers.lock().unwrap();
        // let mut i = 0;

        for desc_chain in desc_chains {
            // i += 1;
            buffers_lck.push(Buffer {
                desc_chain,
                vring: vring.clone(),
            });
        }

        self.new_event_buffer_event.write(42).unwrap();
        // CHATTY: log::debug!("Received {} rxq buffer(s)", i);
    }

    #[inline]
    pub fn start(&self, sender: &std::sync::mpsc::Sender<StatusCommand>) {
        sender.send(StatusCommand::Start).unwrap();
        self.status_event.write(1).unwrap();
    }

    #[inline]
    pub fn stop(&self, sender: &std::sync::mpsc::Sender<StatusCommand>) {
        sender.send(StatusCommand::Stop).unwrap();
        self.status_event.write(1).unwrap();
    }

    #[inline]
    pub fn exit(&self, sender: &std::sync::mpsc::Sender<StatusCommand>) {
        _ = sender.send(StatusCommand::Exit);
        // Called from Drop so must not panic
        _ = self.status_event.write(1);
    }

    #[inline]
    pub fn add_ifsocket(&self, index: u32, val: IfSocket) {
        self.ifsockets.lock().unwrap().insert(index, val);
        self.socket_change_event.write(42).unwrap();
    }

    #[inline]
    pub fn del_ifsocket(&self, index: u32) {
        self.ifsockets.lock().unwrap().remove(&index);
        self.socket_change_event.write(42).unwrap();
    }

    pub fn work(
        self,
        status_receiver: std::sync::mpsc::Receiver<StatusCommand>,
        event_notifier: crate::nl80211::EventNotifier,
    ) {
        use nix::poll::{poll, PollFd, PollFlags, PollTimeout};

        struct DuplicatedSocket {
            fd: OwnedFd,
            iface_index: u32,
        }

        #[derive(Copy, Clone, Debug)]
        #[repr(C)]
        struct TpacketAuxdata {
            tp_status: u32,
            tp_len: u32,     /* packet size */
            tp_snaplen: u32, /* captured size */
            tp_mac: u16,
            tp_net: u16,
            tp_vlan_tci: u16,
            tp_vlan_tpid: u16, /* Since Linux 3.14; earlier, these
                               were unused padding bytes */
        }

        let mut cmsg = nix::cmsg_space!(TpacketAuxdata);

        let mut socket_fds: Vec<DuplicatedSocket> = vec![];
        let mut read_buf = vec![0; 4096];

        let mut delayed_rx_work = VecDeque::<(u32, Vec<u8>)>::new();
        let mut delayed_event_work = VecDeque::<Vec<u8>>::new();

        let mut started = false;

        loop {
            // CHATTY: log::debug!("work new loop got {} socket_fds", socket_fds.len());
            let mut pollfds = vec![];
            macro_rules! push_event_fd {
                (event_notifier) => {{
                    let idx = pollfds.len();
                    pollfds.push(PollFd::new(
                        event_notifier.event_fd.as_fd(),
                        PollFlags::POLLIN,
                    ));
                    idx
                }};
                ($f:ident) => {{
                    let idx = pollfds.len();
                    pollfds.push(PollFd::new(self.$f.as_fd(), PollFlags::POLLIN));
                    idx
                }};
            }
            let new_rx_buffer_event_idx = push_event_fd!(new_rx_buffer_event);
            let new_event_buffer_event_idx = push_event_fd!(new_event_buffer_event);
            let new_event_idx = push_event_fd!(event_notifier);
            let socket_change_event_idx = push_event_fd!(socket_change_event);
            let status_event_idx = push_event_fd!(status_event);

            let eventfds_len = pollfds.len();
            for fd in &socket_fds {
                pollfds.push(PollFd::new(fd.fd.as_fd(), PollFlags::POLLIN));
            }

            let nready = poll(&mut pollfds, PollTimeout::NONE).unwrap();
            // CHATTY: log::debug!("work nready {nready}");
            assert!(nready >= 1); // Since there is no timeout

            if pollfds[new_rx_buffer_event_idx].any().unwrap_or_default() {
                // CHATTY: log::debug!("work new_rx_buffer_event");
                self.new_rx_buffer_event.read().unwrap();
                // new buffer event
                // TODO check delayed work queue?
                while !delayed_rx_work.is_empty() {
                    let Some(buf) = self.rx_buffers.lock().unwrap().pop() else {
                        break;
                    };
                    let (i, data) = delayed_rx_work.pop_front().unwrap();
                    feed_rxq(buf, i, data.as_slice());
                }
            } else if pollfds[new_event_buffer_event_idx]
                .any()
                .unwrap_or_default()
            {
                self.new_event_buffer_event.read().unwrap();
                while !delayed_event_work.is_empty() {
                    let Some(buf) = self.event_buffers.lock().unwrap().pop() else {
                        break;
                    };
                    let ev = delayed_event_work.pop_front().unwrap();
                    feed_eventq(buf, ev);
                }
            } else if pollfds[new_event_idx].any().unwrap_or_default() {
                event_notifier.event_fd.read().unwrap();
                while let Ok(ev) = event_notifier.receiver.try_recv() {
                    let Some(buf) = self.event_buffers.lock().unwrap().pop() else {
                        delayed_event_work.push_back(ev);
                        break;
                    };
                    feed_eventq(buf, ev);
                }
            } else if pollfds[socket_change_event_idx].any().unwrap_or_default() {
                // socket change buffer event
                log::debug!("work socket change buffer event");
                self.socket_change_event.read().unwrap();
                socket_fds.clear();
                for (iface_index, sock) in self.ifsockets.lock().unwrap().iter() {
                    socket_fds.push(DuplicatedSocket {
                        iface_index: *iface_index,
                        fd: nix::unistd::dup(sock.socket.as_fd()).unwrap(),
                    });
                }
            } else if pollfds[status_event_idx].any().unwrap_or_default() {
                // worker status event
                let _no_of_updates = self.status_event.read().unwrap();
                log::debug!("worker status {} event(s)", _no_of_updates);
                while let Ok(command) = status_receiver.try_recv() {
                    match command {
                        StatusCommand::Start => {
                            log::debug!("worker status event = start");
                            started = true;
                        }
                        StatusCommand::Stop => {
                            log::debug!("worker status event = stop");
                            started = false;
                        }
                        StatusCommand::Exit => {
                            log::debug!("worker status event = exit");
                            return;
                        }
                    }
                }
            } else {
                // CHATTY: log::debug!("work socket read");
                // check read sockets
                for (i, fd) in pollfds.iter().skip(eventfds_len).enumerate() {
                    if !fd.any().unwrap_or_default() {
                        continue;
                    }
                    let dup_fd = &socket_fds[i];
                    // CHATTY: log::debug!(
                    //     "work socket reading from {i} iface = {}",
                    //     dup_fd.iface_index
                    // );
                    let mut buf = [std::io::IoSliceMut::new(&mut read_buf)];
                    let read: RecvMsg<'_, '_, ()> = match nix::sys::socket::recvmsg(
                        dup_fd.fd.as_raw_fd(),
                        &mut buf,
                        Some(&mut cmsg),
                        MsgFlags::empty(),
                    ) {
                        Ok(read) => read,
                        Err(Errno::ENETDOWN) => {
                            // Ignore, guest might have brought interface down.
                            // TODO: maybe create ENETDOWN event to guest.
                            continue;
                        }
                        Err(err) => {
                            panic!("{err}");
                        }
                    };
                    let cmsg = read.cmsgs().unwrap().next().unwrap();
                    let packet_data = {
                        use nix::sys::socket::ControlMessageOwned;
                        let ControlMessageOwned::Unknown(cmsg) = cmsg else {
                            // TODO: add error checking
                            unreachable!();
                        };
                        let ptr: *const TpacketAuxdata = cmsg.data_bytes.as_ptr().cast();
                        unsafe { *ptr }
                    };
                    dbg!(&packet_data);
                    let iov = read.iovs().next().unwrap();
                    // let read = match nix::unistd::read(dup_fd.fd.as_fd(), &mut read_buf) {
                    //     Ok(read) => read,
                    //     Err(Errno::ENETDOWN) => {
                    //         // Ignore, guest might have brought interface down.
                    //         // TODO: maybe create ENETDOWN event to guest.
                    //         continue;
                    //     }
                    //     Err(err) => {
                    //         panic!("{err}");
                    //     }
                    // };
                    // if !started {
                    //     continue;
                    // }

                    assert_eq!(packet_data.tp_len as usize, read.bytes);
                    let read = read.bytes.min(packet_data.tp_len as usize);
                    // CHATTY: log::debug!(
                    //     "read {read} bytes from socket {i} iface = {}",
                    //     dup_fd.iface_index
                    // );
                    let Some(buf) = self.rx_buffers.lock().unwrap().pop() else {
                        delayed_rx_work.push_back((dup_fd.iface_index, iov[0..read].to_vec()));
                        continue;
                    };
                    feed_rxq(buf, dup_fd.iface_index, &iov[0..read]);
                }
            }
        }
    }
}
