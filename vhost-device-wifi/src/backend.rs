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
    bindings::virtio_wifi_uapi::{self as virtio_wifi, VIRTIO_WIFI_VQ_MAX},
    nl80211::{FrameType, Interface, Nl80211Socket, Wiphy},
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
    virtio_cfg: virtio_wifi::virtio_wifi_config,
    wiphy: Wiphy,
    interfaces: BTreeMap<u32, Interface>,
    io_data: crate::io::IoWorkerData,
    io_data_status_sender: std::sync::mpsc::Sender<crate::io::StatusCommand>,
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
        wiphy: Wiphy,
        socket: Nl80211Socket,
        io_data_event_notifier: crate::nl80211::Notifier,
    ) -> Result<Self> {
        let io_data = crate::io::IoWorkerData::new();
        let (io_data_status_sender, io_data_status_receiver) = std::sync::mpsc::channel();
        // Must spawn thread first to receive new ifsocket events
        std::thread::spawn({
            let io_data = io_data.clone();
            move || {
                io_data.work(io_data_status_receiver, io_data_event_notifier);
            }
        });
        let mut interfaces = BTreeMap::new();
        for (idx, iface) in socket.list_ifaces(&wiphy).unwrap() {
            debug_assert_eq!(idx, iface.index);
            log::info!("adding pre-existing iface {idx} of wiphy {}", wiphy.name);

            log::debug!("Registering frames...");
            for (ft, match_bytes) in &[(FrameType::MGMT_AUTH, b"\x03\x00")] {
                if wiphy.supported_rx_frames.contains(ft) {
                    let result =
                        socket.register_frame(iface.wdev, iface.index, *ft, match_bytes.as_slice());
                    log::debug!("rx {ft:?}: {result:?}");
                }
            }
            for ft in &wiphy.supported_rx_frames {
                let result = socket.register_frame(iface.wdev, iface.index, *ft, &[]);
                log::debug!("rx {ft:?}: {result:?}");
            }
            log::debug!("Registering frames: done");
            io_data.add_ifsocket(
                iface.index,
                crate::io::IfSocket::new(iface.name.clone(), iface.index).unwrap(),
            );
            interfaces.insert(idx, iface);
        }
        let virtio_cfg = virtio_wifi::virtio_wifi_config {
            num_of_ifaces: u32::try_from(interfaces.len()).unwrap().into(),
            ..(&wiphy).into()
        };
        let (exit_consumer, exit_notifier) = new_event_consumer_and_notifier(EventFlag::NONBLOCK)
            .map_err(|_| Error::EventFdFailed)?;

        Ok(VhostUserWifiBackend {
            event_idx: false,
            negotiated_features: 0,
            wiphy,
            interfaces,
            io_data,
            io_data_status_sender,
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
                .read_obj::<virtio_wifi::virtio_wifi_hdr>()
                .map_err(|_| Error::DescriptorReadFailed)?;
            let mut writer = desc_chain
                .clone()
                .writer(&mem)
                .map_err(|_| Error::DescriptorWriteFailed)?;

            let mut used_len = 0;

            let request_kind: u32 = request.code.into();
            log::trace!("Commandq request: {}", request_kind);
            match request_kind {
                virtio_wifi::VIRTIO_WIFI_CMD_GET_INTERFACES => {
                    log::debug!("VIRTIO_WIFI_CMD_GET_INTERFACES");
                    match self.socket.list_ifaces(&self.wiphy).ok() {
                        Some(ifaces) => {
                            let resp = ifaces
                                .values()
                                .map(virtio_wifi::virtio_wifi_interface::from)
                                .collect::<Vec<_>>();
                            log::debug!("VIRTIO_WIFI_CMD_GET_INTERFACES replying with {resp:?}");
                            for i in ifaces.values() {
                                writer
                                    .write_obj(virtio_wifi::virtio_wifi_interface::from(i))
                                    .map_err(|_| Error::DescriptorWriteFailed)?;
                            }
                        }
                        None => {
                            writer
                                .write_obj(virtio_wifi::virtio_wifi_hdr {
                                    code: virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                                    index: 0.into(),
                                })
                                .map_err(|_| Error::DescriptorWriteFailed)?;
                        }
                    }
                }
                virtio_wifi::VIRTIO_WIFI_CMD_GET_BAND => {
                    let band_kind = u32::from(request.index);
                    log::debug!("VIRTIO_WIFI_CMD_GET_BAND band_kind={band_kind}");
                    match self.wiphy.bands.get(band_kind as usize) {
                        Some(band) => {
                            log::debug!("VIRTIO_WIFI_CMD_GET_BAND  band_kind={band_kind} replying with {:?}",virtio_wifi::virtio_wifi_band::from(band));
                            writer
                                .write_obj(virtio_wifi::virtio_wifi_band::from(band))
                                .map_err(|_| Error::DescriptorWriteFailed)?;
                        }
                        None => {
                            log::debug!("VIRTIO_WIFI_CMD_GET_BAND  band_kind={band_kind} not found, have: {:?}", self.wiphy.bands);
                            writer
                                .write_obj(virtio_wifi::virtio_wifi_hdr {
                                    code: virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                                    index: 0.into(),
                                })
                                .map_err(|_| Error::DescriptorWriteFailed)?;
                        }
                    }
                }
                virtio_wifi::VIRTIO_WIFI_CMD_GET_BAND_CHANNEL => {
                    let band_kind = u32::from(request.index);
                    let mut reader = desc_chain
                        .clone()
                        .reader(&mem)
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let request = reader
                        .read_obj::<virtio_wifi::virtio_wifi_cmd_get_band_channel_request>()
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let channel_index = u32::from(request.channel_index);
                    log::debug!("VIRTIO_WIFI_CMD_GET_BAND_CHANNEL channel={channel_index:?} band_kind={band_kind}");
                    match self
                        .wiphy
                        .bands
                        .get(band_kind as usize)
                        .and_then(|b| b.freqs.get(channel_index as usize))
                    {
                        Some(channel) => {
                            log::debug!("VIRTIO_WIFI_CMD_GET_BAND_CHANNEL channel={channel_index:?} band_kind={band_kind} replying with {:?}",virtio_wifi::virtio_wifi_channel::from(channel));
                            writer
                                .write_obj(virtio_wifi::virtio_wifi_channel::from(channel))
                                .map_err(|_| Error::DescriptorWriteFailed)?;
                        }
                        None => {
                            log::debug!(
                                "VIRTIO_WIFI_CMD_GET_BAND_CHANNEL channel={channel_index:?} band_kind={band_kind} not found, have: bands={:?} freqs={:?} ",
                                self
                                .wiphy
                                .bands
                                .get(band_kind as usize),
                                self
                                .wiphy
                                .bands
                                .get(band_kind as usize)
                                .map(|b| &b.freqs)
                            );
                            writer
                                .write_obj(virtio_wifi::virtio_wifi_hdr {
                                    code: virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                                    index: 0.into(),
                                })
                                .map_err(|_| Error::DescriptorWriteFailed)?;
                        }
                    }
                }
                virtio_wifi::VIRTIO_WIFI_CMD_GET_BAND_BITRATE => {
                    let band_kind = u32::from(request.index);
                    let mut reader = desc_chain
                        .clone()
                        .reader(&mem)
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let request = reader
                        .read_obj::<virtio_wifi::virtio_wifi_cmd_get_band_bitrate_request>()
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let bitrate_index = u32::from(request.bitrate_index);
                    log::debug!("VIRTIO_WIFI_CMD_GET_BAND_BITRATE bitrate_index={bitrate_index:?} band_kind={band_kind}");
                    match self
                        .wiphy
                        .bands
                        .get(band_kind as usize)
                        .and_then(|b| b.rates.get(bitrate_index as usize))
                    {
                        Some(b) => {
                            log::debug!("VIRTIO_WIFI_CMD_GET_BAND_BITRATE bitrate_index={bitrate_index:?} band_kind={band_kind} replying with {:?}",virtio_wifi::virtio_wifi_band_bitrate::from(b));
                            writer
                                .write_obj(virtio_wifi::virtio_wifi_band_bitrate::from(b))
                                .map_err(|_| Error::DescriptorWriteFailed)?;
                        }
                        None => {
                            log::debug!("VIRTIO_WIFI_CMD_GET_BAND_BITRATE bitrate_index={bitrate_index:?} band_kind={band_kind} not found, have: bands={:?} bitrates={:?}",
                            self
                                .wiphy
                                .bands
                                .get(band_kind as usize),

                            self
                                .wiphy
                                .bands
                                .get(band_kind as usize)
                                .map(|b| &b.rates)
                                );
                            writer
                                .write_obj(virtio_wifi::virtio_wifi_hdr {
                                    code: virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                                    index: 0.into(),
                                })
                                .map_err(|_| Error::DescriptorWriteFailed)?;
                        }
                    }
                }
                virtio_wifi::VIRTIO_WIFI_CMD_NEW_INTERFACE => {
                    log::debug!("VIRTIO_WIFI_CMD_NEW_INTERFACE");
                    let mut reader = desc_chain
                        .clone()
                        .reader(&mem)
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let request = reader
                        .read_obj::<virtio_wifi::virtio_wifi_interface>()
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    log::debug!("VIRTIO_WIFI_CMD_NEW_INTERFACE request = {request:?}");
                    match self.socket.add_iface(&self.wiphy, request) {
                        Ok(mut iface) => {
                            self.io_data.add_ifsocket(
                                iface.index,
                                crate::io::IfSocket::new(iface.name.clone(), iface.index).unwrap(),
                            );
                            iface.response.hdr.code = virtio_wifi::VIRTIO_WIFI_S_OK.into();
                            writer
                                .write_obj(iface.response)
                                .map_err(|_| Error::DescriptorWriteFailed)?;
                            self.interfaces.insert(iface.index, iface);
                        }
                        Err(err) => {
                            log::error!("{err}");
                            let code = u32::from(err).into();
                            writer
                                .write_obj(virtio_wifi::virtio_wifi_hdr {
                                    code,
                                    index: request.hdr.index,
                                })
                                .map_err(|_| Error::DescriptorWriteFailed)?;
                        }
                    }
                }
                virtio_wifi::VIRTIO_WIFI_CMD_DELETE_INTERFACE => {
                    let mut reader = desc_chain
                        .clone()
                        .reader(&mem)
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let request = reader
                        .read_obj::<virtio_wifi::virtio_wifi_interface>()
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let iface_index = request.index.into();
                    log::debug!("VIRTIO_WIFI_CMD_DELETE_INTERFACE iface_index={iface_index} request = {request:?}");
                    let code = match self.socket.del_iface(&self.wiphy, iface_index) {
                        Ok(()) => {
                            self.io_data.del_ifsocket(iface_index);
                            self.interfaces.remove(&iface_index);
                            virtio_wifi::VIRTIO_WIFI_S_OK.into()
                        }
                        Err(err) => {
                            log::error!("{err}");
                            u32::from(err).into()
                        }
                    };
                    writer
                        .write_obj(virtio_wifi::virtio_wifi_hdr {
                            code,
                            index: request.index,
                        })
                        .map_err(|_| Error::DescriptorWriteFailed)?;
                }
                virtio_wifi::VIRTIO_WIFI_CMD_CHANGE_INTERFACE => {
                    let mut reader = desc_chain
                        .clone()
                        .reader(&mem)
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let request = reader
                        .read_obj::<virtio_wifi::virtio_wifi_interface>()
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let iface_index = request.index.into();
                    log::debug!("VIRTIO_WIFI_CMD_CHANGE_INTERFACE iface_index={iface_index} request = {request:?}");
                    let code = match self.socket.change_iface(&self.wiphy, request) {
                        Ok(mut iface) => {
                            self.interfaces.remove(&iface_index);
                            iface.response.hdr.code = virtio_wifi::VIRTIO_WIFI_S_OK.into();
                            self.io_data.update_notifier(
                                &self.io_data_status_sender,
                                self.socket.reload().unwrap(),
                            );

                            for ft in &self.wiphy.supported_rx_frames {
                                let result =
                                    self.socket
                                        .register_frame(iface.wdev, iface.index, *ft, &[]);
                                log::debug!("rx {ft:?}: {result:?}");
                            }
                            self.interfaces.insert(iface.index, iface);
                            virtio_wifi::VIRTIO_WIFI_S_OK.into()
                        }
                        Err(err) => {
                            log::error!("{err}");

                            u32::from(err).into()
                        }
                    };
                    writer
                        .write_obj(virtio_wifi::virtio_wifi_hdr {
                            code,
                            index: request.index,
                        })
                        .map_err(|_| Error::DescriptorWriteFailed)?;
                }
                virtio_wifi::VIRTIO_WIFI_CMD_TRIGGER_SCAN => {
                    let index = u32::from(request.index);
                    // log::debug!("VIRTIO_WIFI_CMD_TRIGGER_SCAN iface={index}");
                    if self.interfaces.contains_key(&index) {
                        let code = match self.socket.trigger_scan(index) {
                            Ok(()) => virtio_wifi::VIRTIO_WIFI_S_OK.into(),
                            Err(err) => {
                                log::error!("trigger scan error: {err}");
                                u32::from(err).into()
                            }
                        };
                        writer
                            .write_obj(virtio_wifi::virtio_wifi_hdr {
                                code,
                                index: request.index,
                            })
                            .map_err(|_| Error::DescriptorWriteFailed)?;
                    } else {
                        writer
                            .write_obj(virtio_wifi::virtio_wifi_hdr {
                                code: virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                                index: request.index,
                            })
                            .map_err(|_| Error::DescriptorWriteFailed)?;
                    }
                }
                virtio_wifi::VIRTIO_WIFI_CMD_GET_CIPHERSUITES => {
                    let index = u32::from(request.index);
                    log::debug!("get cipher suites: {index:?}");
                    writer
                        .write_obj(virtio_wifi::virtio_wifi_hdr {
                            code: virtio_wifi::VIRTIO_WIFI_S_OK.into(),
                            index: index.into(),
                        })
                        .map_err(|_| Error::DescriptorWriteFailed)?;
                    log::debug!("writing ciphers: {:?}", self.wiphy.cipher_suites);
                    for c in &self.wiphy.cipher_suites {
                        writer
                            .write_obj(*c)
                            .map_err(|_| Error::DescriptorWriteFailed)?;
                    }
                    log::debug!("wrote cipher suites: {:?}", self.wiphy.cipher_suites);
                }
                virtio_wifi::VIRTIO_WIFI_CMD_AUTHENTICATE => {
                    let index = u32::from(request.index);
                    let mut reader = desc_chain
                        .clone()
                        .reader(&mem)
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let request = reader
                        .read_obj::<virtio_wifi::virtio_wifi_authentication_request>()
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let code = if !self.wiphy.auth_supported() {
                        virtio_wifi::VIRTIO_WIFI_S_NOT_SUPP.into()
                    } else {
                        match self.socket.authenticate(index, &request, reader) {
                            Ok(()) => virtio_wifi::VIRTIO_WIFI_S_OK.into(),
                            Err(err) => {
                                log::error!("authenticate error: {err}");
                                u32::from(err).into()
                            }
                        }
                    };
                    log::debug!(
                        "VIRTIO_WIFI_CMD_AUTHENTICATE req = {request:?} response = {code:?}"
                    );
                    writer
                        .write_obj(virtio_wifi::virtio_wifi_hdr {
                            code,
                            index: index.into(),
                        })
                        .map_err(|_| Error::DescriptorWriteFailed)?;
                }
                virtio_wifi::VIRTIO_WIFI_CMD_CONNECT => {
                    let index = u32::from(request.index);
                    let mut reader = desc_chain
                        .clone()
                        .reader(&mem)
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let request = reader
                        .read_obj::<virtio_wifi::virtio_wifi_authentication_request>()
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let code = if !self.wiphy.connect_supported() {
                        virtio_wifi::VIRTIO_WIFI_S_NOT_SUPP.into()
                    } else {
                        match self.socket.connect(&self.wiphy, index, &request, reader) {
                            Ok(()) => virtio_wifi::VIRTIO_WIFI_S_OK.into(),
                            Err(err) => {
                                log::error!("connect error: {err}");
                                u32::from(err).into()
                            }
                        }
                    };
                    log::debug!("VIRTIO_WIFI_CMD_CONNECT req = {request:?} response = {code:?}");
                    writer
                        .write_obj(virtio_wifi::virtio_wifi_hdr {
                            code,
                            index: index.into(),
                        })
                        .map_err(|_| Error::DescriptorWriteFailed)?;
                }
                virtio_wifi::VIRTIO_WIFI_CMD_ASSOCIATE => {
                    let index = u32::from(request.index);
                    let mut reader = desc_chain
                        .clone()
                        .reader(&mem)
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let request = reader
                        .read_obj::<virtio_wifi::virtio_wifi_association_request>()
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    log::debug!("VIRTIO_WIFI_CMD_ASSOCIATE req = {request:?}");
                    if self.interfaces.contains_key(&index) {
                        let code = match self.socket.associate(index, &request, reader) {
                            Ok(()) => virtio_wifi::VIRTIO_WIFI_S_OK.into(),
                            Err(err) => {
                                log::error!("associate error: {err}");
                                u32::from(err).into()
                            }
                        };
                        writer
                            .write_obj(virtio_wifi::virtio_wifi_hdr {
                                code,
                                index: index.into(),
                            })
                            .map_err(|_| Error::DescriptorWriteFailed)?;
                    } else {
                        writer
                            .write_obj(virtio_wifi::virtio_wifi_hdr {
                                code: virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                                index: index.into(),
                            })
                            .map_err(|_| Error::DescriptorWriteFailed)?;
                    }
                }
                virtio_wifi::VIRTIO_WIFI_CMD_ADD_KEY => {
                    let index = u32::from(request.index);
                    let mut reader = desc_chain
                        .clone()
                        .reader(&mem)
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let request = reader
                        .read_obj::<virtio_wifi::virtio_wifi_key>()
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    log::debug!("VIRTIO_WIFI_CMD_ADD_KEY req = {request:?}");
                    if self.interfaces.contains_key(&index) {
                        let code = match self.socket.add_key(index, &request, reader) {
                            Ok(()) => virtio_wifi::VIRTIO_WIFI_S_OK.into(),
                            Err(err) => {
                                log::error!("add_key error: {err}");
                                u32::from(err).into()
                            }
                        };
                        writer
                            .write_obj(virtio_wifi::virtio_wifi_hdr {
                                code,
                                index: index.into(),
                            })
                            .map_err(|_| Error::DescriptorWriteFailed)?;
                    } else {
                        writer
                            .write_obj(virtio_wifi::virtio_wifi_hdr {
                                code: virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                                index: index.into(),
                            })
                            .map_err(|_| Error::DescriptorWriteFailed)?;
                    }
                }
                virtio_wifi::VIRTIO_WIFI_CMD_DEL_KEY => {
                    let index = u32::from(request.index);
                    let mut reader = desc_chain
                        .clone()
                        .reader(&mem)
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let request = reader
                        .read_obj::<virtio_wifi::virtio_wifi_key>()
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    log::debug!("VIRTIO_WIFI_CMD_DEL_KEY req = {request:?}");
                    if self.interfaces.contains_key(&index) {
                        let code = match self.socket.del_key(index, &request) {
                            Ok(()) => virtio_wifi::VIRTIO_WIFI_S_OK.into(),
                            Err(err) => {
                                log::error!("del_key error: {err}");
                                u32::from(err).into()
                            }
                        };
                        writer
                            .write_obj(virtio_wifi::virtio_wifi_hdr {
                                code,
                                index: index.into(),
                            })
                            .map_err(|_| Error::DescriptorWriteFailed)?;
                    } else {
                        writer
                            .write_obj(virtio_wifi::virtio_wifi_hdr {
                                code: virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                                index: index.into(),
                            })
                            .map_err(|_| Error::DescriptorWriteFailed)?;
                    }
                }
                virtio_wifi::VIRTIO_WIFI_CMD_SET_KEY => {
                    let index = u32::from(request.index);
                    let mut reader = desc_chain
                        .clone()
                        .reader(&mem)
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let request = reader
                        .read_obj::<virtio_wifi::virtio_wifi_key>()
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    log::debug!("VIRTIO_WIFI_CMD_SET_KEY req = {request:?}");
                    if self.interfaces.contains_key(&index) {
                        let code = match self.socket.set_key(index, &request) {
                            Ok(()) => virtio_wifi::VIRTIO_WIFI_S_OK.into(),
                            Err(err) => {
                                log::error!("set_key error: {err}");
                                u32::from(err).into()
                            }
                        };
                        writer
                            .write_obj(virtio_wifi::virtio_wifi_hdr {
                                code,
                                index: index.into(),
                            })
                            .map_err(|_| Error::DescriptorWriteFailed)?;
                    } else {
                        writer
                            .write_obj(virtio_wifi::virtio_wifi_hdr {
                                code: virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                                index: index.into(),
                            })
                            .map_err(|_| Error::DescriptorWriteFailed)?;
                    }
                }
                virtio_wifi::VIRTIO_WIFI_CMD_GET_KEY => {
                    let index = u32::from(request.index);
                    let mut reader = desc_chain
                        .clone()
                        .reader(&mem)
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let request = reader
                        .read_obj::<virtio_wifi::virtio_wifi_key>()
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    log::debug!("VIRTIO_WIFI_CMD_GET_KEY req = {request:?}");
                    if self.interfaces.contains_key(&index) {
                        match self.socket.get_key(index, &request) {
                            Ok(key) => {
                                log::info!("get_key got: {key:?}");
                                writer
                                    .write_obj(key)
                                    .map_err(|_| Error::DescriptorWriteFailed)?;
                            }
                            Err(err) => {
                                log::error!("get_key error: {err}");

                                writer
                                    .write_obj(virtio_wifi::virtio_wifi_hdr {
                                        code: u32::from(err).into(),
                                        index: index.into(),
                                    })
                                    .map_err(|_| Error::DescriptorWriteFailed)?;
                            }
                        };
                    } else {
                        writer
                            .write_obj(virtio_wifi::virtio_wifi_hdr {
                                code: virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                                index: index.into(),
                            })
                            .map_err(|_| Error::DescriptorWriteFailed)?;
                    }
                }
                virtio_wifi::VIRTIO_WIFI_CMD_SET_STATION => {
                    let index = u32::from(request.index);
                    let mut reader = desc_chain
                        .clone()
                        .reader(&mem)
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let request = reader
                        .read_obj::<virtio_wifi::virtio_wifi_change_station>()
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    log::debug!("VIRTIO_WIFI_CMD_SET_STATION req = {request:?}");
                    if self.interfaces.contains_key(&index) {
                        let code = match self.socket.set_station(index, &request) {
                            Ok(()) => virtio_wifi::VIRTIO_WIFI_S_OK.into(),
                            Err(err) => {
                                log::error!("set_station error: {err}");
                                u32::from(err).into()
                            }
                        };
                        writer
                            .write_obj(virtio_wifi::virtio_wifi_hdr {
                                code,
                                index: index.into(),
                            })
                            .map_err(|_| Error::DescriptorWriteFailed)?;
                    } else {
                        writer
                            .write_obj(virtio_wifi::virtio_wifi_hdr {
                                code: virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                                index: index.into(),
                            })
                            .map_err(|_| Error::DescriptorWriteFailed)?;
                    }
                }
                virtio_wifi::VIRTIO_WIFI_CMD_DEAUTHENTICATE
                | virtio_wifi::VIRTIO_WIFI_CMD_DISASSOCIATE => {
                    let index = u32::from(request.index);
                    let mut reader = desc_chain
                        .clone()
                        .reader(&mem)
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let request = reader
                        .read_obj::<virtio_wifi::virtio_wifi_disconnect_request>()
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    if self.interfaces.contains_key(&index) {
                        let code = match self.socket.disconnect(index, &request, reader) {
                            Ok(()) => virtio_wifi::VIRTIO_WIFI_S_OK.into(),
                            Err(err) => {
                                log::error!("disconnect error: {err}");
                                u32::from(err).into()
                            }
                        };
                        writer
                            .write_obj(virtio_wifi::virtio_wifi_hdr {
                                code,
                                index: index.into(),
                            })
                            .map_err(|_| Error::DescriptorWriteFailed)?;
                    } else {
                        writer
                            .write_obj(virtio_wifi::virtio_wifi_hdr {
                                code: virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                                index: index.into(),
                            })
                            .map_err(|_| Error::DescriptorWriteFailed)?;
                    }
                }
                virtio_wifi::VIRTIO_WIFI_CMD_IF_OPERSTATE => {
                    let index = u32::from(request.index);
                    let mut reader = desc_chain
                        .clone()
                        .reader(&mem)
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let request = reader
                        .read_obj::<virtio_wifi::virtio_wifi_set_if_operstate>()
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    if self.interfaces.contains_key(&index) {
                        let code = match self.socket.if_operstate(index, &request) {
                            Ok(()) => virtio_wifi::VIRTIO_WIFI_S_OK.into(),
                            Err(err) => {
                                log::error!("set if operstate error: {err}");
                                virtio_wifi::VIRTIO_WIFI_S_IO_ERR.into()
                            }
                        };
                        writer
                            .write_obj(virtio_wifi::virtio_wifi_hdr {
                                code,
                                index: index.into(),
                            })
                            .map_err(|_| Error::DescriptorWriteFailed)?;
                    } else {
                        writer
                            .write_obj(virtio_wifi::virtio_wifi_hdr {
                                code: virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                                index: index.into(),
                            })
                            .map_err(|_| Error::DescriptorWriteFailed)?;
                    }
                }
                virtio_wifi::VIRTIO_WIFI_CMD_START_AP => {
                    let index = u32::from(request.index);
                    let mut reader = desc_chain
                        .clone()
                        .reader(&mem)
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    log::debug!("start_ap req_size={}", reader.available_bytes());
                    let request = reader
                        .read_obj::<virtio_wifi::virtio_wifi_beacon>()
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    log::debug!("start_ap tlv bytes={}", reader.available_bytes());
                    if self.interfaces.contains_key(&index) {
                        let code = match self.socket.start_ap(index, &request, reader) {
                            Ok(()) => virtio_wifi::VIRTIO_WIFI_S_OK.into(),
                            Err(err) => {
                                log::error!("start ap error: {err}");
                                u32::from(err).into()
                            }
                        };
                        writer
                            .write_obj(virtio_wifi::virtio_wifi_hdr {
                                code,
                                index: index.into(),
                            })
                            .map_err(|_| Error::DescriptorWriteFailed)?;
                    } else {
                        writer
                            .write_obj(virtio_wifi::virtio_wifi_hdr {
                                code: virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into(),
                                index: index.into(),
                            })
                            .map_err(|_| Error::DescriptorWriteFailed)?;
                    }
                }
                virtio_wifi::VIRTIO_WIFI_CMD_REGISTER_FRAME => {
                    let index = u32::from(request.index);
                    let mut reader = desc_chain
                        .clone()
                        .reader(&mem)
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    let request = reader
                        .read_obj::<virtio_wifi::virtio_wifi_register_frame>()
                        .map_err(|_| Error::DescriptorReadFailed)?;
                    log::debug!("VIRTIO_WIFI_CMD_REGISTER_FRAME req = {request:?}");
                    let code = if !self.wiphy.register_frame_supported() {
                        virtio_wifi::VIRTIO_WIFI_S_NOT_SUPP.into()
                    } else {
                        let ft = FrameType {
                            ftype: request.ftype.into(),
                            stype: request.stype.into(),
                        };
                        log::debug!("VIRTIO_WIFI_CMD_REGISTER_FRAME ft = {ft:?}");
                        match self
                            .interfaces
                            .get(&index)
                            .ok_or_else(|| crate::nl80211::BadMessageError::NotFound.into())
                            .and_then(|iface| {
                                self.socket.register_frame(iface.wdev, index, ft, &[])
                            }) {
                            Ok(()) => virtio_wifi::VIRTIO_WIFI_S_OK.into(),
                            Err(err) => {
                                log::error!("register frame error: {err}");
                                u32::from(err).into()
                            }
                        }
                    };
                    log::debug!(
                        "VIRTIO_WIFI_CMD_REGISTER_FRAME req = {request:?} response = {code:?}"
                    );
                    writer
                        .write_obj(virtio_wifi::virtio_wifi_hdr {
                            code,
                            index: index.into(),
                        })
                        .map_err(|_| Error::DescriptorWriteFailed)?;
                }
                _ => {
                    writer
                        .write_obj(virtio_wifi::virtio_wifi_hdr {
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
                .read_obj::<virtio_wifi::virtio_wifi_hdr>()
                .map_err(|_| Error::DescriptorReadFailed)?;
            let mut writer = desc_chain
                .clone()
                .writer(&mem)
                .map_err(|_| Error::DescriptorWriteFailed)?;

            let mut used_len = 0;

            let request_kind: u32 = request.code.into();
            if request_kind != virtio_wifi::VIRTIO_WIFI_802_3 {
                let code = if request_kind == virtio_wifi::VIRTIO_WIFI_802_11 {
                    log::error!("Driver submitted 802.11 frame to txq but this implementation does not support 802.11 packet injection");
                    virtio_wifi::VIRTIO_WIFI_S_NOT_SUPP.into()
                } else {
                    log::error!("Driver submitted invalid code {request_kind} to txq");
                    virtio_wifi::VIRTIO_WIFI_S_BAD_MSG.into()
                };
                writer
                    .write_obj(virtio_wifi::virtio_wifi_hdr {
                        code,
                        index: request.index,
                    })
                    .map_err(|_| Error::DescriptorWriteFailed)?;

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
                continue;
            }

            let if_index: u32 = request.index.into();
            let available_bytes = reader.available_bytes();
            let mut buffer = vec![0; available_bytes];

            reader
                .read_exact(&mut buffer)
                .map_err(|_| Error::DescriptorReadFailed)?;

            // if log::log_enabled!(log::Level::Trace) {
            //     match etherparse::PacketHeaders::from_ethernet_slice(&buffer) {
            //         Err(err) => log::trace!("Error during parsing TX:\n{err:?}"),
            //         Ok(frame) => {
            //             log::trace!("Got TX 802.3 frame: {frame:?}");
            //         }
            //     }
            // }

            match self.io_data.ifsockets.lock().unwrap().get(&if_index) {
                Some(ifsocket) => {
                    let code = if let Err(err) = ifsocket.write_all(&buffer) {
                        log::error!("txq error: {err}");
                        virtio_wifi::VIRTIO_WIFI_S_IO_ERR.into()
                    } else {
                        log::debug!("txq wrote: {} bytes", buffer.len());
                        virtio_wifi::VIRTIO_WIFI_S_OK.into()
                    };
                    writer
                        .write_obj(virtio_wifi::virtio_wifi_hdr {
                            code,
                            index: request.index,
                        })
                        .map_err(|_| Error::DescriptorWriteFailed)?;
                }
                None => {
                    log::error!("txq error driver requested if_index={if_index} which doesn't exist, we have: {:?}", self.io_data.ifsockets.lock().unwrap().keys().collect::<Vec<_>>());
                    writer
                        .write_obj(virtio_wifi::virtio_wifi_hdr {
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
            | u64::from(
                u32::from(self.wiphy.auth_supported()) * virtio_wifi::VIRTIO_WIFI_F_AUTHENTICATE,
            )
            | u64::from(
                u32::from(self.wiphy.connect_supported()) * virtio_wifi::VIRTIO_WIFI_F_CONNECT,
            )
            | u64::from(
                u32::from(self.wiphy.set_interface_supported())
                    * virtio_wifi::VIRTIO_WIFI_F_CHANGE_INTERFACE,
            )
            | u64::from(
                u32::from(self.wiphy.start_ap_supported()) * virtio_wifi::VIRTIO_WIFI_F_START_AP,
            )
            | u64::from(
                u32::from(self.wiphy.register_frame_supported())
                    * virtio_wifi::VIRTIO_WIFI_F_REGISTER_FRAME,
            )
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
        log::trace!("offset={offset} size={size} actual size = {}", buf.len());

        if offset + size > buf.len() {
            return Vec::new();
        }

        buf[offset..][..size].to_vec()
    }

    fn set_event_idx(&mut self, enabled: bool) {
        self.event_idx = enabled;
    }

    fn update_memory(&mut self, mem: GuestMemoryAtomic<GuestMemoryMmap>) -> IoResult<()> {
        self.io_data
            .update_memory(&self.io_data_status_sender, mem.clone());
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
                                    if let Err(err) = self.$cb(vring) {
                                        log::error!("device event {device_event}: {err}");
                                        return Err(err.into());
                                    }

                                    // Do not enable notifications for RX queue.
                                    // IO thread in `io` module will request buffers on demand
                                    // when it runs out of them.
                                    if matches!(
                                        virtio_wifi::$queue_idx,
                                        virtio_wifi::VIRTIO_WIFI_VQ_RX
                                    ) || !vring.enable_notification().unwrap() {
                                            break;
                                    }
                                }
                            } else {
                                // Without EVENT_IDX, a single call is enough.
                                if let Err(err) = self.$cb(vring) {
                                    log::error!("device event {device_event}: {err}");
                                    return Err(err.into());
                                }
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
