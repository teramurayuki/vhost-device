// SPDX-License-Identifier: Apache-2.0 or BSD-3-Clause
// Copyright 2026 Panasonic Automotive Systems Co., Ltd.
// Author: Manos Pitsidianakis <manos.pitsidianakis@linaro.org>

use std::{
    collections::BTreeMap,
    io::{Read, Result as IoResult},
};

use thiserror::Error as ThisError;
use vhost::vhost_user::message::{VhostUserProtocolFeatures, VhostUserVirtioFeatures};
use vhost_user_backend::{VhostUserBackendMut, VringRwLock, VringT};
use virtio_bindings::bindings::{
    virtio_config::{VIRTIO_F_NOTIFY_ON_EMPTY, VIRTIO_F_VERSION_1},
    virtio_ring::{VIRTIO_RING_F_EVENT_IDX, VIRTIO_RING_F_INDIRECT_DESC},
};
use virtio_queue::QueueOwnedT;
use vm_memory::{ByteValued, GuestAddressSpace, GuestMemoryAtomic, GuestMemoryMmap};
use vmm_sys_util::{
    epoll::EventSet,
    event::{new_event_consumer_and_notifier, EventConsumer, EventFlag, EventNotifier},
};

use crate::{
    nl80211::{Interface, Nl80211Socket, Wiphy},
    virtio_wifi::{self as virtio_wifi, VirtioWifiConfig, VirtioWifiHdr, VIRTIO_WIFI_VQ_MAX},
};

type Result<T> = std::result::Result<T, Error>;

#[derive(Copy, Clone, Debug, Eq, PartialEq, ThisError)]
/// Errors related to vhost-device-wifi daemon.
pub enum Error {
    #[error("Failed to handle event, didn't match EPOLLIN")]
    HandleEventNotEpollIn,
    #[error("Failed to handle unknown event")]
    HandleEventUnknown,
    #[error("Descriptor not found")]
    DescriptorNotFound,
    #[error("Reading from descriptor failed")]
    DescriptorReadFailed,
    #[error("Writing to descriptor failed")]
    DescriptorWriteFailed,
    #[error("Failed to send notification")]
    NotificationFailed,
    #[error("No memory configured")]
    NoMemoryConfigured,
    #[error("Failed to create new EventFd")]
    EventFdFailed,
}

impl From<Error> for std::io::Error {
    fn from(e: Error) -> Self {
        std::io::Error::other(e)
    }
}

pub struct VhostUserWifiBackend {
    event_idx: bool,
    negotiated_features: u64,
    virtio_cfg: VirtioWifiConfig,
    wiphys: BTreeMap<u32, Wiphy>,
    interfaces: BTreeMap<u32, Interface>,
    io_data: crate::io::IoWorkerData,
    io_data_status_sender: std::sync::mpsc::Sender<crate::io::StatusCommand>,
    data_seqno: usize,
    socket: Nl80211Socket,
    exit_consumer: EventConsumer,
    pub(crate) exit_notifier: EventNotifier,
    mem: Option<GuestMemoryAtomic<GuestMemoryMmap>>,
}

impl Drop for VhostUserWifiBackend {
    fn drop(&mut self) {
        self.io_data.exit(&self.io_data_status_sender);
    }
}

impl VhostUserWifiBackend {
    pub fn new(
        socket: Nl80211Socket,
        io_data_event_notifier: crate::nl80211::Notifier,
    ) -> Result<Self> {
        let wiphys = socket.list_wiphys().unwrap();
        let virtio_cfg = VirtioWifiConfig {
            phys: u32::try_from(wiphys.len()).unwrap().into(),
        };
        let (exit_consumer, exit_notifier) = new_event_consumer_and_notifier(EventFlag::NONBLOCK)
            .map_err(|_| Error::EventFdFailed)?;
        let io_data = crate::io::IoWorkerData::new();
        let (io_data_status_sender, io_data_status_receiver) = std::sync::mpsc::channel();
        std::thread::spawn({
            let io_data = io_data.clone();
            move || {
                io_data.work(io_data_status_receiver, io_data_event_notifier);
            }
        });
        Ok(VhostUserWifiBackend {
            event_idx: false,
            negotiated_features: 0,
            wiphys,
            interfaces: BTreeMap::new(),
            io_data,
            io_data_status_sender,
            data_seqno: 0,
            socket,
            virtio_cfg,
            exit_consumer,
            exit_notifier,
            mem: None,
        })
    }

    /// Process controlq buffers.
    fn process_control_queue(&mut self, vring: &VringRwLock) -> Result<()> {
        let Some(ref atomic_mem) = self.mem else {
            return Err(Error::NoMemoryConfigured);
        };
        let requests: Vec<_> = vring
            .get_mut()
            .get_queue_mut()
            .iter(atomic_mem.memory())
            .map_err(|_| Error::DescriptorNotFound)?
            .collect();

        if requests.is_empty() {
            return Ok(());
        }
        for desc_chain in requests {
            let mem = atomic_mem.memory();

            let mut reader = desc_chain
                .clone()
                .reader(&mem)
                .map_err(|_| Error::DescriptorReadFailed)?;
            let request = reader
                .read_obj::<VirtioWifiHdr>()
                .map_err(|_| Error::DescriptorReadFailed)?;
            let mut writer = desc_chain
                .clone()
                .writer(&mem)
                .map_err(|_| Error::DescriptorWriteFailed)?;

            let mut used_len = 0;

            let request_kind: u32 = request.code.into();
            log::debug!("Commandq request: {}", request_kind);
            match request_kind {
                virtio_wifi::VIRTIO_WIFI_CMD_GET_WIPHY => {
                    let phy_index = u32::from(request.index);
                    log::debug!("VIRTIO_WIFI_CMD_GET_WIPHY phy_index={phy_index}");
                    match self.wiphys.get(&phy_index) {
                        Some(wiphy) => {
                            let mut resp = virtio_wifi::VirtioWifiRadio::from(wiphy);
                            resp.ifaces =
                                (self.socket.list_ifaces(wiphy).unwrap().len() as u32).into();
                            log::debug!(
                                "VIRTIO_WIFI_CMD_GET_WIPHY phy_index={phy_index} replying with {resp:?}"
                            );
                            writer
                                .write_obj(resp)
                                .map_err(|_| Error::DescriptorWriteFailed)?;
                        }
                        None => {
                            writer
                                .write_obj(VirtioWifiHdr {
                                    code: virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                                    index: phy_index.into(),
                                })
                                .map_err(|_| Error::DescriptorWriteFailed)?;
                        }
                    }
                }
                virtio_wifi::VIRTIO_WIFI_CMD_GET_INTERFACE => {
                    let iface_index = u32::from(request.index);
                    let mut reader = desc_chain
                        .clone()
                        .reader(&mem)
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let request = reader
                        .read_obj::<virtio_wifi::VirtioWifiCmdGetInterfaceRequest>()
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let phy_index = u32::from(request.phy_index);
                    match self.wiphys.get(&phy_index) {
                        Some(wiphy) => match self.socket.list_ifaces(wiphy) {
                            Ok(ifaces) => match ifaces.get(&iface_index) {
                                Some(iface) => {
                                    writer
                                        .write_obj(virtio_wifi::VirtioWifiInterfaceData::from(
                                            iface,
                                        ))
                                        .map_err(|_| Error::DescriptorWriteFailed)?;
                                }
                                None => {
                                    writer
                                        .write_obj(VirtioWifiHdr {
                                            code: virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                                            index: iface_index.into(),
                                        })
                                        .map_err(|_| Error::DescriptorWriteFailed)?;
                                }
                            },
                            Err(err) => {
                                log::error!("{err}");
                                writer
                                    .write_obj(VirtioWifiHdr {
                                        code: virtio_wifi::VIRTIO_WIFI_S_IO_ERR.into(),
                                        index: iface_index.into(),
                                    })
                                    .map_err(|_| Error::DescriptorWriteFailed)?;
                            }
                        },
                        None => {
                            writer
                                .write_obj(VirtioWifiHdr {
                                    code: virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                                    index: iface_index.into(),
                                })
                                .map_err(|_| Error::DescriptorWriteFailed)?;
                        }
                    }
                }
                virtio_wifi::VIRTIO_WIFI_CMD_GET_BAND => {
                    let band_index = u32::from(request.index);
                    let mut reader = desc_chain
                        .clone()
                        .reader(&mem)
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let request = reader
                        .read_obj::<virtio_wifi::VirtioWifiCmdGetInterfaceRequest>()
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let phy_index = u32::from(request.phy_index);
                    log::debug!(
                        "VIRTIO_WIFI_CMD_GET_BAND phy_index={phy_index} band_index={band_index}"
                    );
                    match self
                        .wiphys
                        .get(&phy_index)
                        .and_then(|w| w.bands.get(band_index as usize))
                    {
                        Some(band) => {
                            log::debug!("VIRTIO_WIFI_CMD_GET_BAND phy_index={phy_index} band_index={band_index} replying with {:?}",virtio_wifi::VirtioWifiBand::from(band));
                            writer
                                .write_obj(virtio_wifi::VirtioWifiBand::from(band))
                                .map_err(|_| Error::DescriptorWriteFailed)?;
                        }
                        None => {
                            writer
                                .write_obj(VirtioWifiHdr {
                                    code: virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                                    index: band_index.into(),
                                })
                                .map_err(|_| Error::DescriptorWriteFailed)?;
                        }
                    }
                }
                virtio_wifi::VIRTIO_WIFI_CMD_GET_BAND_CHANNEL => {
                    let channel_index = u32::from(request.index);
                    let mut reader = desc_chain
                        .clone()
                        .reader(&mem)
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let request = reader
                        .read_obj::<virtio_wifi::VirtioWifiCmdGetBandChannelRequest>()
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let phy_index = u32::from(request.phy_index);
                    let band_index = u32::from(request.band_index);
                    log::debug!("VIRTIO_WIFI_CMD_GET_BAND_CHANNEL channel={channel_index:?} phy_index={phy_index} band_index={band_index}");
                    match self
                        .wiphys
                        .get(&phy_index)
                        .and_then(|w| w.bands.get(band_index as usize))
                        .and_then(|b| b.freqs.get(channel_index as usize))
                    {
                        Some(channel) => {
                            log::debug!("VIRTIO_WIFI_CMD_GET_BAND_CHANNEL channel={channel_index:?} phy_index={phy_index} band_index={band_index} replying with {:?}",virtio_wifi::VirtioWifiChannel::from(channel));
                            writer
                                .write_obj(virtio_wifi::VirtioWifiChannel::from(channel))
                                .map_err(|_| Error::DescriptorWriteFailed)?;
                        }
                        None => {
                            writer
                                .write_obj(VirtioWifiHdr {
                                    code: virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                                    index: band_index.into(),
                                })
                                .map_err(|_| Error::DescriptorWriteFailed)?;
                        }
                    }
                }
                virtio_wifi::VIRTIO_WIFI_CMD_GET_BAND_BITRATE => {
                    let bitrate_index = u32::from(request.index);
                    let mut reader = desc_chain
                        .clone()
                        .reader(&mem)
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let request = reader
                        .read_obj::<virtio_wifi::VirtioWifiCmdGetBandChannelRequest>()
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let phy_index = u32::from(request.phy_index);
                    let band_index = u32::from(request.band_index);
                    log::debug!("VIRTIO_WIFI_CMD_GET_BAND_BITRATE bitrate_index={bitrate_index:?} phy_index={phy_index} band_index={band_index}");
                    match self
                        .wiphys
                        .get(&phy_index)
                        .and_then(|w| w.bands.get(band_index as usize))
                        .and_then(|b| b.rates.get(bitrate_index as usize))
                    {
                        Some(b) => {
                            log::debug!("VIRTIO_WIFI_CMD_GET_BAND_BITRATE bitrate_index={bitrate_index:?} phy_index={phy_index} band_index={band_index} replying with {:?}",virtio_wifi::VirtioWifiBandBitrate::from(b));
                            writer
                                .write_obj(virtio_wifi::VirtioWifiBandBitrate::from(b))
                                .map_err(|_| Error::DescriptorWriteFailed)?;
                        }
                        None => {
                            writer
                                .write_obj(VirtioWifiHdr {
                                    code: virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                                    index: band_index.into(),
                                })
                                .map_err(|_| Error::DescriptorWriteFailed)?;
                        }
                    }
                }
                virtio_wifi::VIRTIO_WIFI_CMD_NEW_INTERFACE => {
                    let phy_index = u32::from(request.index);
                    log::debug!("VIRTIO_WIFI_CMD_NEW_INTERFACE phy_index={phy_index}");
                    match self.wiphys.get(&phy_index) {
                        Some(wiphy) => {
                            let mut reader = desc_chain
                                .clone()
                                .reader(&mem)
                                .map_err(|_| Error::DescriptorReadFailed)?;
                            let request = reader
                                .read_obj::<virtio_wifi::VirtioWifiInterface>()
                                .map_err(|_| Error::DescriptorReadFailed)?;
                            log::debug!("VIRTIO_WIFI_CMD_NEW_INTERFACE phy_index={phy_index} request = {request:?}");
                            match self.socket.add_iface(wiphy, request) {
                                Ok(mut iface) => {
                                    self.io_data.add_ifsocket(
                                        iface.index,
                                        crate::io::IfSocket::new(iface.name.clone(), iface.index)
                                            .unwrap(),
                                    );
                                    iface.response.hdr.code = virtio_wifi::VIRTIO_WIFI_S_OK.into();
                                    writer
                                        .write_obj(iface.response)
                                        .map_err(|_| Error::DescriptorWriteFailed)?;
                                    self.interfaces.insert(iface.index, iface);
                                }
                                Err(err) => {
                                    log::error!("{err}");
                                    writer
                                        .write_obj(VirtioWifiHdr {
                                            code: virtio_wifi::VIRTIO_WIFI_S_IO_ERR.into(),
                                            index: request.hdr.index,
                                        })
                                        .map_err(|_| Error::DescriptorWriteFailed)?;
                                }
                            }
                        }
                        None => {
                            writer
                                .write_obj(VirtioWifiHdr {
                                    code: virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                                    index: request.index,
                                })
                                .map_err(|_| Error::DescriptorWriteFailed)?;
                        }
                    }
                }
                virtio_wifi::VIRTIO_WIFI_CMD_DELETE_INTERFACE => {
                    let phy_index = u32::from(request.index);
                    log::debug!("VIRTIO_WIFI_CMD_DELETE_INTERFACE phy_index={phy_index}");
                    match self.wiphys.get(&phy_index) {
                        Some(wiphy) => {
                            let mut reader = desc_chain
                                .clone()
                                .reader(&mem)
                                .map_err(|_| Error::DescriptorReadFailed)?;
                            let request = reader
                                .read_obj::<virtio_wifi::VirtioWifiInterface>()
                                .map_err(|_| Error::DescriptorReadFailed)?;
                            log::debug!("VIRTIO_WIFI_CMD_DELETE_INTERFACE phy_index={phy_index} request = {request:?}");
                            let iface_index = request.index.into();
                            let code = match self.socket.del_iface(wiphy, iface_index) {
                                Ok(()) => {
                                    self.io_data.del_ifsocket(iface_index);
                                    self.interfaces.remove(&iface_index);
                                    virtio_wifi::VIRTIO_WIFI_S_OK.into()
                                }
                                Err(err) => {
                                    log::error!("{err}");
                                    virtio_wifi::VIRTIO_WIFI_S_IO_ERR.into()
                                }
                            };
                            writer
                                .write_obj(VirtioWifiHdr {
                                    code,
                                    index: request.index,
                                })
                                .map_err(|_| Error::DescriptorWriteFailed)?;
                        }
                        None => {
                            writer
                                .write_obj(VirtioWifiHdr {
                                    code: virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                                    index: request.index,
                                })
                                .map_err(|_| Error::DescriptorWriteFailed)?;
                        }
                    }
                }
                // virtio_wifi::VIRTIO_WIFI_CMD_SET_CHANNEL => {
                //     let phy_index = u32::from(request.index);
                //     log::debug!("VIRTIO_WIFI_CMD_SET_CHANNEL phy_index={phy_index}");
                //     match self.wiphys.get(&phy_index) {
                //         Some(wiphy) => {
                //             // TODO/FIXME: we need a channel context per interface, but for now just
                //             // assume we have one vif in the guest:
                //             let iface_index = self
                //                 .socket
                //                 .list_ifaces(wiphy)
                //                 .map(|ifaces| ifaces.first_key_value().map(|iface| *iface.0));
                //             let mut reader = desc_chain
                //                 .clone()
                //                 .reader(&mem)
                //                 .map_err(|_| Error::DescriptorReadFailed)?;
                //             let chandef = reader
                //                 .read_obj::<virtio_wifi::VirtioWifiChannel>()
                //                 .map_err(|_| Error::DescriptorReadFailed)?;
                //             log::debug!("VIRTIO_WIFI_CMD_SET_CHANNEL chandef = {chandef:?}");
                //             let code = match iface_index.and_then(|iface_index| {
                //                 let Some(iface_index) = iface_index else {
                //                     return Ok(None);
                //                 };
                //                 self.socket.set_channel(wiphy, iface_index, &chandef)?;
                //                 Ok(Some(()))
                //             }) {
                //                 Ok(Some(())) => virtio_wifi::VIRTIO_WIFI_S_OK.into(),
                //                 Ok(None) => virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                //                 Err(err) => {
                //                     log::debug!("set channel error: {err}");
                //                     virtio_wifi::VIRTIO_WIFI_S_IO_ERR.into()
                //                 }
                //             };
                //             writer
                //                 .write_obj(VirtioWifiHdr {
                //                     code,
                //                     index: request.index,
                //                 })
                //                 .map_err(|_| Error::DescriptorWriteFailed)?;
                //         }
                //         None => {
                //             writer
                //                 .write_obj(VirtioWifiHdr {
                //                     code: virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                //                     index: request.index,
                //                 })
                //                 .map_err(|_| Error::DescriptorWriteFailed)?;
                //         }
                //     }
                // }
                virtio_wifi::VIRTIO_WIFI_CMD_TRIGGER_SCAN => {
                    let index = u32::from(request.index);
                    // log::trace!("VIRTIO_WIFI_CMD_TRIGGER_SCAN iface={index}");
                    if self.interfaces.contains_key(&index) {
                        let code = match self.socket.trigger_scan(index) {
                            Ok(()) => virtio_wifi::VIRTIO_WIFI_S_OK.into(),
                            Err(err) => {
                                log::error!("trigger scan error: {err}");
                                virtio_wifi::VIRTIO_WIFI_S_IO_ERR.into()
                            }
                        };
                        writer
                            .write_obj(VirtioWifiHdr {
                                code,
                                index: request.index,
                            })
                            .map_err(|_| Error::DescriptorWriteFailed)?;
                    } else {
                        writer
                            .write_obj(VirtioWifiHdr {
                                code: virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                                index: request.index,
                            })
                            .map_err(|_| Error::DescriptorWriteFailed)?;
                    }
                }
                virtio_wifi::VIRTIO_WIFI_CMD_GET_CIPHERSUITES => {
                    let index = u32::from(request.index);
                    log::debug!("cipher suites: {index:?}");
                    match dbg!(self.wiphys.get(&index)) {
                        Some(wiphy) => {
                            writer
                                .write_obj(VirtioWifiHdr {
                                    code: virtio_wifi::VIRTIO_WIFI_S_OK.into(),
                                    index: index.into(),
                                })
                                .map_err(|_| Error::DescriptorWriteFailed)?;
                            log::debug!("writing ciphers: {:?}", wiphy.cipher_suites);
                            for c in &wiphy.cipher_suites {
                                dbg!(writer
                                    .write_obj(*c)
                                    .map_err(|_| Error::DescriptorWriteFailed))?;
                            }
                            log::debug!("wrote cipher suites: {:?}", wiphy.cipher_suites);
                        }
                        None => {
                            writer
                                .write_obj(VirtioWifiHdr {
                                    code: virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                                    index: index.into(),
                                })
                                .map_err(|_| Error::DescriptorWriteFailed)?;
                        }
                    }
                }
                virtio_wifi::VIRTIO_WIFI_CMD_AUTHENTICATE => {
                    let index = u32::from(request.index);
                    if self.interfaces.contains_key(&index) {
                        let code = match self.socket.authenticate(index) {
                            Ok(()) => virtio_wifi::VIRTIO_WIFI_S_OK.into(),
                            Err(err) => {
                                log::error!("authenticate error: {err}");
                                virtio_wifi::VIRTIO_WIFI_S_IO_ERR.into()
                            }
                        };
                        writer
                            .write_obj(VirtioWifiHdr {
                                code,
                                index: request.index,
                            })
                            .map_err(|_| Error::DescriptorWriteFailed)?;
                    } else {
                        writer
                            .write_obj(VirtioWifiHdr {
                                code: virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                                index: request.index,
                            })
                            .map_err(|_| Error::DescriptorWriteFailed)?;
                    }
                }
                _ => {
                    writer
                        .write_obj(VirtioWifiHdr {
                            code: virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                            index: 0.into(),
                        })
                        .map_err(|_| Error::DescriptorWriteFailed)?;
                }
            }

            used_len += writer.bytes_written();
            assert!(used_len > 0);

            let used_len = match u32::try_from(used_len) {
                Ok(len) => len,
                Err(len) => {
                    log::warn!("used_len {len} overflows u32");
                    u32::MAX
                }
            };

            if vring.add_used(desc_chain.head_index(), used_len).is_err() {
                log::error!("Couldn't return used descriptors to the ring");
            }
        }

        // Send notification once all the requests are processed
        vring
            .signal_used_queue()
            .map_err(|_| Error::NotificationFailed)?;

        Ok(())
    }

    /// Process eventq buffers.
    fn process_event_queue(&mut self, vring: &VringRwLock) -> Result<()> {
        let Some(ref atomic_mem) = self.mem else {
            return Err(Error::NoMemoryConfigured);
        };
        self.io_data.add_event_buffer(
            vring
                .get_mut()
                .get_queue_mut()
                .iter(atomic_mem.memory())
                .map_err(|_| Error::DescriptorNotFound)?,
            vring.clone(),
        );

        Ok(())
    }

    /// Process txq buffers.
    fn process_tx_queue(&mut self, vring: &VringRwLock) -> Result<()> {
        let Some(ref atomic_mem) = self.mem else {
            return Err(Error::NoMemoryConfigured);
        };
        let requests: Vec<_> = vring
            .get_mut()
            .get_queue_mut()
            .iter(atomic_mem.memory())
            .map_err(|_| Error::DescriptorNotFound)?
            .collect();

        if requests.is_empty() {
            return Ok(());
        }
        for desc_chain in requests {
            let mem = atomic_mem.memory();

            let mut reader = desc_chain
                .clone()
                .reader(&mem)
                .map_err(|_| Error::DescriptorReadFailed)?;
            let request = reader
                .read_obj::<VirtioWifiHdr>()
                .map_err(|_| Error::DescriptorReadFailed)?;
            let mut writer = desc_chain
                .clone()
                .writer(&mem)
                .map_err(|_| Error::DescriptorWriteFailed)?;

            let mut used_len = 0;

            let request_kind: u32 = request.code.into();
            let if_index: u32 = request.index.into();
            // FIXME: check if request_kind is TX?
            let available_bytes = reader.available_bytes();
            let mut buffer = vec![0; available_bytes];

            reader
                .read_exact(&mut buffer)
                .map_err(|_| Error::DescriptorReadFailed)?;

            match etherparse::PacketHeaders::from_ethernet_slice(&buffer) {
                Err(err) => log::debug!("Error during parsing TX:\n{err:?}"),
                Ok(frame) => {
                    log::debug!("Got TX 802.3 frame: {frame:?}");
                }
            }
            match self.io_data.ifsockets.lock().unwrap().get(&if_index) {
                Some(ifsocket) => {
                    let code = if let Err(err) = ifsocket.write_all(&buffer) {
                        log::debug!("txq error: {err}");
                        virtio_wifi::VIRTIO_WIFI_S_IO_ERR.into()
                    } else {
                        log::debug!("txq wrote: {} bytes", buffer.len());
                        let data_seqno = self.data_seqno;
                        std::thread::spawn(move || {
                            std::fs::write(
                                std::path::Path::new("./data/tx").join(data_seqno.to_string()),
                                &buffer,
                            )
                            .unwrap();
                        });
                        self.data_seqno += 1;
                        virtio_wifi::VIRTIO_WIFI_S_OK.into()
                    };
                    writer
                        .write_obj(VirtioWifiHdr {
                            code,
                            index: request.index,
                        })
                        .map_err(|_| Error::DescriptorWriteFailed)?;
                }
                None => {
                    writer
                        .write_obj(VirtioWifiHdr {
                            code: virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                            index: request.index,
                        })
                        .map_err(|_| Error::DescriptorWriteFailed)?;
                }
            }

            used_len += writer.bytes_written();
            assert!(used_len > 0);

            let used_len = match u32::try_from(used_len) {
                Ok(len) => len,
                Err(len) => {
                    log::warn!("used_len {len} overflows u32");
                    u32::MAX
                }
            };

            if vring.add_used(desc_chain.head_index(), used_len).is_err() {
                log::error!("Couldn't return used descriptors to the ring");
            }
        }

        // Send notification once all the requests are processed
        vring
            .signal_used_queue()
            .map_err(|_| Error::NotificationFailed)?;

        Ok(())
    }

    /// Process rxq buffers.
    fn process_rx_queue(&mut self, vring: &VringRwLock) -> Result<()> {
        let Some(ref atomic_mem) = self.mem else {
            return Err(Error::NoMemoryConfigured);
        };
        self.io_data.add_rx_buffer(
            vring
                .get_mut()
                .get_queue_mut()
                .iter(atomic_mem.memory())
                .map_err(|_| Error::DescriptorNotFound)?,
            vring.clone(),
        );

        Ok(())
    }
}

/// VhostUserBackendMut trait methods
impl VhostUserBackendMut for VhostUserWifiBackend {
    type Vring = VringRwLock;
    type Bitmap = ();

    fn num_queues(&self) -> usize {
        VIRTIO_WIFI_VQ_MAX as usize
    }

    fn max_queue_size(&self) -> usize {
        1024
    }

    fn features(&self) -> u64 {
        (1 << VIRTIO_F_VERSION_1)
            | (1 << VIRTIO_F_NOTIFY_ON_EMPTY)
            | (1 << VIRTIO_RING_F_INDIRECT_DESC)
            | (1 << VIRTIO_RING_F_EVENT_IDX)
            | VhostUserVirtioFeatures::PROTOCOL_FEATURES.bits()
    }

    fn acked_features(&mut self, features: u64) {
        self.negotiated_features = features;
    }

    fn protocol_features(&self) -> VhostUserProtocolFeatures {
        VhostUserProtocolFeatures::MQ | VhostUserProtocolFeatures::CONFIG
    }

    fn get_config(&self, offset: u32, size: u32) -> Vec<u8> {
        let offset = offset as usize;
        let size = size as usize;

        let buf = self.virtio_cfg.as_slice();
        log::trace!(
            "get_config offset={offset} size={size}actual size = {}",
            buf.len()
        );

        if offset + size > buf.len() {
            return Vec::new();
        }

        buf[offset..offset + size].to_vec()
    }

    fn set_event_idx(&mut self, enabled: bool) {
        self.event_idx = enabled;
    }

    fn update_memory(&mut self, mem: GuestMemoryAtomic<GuestMemoryMmap>) -> IoResult<()> {
        self.mem = Some(mem);
        Ok(())
    }

    fn handle_event(
        &mut self,
        device_event: u16,
        evset: EventSet,
        vrings: &[VringRwLock],
        _thread_id: usize,
    ) -> IoResult<()> {
        // CHATTY: log::debug!("handle event: {} {:?}", device_event, evset);
        if evset != EventSet::IN {
            return Err(Error::HandleEventNotEpollIn.into());
        }

        macro_rules! dispatch_queue {
            ($((virtio_wifi::$queue_idx:ident => $cb:ident)),*$(,)?) => {
                match device_event.into() {
                    $(
                        virtio_wifi::$queue_idx => {
                            let vring = &vrings[virtio_wifi::$queue_idx as usize];

                            if self.event_idx {
                                // vm-virtio's Queue implementation only checks avail_index
                                // once, so to properly support EVENT_IDX we need to keep
                                // calling process_control_queue() until it stops finding new
                                // requests on the queue.
                                loop {
                                    vring.disable_notification().unwrap();
                                    self.$cb(vring)?;
                                    if !vring.enable_notification().unwrap() {
                                        break;
                                    }
                                }
                            } else {
                                // Without EVENT_IDX, a single call is enough.
                                self.$cb(vring)?;
                            }
                        }
                    )*,
                    _ => {
                        log::warn!("unhandled device_event: {device_event}");
                        return Err(Error::HandleEventUnknown.into());
                    }
                }
            }
        }

        dispatch_queue! {
            (virtio_wifi::VIRTIO_WIFI_VQ_CONTROL => process_control_queue),
            (virtio_wifi::VIRTIO_WIFI_VQ_EVENT => process_event_queue),
            (virtio_wifi::VIRTIO_WIFI_VQ_RX => process_rx_queue),
            (virtio_wifi::VIRTIO_WIFI_VQ_TX => process_tx_queue),

        };
        Ok(())
    }

    fn exit_event(&self, _thread_index: usize) -> Option<(EventConsumer, EventNotifier)> {
        let consumer = self.exit_consumer.try_clone().ok()?;
        let notifier = self.exit_notifier.try_clone().ok()?;
        Some((consumer, notifier))
    }
}
