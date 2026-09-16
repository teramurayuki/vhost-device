// SPDX-License-Identifier: Apache-2.0 or BSD-3-Clause
// Copyright 2026 Panasonic Automotive Systems Co., Ltd.
// Author: Manos Pitsidianakis <manos.pitsidianakis@linaro.org>

//! Parsing `NL80211_ATTR_STA_INFO`/`enum nl80211_sta_info - station
//! information` properties

// use neli::{attr::Attribute, genl::GenlAttrHandle};

// use super::*;

// #[derive(Debug, Default)]
// pub struct Station {
//     ///  time since last activity (u32, msecs)
//     pub inactive_time: u32,
//     /// total received bytes (MPDU length)	(u32, from this station)
//     pub rx_bytes: u32,
//     ///  total transmitted bytes (MPDU length) (u32, to this station)
//     pub tx_bytes: u32,
//     ///  total received bytes (MPDU length) (u64, from this station)
//     pub rx_bytes64: u64,
//     ///  total transmitted bytes (MPDU length) (u64, to this station)
//     pub tx_bytes64: u64,
//     ///  signal strength of last received PPDU (u8, dBm)
//     pub signal: u8,
//     ///  current unicast tx rate, nested attribute containing info as
// possible,     /// see &enum nl80211_rate_info
//     pub tx_bitrate: u32,
//     ///  total received packet (MSDUs and MMPDUs) (u32, from this station)
//     pub rx_packets: u32,
//     ///  total transmitted packets (MSDUs and MMPDUs) (u32, to this station)
//     pub tx_packets: u32,
//     ///  total retries (MPDUs) (u32, to this station)
//     pub tx_retries: u32,
//     ///  total failed packets (MPDUs) (u32, to this station)
//     pub tx_failed: u32,
//     ///  signal strength average (u8, dBm)
//     pub signal_avg: u8,
//     ///  the station's mesh LLID
//     pub llid: u16,
//     ///  the station's mesh PLID
//     pub plid: u16,
//     ///  peer link state for the station (see %enum nl80211_plink_state)
//     pub plink_state: u32,
//     ///  last unicast data frame rx rate, nested attribute, like
//     /// NL80211_STA_INFO_TX_BITRATE.
//     pub rx_bitrate: u32,
//     ///  current station's view of BSS, nested attribute containing info as
//     /// possible, see &enum nl80211_sta_bss_param
//     pub bss_param: u32,
//     ///  time since the station is last connected
//     pub connected_time: u32,
//     ///  Contains a struct nl80211_sta_flag_update.
//     pub sta_flags: u32,
//     ///  count of times beacon loss was detected (u32)
//     pub beacon_loss: u32,
//     ///  timing offset with respect to this STA (s64)
//     pub t_offset: i64,
//     ///  local mesh STA link-specific power mode
//     pub local_pm: u32,
//     ///  peer mesh STA link-specific power mode
//     pub peer_pm: u32,
//     ///  neighbor mesh STA power save mode towards non-peer STA
//     pub nonpeer_pm: u32,
//     ///  per-chain signal strength of last PPDU Contains a nested array of
//     /// signal strength attributes (u8, dBm)
//     pub chain_signal: u8,
//     ///  per-chain signal strength average Same format as
//     /// NL80211_STA_INFO_CHAIN_SIGNAL.
//     pub chain_signal_avg: u8,
//     ///  expected throughput considering also the 802.11 header (u32, kbps)
//     pub expected_throughput: u32,
//     ///  RX packets dropped for unspecified reasons (u64)
//     pub rx_drop_misc: u64,
//     ///  number of beacons received from this peer (u64)
//     pub beacon_rx: u64,
//     ///  signal strength average for beacons only (u8, dBm)
//     pub beacon_signal_avg: u8,
//     ///  per-TID statistics (see &enum nl80211_tid_stats) This is a nested
//     /// attribute where each the inner attribute number is the TID+1 and the
//     /// special TID 16 (i.e. value 17) is used for non-QoS frames; each one
// of     /// those is again nested with &enum nl80211_tid_stats attributes
// carrying     /// the actual values.
//     pub tid_stats: u32,
//     ///  aggregate PPDU duration for all frames received from the station
// (u64,     /// usec)
//     pub rx_duration: u64,
//     ///  attribute used for padding for 64-bit alignment
//     pub pad: u32,
//     ///  signal strength of the last ACK frame(u8, dBm)
//     pub ack_signal: u8,
//     ///  avg signal strength of ACK frames (s8, dBm)
//     pub ack_signal_avg: i8,
//     ///  total number of received packets (MPDUs) (u32, from this station)
//     pub rx_mpdus: u32,
//     ///  total number of packets (MPDUs) received with an FCS error (u32,
// from     /// this station). This count may not include some packets with an
// FCS error     /// due to TA corruption. Hence this counter might not be fully
// accurate.     pub fcs_error_count: u32,
//     ///  set to true if STA has a path to a mesh gate (u8, 0 or 1)
//     pub connected_to_gate: u8,
//     ///  aggregate PPDU duration for all frames sent to the station (u64,
// usec)     pub tx_duration: u64,
//     ///  current airtime weight for station (u16)
//     pub airtime_weight: u16,
//     ///  airtime link metric for mesh station
//     pub airtime_link_metric: u32,
//     ///  Timestamp (CLOCK_BOOTTIME, nanoseconds) of STA's association
//     pub assoc_at_boottime: u32,
//     ///  set to true if STA has a path to a authentication server (u8, 0 or
// 1)     pub connected_to_as: u8,
// }

// impl TryFrom<GenlAttrHandle<'_, Nl80211StaInfo>> for Station {
//     type Error = Nl80211Error;

//     fn try_from(
//         attr_handle: GenlAttrHandle<'_, Nl80211StaInfo>,
//     ) -> std::result::Result<Self, Self::Error> {
//         let mut attr_handles = attr_handle.iter().collect::<Vec<_>>();
//         macro_rules! extract {
//             ($attr:ident : bool) => {{
//                 attr_handles
//                     .extract_if(.., |attr| {
//                         matches!(attr.nla_type().nla_type(),
// Nl80211StaInfo::$attr)                     })
//                     .next()
//                     .map(|t| t.get_payload_as::<()>().is_ok())
//                     .unwrap_or(false)
//             }};
//             ($attr:ident : String) => {{
//                 let temp = attr_handles
//                     .extract_if(.., |attr| {
//                         matches!(attr.nla_type().nla_type(),
// Nl80211StaInfo::$attr)                     })
//                     .next()
//                     .ok_or_else(|| {
//                         Nl80211Error::LogicError(format!(
//                             "No {} in Station",
//                             stringify!(Nl80211StaInfo::$attr)
//                         ))
//                     })?;
//                 temp.get_payload_as_with_len::<String>().unwrap()
//             }};
//             ($attr:ident: Vec<$t:ty>) => {{
//                 let temp = attr_handles
//                     .extract_if(.., |attr| {
//                         matches!(attr.nla_type().nla_type(),
// Nl80211StaInfo::$attr)                     })
//                     .next()
//                     .ok_or_else(|| {
//                         Nl80211Error::LogicError(format!(
//                             "No {} in Station",
//                             stringify!(Nl80211StaInfo::$attr)
//                         ))
//                     })?;
//                 temp.get_payload_as_with_len::<Vec<$t>>().unwrap()
//             }};
//             ($attr:ident : Option<$t:ty>) => {{
//                 attr_handles
//                     .extract_if(.., |attr| {
//                         matches!(attr.nla_type().nla_type(),
// Nl80211StaInfo::$attr)                     })
//                     .next()
//                     .map(|temp| temp.get_payload_as::<$t>().unwrap())
//             }};
//             ($attr:ident : $t:ty) => {{
//                 let temp = attr_handles
//                     .extract_if(.., |attr| {
//                         matches!(attr.nla_type().nla_type(),
// Nl80211StaInfo::$attr)                     })
//                     .next()
//                     .ok_or_else(|| {
//                         Nl80211Error::LogicError(format!(
//                             "No {} in Station",
//                             stringify!(Nl80211StaInfo::$attr)
//                         ))
//                     })?;
//                 temp.get_payload_as::<$t>().unwrap()
//             }};
//         }
//         Ok(Self {
//             inactive_time: extract!(InactiveTime: u32),
//             rx_bytes: extract!(RxBytes: u32),
//             tx_bytes: extract!(TxBytes: u32),
//             llid: extract!(Llid: u16),
//             plid: extract!(Plid: u16),
//             plink_state: extract!(PlinkState: u32),
//             signal: extract!(Signal: u8),
//             tx_bitrate: extract!(TxBitrate: u32),
//             rx_packets: extract!(RxPackets: u32),
//             tx_packets: extract!(TxPackets: u32),
//             tx_retries: extract!(TxRetries: u32),
//             tx_failed: extract!(TxFailed: u32),
//             signal_avg: extract!(SignalAvg: u8),
//             rx_bitrate: extract!(RxBitrate: u32),
//             bss_param: extract!(BssParam: u32),
//             connected_time: extract!(ConnectedTime: u32),
//             sta_flags: extract!(StaFlags: u32),
//             beacon_loss: extract!(BeaconLoss: u32),
//             t_offset: extract!(TOffset: i64),
//             local_pm: extract!(LocalPm: u32),
//             peer_pm: extract!(PeerPm: u32),
//             nonpeer_pm: extract!(NonpeerPm: u32),
//             rx_bytes64: extract!(RxBytes64: u64),
//             tx_bytes64: extract!(TxBytes64: u64),
//             chain_signal: extract!(ChainSignal: u8),
//             chain_signal_avg: extract!(ChainSignalAvg: u8),
//             expected_throughput: extract!(ExpectedThroughput: u32),
//             rx_drop_misc: extract!(RxDropMisc: u64),
//             beacon_rx: extract!(BeaconRx: u64),
//             beacon_signal_avg: extract!(BeaconSignalAvg: u8),
//             tid_stats: extract!(TidStats: u32),
//             rx_duration: extract!(RxDuration: u64),
//             pad: extract!(Pad: u32),
//             ack_signal: extract!(AckSignal: u8),
//             ack_signal_avg: extract!(AckSignalAvg: i8),
//             rx_mpdus: extract!(RxMpdus: u32),
//             fcs_error_count: extract!(FcsErrorCount: u32),
//             connected_to_gate: extract!(ConnectedToGate: u8),
//             tx_duration: extract!(TxDuration: u64),
//             airtime_weight: extract!(AirtimeWeight: u16),
//             airtime_link_metric: extract!(AirtimeLinkMetric: u32),
//             assoc_at_boottime: extract!(AssocAtBoottime: u32),
//             connected_to_as: extract!(ConnectedToAs: u8),
//         })
//     }
// }

// // `enum nl80211_sta_info`
// #[neli::neli_enum(serialized_type = "u16")]
// #[repr(u32)]
// pub enum Nl80211StaInfo {
//     /// `NL80211_STA_INFO_INVALID`
//     Invalid = 0,
//     /// `NL80211_STA_INFO_INACTIVE_TIME`
//     InactiveTime = 1,
//     /// `NL80211_STA_INFO_RX_BYTES`
//     RxBytes = 2,
//     /// `NL80211_STA_INFO_TX_BYTES`
//     TxBytes = 3,
//     /// `NL80211_STA_INFO_LLID`
//     Llid = 4,
//     /// `NL80211_STA_INFO_PLID`
//     Plid = 5,
//     /// `NL80211_STA_INFO_PLINK_STATE`
//     PlinkState = 6,
//     /// `NL80211_STA_INFO_SIGNAL`
//     Signal = 7,
//     /// `NL80211_STA_INFO_TX_BITRATE`
//     TxBitrate = 8,
//     /// `NL80211_STA_INFO_RX_PACKETS`
//     RxPackets = 9,
//     /// `NL80211_STA_INFO_TX_PACKETS`
//     TxPackets = 10,
//     /// `NL80211_STA_INFO_TX_RETRIES`
//     TxRetries = 11,
//     /// `NL80211_STA_INFO_TX_FAILED`
//     TxFailed = 12,
//     /// `NL80211_STA_INFO_SIGNAL_AVG`
//     SignalAvg = 13,
//     /// `NL80211_STA_INFO_RX_BITRATE`
//     RxBitrate = 14,
//     /// `NL80211_STA_INFO_BSS_PARAM`
//     BssParam = 15,
//     /// `NL80211_STA_INFO_CONNECTED_TIME`
//     ConnectedTime = 16,
//     /// `NL80211_STA_INFO_STA_FLAGS`
//     StaFlags = 17,
//     /// `NL80211_STA_INFO_BEACON_LOSS`
//     BeaconLoss = 18,
//     /// `NL80211_STA_INFO_T_OFFSET`
//     TOffset = 19,
//     /// `NL80211_STA_INFO_LOCAL_PM`
//     LocalPm = 20,
//     /// `NL80211_STA_INFO_PEER_PM`
//     PeerPm = 21,
//     /// `NL80211_STA_INFO_NONPEER_PM`
//     NonpeerPm = 22,
//     /// `NL80211_STA_INFO_RX_BYTES64`
//     RxBytes64 = 23,
//     /// `NL80211_STA_INFO_TX_BYTES64`
//     TxBytes64 = 24,
//     /// `NL80211_STA_INFO_CHAIN_SIGNAL`
//     ChainSignal = 25,
//     /// `NL80211_STA_INFO_CHAIN_SIGNAL_AVG`
//     ChainSignalAvg = 26,
//     /// `NL80211_STA_INFO_EXPECTED_THROUGHPUT`
//     ExpectedThroughput = 27,
//     /// `NL80211_STA_INFO_RX_DROP_MISC`
//     RxDropMisc = 28,
//     /// `NL80211_STA_INFO_BEACON_RX`
//     BeaconRx = 29,
//     /// `NL80211_STA_INFO_BEACON_SIGNAL_AVG`
//     BeaconSignalAvg = 30,
//     /// `NL80211_STA_INFO_TID_STATS`
//     TidStats = 31,
//     /// `NL80211_STA_INFO_RX_DURATION`
//     RxDuration = 32,
//     /// `NL80211_STA_INFO_PAD`
//     Pad = 33,
//     /// `NL80211_STA_INFO_ACK_SIGNAL`
//     AckSignal = 34,
//     /// `NL80211_STA_INFO_ACK_SIGNAL_AVG`
//     AckSignalAvg = 35,
//     /// `NL80211_STA_INFO_RX_MPDUS`
//     RxMpdus = 36,
//     /// `NL80211_STA_INFO_FCS_ERROR_COUNT`
//     FcsErrorCount = 37,
//     /// `NL80211_STA_INFO_CONNECTED_TO_GATE`
//     ConnectedToGate = 38,
//     /// `NL80211_STA_INFO_TX_DURATION`
//     TxDuration = 39,
//     /// `NL80211_STA_INFO_AIRTIME_WEIGHT`
//     AirtimeWeight = 40,
//     /// `NL80211_STA_INFO_AIRTIME_LINK_METRIC`
//     AirtimeLinkMetric = 41,
//     /// `NL80211_STA_INFO_ASSOC_AT_BOOTTIME`
//     AssocAtBoottime = 42,
//     /// `NL80211_STA_INFO_CONNECTED_TO_AS`
//     ConnectedToAs = 43,
// }

// impl neli::consts::genl::NlAttrType for Nl80211StaInfo {}
