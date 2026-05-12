// SPDX-License-Identifier: Apache-2.0 or BSD-3-Clause
// Copyright 2026 Panasonic Automotive Systems Co., Ltd.
// Author: Manos Pitsidianakis <manos.pitsidianakis@linaro.org>

use crate::bindings::virtio_wifi_uapi;
use vm_memory::{Be32, ByteValued, Le16, Le32, Le64};

// virtqueues

pub const VIRTIO_WIFI_RADIO_F_REG_STRICT: u32 = 1;
pub const VIRTIO_WIFI_RADIO_F_P2P_DEVICE: u32 = 2;
pub const VIRTIO_WIFI_RADIO_F_USE_CHANCTX: u32 = 4;
pub const VIRTIO_WIFI_RADIO_F_MULTI_RADIO: u32 = 8;
pub const VIRTIO_WIFI_RADIO_F_DESTROY_ON_CLOSE: u32 = 16;
pub const VIRTIO_WIFI_RADIO_F_NO_VIF: u32 = 32;
pub const VIRTIO_WIFI_RADIO_F_MLO: u32 = 64;
pub const VIRTIO_WIFI_RADIO_F_NAN_DEVICE: u32 = 128;

#[doc(alias = "virtio_wifi_config")]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct VirtioWifiConfig {
    pub phys: Le32,
}

// SAFETY: This struct is plain-old-data.
unsafe impl ByteValued for VirtioWifiConfig {}

pub const VIRTIO_WIFI_VQ_CONTROL: u32 = 0;
pub const VIRTIO_WIFI_VQ_EVENT: u32 = 1;
pub const VIRTIO_WIFI_VQ_TX: u32 = 2;
pub const VIRTIO_WIFI_VQ_RX: u32 = 3;
pub const VIRTIO_WIFI_VQ_MAX: u32 = 4;

pub const VIRTIO_WIFI_CMD_GET_WIPHY: u32 = virtio_wifi_uapi::VIRTIO_WIFI_CMD_GET_WIPHY;
pub const VIRTIO_WIFI_CMD_SET_WIPHY: u32 = virtio_wifi_uapi::VIRTIO_WIFI_CMD_SET_WIPHY;
pub const VIRTIO_WIFI_CMD_GET_INTERFACE: u32 = virtio_wifi_uapi::VIRTIO_WIFI_CMD_GET_INTERFACE;
pub const VIRTIO_WIFI_CMD_NEW_INTERFACE: u32 = virtio_wifi_uapi::VIRTIO_WIFI_CMD_NEW_INTERFACE;
pub const VIRTIO_WIFI_CMD_DELETE_INTERFACE: u32 =
    virtio_wifi_uapi::VIRTIO_WIFI_CMD_DELETE_INTERFACE;
pub const VIRTIO_WIFI_CMD_GET_BAND: u32 = virtio_wifi_uapi::VIRTIO_WIFI_CMD_GET_BAND;
pub const VIRTIO_WIFI_CMD_GET_BAND_CHANNEL: u32 =
    virtio_wifi_uapi::VIRTIO_WIFI_CMD_GET_BAND_CHANNEL;
pub const VIRTIO_WIFI_CMD_GET_BAND_BITRATE: u32 =
    virtio_wifi_uapi::VIRTIO_WIFI_CMD_GET_BAND_BITRATE;
pub const VIRTIO_WIFI_CMD_TRIGGER_SCAN: u32 = virtio_wifi_uapi::VIRTIO_WIFI_CMD_TRIGGER_SCAN;
pub const VIRTIO_WIFI_CMD_GET_CIPHERSUITES: u32 =
    virtio_wifi_uapi::VIRTIO_WIFI_CMD_GET_CIPHERSUITES;
pub const VIRTIO_WIFI_CMD_AUTHENTICATE: u32 = virtio_wifi_uapi::VIRTIO_WIFI_CMD_AUTHENTICATE;

pub const VIRTIO_WIFI_S_OK: u32 = virtio_wifi_uapi::VIRTIO_WIFI_S_OK;
pub const VIRTIO_WIFI_S_BAD_MSG: u32 = virtio_wifi_uapi::VIRTIO_WIFI_S_BAD_MSG;
pub const VIRTIO_WIFI_S_NOT_SUPP: u32 = virtio_wifi_uapi::VIRTIO_WIFI_S_NOT_SUPP;
pub const VIRTIO_WIFI_S_IO_ERR: u32 = virtio_wifi_uapi::VIRTIO_WIFI_S_IO_ERR;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct VirtioWifiHdr {
    pub code: Le32,
    pub index: Le32,
}

// SAFETY: This struct is plain-old-data.
unsafe impl ByteValued for VirtioWifiHdr {}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct VirtioWifiCmdRequest {
    pub hdr: VirtioWifiHdr,
}

// SAFETY: This struct is plain-old-data.
unsafe impl ByteValued for VirtioWifiCmdRequest {}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct VirtioWifiRadio {
    pub hdr: VirtioWifiHdr,
    pub ifaces: Le32,
    pub flags: Le32,
    pub reserved: Le32,
    pub hwname_len: Le32,
    pub hwname: [u8; 64usize],
    pub iftypes: Le32,
    pub available_antennas_tx: Le32,
    pub available_antennas_rx: Le32,
    pub n_cipher_suites: Le32,
}

// SAFETY: This struct is plain-old-data.
unsafe impl ByteValued for VirtioWifiRadio {}

impl Default for VirtioWifiRadio {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        // SAFETY: Self is plain-old-data
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct VirtioWifiCmdGetInterfaceRequest {
    pub hdr: VirtioWifiHdr,
    pub phy_index: Le32,
}

// SAFETY: This struct is plain-old-data.
unsafe impl ByteValued for VirtioWifiCmdGetInterfaceRequest {}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct VirtioWifiInterfaceData {
    pub hdr: VirtioWifiHdr,
    pub ifname_len: Le32,
    pub ifname: [u8; 64usize],
    pub iftype: Le32,
}

// SAFETY: This struct is plain-old-data.
unsafe impl ByteValued for VirtioWifiInterfaceData {}

impl Default for VirtioWifiInterfaceData {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        // SAFETY: Self is plain-old-data
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}

pub const VIRTIO_WIFI_IFTYPE_UNSPECIFIED: u32 =
    virtio_wifi_uapi::virtio_wifi_iftype::VIRTIO_WIFI_IFTYPE_UNSPECIFIED as u32;
pub const VIRTIO_WIFI_IFTYPE_ADHOC: u32 =
    virtio_wifi_uapi::virtio_wifi_iftype::VIRTIO_WIFI_IFTYPE_ADHOC as u32;
pub const VIRTIO_WIFI_IFTYPE_STATION: u32 =
    virtio_wifi_uapi::virtio_wifi_iftype::VIRTIO_WIFI_IFTYPE_STATION as u32;
pub const VIRTIO_WIFI_IFTYPE_AP: u32 =
    virtio_wifi_uapi::virtio_wifi_iftype::VIRTIO_WIFI_IFTYPE_AP as u32;
pub const VIRTIO_WIFI_IFTYPE_AP_VLAN: u32 =
    virtio_wifi_uapi::virtio_wifi_iftype::VIRTIO_WIFI_IFTYPE_AP_VLAN as u32;
pub const VIRTIO_WIFI_IFTYPE_WDS: u32 =
    virtio_wifi_uapi::virtio_wifi_iftype::VIRTIO_WIFI_IFTYPE_WDS as u32;
pub const VIRTIO_WIFI_IFTYPE_MONITOR: u32 =
    virtio_wifi_uapi::virtio_wifi_iftype::VIRTIO_WIFI_IFTYPE_MONITOR as u32;
pub const VIRTIO_WIFI_IFTYPE_MESH_POINT: u32 =
    virtio_wifi_uapi::virtio_wifi_iftype::VIRTIO_WIFI_IFTYPE_MESH_POINT as u32;
pub const VIRTIO_WIFI_IFTYPE_P2P_CLIENT: u32 =
    virtio_wifi_uapi::virtio_wifi_iftype::VIRTIO_WIFI_IFTYPE_P2P_CLIENT as u32;
pub const VIRTIO_WIFI_IFTYPE_P2P_GO: u32 =
    virtio_wifi_uapi::virtio_wifi_iftype::VIRTIO_WIFI_IFTYPE_P2P_GO as u32;
pub const VIRTIO_WIFI_IFTYPE_P2P_DEVICE: u32 =
    virtio_wifi_uapi::virtio_wifi_iftype::VIRTIO_WIFI_IFTYPE_P2P_DEVICE as u32;
pub const VIRTIO_WIFI_IFTYPE_OCB: u32 =
    virtio_wifi_uapi::virtio_wifi_iftype::VIRTIO_WIFI_IFTYPE_OCB as u32;
pub const VIRTIO_WIFI_IFTYPE_NAN: u32 =
    virtio_wifi_uapi::virtio_wifi_iftype::VIRTIO_WIFI_IFTYPE_NAN as u32;
pub const VIRTIO_WIFI_IFTYPE_MAX: u32 =
    virtio_wifi_uapi::virtio_wifi_iftype::VIRTIO_WIFI_IFTYPE_MAX as u32;

#[derive(Copy, Clone, Debug)]
#[repr(u32)]
pub enum IfType {
    Unspecified = VIRTIO_WIFI_IFTYPE_UNSPECIFIED,
    Adhoc = VIRTIO_WIFI_IFTYPE_ADHOC,
    Station = VIRTIO_WIFI_IFTYPE_STATION,
    Ap = VIRTIO_WIFI_IFTYPE_AP,
    ApVlan = VIRTIO_WIFI_IFTYPE_AP_VLAN,
    Wds = VIRTIO_WIFI_IFTYPE_WDS,
    Monitor = VIRTIO_WIFI_IFTYPE_MONITOR,
    MeshPoint = VIRTIO_WIFI_IFTYPE_MESH_POINT,
    P2pClient = VIRTIO_WIFI_IFTYPE_P2P_CLIENT,
    P2pGo = VIRTIO_WIFI_IFTYPE_P2P_GO,
    P2pDevice = VIRTIO_WIFI_IFTYPE_P2P_DEVICE,
    Ocb = VIRTIO_WIFI_IFTYPE_OCB,
    Nan = VIRTIO_WIFI_IFTYPE_NAN,
}

bitflags::bitflags! {
    pub struct IfTypes: u32 {
        const UNSPECIFIED = 1 << VIRTIO_WIFI_IFTYPE_UNSPECIFIED;
        const ADHOC = 1 << VIRTIO_WIFI_IFTYPE_ADHOC;
        const STATION =  1<<VIRTIO_WIFI_IFTYPE_STATION;
        const AP =  1<<VIRTIO_WIFI_IFTYPE_AP;
        const AP_VLAN =  1<<VIRTIO_WIFI_IFTYPE_AP_VLAN;
        const WDS =  1<<VIRTIO_WIFI_IFTYPE_WDS;
        const MONITOR =  1<<VIRTIO_WIFI_IFTYPE_MONITOR;
        const MESH_POINT =  1<<VIRTIO_WIFI_IFTYPE_MESH_POINT;
        const P2P_CLIENT =  1<<VIRTIO_WIFI_IFTYPE_P2P_CLIENT;
        const P2P_GO =  1<<VIRTIO_WIFI_IFTYPE_P2P_GO;
        const P2P_DEVICE =  1<<VIRTIO_WIFI_IFTYPE_P2P_DEVICE;
        const OCB =  1<<VIRTIO_WIFI_IFTYPE_OCB;
        const NAN =  1<<VIRTIO_WIFI_IFTYPE_NAN;
    }
}

#[doc(alias = "virtio_wifi_band")]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct VirtioWifiBand {
    pub hdr: VirtioWifiHdr,
    pub band: Le32,
    pub n_channels: Le32,
    pub n_bitrates: Le32,
    pub n_iftype_data: Le16,
    pub ht_cap: VirtioWifiStaHtCap,
    pub padding: Le16,
    pub vht_cap: VirtioWifiStaVhtCap,
    pub s1g_cap: VirtioWifiStaS1gCap,
    pub edmg_cap: VirtioWifiEdmg,
}

// SAFETY: This struct is plain-old-data.
unsafe impl ByteValued for VirtioWifiBand {}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct VirtioWifiCmdGetBandChannelRequest {
    pub hdr: VirtioWifiHdr,
    pub phy_index: Le32,
    pub band_index: Le32,
}

// SAFETY: This struct is plain-old-data.
unsafe impl ByteValued for VirtioWifiCmdGetBandChannelRequest {}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct VirtioWifiCmdGetBandBitrateRequest {
    pub hdr: VirtioWifiHdr,
    pub phy_index: Le32,
    pub band_index: Le32,
}

// SAFETY: This struct is plain-old-data.
unsafe impl ByteValued for VirtioWifiCmdGetBandBitrateRequest {}

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

#[doc(alias = "virtio_wifi_channel")]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct VirtioWifiChannel {
    pub hdr: VirtioWifiHdr,
    pub band: Le32,
    pub center_freq: Le32,
    pub center_freq1: Le32,
    pub center_freq2: Le32,
    pub freq_offset: Le16,
    pub hw_value: Le16,
    pub width: Le32,
    /* enum ieee80211_channel_flags */
    pub flags: Le32,
    pub max_antenna_gain: Le32,
    pub max_power: Le32,
    pub max_reg_power: Le32,
    /* enum nl80211_dfs_state dfs_state; */
    pub dfs_state_entered: Le32,
    pub dfs_cac_ms: Le32,
    pub beacon_found: u8,
    pub psd: u8,
    pub reserved: Le16,
}

// SAFETY: This struct is plain-old-data.
unsafe impl ByteValued for VirtioWifiChannel {}

pub enum VirtioWifiFreqBand {
    _2GHZ = 0,
    _5GHZ,
    _60GHZ,
    _6GHZ,
    S1GHZ,
    LC,
}

#[doc(alias = "virtio_wifi_band_bitrate")]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct VirtioWifiBandBitrate {
    pub hdr: VirtioWifiHdr,
    pub flags: Le32,
    pub bitrate: Le16,
    pub hw_value: Le16,
    pub hw_value_short: Le16,
    pub reserved: Le16,
}

// SAFETY: This struct is plain-old-data.
unsafe impl ByteValued for VirtioWifiBandBitrate {}

pub const ETH_ALEN: usize = 6;

pub const IEEE80211_BSS_ARP_ADDR_LIST_LEN: usize = 4;

pub const IEEE80211_MAX_SSID_LEN: usize = 32;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct VirtioWifiInterfaceCfg {
    pub flags: Le32,
    pub aid: Le16,
    pub eml_cap: Le16,
    pub eml_med_sync_delay: Le16,
    pub mld_capa_op: Le16,
    pub arp_addr_list: [Be32; IEEE80211_BSS_ARP_ADDR_LIST_LEN],
    pub arp_addr_cnt: Le32,
    pub ssid: [u8; IEEE80211_MAX_SSID_LEN],
    pub ssid_len: Le16,
    pub ap_addr: [u8; ETH_ALEN],
}

// SAFETY: This struct is plain-old-data.
unsafe impl ByteValued for VirtioWifiInterfaceCfg {}

impl Default for VirtioWifiInterfaceCfg {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        // SAFETY: Self is plain-old-data
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct VirtioWifiInterface {
    pub hdr: VirtioWifiHdr,
    pub iftype: Le32,
    pub index: Le32,
    pub cfg: VirtioWifiInterfaceCfg,
    pub addr: [u8; ETH_ALEN],
    pub reserved: Le16,
    pub driver_flags: Le32,
}

// SAFETY: This struct is plain-old-data.
unsafe impl ByteValued for VirtioWifiInterface {}

impl Default for VirtioWifiInterface {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        // SAFETY: Self is plain-old-data
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}

#[doc(alias = "virtio_wifi_configure_filter")]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct VirtioWifiConfigureFilter {
    pub hdr: VirtioWifiHdr,
    pub changed_flags: Le32,
    pub total_flags: Le32,
    pub multicast: Le64,
}

// SAFETY: This struct is plain-old-data.
unsafe impl ByteValued for VirtioWifiConfigureFilter {}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
#[doc(alias = "virtio_wifi_mcs_info")]
pub struct VirtioWifiMcsInfo {
    pub rx_highest: Le16,
    pub rx_mask: [u8; 10usize],
    pub tx_params: u8,
    pub reserved: u8,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
#[doc(alias = "virtio_wifi_vht_mcs_info")]
pub struct VirtioWifiVhtMcsInfo {
    pub rx_mcs_map: Le16,
    pub rx_highest: Le16,
    pub tx_mcs_map: Le16,
    pub tx_highest: Le16,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
#[doc(alias = "virtio_wifi_vht_cap")]
pub struct VirtioWifiVhtCap {
    pub vht_cap_info: Le32,
    pub supp_mcs: VirtioWifiVhtMcsInfo,
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
#[doc(alias = "virtio_wifi_sta_vht_cap")]
pub struct VirtioWifiStaVhtCap {
    pub vht_supported: Le32,
    pub cap: Le32,
    pub vht_mcs: VirtioWifiVhtMcsInfo,
}

#[doc(alias = "virtio_wifi_edmg_bw_config")]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u32)]
pub enum VirtioWifiEdmgBwConfig {
    _4 = 4,
    _5 = 5,
    _6 = 6,
    _7 = 7,
    _8 = 8,
    _9 = 9,
    _10 = 10,
    _11 = 11,
    _12 = 12,
    _13 = 13,
    _14 = 14,
    _15 = 15,
}

#[repr(C)]
#[doc(alias = "virtio_wifi_edmg")]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct VirtioWifiEdmg {
    pub channels: u8,
    pub padding: [u8; 3usize],
    /* VirtioWifiEdmgBwConfig */
    pub bw_config: Le32,
}

impl Default for VirtioWifiEdmg {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}

#[doc(alias = "virtio_wifi_sta_ht_cap")]
#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct VirtioWifiStaHtCap {
    pub cap: Le16,
    pub ht_supported: u8,
    pub ampdu_factor: u8,
    pub ampdu_density: u8,
    pub padding: u8,
    pub mcs: VirtioWifiMcsInfo,
}

#[doc(alias = "virtio_wifi_sta_s1g_cap")]
#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct VirtioWifiStaS1gCap {
    pub s1g: u8,
    pub cap: [u8; 10usize],
    pub nss_mcs: [u8; 5usize],
}

#[repr(u32)]
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum VirtioWifiEventCode {
    ScanResult = 0,
    EndScan = 1,
    CancelScan = 2,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct VirtioWifiEvent {
    pub code: Le32,
    pub if_index: Le32,
}

// SAFETY: This struct is plain-old-data.
unsafe impl ByteValued for VirtioWifiEvent {}

impl Default for VirtioWifiEvent {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct VirtioWifiScanResult {
    pub hdr: VirtioWifiEvent,
    pub tsf: Le64,
    pub ftype: Le32,
    pub capability: Le16,
    pub beacon_interval: Le16,
    pub band: Le32,
    pub center_freq: Le32,
    pub hw_value: Le16,
    pub bssid: [u8; ETH_ALEN],
    pub padding: Le32,
    pub signal: Le32,
    pub ielen: Le64,
}

// SAFETY: This struct is plain-old-data.
unsafe impl ByteValued for VirtioWifiScanResult {}

impl Default for VirtioWifiScanResult {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
