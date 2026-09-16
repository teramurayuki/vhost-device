// SPDX-License-Identifier: Apache-2.0 or BSD-3-Clause
// Copyright 2026 Panasonic Automotive Systems Co., Ltd.
// Author: Manos Pitsidianakis <manos.pitsidianakis@linaro.org>

//! Parsing Wiphy properties
//!
//! nl80211 sends the full Wiphy properties in response to a `GET_WIPHY` command
//! only if the reply can be split to multiple messages.
//! So what we do is parse properties from each reply and then merge them back
//! together.

use std::collections::BTreeSet;

use neli::{
    attr::Attribute,
    consts::nl::GenlId,
    genl::Genlmsghdr,
    nl::{NlPayload, Nlmsghdr},
};

use super::*;

/**
 * enum nl80211_ap_sme_features - device-integrated AP features
 * @NL80211_AP_SME_SA_QUERY_OFFLOAD: SA Query procedures offloaded to driver
 * when user space indicates support for SA Query procedures offload during
 * "start ap" with %NL80211_AP_SETTINGS_SA_QUERY_OFFLOAD_SUPPORT.
 */
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub enum Nl80211ApSmeFeatures {
    // NL80211_AP_SME_SA_QUERY_OFFLOAD		= 1 << 0,
    SaQueryOffload = 1,
}

#[derive(Debug)]
pub enum WiphyProperty {
    TxFrameType(BTreeSet<FrameType>),
    RxFrameType(BTreeSet<FrameType>),
    SupportedCommands(BTreeSet<Nl80211Command>),
    ExtFeatures(BTreeSet<Nl80211ExtFeatureIndex>),
    // NL80211_ATTR_DEVICE_AP_SME: This u32 attribute may be listed for devices
    //  *	that have AP support to indicate that they have the AP SME integrated
    //  *	with support for the features listed in this attribute, see
    //  *	&enum nl80211_ap_sme_features.
    ApSme(BTreeSet<Nl80211ApSmeFeatures>),
}

/// `nl80211_ext_feature_index`
#[derive(Copy, Clone, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub enum Nl80211ExtFeatureIndex {
    /// `NL80211_EXT_FEATURE_VHT_IBSS`
    VhtIbss = 0,
    /// `NL80211_EXT_FEATURE_RRM`
    Rrm = 1,
    /// `NL80211_EXT_FEATURE_MU_MIMO_AIR_SNIFFER`
    MuMimoAirSniffer = 2,
    /// `NL80211_EXT_FEATURE_SCAN_START_TIME`
    ScanStartTime = 3,
    /// `NL80211_EXT_FEATURE_BSS_PARENT_TSF`
    BssParentTsf = 4,
    /// `NL80211_EXT_FEATURE_SET_SCAN_DWELL`
    SetScanDwell = 5,
    /// `NL80211_EXT_FEATURE_BEACON_RATE_LEGACY`
    BeaconRateLegacy = 6,
    /// `NL80211_EXT_FEATURE_BEACON_RATE_HT`
    BeaconRateHt = 7,
    /// `NL80211_EXT_FEATURE_BEACON_RATE_VHT`
    BeaconRateVht = 8,
    /// `NL80211_EXT_FEATURE_FILS_STA`
    FilsSta = 9,
    /// `NL80211_EXT_FEATURE_MGMT_TX_RANDOM_TA`
    MgmtTxRandomTa = 10,
    /// `NL80211_EXT_FEATURE_MGMT_TX_RANDOM_TA_CONNECTED`
    MgmtTxRandomTaConnected = 11,
    /// `NL80211_EXT_FEATURE_SCHED_SCAN_RELATIVE_RSSI`
    SchedScanRelativeRssi = 12,
    /// `NL80211_EXT_FEATURE_CQM_RSSI_LIST`
    CqmRssiList = 13,
    /// `NL80211_EXT_FEATURE_FILS_SK_OFFLOAD`
    FilsSkOffload = 14,
    /// `NL80211_EXT_FEATURE_4WAY_HANDSHAKE_STA_PSK`
    _4wayHandshakeStaPsk = 15,
    /// `NL80211_EXT_FEATURE_4WAY_HANDSHAKE_STA_1X`
    _4wayHandshakeSta1x = 16,
    /// `NL80211_EXT_FEATURE_FILS_MAX_CHANNEL_TIME`
    FilsMaxChannelTime = 17,
    /// `NL80211_EXT_FEATURE_ACCEPT_BCAST_PROBE_RESP`
    AcceptBcastProbeResp = 18,
    /// `NL80211_EXT_FEATURE_OCE_PROBE_REQ_HIGH_TX_RATE`
    OceProbeReqHighTxRate = 19,
    /// `NL80211_EXT_FEATURE_OCE_PROBE_REQ_DEFERRAL_SUPPRESSION`
    OceProbeReqDeferralSuppression = 20,
    /// `NL80211_EXT_FEATURE_MFP_OPTIONAL`
    MfpOptional = 21,
    /// `NL80211_EXT_FEATURE_LOW_SPAN_SCAN`
    LowSpanScan = 22,
    /// `NL80211_EXT_FEATURE_LOW_POWER_SCAN`
    LowPowerScan = 23,
    /// `NL80211_EXT_FEATURE_HIGH_ACCURACY_SCAN`
    HighAccuracyScan = 24,
    /// `NL80211_EXT_FEATURE_DFS_OFFLOAD`
    DfsOffload = 25,
    /// `NL80211_EXT_FEATURE_CONTROL_PORT_OVER_NL80211`
    ControlPortOverNl80211 = 26,
    /// `NL80211_EXT_FEATURE_ACK_SIGNAL_SUPPORT`
    /// `NL80211_EXT_FEATURE_DATA_ACK_SIGNAL_SUPPORT`
    AckSignalSupport = 27,
    /// `NL80211_EXT_FEATURE_TXQS`
    Txqs = 28,
    /// `NL80211_EXT_FEATURE_SCAN_RANDOM_SN`
    ScanRandomSn = 29,
    /// `NL80211_EXT_FEATURE_SCAN_MIN_PREQ_CONTENT`
    ScanMinPreqContent = 30,
    /// `NL80211_EXT_FEATURE_CAN_REPLACE_PTK0`
    CanReplacePtk0 = 31,
    /// `NL80211_EXT_FEATURE_ENABLE_FTM_RESPONDER`
    EnableFtmResponder = 32,
    /// `NL80211_EXT_FEATURE_AIRTIME_FAIRNESS`
    AirtimeFairness = 33,
    /// `NL80211_EXT_FEATURE_AP_PMKSA_CACHING`
    ApPmksaCaching = 34,
    /// `NL80211_EXT_FEATURE_SCHED_SCAN_BAND_SPECIFIC_RSSI_THOLD`
    SchedScanBandSpecificRssiThold = 35,
    /// `NL80211_EXT_FEATURE_EXT_KEY_ID`
    ExtKeyId = 36,
    /// `NL80211_EXT_FEATURE_STA_TX_PWR`
    StaTxPwr = 37,
    /// `NL80211_EXT_FEATURE_SAE_OFFLOAD`
    SaeOffload = 38,
    /// `NL80211_EXT_FEATURE_VLAN_OFFLOAD`
    VlanOffload = 39,
    /// `NL80211_EXT_FEATURE_AQL`
    Aql = 40,
    /// `NL80211_EXT_FEATURE_BEACON_PROTECTION`
    BeaconProtection = 41,
    /// `NL80211_EXT_FEATURE_CONTROL_PORT_NO_PREAUTH`
    ControlPortNoPreauth = 42,
    /// `NL80211_EXT_FEATURE_PROTECTED_TWT`
    ProtectedTwt = 43,
    /// `NL80211_EXT_FEATURE_DEL_IBSS_STA`
    DelIbssSta = 44,
    /// `NL80211_EXT_FEATURE_MULTICAST_REGISTRATIONS`
    MulticastRegistrations = 45,
    /// `NL80211_EXT_FEATURE_BEACON_PROTECTION_CLIENT`
    BeaconProtectionClient = 46,
    /// `NL80211_EXT_FEATURE_SCAN_FREQ_KHZ`
    ScanFreqKhz = 47,
    /// `NL80211_EXT_FEATURE_CONTROL_PORT_OVER_NL80211_TX_STATUS`
    ControlPortOverNl80211TxStatus = 48,
    /// `NL80211_EXT_FEATURE_OPERATING_CHANNEL_VALIDATION`
    OperatingChannelValidation = 49,
    /// `NL80211_EXT_FEATURE_4WAY_HANDSHAKE_AP_PSK`
    _4wayHandshakeApPsk = 50,
    /// `NL80211_EXT_FEATURE_SAE_OFFLOAD_AP`
    SaeOffloadAp = 51,
    /// `NL80211_EXT_FEATURE_FILS_DISCOVERY`
    FilsDiscovery = 52,
    /// `NL80211_EXT_FEATURE_UNSOL_BCAST_PROBE_RESP`
    UnsolBcastProbeResp = 53,
    /// `NL80211_EXT_FEATURE_BEACON_RATE_HE`
    BeaconRateHe = 54,
    /// `NL80211_EXT_FEATURE_SECURE_LTF`
    SecureLtf = 55,
    /// `NL80211_EXT_FEATURE_SECURE_RTT`
    SecureRtt = 56,
    /// `NL80211_EXT_FEATURE_PROT_RANGE_NEGO_AND_MEASURE`
    ProtRangeNegoAndMeasure = 57,
    /// `NL80211_EXT_FEATURE_BSS_COLOR`
    BssColor = 58,
    /// `NL80211_EXT_FEATURE_FILS_CRYPTO_OFFLOAD`
    FilsCryptoOffload = 59,
    /// `NL80211_EXT_FEATURE_RADAR_BACKGROUND`
    RadarBackground = 60,
    /// `NL80211_EXT_FEATURE_POWERED_ADDR_CHANGE`
    PoweredAddrChange = 61,
    /// `NL80211_EXT_FEATURE_PUNCT`
    Punct = 62,
    /// `NL80211_EXT_FEATURE_SECURE_NAN`
    SecureNan = 63,
    /// `NL80211_EXT_FEATURE_AUTH_AND_DEAUTH_RANDOM_TA`
    AuthAndDeauthRandomTa = 64,
    /// `NL80211_EXT_FEATURE_OWE_OFFLOAD`
    OweOffload = 65,
    /// `NL80211_EXT_FEATURE_OWE_OFFLOAD_AP`
    OweOffloadAp = 66,
    /// `NL80211_EXT_FEATURE_DFS_CONCURRENT`
    DfsConcurrent = 67,
    /// `NL80211_EXT_FEATURE_SPP_AMSDU_SUPPORT`
    SppAmsduSupport = 68,
    /// `NL80211_EXT_FEATURE_BEACON_RATE_EHT`
    BeaconRateEht = 69,
    /// `NL80211_EXT_FEATURE_EPPKE`
    Eppke = 70,
    /// `NL80211_EXT_FEATURE_ASSOC_FRAME_ENCRYPTION`
    AssocFrameEncryption = 71,
    /// `NL80211_EXT_FEATURE_IEEE8021X_AUTH`
    Ieee8021xAuth = 72,
}

impl Nl80211ExtFeatureIndex {
    // `ext_feature_isset` from wpa
    fn is_set(self, ext_features: &[u8]) -> bool {
        if (self as u32 / 8) as usize >= ext_features.len() {
            return false;
        }

        let ft_byte: u8 = ext_features[(self as u32 / 8) as usize];

        ft_byte & (1 << (self as u32 % 8)) != 0
    }

    fn from_bit_array(ext_features: &[u8]) -> BTreeSet<Self> {
        let mut ret = BTreeSet::default();

        for f in [
            Self::VhtIbss,
            Self::Rrm,
            Self::MuMimoAirSniffer,
            Self::ScanStartTime,
            Self::BssParentTsf,
            Self::SetScanDwell,
            Self::BeaconRateLegacy,
            Self::BeaconRateHt,
            Self::BeaconRateVht,
            Self::FilsSta,
            Self::MgmtTxRandomTa,
            Self::MgmtTxRandomTaConnected,
            Self::SchedScanRelativeRssi,
            Self::CqmRssiList,
            Self::FilsSkOffload,
            Self::_4wayHandshakeStaPsk,
            Self::_4wayHandshakeSta1x,
            Self::FilsMaxChannelTime,
            Self::AcceptBcastProbeResp,
            Self::OceProbeReqHighTxRate,
            Self::OceProbeReqDeferralSuppression,
            Self::MfpOptional,
            Self::LowSpanScan,
            Self::LowPowerScan,
            Self::HighAccuracyScan,
            Self::DfsOffload,
            Self::ControlPortOverNl80211,
            Self::AckSignalSupport,
            Self::Txqs,
            Self::ScanRandomSn,
            Self::ScanMinPreqContent,
            Self::CanReplacePtk0,
            Self::EnableFtmResponder,
            Self::AirtimeFairness,
            Self::ApPmksaCaching,
            Self::SchedScanBandSpecificRssiThold,
            Self::ExtKeyId,
            Self::StaTxPwr,
            Self::SaeOffload,
            Self::VlanOffload,
            Self::Aql,
            Self::BeaconProtection,
            Self::ControlPortNoPreauth,
            Self::ProtectedTwt,
            Self::DelIbssSta,
            Self::MulticastRegistrations,
            Self::BeaconProtectionClient,
            Self::ScanFreqKhz,
            Self::ControlPortOverNl80211TxStatus,
            Self::OperatingChannelValidation,
            Self::_4wayHandshakeApPsk,
            Self::SaeOffloadAp,
            Self::FilsDiscovery,
            Self::UnsolBcastProbeResp,
            Self::BeaconRateHe,
            Self::SecureLtf,
            Self::SecureRtt,
            Self::ProtRangeNegoAndMeasure,
            Self::BssColor,
            Self::FilsCryptoOffload,
            Self::RadarBackground,
            Self::PoweredAddrChange,
            Self::Punct,
            Self::SecureNan,
            Self::AuthAndDeauthRandomTa,
            Self::OweOffload,
            Self::OweOffloadAp,
            Self::DfsConcurrent,
            Self::SppAmsduSupport,
            Self::BeaconRateEht,
            Self::Eppke,
            Self::AssocFrameEncryption,
            Self::Ieee8021xAuth,
        ] {
            if f.is_set(ext_features) {
                ret.insert(f);
            }
        }
        ret
    }
}

pub fn parse_wiphy_properties(
    msg: &Nlmsghdr<GenlId, Genlmsghdr<Nl80211Command, Nl80211Attribute>>,
) -> Vec<WiphyProperty> {
    // Messages with the NlmF::DUMP flag end with an empty payload message
    // Don't parse message unless receive proper payload (non-error, non-empty,
    // non-ack)
    let payload = match msg.nl_payload() {
        NlPayload::Payload(p) => p,
        _ => return vec![],
    };

    let attr_handle = payload.attrs().get_attr_handle();
    let mut ret = vec![];
    if let Ok(tx_iter) = attr_handle.get_nested_attributes::<u16>(Nl80211Attribute::TxFrameTypes) {
        let mut types = BTreeSet::default();
        for ft in tx_iter.iter() {
            let b = ft.get_attr_handle::<Nl80211Attribute>().unwrap();
            for b in b.iter() {
                let frame_type = b.get_payload_as::<u16>().unwrap();
                types.insert(FrameType::from(frame_type));
            }
        }
        ret.push(WiphyProperty::TxFrameType(types));
    }
    if let Ok(rx_iter) = attr_handle.get_nested_attributes::<u16>(Nl80211Attribute::RxFrameTypes) {
        let mut types = BTreeSet::default();
        for ft in rx_iter.iter() {
            let b = ft.get_attr_handle::<Nl80211Attribute>().unwrap();
            for b in b.iter() {
                let frame_type = b.get_payload_as::<u16>().unwrap();
                types.insert(FrameType::from(frame_type));
            }
        }
        ret.push(WiphyProperty::RxFrameType(types));
    }
    if let Ok(cmd_iter) =
        attr_handle.get_nested_attributes::<u16>(Nl80211Attribute::SupportedCommands)
    {
        let mut commands = BTreeSet::default();
        for cmd in cmd_iter.iter() {
            let command = cmd.get_payload_as::<Nl80211Command>().unwrap();
            if matches!(command, Nl80211Command::UnrecognizedConst(_)) {
                log::debug!("Unrecognized supported command {command:?}");
                continue;
            }
            commands.insert(command);
        }
        ret.push(WiphyProperty::SupportedCommands(commands));
    }
    if let Ok(ext_features) =
        attr_handle.get_attr_payload_as_with_len::<Vec<u8>>(Nl80211Attribute::ExtFeatures)
    {
        ret.push(WiphyProperty::ExtFeatures(
            Nl80211ExtFeatureIndex::from_bit_array(&ext_features),
        ));
    }
    if let Ok(ap_sme) = attr_handle.get_attr_payload_as::<u32>(Nl80211Attribute::DeviceApSme) {
        let mut ap_sme_properties = BTreeSet::default();
        if ap_sme & Nl80211ApSmeFeatures::SaQueryOffload as u32 > 0 {
            ap_sme_properties.insert(Nl80211ApSmeFeatures::SaQueryOffload);
        }
        ret.push(WiphyProperty::ApSme(ap_sme_properties));
    }
    ret
}

#[derive(Copy, Clone, Hash, PartialEq, Eq, Ord, PartialOrd)]
pub struct FrameType {
    pub ftype: u16,
    pub stype: u16,
}

impl FrameType {
    pub const MGMT_AUTH: Self = Self {
        ftype: IEEE80211_FTYPE_MGMT,
        stype: IEEE80211_STYPE_AUTH,
    };

    pub const fn as_u16(&self) -> u16 {
        self.ftype | self.stype
    }
}

impl From<u16> for FrameType {
    fn from(val: u16) -> Self {
        let ftype = val & 0xf;
        let stype = val & !(0xf);

        let ret = Self { ftype, stype };
        assert_eq!(ret.as_u16(), val);

        ret
    }
}

impl std::fmt::Debug for FrameType {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        let ftype = match self.ftype {
            v if v == IEEE80211_FTYPE_MGMT => "mgmt",
            v if v == IEEE80211_FTYPE_CTL => "ctl",
            v if v == IEEE80211_FTYPE_DATA => "data",
            v if v == IEEE80211_FTYPE_EXT => "ext",
            _ => "unknown",
        };
        let stype = match self.stype {
            v if v == IEEE80211_STYPE_ASSOC_REQ => "assoc_req",
            v if v == IEEE80211_STYPE_ASSOC_RESP => "assoc_resp",
            v if v == IEEE80211_STYPE_REASSOC_REQ => "reassoc_req",
            v if v == IEEE80211_STYPE_REASSOC_RESP => "reassoc_resp",
            v if v == IEEE80211_STYPE_PROBE_REQ => "probe_req",
            v if v == IEEE80211_STYPE_PROBE_RESP => "probe_resp",
            v if v == IEEE80211_STYPE_BEACON => "beacon",
            v if v == IEEE80211_STYPE_ATIM => "atim",
            v if v == IEEE80211_STYPE_DISASSOC => "disassoc",
            v if v == IEEE80211_STYPE_AUTH => "auth",
            v if v == IEEE80211_STYPE_DEAUTH => "deauth",
            v if v == IEEE80211_STYPE_ACTION => "action",
            v if v == IEEE80211_STYPE_TRIGGER => "trigger",
            v if v == IEEE80211_STYPE_CTL_EXT => "ctl_ext",
            v if v == IEEE80211_STYPE_BACK_REQ => "back_req",
            v if v == IEEE80211_STYPE_BACK => "back",
            v if v == IEEE80211_STYPE_PSPOLL => "pspoll",
            v if v == IEEE80211_STYPE_RTS => "rts",
            v if v == IEEE80211_STYPE_CTS => "cts",
            v if v == IEEE80211_STYPE_ACK => "ack",
            v if v == IEEE80211_STYPE_CFEND => "cfend",
            v if v == IEEE80211_STYPE_CFENDACK => "cfendack",
            v if v == IEEE80211_STYPE_DATA => "data",
            v if v == IEEE80211_STYPE_DATA_CFACK => "data_cfack",
            v if v == IEEE80211_STYPE_DATA_CFPOLL => "data_cfpoll",
            v if v == IEEE80211_STYPE_DATA_CFACKPOLL => "data_cfackpoll",
            v if v == IEEE80211_STYPE_NULLFUNC => "nullfunc",
            v if v == IEEE80211_STYPE_CFACK => "cfack",
            v if v == IEEE80211_STYPE_CFPOLL => "cfpoll",
            v if v == IEEE80211_STYPE_CFACKPOLL => "cfackpoll",
            v if v == IEEE80211_STYPE_QOS_DATA => "qos_data",
            v if v == IEEE80211_STYPE_QOS_DATA_CFACK => "qos_data_cfack",
            v if v == IEEE80211_STYPE_QOS_DATA_CFPOLL => "qos_data_cfpoll",
            v if v == IEEE80211_STYPE_QOS_DATA_CFACKPOLL => "qos_data_cfackpoll",
            v if v == IEEE80211_STYPE_QOS_NULLFUNC => "qos_nullfunc",
            v if v == IEEE80211_STYPE_QOS_CFACK => "qos_cfack",
            v if v == IEEE80211_STYPE_QOS_CFPOLL => "qos_cfpoll",
            v if v == IEEE80211_STYPE_QOS_CFACKPOLL => "qos_cfackpoll",
            v if v == IEEE80211_STYPE_DMG_BEACON => "dmg_beacon",
            v if v == IEEE80211_STYPE_S1G_BEACON => "s1g_beacon",
            _ => "unknown",
        };
        fmt.debug_struct("FrameType")
            .field("ftype", &ftype)
            .field("stype", &stype)
            .finish()
    }
}

pub const IEEE80211_FTYPE_MGMT: u16 = 0;
pub const IEEE80211_FTYPE_CTL: u16 = 4;
pub const IEEE80211_FTYPE_DATA: u16 = 8;
pub const IEEE80211_FTYPE_EXT: u16 = 12;
pub const IEEE80211_STYPE_ASSOC_REQ: u16 = 0;
pub const IEEE80211_STYPE_ASSOC_RESP: u16 = 16;
pub const IEEE80211_STYPE_REASSOC_REQ: u16 = 32;
pub const IEEE80211_STYPE_REASSOC_RESP: u16 = 48;
pub const IEEE80211_STYPE_PROBE_REQ: u16 = 64;
pub const IEEE80211_STYPE_PROBE_RESP: u16 = 80;
pub const IEEE80211_STYPE_BEACON: u16 = 128;
pub const IEEE80211_STYPE_ATIM: u16 = 144;
pub const IEEE80211_STYPE_DISASSOC: u16 = 160;
pub const IEEE80211_STYPE_AUTH: u16 = 176;
pub const IEEE80211_STYPE_DEAUTH: u16 = 192;
pub const IEEE80211_STYPE_ACTION: u16 = 208;
pub const IEEE80211_STYPE_TRIGGER: u16 = 32;
pub const IEEE80211_STYPE_CTL_EXT: u16 = 96;
pub const IEEE80211_STYPE_BACK_REQ: u16 = 128;
pub const IEEE80211_STYPE_BACK: u16 = 144;
pub const IEEE80211_STYPE_PSPOLL: u16 = 160;
pub const IEEE80211_STYPE_RTS: u16 = 176;
pub const IEEE80211_STYPE_CTS: u16 = 192;
pub const IEEE80211_STYPE_ACK: u16 = 208;
pub const IEEE80211_STYPE_CFEND: u16 = 224;
pub const IEEE80211_STYPE_CFENDACK: u16 = 240;
pub const IEEE80211_STYPE_DATA: u16 = 0;
pub const IEEE80211_STYPE_DATA_CFACK: u16 = 16;
pub const IEEE80211_STYPE_DATA_CFPOLL: u16 = 32;
pub const IEEE80211_STYPE_DATA_CFACKPOLL: u16 = 48;
pub const IEEE80211_STYPE_NULLFUNC: u16 = 64;
pub const IEEE80211_STYPE_CFACK: u16 = 80;
pub const IEEE80211_STYPE_CFPOLL: u16 = 96;
pub const IEEE80211_STYPE_CFACKPOLL: u16 = 112;
pub const IEEE80211_STYPE_QOS_DATA: u16 = 128;
pub const IEEE80211_STYPE_QOS_DATA_CFACK: u16 = 144;
pub const IEEE80211_STYPE_QOS_DATA_CFPOLL: u16 = 160;
pub const IEEE80211_STYPE_QOS_DATA_CFACKPOLL: u16 = 176;
pub const IEEE80211_STYPE_QOS_NULLFUNC: u16 = 192;
pub const IEEE80211_STYPE_QOS_CFACK: u16 = 208;
pub const IEEE80211_STYPE_QOS_CFPOLL: u16 = 224;
pub const IEEE80211_STYPE_QOS_CFACKPOLL: u16 = 240;
pub const IEEE80211_STYPE_DMG_BEACON: u16 = 0;
pub const IEEE80211_STYPE_S1G_BEACON: u16 = 16;

#[derive(Debug)]
pub struct Wiphy {
    pub name: String,
    pub index: u32,
    pub bands: Vec<Band>,
    pub feature_flags: u32,
    pub supports_t_dls: bool,
    pub retry_short: u8,
    pub retry_long: u8,
    pub frag_threshold: u32,
    pub rts_threshold: u32,
    pub coverage_class: u8,
    pub max_num_scan_ssids: u8,
    pub max_num_sched_scan_ssids: u8,
    pub generation: u32,
    pub max_scan_ie_len: u8,
    pub max_sched_scan_ie_len: u8,
    pub max_match_sets: u8,
    pub support_ibss_rsn: bool,
    pub tdls_external_setup: bool,
    pub cipher_suites: Vec<u32>,
    pub max_num_pmkids: u8,
    // Protocols other than PAE are supported.
    pub control_port_ethertype: bool,
    pub wiphy_antenna_avail_tx: u32,
    pub wiphy_antenna_avail_rx: u32,
    pub wiphy_antenna_tx: Option<u32>,
    pub wiphy_antenna_rx: Option<u32>,
    pub supported_iftypes: u32,
    pub wiphy_bands: u32,
    pub supported_commands: BTreeSet<Nl80211Command>,
    pub ext_features: BTreeSet<Nl80211ExtFeatureIndex>,
    pub max_remain_on_channel_duration: u32,
    pub offchannel_tx_ok: bool,
    //    pub software_iftypes: Option<u32>,
    pub interface_combinations: u32,
    //    pub ht_capability_mask: u32,
    pub supported_tx_frames: BTreeSet<FrameType>,
    pub supported_rx_frames: BTreeSet<FrameType>,
    pub ap_sme: Option<BTreeSet<Nl80211ApSmeFeatures>>,
}

impl TryFrom<NlMessage> for Wiphy {
    type Error = Nl80211Error;

    fn try_from(msg: NlMessage) -> std::result::Result<Self, Self::Error> {
        let extra_properties = parse_wiphy_properties(&msg);
        // Messages with the NlmF::DUMP flag end with an empty payload message
        // Don't parse message unless receive proper payload (non-error, non-empty,
        // non-ack)
        let NlPayload::Payload(payload) = msg.nl_payload() else {
            return Err(Nl80211Error::LogicError(format!(
                "No payload in msg: {msg:?}"
            )));
        };

        let attr_handle = payload.attrs().get_attr_handle();
        let bands_iter = attr_handle
            .get_nested_attributes::<Nl80211BandAttr>(Nl80211Attribute::WiphyBands)
            .unwrap();
        let mut attr_handles = attr_handle.iter().collect::<Vec<_>>();
        macro_rules! extract {
            ($attr:ident : bool) => {{
                attr_handles
                    .extract_if(.., |attr| {
                        matches!(attr.nla_type().nla_type(), Nl80211Attribute::$attr)
                    })
                    .next()
                    .map(|t| t.get_payload_as::<()>().is_ok())
                    .unwrap_or(false)
            }};
            ($attr:ident : String) => {{
                let temp = attr_handles
                    .extract_if(.., |attr| {
                        matches!(attr.nla_type().nla_type(), Nl80211Attribute::$attr)
                    })
                    .next()
                    .ok_or_else(|| {
                        Nl80211Error::LogicError(format!(
                            "No {} in Wiphy",
                            stringify!(Nl80211Attribute::$attr)
                        ))
                    })?;
                temp.get_payload_as_with_len::<String>().unwrap()
            }};
            ($attr:ident: Vec<$t:ty>) => {{
                let temp = attr_handles
                    .extract_if(.., |attr| {
                        matches!(attr.nla_type().nla_type(), Nl80211Attribute::$attr)
                    })
                    .next()
                    .ok_or_else(|| {
                        Nl80211Error::LogicError(format!(
                            "No {} in Wiphy",
                            stringify!(Nl80211Attribute::$attr)
                        ))
                    })?;
                temp.get_payload_as_with_len::<Vec<$t>>().unwrap()
            }};
            ($attr:ident : Option<$t:ty>) => {{
                attr_handles
                    .extract_if(.., |attr| {
                        matches!(attr.nla_type().nla_type(), Nl80211Attribute::$attr)
                    })
                    .next()
                    .map(|temp| temp.get_payload_as::<$t>().unwrap())
            }};
            ($attr:ident : $t:ty) => {{
                let temp = attr_handles
                    .extract_if(.., |attr| {
                        matches!(attr.nla_type().nla_type(), Nl80211Attribute::$attr)
                    })
                    .next()
                    .ok_or_else(|| {
                        Nl80211Error::LogicError(format!(
                            "No {} in Wiphy",
                            stringify!(Nl80211Attribute::$attr)
                        ))
                    })?;
                temp.get_payload_as::<$t>().unwrap()
            }};
        }
        let index = extract!(Wiphy: u32);
        let name = extract!(WiphyName: String);
        // See https://github.com/ioraff/eiwd/blob/74a4bf4d93feb0fc44f399cad6786b19ddeb4372/monitor/nlmon.c#L5800
        // And https://lxr.openwrt.org/source/iwinfo/iwinfo_nl80211.c#L2551

        let mut bands = vec![];
        for b in bands_iter.iter() {
            let index = u16::from(b.nla_type()).into();
            let b = b.get_attr_handle::<Nl80211BandAttr>().unwrap();
            let ht_mcs_set = b
                .get_attr_payload_as_with_len::<Vec<u8>>(Nl80211BandAttr::HtMcsSet)
                .unwrap();
            let ht_capa = b
                .get_attr_payload_as::<u16>(Nl80211BandAttr::HtCapa)
                .unwrap();
            let ht_ampdu_factor = b
                .get_attr_payload_as::<u8>(Nl80211BandAttr::HtAmpduFactor)
                .unwrap();
            let ht_ampdu_density = b
                .get_attr_payload_as::<u8>(Nl80211BandAttr::HtAmpduDensity)
                .unwrap();

            let mut freqs = vec![];
            for f in b
                .get_nested_attributes::<Nl80211FreqAttr>(Nl80211BandAttr::Freqs)
                .unwrap()
                .iter()
            {
                let f = f.get_attr_handle::<Nl80211FreqAttr>().unwrap();
                let disabled = f
                    .get_attr_payload_as::<()>(Nl80211FreqAttr::Disabled)
                    .is_ok();
                // TODO: rest of flags
                let no_ir = f.get_attr_payload_as::<()>(Nl80211FreqAttr::NoIr).is_ok();
                let freq = f.get_attr_payload_as::<u32>(Nl80211FreqAttr::Freq).unwrap();
                let offset = f
                    .get_attr_payload_as::<u32>(Nl80211FreqAttr::Offset)
                    .unwrap();
                let max_tx_power = f
                    .get_attr_payload_as::<u32>(Nl80211FreqAttr::MaxTxPower)
                    .unwrap();
                freqs.push(Frequency {
                    freq,
                    disabled,
                    no_ir,
                    offset,
                    max_tx_power,
                });
            }

            let mut rates = vec![];
            for r in b
                .get_nested_attributes::<Nl80211BitrateAttr>(Nl80211BandAttr::Rates)
                .unwrap()
                .iter()
            {
                let r = r.get_attr_handle::<Nl80211BitrateAttr>().unwrap();
                let rate = r
                    .get_attr_payload_as::<u32>(Nl80211BitrateAttr::Rate)
                    .unwrap();
                let short_preamble = r
                    .get_attr_payload_as::<()>(Nl80211BitrateAttr::_2ghzShortpreamble)
                    .is_ok();
                rates.push(Rate {
                    rate,
                    short_preamble,
                });
            }
            bands.push(Band {
                index,
                freqs,
                rates,
                ht_mcs_set: ht_mcs_set.try_into().unwrap(),
                ht_capa,
                ht_ampdu_factor,
                ht_ampdu_density,
            });
        }

        let feature_flags = extract!(FeatureFlags: u32);
        let supports_t_dls = extract!(TdlsSupport: bool);
        let retry_short = extract!(WiphyRetryShort: u8);
        let retry_long = extract!(WiphyRetryLong: u8);
        let frag_threshold = extract!(WiphyFragThreshold: u32);
        let rts_threshold = extract!(WiphyRtsThreshold: u32);
        let coverage_class = extract!(WiphyCoverageClass: u8);
        let max_num_scan_ssids = extract!(MaxNumScanSsids: u8);
        let max_num_sched_scan_ssids = extract!(MaxNumSchedScanSsids: u8);
        let generation = extract!(Generation: u32);
        let max_scan_ie_len = extract!(MaxScanIeLen: u8);
        let max_sched_scan_ie_len = extract!(MaxSchedScanIeLen: u8);
        let max_match_sets = extract!(MaxMatchSets: u8);
        let support_ibss_rsn = extract!(SupportIbssRsn: bool);
        let tdls_external_setup = extract!(TdlsExternalSetup: bool);
        let cipher_suites = extract!(CipherSuites: Vec<u32>);
        let max_num_pmkids = extract!(MaxNumPmkids: u8);
        let control_port_ethertype = extract!(ControlPortEthertype: bool);
        let wiphy_antenna_avail_tx = extract!(WiphyAntennaAvailTx: u32);
        let wiphy_antenna_avail_rx = extract!(WiphyAntennaAvailRx: u32);
        let wiphy_antenna_tx = extract!(WiphyAntennaTx: Option<u32>);
        let wiphy_antenna_rx = extract!(WiphyAntennaRx: Option<u32>);
        let supported_iftypes = extract!(SupportedIftypes: u32);
        let wiphy_bands = extract!(WiphyBands: u32);
        let max_remain_on_channel_duration = extract!(MaxRemainOnChannelDuration: u32);
        let offchannel_tx_ok = extract!(OffchannelTxOk: bool);
        // let software_iftypes = extract!(SoftwareIftypes: Option<u32>);
        let interface_combinations = extract!(InterfaceCombinations: u32);
        //        let ht_capability_mask = extract!(HtCapabilityMask: u32);
        let mut ret = Self {
            name,
            index,
            bands,
            feature_flags,
            supports_t_dls,
            retry_short,
            retry_long,
            frag_threshold,
            rts_threshold,
            coverage_class,
            max_num_scan_ssids,
            max_num_sched_scan_ssids,
            generation,
            max_scan_ie_len,
            max_sched_scan_ie_len,
            max_match_sets,
            support_ibss_rsn,
            tdls_external_setup,
            cipher_suites,
            max_num_pmkids,
            control_port_ethertype,
            wiphy_antenna_avail_tx,
            wiphy_antenna_avail_rx,
            wiphy_antenna_tx,
            wiphy_antenna_rx,
            supported_iftypes,
            wiphy_bands,
            supported_commands: BTreeSet::default(),
            ext_features: BTreeSet::default(),
            max_remain_on_channel_duration,
            offchannel_tx_ok,
            // software_iftypes,
            interface_combinations,
            //ht_capability_mask,
            supported_tx_frames: BTreeSet::default(),
            supported_rx_frames: BTreeSet::default(),
            ap_sme: None,
        };
        ret.set_properties(extra_properties);
        Ok(ret)
    }
}

impl From<&Wiphy> for virtio_wifi::virtio_wifi_config {
    fn from(wiphy: &Wiphy) -> Self {
        let flags = u32::from(wiphy.ap_sme.is_some()) * virtio_wifi::VIRTIO_WIFI_FLAG_AP_SME;
        Self {
            // Must be set by caller.
            num_of_ifaces: 0.into(),
            available_antennas_tx: wiphy.wiphy_antenna_avail_tx.into(),
            available_antennas_rx: wiphy.wiphy_antenna_avail_rx.into(),
            n_cipher_suites: (wiphy.cipher_suites.len() as u32).into(),
            flags: flags.into(),
            iftypes: wiphy.supported_iftypes.into(),
        }
    }
}

macro_rules! supported {
    ($($f:ident => Nl80211Command::$cmd:ident),*$(,)?) => {
        $(
            #[inline]
            pub fn $f(&self) -> bool {
                self.supported_commands
                    .contains(&Nl80211Command::$cmd)
            }
        )*

    };
}

impl Wiphy {
    pub fn set_properties(&mut self, properties: Vec<WiphyProperty>) {
        log::debug!("setting wiphy {} properties: {:?}", self.index, &properties);
        for prop in properties {
            match prop {
                WiphyProperty::TxFrameType(types) => {
                    self.supported_tx_frames.extend(types);
                }
                WiphyProperty::RxFrameType(types) => {
                    self.supported_rx_frames.extend(types);
                }
                WiphyProperty::SupportedCommands(commands) => {
                    self.supported_commands.extend(commands);
                }
                WiphyProperty::ExtFeatures(ext_features) => {
                    self.ext_features.extend(ext_features);
                }
                WiphyProperty::ApSme(val) => {
                    let ap_sme = self.ap_sme.get_or_insert_default();
                    log::warn!("ApSme = {val:?}");
                    ap_sme.extend(val);
                }
            }
        }
    }

    supported! {
        auth_supported => Nl80211Command::Authenticate,
        deauth_supported => Nl80211Command::Deauthenticate,
        connect_supported => Nl80211Command::Connect,
        disconnect_supported => Nl80211Command::Disconnect,
        set_interface_supported => Nl80211Command::SetInterface,
        start_ap_supported => Nl80211Command::StartAp,
        register_frame_supported => Nl80211Command::Frame,
    }
}
