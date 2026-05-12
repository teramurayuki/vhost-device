use neli::{
    attr::Attribute,
    consts::nl::GenlId,
    genl::Genlmsghdr,
    nl::{NlPayload, Nlmsghdr},
};

use super::*;

#[derive(Debug)]
pub enum WiphyProperty {
    TxFrameType(Vec<FrameType>),
    RxFrameType(Vec<FrameType>),
}

pub fn parse_wiphy_properties(
    msg: &Nlmsghdr<GenlId, Genlmsghdr<Nl80211Command, Nl80211Attribute>>,
) -> Vec<WiphyProperty> {
    // Messages with the NlmF::DUMP flag end with an empty payload message
    // Don't parse message unless receive proper payload (non-error, non-empty, non-ack)
    let payload = match msg.nl_payload() {
        NlPayload::Payload(p) => p,
        _ => return vec![],
    };

    let attr_handle = payload.attrs().get_attr_handle();
    let mut ret = vec![];
    if let Ok(tx_iter) = attr_handle.get_nested_attributes::<u16>(Nl80211Attribute::TxFrameTypes) {
        let mut types = vec![];
        for ft in tx_iter.iter() {
            let _index: u16 = u16::from(ft.nla_type()).into();
            let b = ft.get_attr_handle::<Nl80211Attribute>().unwrap();
            for b in b.iter() {
                let frame_type = b.get_payload_as::<u16>().unwrap();
                types.push(FrameType::from(frame_type));
            }
        }
        ret.push(WiphyProperty::TxFrameType(types));
    }
    if let Ok(rx_iter) = attr_handle.get_nested_attributes::<u16>(Nl80211Attribute::RxFrameTypes) {
        let mut types = vec![];
        for ft in rx_iter.iter() {
            let b = ft.get_attr_handle::<Nl80211Attribute>().unwrap();
            for b in b.iter() {
                let frame_type = b.get_payload_as::<u16>().unwrap();
                types.push(FrameType::from(frame_type));
            }
        }
        ret.push(WiphyProperty::RxFrameType(types));
    }
    ret
}

#[derive(Copy, Clone, Hash, PartialEq, Eq, Ord, PartialOrd)]
pub struct FrameType {
    pub ftype: u16,
    pub stype: u16,
}

impl FrameType {
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
