//! Multicast notification processing

use std::{
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
use vm_memory::ByteValued;

use crate::virtio_wifi;

use super::{Nl80211Attribute, Nl80211BSS, Nl80211Command, BSS};

type NlMessage = Nlmsghdr<GenlId, Genlmsghdr<Nl80211Command, Nl80211Attribute>>;

pub struct McastListener {
    pub multicast_receiver: NlRouterReceiverHandle<u16, Genlmsghdr<u8, u16, NoUserHeader>>,
    pub sock: Arc<NlRouter>,
    pub event_fd: Arc<EventFd>,
    pub rx_event_fd: Arc<EventFd>,
    pub event_sender: std::sync::mpsc::Sender<Vec<u8>>,
    pub rx_sender: std::sync::mpsc::Sender<(u32, Vec<u8>)>,
    pub family_id: u16,
}

impl McastListener {
    pub fn listen(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        while let Some(msg) =
            self.multicast_receiver
                .next_typed::<GenlId, Genlmsghdr<Nl80211Command, Nl80211Attribute, NoUserHeader>>()
        {
            // log::trace!("mcast {msg:?}");
            let Ok(msg) = msg else {
                break;
            };
            let NlPayload::Payload(payload) = msg.nl_payload() else {
                continue;
            };
            if matches!(payload.cmd(), Nl80211Command::NewScanResults) && *msg.nl_pid() == 0 {
                let index = payload
                    .attrs()
                    .get_attr_handle()
                    .iter()
                    .find(|a| matches!(a.nla_type().nla_type(), Nl80211Attribute::IfIndex))
                    .unwrap()
                    .get_payload_as::<u32>()
                    .unwrap();

                self.get_scan(index)?;
            } else if matches!(payload.cmd(), Nl80211Command::Frame) && *msg.nl_pid() == 0 {
                log::debug!("MLME FRAME RX");
                let attr_handle = payload.attrs().get_attr_handle();
                let frame =
                    attr_handle.get_attr_payload_as_with_len::<Vec<u8>>(Nl80211Attribute::Frame)?;
                let if_index = attr_handle.get_attr_payload_as::<u32>(Nl80211Attribute::IfIndex)?;
                let mut attr_handles = attr_handle.iter().collect::<Vec<_>>();
                dbg!(&attr_handles);
                self.send_rx((if_index, frame));
            } else if matches!(payload.cmd(), Nl80211Command::NewInterface) && *msg.nl_pid() == 0 {
                // Ignore
                // TODO: send guest NEW_IF event?
            } else if matches!(payload.cmd(), Nl80211Command::DelInterface) && *msg.nl_pid() == 0 {
                // Ignore
                // TODO: send guest DEL_IF event?
            } else if matches!(payload.cmd(), Nl80211Command::TriggerScan) {
                // Ignore
            } else {
                let attr_handle = payload.attrs().get_attr_handle();
                let mut attr_handles = attr_handle.iter().collect::<Vec<_>>();
                dbg!(payload.cmd(), &attr_handles);
            }
        }
        Ok(())
    }

    fn send_rx(&self, mut frame: (u32, Vec<u8>)) {
        while let Err(std::sync::mpsc::SendError(again)) = self.rx_sender.send(frame) {
            frame = again;
            log::error!("Send error!");
        }
        self.rx_event_fd.write(1).unwrap();
    }

    fn send_scan(&self, mut result: Vec<u8>) {
        while let Err(std::sync::mpsc::SendError(again)) = self.event_sender.send(result) {
            result = again;
            log::error!("Send error!");
        }
        self.event_fd.write(1).unwrap();
    }

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
        let recv = recv
            .into_iter()
            .collect::<Result<Vec<NlMessage>, _>>()
            .unwrap();
        // for each ssid send eventq bssid inform
        let mut _if_index = None;
        for msg in recv {
            let NlPayload::Payload(payload) = msg.nl_payload() else {
                continue;
            };
            let attr_handle = payload.attrs().get_attr_handle();
            let mut attr_handles = attr_handle.iter().collect::<Vec<_>>();
            let if_index = attr_handles
                .extract_if(.., |attr| {
                    matches!(attr.nla_type().nla_type(), Nl80211Attribute::IfIndex)
                })
                .next()
                .and_then(|t| t.get_payload_as::<u32>().ok())
                .unwrap();
            _if_index = Some(if_index);
            let _wdev = attr_handles
                .extract_if(.., |attr| {
                    matches!(attr.nla_type().nla_type(), Nl80211Attribute::WDev)
                })
                .next()
                .and_then(|t| t.get_payload_as::<u32>().ok())
                .unwrap();
            let bss: BSS = attr_handle
                .get_nested_attributes::<Nl80211BSS>(Nl80211Attribute::Bss)
                .unwrap()
                .try_into()
                .unwrap();
            let mut result = virtio_wifi::VirtioWifiScanResult {
                hdr: virtio_wifi::VirtioWifiEvent {
                    code: (virtio_wifi::VirtioWifiEventCode::ScanResult as u32).into(),
                    if_index: if_index.into(),
                },
                ielen: u64::try_from(bss.information_elements.len())
                    .unwrap()
                    .into(),
                ..virtio_wifi::VirtioWifiScanResult::from(&bss)
            }
            .as_slice()
            .to_vec();
            result.extend(&bss.information_elements);
            self.send_scan(result);
        }
        self.send_scan(
            virtio_wifi::VirtioWifiEvent {
                code: (virtio_wifi::VirtioWifiEventCode::EndScan as u32).into(),
                if_index: _if_index.unwrap().into(),
            }
            .as_slice()
            .to_vec(),
        );
        Ok(())
    }
}
