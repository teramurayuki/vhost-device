// SPDX-License-Identifier: Apache-2.0 or BSD-3-Clause
// Copyright 2026 Panasonic Automotive Systems Co., Ltd.
// Author: Manos Pitsidianakis <manos.pitsidianakis@linaro.org>

use std::io::Read;

use thiserror::Error as ThisError;
use vm_memory::Le32;

bitflags::bitflags! {
    #[doc(alias = "ieee80211_channel_flags")]
    pub struct IEEE80211ChannelFlags: u32 {
        /// `IEEE80211_CHAN_DISABLED`
       const DISABLED			= 1 << 0;
        /// `IEEE80211_CHAN_NO_IR`
       const NO_IR			= 1 << 1;
        /// `IEEE80211_CHAN_PSD`
       const PSD			= 1 << 2;
        /// `IEEE80211_CHAN_RADAR`
       const RADAR			= 1 << 3;
        /// `IEEE80211_CHAN_NO_HT40PLUS`
       const NO_HT40PLUS		= 1 << 4;
        /// `IEEE80211_CHAN_NO_HT40MINUS`
       const NO_HT40MINUS		= 1 << 5;
        /// `IEEE80211_CHAN_NO_OFDM`
       const NO_OFDM			= 1 << 6;
        /// `IEEE80211_CHAN_NO_80MHZ`
       const NO_80MHZ			= 1 << 7;
        /// `IEEE80211_CHAN_NO_160MHZ`
       const NO_160MHZ		= 1 << 8;
        /// `IEEE80211_CHAN_INDOOR_ONLY`
       const INDOOR_ONLY		= 1 << 9;
        /// `IEEE80211_CHAN_IR_CONCURRENT`
       const IR_CONCURRENT		= 1 << 10;
        /// `IEEE80211_CHAN_NO_20MHZ`
       const NO_20MHZ			= 1 << 11;
        /// `IEEE80211_CHAN_NO_10MHZ`
       const NO_10MHZ			= 1 << 12;
        /// `IEEE80211_CHAN_NO_HE`
       const NO_HE			= 1 << 13;
        /* can use free bits here */
        /// `IEEE80211_CHAN_NO_UHR`
       const NO_UHR			= 1 << 18;
        /// `IEEE80211_CHAN_NO_320MHZ`
       const NO_320MHZ		= 1 << 19;
        /// `IEEE80211_CHAN_NO_EHT`
       const NO_EHT			= 1 << 20;
        /// `IEEE80211_CHAN_DFS_CONCURRENT`
       const DFS_CONCURRENT		= 1 << 21;
        /// `IEEE80211_CHAN_NO_6GHZ_VLP_CLIENT`
       const NO_6GHZ_VLP_CLIENT	= 1 << 22;
        /// `IEEE80211_CHAN_NO_6GHZ_AFC_CLIENT`
       const NO_6GHZ_AFC_CLIENT	= 1 << 23;
        /// `IEEE80211_CHAN_CAN_MONITOR`
       const CAN_MONITOR		= 1 << 24;
        /// `IEEE80211_CHAN_ALLOW_6GHZ_VLP_AP`
       const ALLOW_6GHZ_VLP_AP	= 1 << 25;
        /// `IEEE80211_CHAN_ALLOW_20MHZ_ACTIVITY`
       const ALLOW_20MHZ_ACTIVITY     = 1 << 26;
        /// `IEEE80211_CHAN_S1G_NO_PRIMARY`
       const S1G_NO_PRIMARY		= 1 << 27;
        /// `IEEE80211_CHAN_NO_4MHZ`
       const NO_4MHZ			= 1 << 28;
        /// `IEEE80211_CHAN_NO_8MHZ`
       const NO_8MHZ			= 1 << 29;
        /// `IEEE80211_CHAN_NO_16MHZ`
       const NO_16MHZ			= 1 << 30;
    }
}

#[derive(Debug, ThisError)]
pub enum Error {
    #[error("Reading from descriptor failed: {0}")]
    DescriptorReadFailed(std::io::Error),
    #[error("Invalid Type-Length-Value, type={0}, length={1}, actual length = {2}")]
    InvalidTLV(u32, u32, usize),
    #[error("Insufficient bytes in reader: {0} bytes")]
    InsufficientData(usize),
}

pub fn read_tlv(reader: &mut virtio_queue::Reader<'_>) -> Result<Option<(u32, Vec<u8>)>, Error> {
    use crate::bindings::virtio_wifi_uapi::virtio_wifi_tlv;

    if reader.available_bytes() == 0 {
        return Ok(None);
    }
    if reader.available_bytes() < core::mem::size_of::<virtio_wifi_tlv>() {
        return Err(Error::InsufficientData(reader.available_bytes()));
    }
    let type_: u32 = reader
        .read_obj::<Le32>()
        .map_err(Error::DescriptorReadFailed)?
        .into();
    let length: u32 = reader
        .read_obj::<Le32>()
        .map_err(Error::DescriptorReadFailed)?
        .into();
    if reader.available_bytes() < length as usize {
        return Err(Error::InvalidTLV(type_, length, reader.available_bytes()));
    }
    let mut data = vec![0; length as usize];
    reader
        .read_exact(&mut data)
        .map_err(Error::DescriptorReadFailed)?;

    Ok(Some((type_, data)))
}
