use neli::{
    attr::Attribute,
    consts::nl::GenlId,
    genl::{GenlAttrHandle, Genlmsghdr},
    nl::Nlmsghdr,
};

use crate::virtio_wifi;

#[neli::neli_enum(serialized_type = "u16")]
pub enum Nl80211BSS {
    /// `NL80211_BSS_INVALID`
    Invalid = 0,
    /// `NL80211_BSS_BSSID`
    Bssid = 1,
    /// `NL80211_BSS_FREQUENCY`
    Frequency = 2,
    /// `NL80211_BSS_TSF`
    Tsf = 3,
    /// `NL80211_BSS_BEACON_INTERVAL`
    BeaconInterval = 4,
    /// `NL80211_BSS_CAPABILITY`
    Capability = 5,
    /// `NL80211_BSS_INFORMATION_ELEMENTS`
    InformationElements = 6,
    /// `NL80211_BSS_SIGNAL_MBM`
    SignalMbm = 7,
    /// `NL80211_BSS_SIGNAL_UNSPEC`
    SignalUnspec = 8,
    /// `NL80211_BSS_STATUS`
    Status = 9,
    /// `NL80211_BSS_SEEN_MS_AGO`
    SeenMsAgo = 10,
    /// `NL80211_BSS_BEACON_IES`
    BeaconIes = 11,
    /// `NL80211_BSS_CHAN_WIDTH`
    ChanWidth = 12,
    /// `NL80211_BSS_BEACON_TSF`
    BeaconTsf = 13,
    /// `NL80211_BSS_PRESP_DATA`
    PrespData = 14,
    /// `NL80211_BSS_LAST_SEEN_BOOTTIME`
    LastSeenBoottime = 15,
    /// `NL80211_BSS_PAD`
    Pad = 16,
    /// `NL80211_BSS_PARENT_TSF`
    ParentTsf = 17,
    /// `NL80211_BSS_PARENT_BSSID`
    ParentBssid = 18,
    /// `NL80211_BSS_CHAIN_SIGNAL`
    ChainSignal = 19,
    /// `NL80211_BSS_FREQUENCY_OFFSET`
    FrequencyOffset = 20,
    /// `NL80211_BSS_MLO_LINK_ID`
    MloLinkId = 21,
    /// `NL80211_BSS_MLD_ADDR`
    MldAddr = 22,
    /// `NL80211_BSS_USE_FOR`
    UseFor = 23,
    /// `NL80211_BSS_CANNOT_USE_REASONS`
    CannotUseReasons = 24,
    /// `NL80211_BSS_AFTER_LAST`
    AfterLast = 25,
}

impl neli::consts::genl::NlAttrType for Nl80211BSS {}
pub struct BSS {
    pub bssid: [u8; 6],
    pub frequency: u32,
    pub frequency_offset: u32,
    /// signal strength of probe response/beacon in mBm (`100 * dBm`) (`i32`)
    pub signal_mbm: u32,
    pub seen_ms_ago: u32,
    pub tsf: u64,
    pub beacon_interval: u16,
    /// (not present if no beacon frame has been received yet)
    pub beacon_tsf: u64,
    /// capability field (CPU order, u16)
    pub capability: u16,
    /// u32 bitmap attribute indicating what the BSS can be used for, see `&enum nl80211_bss_use_for`.
    pub use_for: u32,
    pub information_elements: Vec<u8>,
}

impl TryFrom<GenlAttrHandle<'_, Nl80211BSS>> for BSS {
    type Error = ();

    fn try_from(
        attr_handle: GenlAttrHandle<'_, Nl80211BSS>,
    ) -> std::result::Result<Self, Self::Error> {
        // Messages with the NlmF::DUMP flag end with an empty payload message
        // Don't parse message unless receive proper payload (non-error, non-empty,
        // non-ack)
        let mut attr_handles = attr_handle.iter().collect::<Vec<_>>();
        macro_rules! extract {
            ($attr:ident: bool) => {{
                attr_handles
                    .extract_if(.., |attr| {
                        matches!(attr.nla_type().nla_type(), Nl80211BSS::$attr)
                    })
                    .next()
                    .map(|t| t.get_payload_as::<()>().is_ok())
                    .unwrap_or(false)
            }};
            ($attr:ident: Vec<u8>) => {{
                let temp = attr_handles
                    .extract_if(.., |attr| {
                        matches!(attr.nla_type().nla_type(), Nl80211BSS::$attr)
                    })
                    .next()
                    .ok_or(())?;
                temp.get_payload_as_with_len::<Vec<u8>>().unwrap()
            }};
            ($attr:ident: $t:ty) => {{
                let temp = attr_handles
                    .extract_if(.., |attr| {
                        matches!(attr.nla_type().nla_type(), Nl80211BSS::$attr)
                    })
                    .next()
                    .ok_or(())?;
                temp.get_payload_as::<$t>().unwrap()
            }};
        }
        let bssid = extract!(Bssid: Vec<u8>).try_into().map_err(|_| ())?;
        let information_elements = extract!(InformationElements: Vec<u8>);
        let frequency = extract!(Frequency: u32);
        let frequency_offset = extract!(FrequencyOffset: u32);
        let signal_mbm = extract!(SignalMbm: u32);
        let seen_ms_ago = extract!(SeenMsAgo: u32);
        let tsf = extract!(Tsf: u64);
        let beacon_interval = extract!(BeaconInterval: u16);
        let beacon_tsf = extract!(BeaconTsf: u64);
        let capability = extract!(Capability: u16);
        let use_for = extract!(UseFor: u32);
        Ok(Self {
            bssid,
            frequency,
            frequency_offset,
            signal_mbm,
            seen_ms_ago,
            tsf,
            beacon_interval,
            beacon_tsf,
            capability,
            use_for,
            information_elements,
        })
    }
}

impl From<&BSS> for virtio_wifi::VirtioWifiScanResult {
    fn from(bss: &BSS) -> Self {
        Self {
            hdr: virtio_wifi::VirtioWifiEvent {
                code: (virtio_wifi::VirtioWifiEventCode::ScanResult as u32).into(),
                ..Default::default()
            },
            tsf: bss.tsf.into(),
            capability: bss.capability.into(),
            beacon_interval: bss.beacon_interval.into(),
            bssid: bss.bssid.into(),
            center_freq: bss.frequency.into(),
            signal: bss.signal_mbm.into(),
            ielen: u64::try_from(bss.information_elements.len())
                .unwrap()
                .into(),
            ..Default::default()
        }
    }
}
