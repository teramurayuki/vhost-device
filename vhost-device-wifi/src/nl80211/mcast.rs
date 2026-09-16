// SPDX-License-Identifier: Apache-2.0 or BSD-3-Clause
// Copyright 2026 Panasonic Automotive Systems Co., Ltd.
// Author: Manos Pitsidianakis <manos.pitsidianakis@linaro.org>

//! Multicast notification processing

use std::{
    sync::atomic::{AtomicBool, Ordering},
    sync::Arc, //
};

use neli::{
    attr::Attribute,
    consts::nl::{GenlId, NlmF},
    genl::{AttrTypeBuilder, Genlmsghdr, GenlmsghdrBuilder, NlattrBuilder, NoUserHeader},
    nl::{NlPayload, Nlmsghdr},
    router::synchronous::{NlRouter, NlRouterReceiverHandle},
    types::GenlBuffer,
};
use nix::sys::eventfd::EventFd;
use virtio_wifi::{
    virtio_wifi_association_response, virtio_wifi_connection_response,
    virtio_wifi_disconnection_response, virtio_wifi_event,
    virtio_wifi_event_code::{
        VIRTIO_WIFI_EVENT_CODE_ASSOCIATE, VIRTIO_WIFI_EVENT_CODE_ASSOCIATE_TIMED_OUT,
        VIRTIO_WIFI_EVENT_CODE_AUTHENTICATE, VIRTIO_WIFI_EVENT_CODE_AUTHENTICATE_TIMED_OUT,
        VIRTIO_WIFI_EVENT_CODE_CANCEL_SCAN, VIRTIO_WIFI_EVENT_CODE_CONNECT,
        VIRTIO_WIFI_EVENT_CODE_CONNECT_TIMED_OUT, VIRTIO_WIFI_EVENT_CODE_DEAUTHENTICATE,
        VIRTIO_WIFI_EVENT_CODE_DELETE_STATION, VIRTIO_WIFI_EVENT_CODE_DISASSOCIATE,
        VIRTIO_WIFI_EVENT_CODE_DISCONNECT, VIRTIO_WIFI_EVENT_CODE_END_SCAN,
        VIRTIO_WIFI_EVENT_CODE_NEW_STATION, VIRTIO_WIFI_EVENT_CODE_SCAN_RESULT,
    },
    virtio_wifi_scan_result, virtio_wifi_station_change,
};
use vm_memory::ByteValued;

use super::{Bss, Nl80211Attribute, Nl80211Bss, Nl80211Command};
use crate::{bindings::virtio_wifi_uapi as virtio_wifi, io::RxWork};

#[neli::neli_enum(serialized_type = "u16")]
pub enum Nl80211StaWmeAttr {
    Invalid = 0,
    UapsdQueues = 1,
    MaxSp = 2,
}

impl neli::consts::genl::NlAttrType for Nl80211StaWmeAttr {}

type NlMessage = Nlmsghdr<GenlId, Genlmsghdr<Nl80211Command, Nl80211Attribute>>;

pub struct McastListener {
    pub multicast_receiver: NlRouterReceiverHandle<u16, Genlmsghdr<u8, u16, NoUserHeader>>,
    pub sock: Arc<NlRouter>,
    /// Signal that we have submitted an event to the eventq
    pub event_fd: Arc<EventFd>,
    /// Signal that we have submitted a management frame to the rxq
    pub rx_event_fd: Arc<EventFd>,
    /// Set to `true` by io thread worker
    pub exit: Arc<AtomicBool>,
    /// Channel for sending event payloads to eventq
    pub event_sender: std::sync::mpsc::Sender<Vec<u8>>,
    /// Channel for sending management frames to rxq
    pub rx_sender: std::sync::mpsc::Sender<RxWork<'static>>,
    pub family_id: u16,
}

impl McastListener {
    /// Listen for mcast events, and handle those we care about
    pub fn listen(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        while let Some(msg) =
            self.multicast_receiver
                .next_typed::<GenlId, Genlmsghdr<Nl80211Command, Nl80211Attribute, NoUserHeader>>()
        {
            let msg = match msg {
                Ok(msg) => msg,
                Err(err) => {
                    log::error!("mcast: {err}");
                    break;
                }
            };
            let NlPayload::Payload(payload) = msg.nl_payload() else {
                log::error!("mcast: No payload in msg = {msg:?}");
                continue;
            };
            if self.exit.load(Ordering::SeqCst) {
                log::warn!("exiting mcast thread");
                return Ok(());
            }
            match payload.cmd() {
                Nl80211Command::NewScanResults if *msg.nl_pid() == 0 => {
                    let index = payload
                        .attrs()
                        .get_attr_handle()
                        .iter()
                        .find(|a| matches!(a.nla_type().nla_type(), Nl80211Attribute::IfIndex))
                        .unwrap()
                        .get_payload_as::<u32>()
                        .unwrap();

                    self.get_scan(index)?;
                }
                Nl80211Command::Frame => {
                    log::error!("Nl80211Command::Frame");
                    let attr_handle = payload.attrs().get_attr_handle();
                    let data = attr_handle
                        .get_attr_payload_as_with_len::<Vec<u8>>(Nl80211Attribute::Frame)?
                        .into();
                    let iface =
                        attr_handle.get_attr_payload_as::<u32>(Nl80211Attribute::IfIndex)?;
                    let attr_handles = attr_handle.iter().collect::<Vec<_>>();
                    log::error!("Nl80211Command::Frame attrs: {:?}", &attr_handles);
                    self.rx_sender
                        .send(RxWork {
                            iface,
                            is_802_11: true,
                            data,
                        })
                        .unwrap();
                    self.rx_event_fd.write(1).unwrap();
                }
                Nl80211Command::NewInterface => {
                    // Ignore
                    let attr_handle = payload.attrs().get_attr_handle();
                    let attr_handles = attr_handle.iter().collect::<Vec<_>>();
                    log::error!("mcast: {:?} attrs: {:?}", payload.cmd(), &attr_handles);
                    // TODO: send guest NEW_IF event?
                }
                Nl80211Command::DelInterface => {
                    // Ignore
                    let attr_handle = payload.attrs().get_attr_handle();
                    let attr_handles = attr_handle.iter().collect::<Vec<_>>();
                    log::error!("mcast: {:?} attrs: {:?}", payload.cmd(), &attr_handles);
                    // TODO: send guest DEL_IF event?
                }
                Nl80211Command::TriggerScan => {
                    // Ignore
                }
                Nl80211Command::ScanAborted => {
                    let ifindex = payload
                        .attrs()
                        .get_attr_handle()
                        .iter()
                        .find(|a| matches!(a.nla_type().nla_type(), Nl80211Attribute::IfIndex))
                        .unwrap()
                        .get_payload_as::<u32>()
                        .unwrap();
                    let event = virtio_wifi_event {
                        code: (VIRTIO_WIFI_EVENT_CODE_CANCEL_SCAN as u32).into(),
                        ifindex: ifindex.into(),
                    }
                    .as_slice()
                    .to_vec();
                    self.send_event(event);
                }
                Nl80211Command::NewStation => {
                    let attr_handle = payload.attrs().get_attr_handle();
                    let attr_handles = attr_handle.iter().collect::<Vec<_>>();
                    log::debug!("NewStation attrs: {:?}", &attr_handles);
                    self.get_station(payload, true)?;
                }
                Nl80211Command::DelStation => {
                    let attr_handle = payload.attrs().get_attr_handle();
                    let attr_handles = attr_handle.iter().collect::<Vec<_>>();
                    log::debug!("DelStation attrs: {:?}", &attr_handles);
                    self.get_station(payload, false)?;
                }
                Nl80211Command::Authenticate => self.get_authenticate(payload)?,
                Nl80211Command::Deauthenticate => self.get_deauthenticate(payload)?,
                Nl80211Command::Associate => self.get_associate(payload)?,
                Nl80211Command::Disassociate => self.get_disassociate(payload)?,
                Nl80211Command::Connect => self.get_connect(payload)?,
                Nl80211Command::Disconnect => self.get_disconnect(payload)?,
                Nl80211Command::StartAp | Nl80211Command::StopAp => {
                    // Ignore
                }
                _other => {
                    let attr_handle = payload.attrs().get_attr_handle();
                    let attr_handles = attr_handle.iter().collect::<Vec<_>>();
                    log::error!("mcast: {:?} attrs: {:?}", payload.cmd(), &attr_handles);
                }
            }
        }
        Ok(())
    }

    #[inline(always)]
    fn send_event(&self, result: Vec<u8>) {
        if self.event_sender.send(result).is_err() {
            return;
        }
        _ = self.event_fd.write(1);
    }

    /// Query nl80211 for scan results and send results back to the guest.
    fn get_scan(&mut self, index: u32) -> Result<(), Box<dyn std::error::Error>> {
        let attrs = vec![NlattrBuilder::default()
            .nla_type(
                AttrTypeBuilder::default()
                    .nla_type(Nl80211Attribute::IfIndex)
                    .build()
                    .unwrap(),
            )
            .nla_payload(index)
            .build()
            .unwrap()]
        .into_iter()
        .collect::<GenlBuffer<_, _>>();

        let recv = self
            .sock
            .send(
                self.family_id,
                NlmF::REQUEST | NlmF::ACK | NlmF::REPLACE | NlmF::ROOT | NlmF::MATCH,
                NlPayload::Payload(
                    GenlmsghdrBuilder::<Nl80211Command, Nl80211Attribute, NoUserHeader>::default()
                        .cmd(Nl80211Command::GetScan)
                        .attrs(attrs)
                        .version(1)
                        .build()
                        .unwrap(),
                ),
            )
            .unwrap();
        // for each ssid, inform with a VIRTIO_WIFI_EVENT_CODE_SCAN_RESULT
        let mut _ifindex = None;
        for msg in recv {
            let msg: NlMessage = msg?;
            let NlPayload::Payload(payload) = msg.nl_payload() else {
                continue;
            };
            let attr_handle = payload.attrs().get_attr_handle();
            let mut attr_handles = attr_handle.iter().collect::<Vec<_>>();
            let ifindex = attr_handles
                .extract_if(.., |attr| {
                    matches!(attr.nla_type().nla_type(), Nl80211Attribute::IfIndex)
                })
                .next()
                .and_then(|t| t.get_payload_as::<u32>().ok())
                .unwrap();
            _ifindex = Some(ifindex);
            let _wdev = attr_handles
                .extract_if(.., |attr| {
                    matches!(attr.nla_type().nla_type(), Nl80211Attribute::WDev)
                })
                .next()
                .and_then(|t| t.get_payload_as::<u32>().ok())
                .unwrap();
            let bss: Bss = attr_handle
                .get_nested_attributes::<Nl80211Bss>(Nl80211Attribute::Bss)
                .unwrap()
                .try_into()
                .unwrap();
            let mut event = virtio_wifi_scan_result {
                hdr: virtio_wifi_event {
                    code: (VIRTIO_WIFI_EVENT_CODE_SCAN_RESULT as u32).into(),
                    ifindex: ifindex.into(),
                },
                ielen: u64::try_from(bss.information_elements.len())
                    .unwrap()
                    .into(),
                ..virtio_wifi_scan_result::from(&bss)
            }
            .as_slice()
            .to_vec();
            event.extend(&bss.information_elements);
            self.send_event(event);
        }
        self.send_event(
            virtio_wifi_event {
                code: (VIRTIO_WIFI_EVENT_CODE_END_SCAN as u32).into(),
                ifindex: _ifindex.unwrap().into(),
            }
            .as_slice()
            .to_vec(),
        );
        Ok(())
    }

    /// Send authenticate event back to guest.
    fn get_authenticate(
        &self,
        payload: &neli::genl::Genlmsghdr<Nl80211Command, Nl80211Attribute, NoUserHeader>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let attr_handle = payload.attrs().get_attr_handle();
        let ifindex = attr_handle.get_attr_payload_as::<u32>(Nl80211Attribute::IfIndex)?;
        if let Ok(frame) =
            attr_handle.get_attr_payload_as_with_len::<Vec<u8>>(Nl80211Attribute::Frame)
        {
            let mut event = virtio_wifi_event {
                code: (VIRTIO_WIFI_EVENT_CODE_AUTHENTICATE as u32).into(),
                ifindex: ifindex.into(),
            }
            .as_slice()
            .to_vec();
            event.extend(&frame);
            self.send_event(event);
            log::debug!("sending auth event");
        } else if let Ok(()) = attr_handle.get_attr_payload_as::<()>(Nl80211Attribute::TimedOut) {
            log::debug!("sending auth cancelled");
            let event = virtio_wifi_event {
                code: (VIRTIO_WIFI_EVENT_CODE_AUTHENTICATE_TIMED_OUT as u32).into(),
                ifindex: ifindex.into(),
            }
            .as_slice()
            .to_vec();
            self.send_event(event);
        } else {
            // Ignore
            log::warn!(
                "Did not find frame attr or timedout attr in auth event, found these attributes: {:?}",
                &attr_handle.iter().collect::<Vec<_>>()
            );
        }
        Ok(())
    }

    /// Send deauthenticate event back to guest.
    fn get_deauthenticate(
        &self,
        payload: &neli::genl::Genlmsghdr<Nl80211Command, Nl80211Attribute, NoUserHeader>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let attr_handle = payload.attrs().get_attr_handle();
        let _wiphy = attr_handle.get_attr_payload_as::<u32>(Nl80211Attribute::Wiphy)?;
        let ifindex = attr_handle.get_attr_payload_as::<u32>(Nl80211Attribute::IfIndex)?;
        if let Ok(frame) =
            attr_handle.get_attr_payload_as_with_len::<Vec<u8>>(Nl80211Attribute::Frame)
        {
            let mut event = virtio_wifi_event {
                code: (VIRTIO_WIFI_EVENT_CODE_DEAUTHENTICATE as u32).into(),
                ifindex: ifindex.into(),
            }
            .as_slice()
            .to_vec();
            event.extend(&frame);
            self.send_event(event);
            log::debug!("sending deauth event");
        } else {
            // Ignore
            log::warn!(
                "Did not find frame attr in deauth event, found these attributes: {:?}",
                &attr_handle.iter().collect::<Vec<_>>()
            );
        }

        Ok(())
    }

    /// Send associate event back to guest.
    fn get_associate(
        &self,
        payload: &neli::genl::Genlmsghdr<Nl80211Command, Nl80211Attribute, NoUserHeader>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let attr_handle = payload.attrs().get_attr_handle();
        let ifindex = attr_handle.get_attr_payload_as::<u32>(Nl80211Attribute::IfIndex)?;
        let mut uapsd_queues = 0;
        if let Ok(sta_wme) =
            attr_handle.get_nested_attributes::<Nl80211StaWmeAttr>(Nl80211Attribute::StaWme)
        {
            if let Ok(v) = sta_wme.get_attr_payload_as::<u8>(Nl80211StaWmeAttr::UapsdQueues) {
                uapsd_queues = v.into();
            }
        }
        if let Ok(frame) =
            attr_handle.get_attr_payload_as_with_len::<Vec<u8>>(Nl80211Attribute::Frame)
        {
            let req_ies = attr_handle
                .get_attr_payload_as_with_len::<Vec<u8>>(Nl80211Attribute::ReqIe)
                .ok();
            let mut event = virtio_wifi_association_response {
                hdr: virtio_wifi_event {
                    code: (VIRTIO_WIFI_EVENT_CODE_ASSOCIATE as u32).into(),
                    ifindex: ifindex.into(),
                },
                frame_len: (frame.len() as u64).into(),
                req_ies_len: req_ies.as_ref().map(|r| r.len() as u64).unwrap_or(0).into(),
                uapsd_queues: uapsd_queues.into(),
                padding0: 0.into(),
            }
            .as_slice()
            .to_vec();
            event.extend(&frame);
            if let Some(req_ies) = req_ies {
                event.extend(&req_ies);
            }
            self.send_event(event);
            log::debug!("sending assoc event");
        } else if let Ok(()) = attr_handle.get_attr_payload_as::<()>(Nl80211Attribute::TimedOut) {
            log::debug!("sending assoc cancelled");
            let event = virtio_wifi_event {
                code: (VIRTIO_WIFI_EVENT_CODE_ASSOCIATE_TIMED_OUT as u32).into(),
                ifindex: ifindex.into(),
            }
            .as_slice()
            .to_vec();
            self.send_event(event);
        } else {
            // Ignore
            log::warn!(
                "Did not find frame attr or timed out attr in assoc event, found these attributes: {:?}",
                &attr_handle.iter().collect::<Vec<_>>()
            );
        }
        Ok(())
    }

    /// Send disassociate event back to guest.
    fn get_disassociate(
        &self,
        payload: &neli::genl::Genlmsghdr<Nl80211Command, Nl80211Attribute, NoUserHeader>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let attr_handle = payload.attrs().get_attr_handle();
        let _wiphy = attr_handle.get_attr_payload_as::<u32>(Nl80211Attribute::Wiphy)?;
        let ifindex = attr_handle.get_attr_payload_as::<u32>(Nl80211Attribute::IfIndex)?;
        let mut uapsd_queues = 0;
        if let Ok(sta_wme) =
            attr_handle.get_nested_attributes::<Nl80211StaWmeAttr>(Nl80211Attribute::StaWme)
        {
            if let Ok(v) = sta_wme.get_attr_payload_as::<u8>(Nl80211StaWmeAttr::UapsdQueues) {
                uapsd_queues = v.into();
            }
        }
        if let Ok(frame) =
            attr_handle.get_attr_payload_as_with_len::<Vec<u8>>(Nl80211Attribute::Frame)
        {
            let req_ies = attr_handle
                .get_attr_payload_as_with_len::<Vec<u8>>(Nl80211Attribute::ReqIe)
                .ok();
            let mut event = virtio_wifi_association_response {
                hdr: virtio_wifi_event {
                    code: (VIRTIO_WIFI_EVENT_CODE_DISASSOCIATE as u32).into(),
                    ifindex: ifindex.into(),
                },
                frame_len: (frame.len() as u64).into(),
                req_ies_len: req_ies.as_ref().map(|r| r.len() as u64).unwrap_or(0).into(),
                uapsd_queues: uapsd_queues.into(),
                padding0: 0.into(),
            }
            .as_slice()
            .to_vec();
            event.extend(&frame);
            if let Some(req_ies) = req_ies {
                event.extend(&req_ies);
            }
            self.send_event(event);
            log::debug!("sending disassoc event");
        } else if let Ok(()) = attr_handle.get_attr_payload_as::<()>(Nl80211Attribute::TimedOut) {
            log::debug!("sending disassoc cancelled");
            let event = virtio_wifi_event {
                code: (VIRTIO_WIFI_EVENT_CODE_ASSOCIATE_TIMED_OUT as u32).into(),
                ifindex: ifindex.into(),
            }
            .as_slice()
            .to_vec();
            self.send_event(event);
        } else {
            // Ignore
            log::warn!(
                "Did not find frame attr or timed out attr in disassoc event, found these attributes: {:?}",
                &attr_handle.iter().collect::<Vec<_>>()
            );
        }
        Ok(())
    }

    /// Send connect event back to guest.
    fn get_connect(
        &self,
        payload: &neli::genl::Genlmsghdr<Nl80211Command, Nl80211Attribute, NoUserHeader>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let attr_handle = payload.attrs().get_attr_handle();
        let _wiphy = attr_handle.get_attr_payload_as::<u32>(Nl80211Attribute::Wiphy)?;
        let ifindex = attr_handle.get_attr_payload_as::<u32>(Nl80211Attribute::IfIndex)?;
        let mac = attr_handle
            .get_attr_payload_as_with_len::<Vec<u8>>(Nl80211Attribute::Mac)?
            .try_into()
            .unwrap();
        let status_code = attr_handle.get_attr_payload_as::<u16>(Nl80211Attribute::StatusCode)?;
        let req_ie = attr_handle
            .get_attr_payload_as_with_len::<Vec<u8>>(Nl80211Attribute::ReqIe)
            .ok();
        let resp_ie = attr_handle
            .get_attr_payload_as_with_len::<Vec<u8>>(Nl80211Attribute::RespIe)
            .ok();
        let timed_out = attr_handle
            .get_attr_payload_as::<()>(Nl80211Attribute::TimedOut)
            .is_ok();
        let mut event = virtio_wifi_connection_response {
            hdr: virtio_wifi_event {
                code: if timed_out {
                    (VIRTIO_WIFI_EVENT_CODE_CONNECT_TIMED_OUT as u32).into()
                } else {
                    (VIRTIO_WIFI_EVENT_CODE_CONNECT as u32).into()
                },
                ifindex: ifindex.into(),
            },
            status_code: status_code.into(),
            mac,
            req_ie_len: req_ie.as_ref().map(|r| r.len() as u64).unwrap_or(0).into(),
            resp_ie_len: resp_ie.as_ref().map(|r| r.len() as u64).unwrap_or(0).into(),
        }
        .as_slice()
        .to_vec();
        if let Some(req_ie) = req_ie {
            event.extend(&req_ie);
        }
        if let Some(resp_ie) = resp_ie {
            event.extend(&resp_ie);
        }
        self.send_event(event);
        log::debug!("sending connect event");
        Ok(())
    }

    /// Send disconnect event back to guest.
    fn get_disconnect(
        &self,
        payload: &neli::genl::Genlmsghdr<Nl80211Command, Nl80211Attribute, NoUserHeader>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let attr_handle = payload.attrs().get_attr_handle();
        let _wiphy = attr_handle.get_attr_payload_as::<u32>(Nl80211Attribute::Wiphy)?;
        let ifindex = attr_handle.get_attr_payload_as::<u32>(Nl80211Attribute::IfIndex)?;
        let reason_code = attr_handle.get_attr_payload_as::<u16>(Nl80211Attribute::ReasonCode)?;
        let ie = attr_handle
            .get_attr_payload_as_with_len::<Vec<u8>>(Nl80211Attribute::Ie)
            .ok();
        let disconnected_by_ap = attr_handle
            .get_attr_payload_as::<()>(Nl80211Attribute::DisconnectedByAp)
            .is_ok();
        log::debug!("sending disconnect event reason = {reason_code} disconnected_by_ap = {disconnected_by_ap:?} ie_len={}", ie.as_ref().map(|r| r.len() as u64).unwrap_or(0));
        let mut event = virtio_wifi_disconnection_response {
            hdr: virtio_wifi_event {
                code: (VIRTIO_WIFI_EVENT_CODE_DISCONNECT as u32).into(),
                ifindex: ifindex.into(),
            },
            reason_code: reason_code.into(),
            disconnected_by_ap: u8::from(disconnected_by_ap),
            padding: [0; 5],
            ie_len: ie.as_ref().map(|r| r.len() as u64).unwrap_or(0).into(),
        }
        .as_slice()
        .to_vec();
        if let Some(ie) = ie {
            event.extend(&ie);
        }
        self.send_event(event);
        Ok(())
    }

    /// Send new/delete station event
    fn get_station(
        &self,
        payload: &neli::genl::Genlmsghdr<Nl80211Command, Nl80211Attribute, NoUserHeader>,
        is_new: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let attr_handle = payload.attrs().get_attr_handle();
        let ifindex = attr_handle.get_attr_payload_as::<u32>(Nl80211Attribute::IfIndex)?;
        let mac: [u8; 6usize] = attr_handle
            .get_attr_payload_as_with_len::<Vec<u8>>(Nl80211Attribute::Mac)?
            .try_into()
            .unwrap();
        let ie = attr_handle
            .get_attr_payload_as_with_len::<Vec<u8>>(Nl80211Attribute::Ie)
            .ok();
        // let _sta_info: Option<Result<Station, _>> = attr_handle
        //     .get_nested_attributes::<Nl80211StaInfo>(Nl80211Attribute::StaInfo)
        //     .ok()
        //     .filter(|a| !a.get_attrs().is_empty())
        //     .map(|a| a.try_into());
        log::debug!(
            "{}_station: ifindex {ifindex:?}, mac {mac:?}, ie {:?} sta_info: {{_sta_info:?}}",
            if is_new { "new" } else { "del" },
            ie.as_ref().map(|b| b.len())
        );
        let mut event = virtio_wifi_station_change {
            hdr: virtio_wifi_event {
                code: if is_new {
                    (VIRTIO_WIFI_EVENT_CODE_NEW_STATION as u32).into()
                } else {
                    (VIRTIO_WIFI_EVENT_CODE_DELETE_STATION as u32).into()
                },
                ifindex: ifindex.into(),
            },
            mac,
            padding: [0; 2],
            ie_len: ie.as_ref().map(|r| r.len() as u64).unwrap_or(0).into(),
        }
        .as_slice()
        .to_vec();
        if let Some(ie) = ie {
            event.extend(&ie);
        }
        self.send_event(event);
        Ok(())
    }
}
