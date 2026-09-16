// SPDX-License-Identifier: Apache-2.0 or BSD-3-Clause
// Copyright 2026 Panasonic Automotive Systems Co., Ltd.
// Author: Manos Pitsidianakis <manos.pitsidianakis@linaro.org>

use neli::{attr::Attribute, nl::NlPayload};

use super::*;
use crate::attr;

// enum nl80211_key_type
#[neli::neli_enum(serialized_type = "u16")]
#[repr(u32)]
pub enum Nl80211KeyType {
    /// `NL80211_KEYTYPE_GROUP`
    Group = 0,
    /// `NL80211_KEYTYPE_PAIRWISE`
    Pairwise = 1,
    /// `NL80211_KEYTYPE_PEERKEY`
    PeerKey = 2,
}

impl neli::consts::genl::NlAttrType for Nl80211KeyType {}

// enum nl80211_key_default_types
#[neli::neli_enum(serialized_type = "u16")]
#[repr(u32)]
pub enum Nl80211KeyDefaultTypes {
    Invalid = 0,
    Unicast = 1,
    Multicast = 2,
}

impl neli::consts::genl::NlAttrType for Nl80211KeyDefaultTypes {}

/// `enum nl80211_key_attributes`
#[neli::neli_enum(serialized_type = "u16")]
#[repr(u16)]
pub enum Nl80211KeyAttribute {
    Invalid = 0,
    /// temporal) key data; for TKIP this consists of 16 bytes encryption key
    /// followed by 8 bytes each for TX and RX MIC keys `NL80211_KEY_DATA`
    Data = 1,
    /// key ID (u8, 0-3)
    /// `NL80211_KEY_IDX`
    Idx = 2,
    /// key cipher suite (u32, as defined by IEEE 802.11 section 7.3.2.25.1,
    /// e.g. 0x000FAC04) `NL80211_KEY_CIPHER`
    Cipher = 3,
    /// transmit key sequence number (IV/PN) for TKIP and CCMP keys, each six
    /// bytes in little endian `NL80211_KEY_SEQ`
    Seq = 4,
    /// flag indicating default key
    /// `NL80211_KEY_DEFAULT`
    Default = 5,
    /// flag indicating default management key
    /// `NL80211_KEY_DEFAULT_MGMT`
    DefaultMgmt = 6,
    /// the key type from enum nl80211_key_type, if not specified the default
    /// depends on whether a MAC address was given with the command using the
    /// key or not (u32) `NL80211_KEY_TYPE`
    Type = 7,
    /// A nested attribute containing flags attributes, specifying what a key
    /// should be set as default as. See `enum nl80211_key_default_types`.
    /// `NL80211_KEY_DEFAULT_TYPES`
    DefaultTypes = 8,
    /// The mode from `enum nl80211_key_mode`
    /// `NL80211_KEY_MODE`
    Mode = 9,
    /// flag indicating default Beacon frame key
    /// `NL80211_KEY_DEFAULT_BEACON`
    DefaultBeacon = 10,
    /// `NL80211_KEY_LTF_SEED`
    LtfSeed = 11,
}

impl neli::consts::genl::NlAttrType for Nl80211KeyAttribute {}

/// `enum nl80211_key_mode`
#[neli::neli_enum(serialized_type = "u16")]
#[repr(u8)]
pub enum Nl80211KeyMode {
    /// `NL80211_KEY_RX_TX`
    RxTx = 0,
    /// `NL80211_KEY_NO_TX`
    NoTx = 1,
    /// `NL80211_KEY_SET_TX`
    SetTx = 2,
}

impl neli::consts::genl::NlAttrType for Nl80211KeyMode {}

pub fn key_attr_from_req(
    req: &virtio_wifi::virtio_wifi_key,
    data: Option<&[u8]>,
    seq: Option<&[u8]>,
) -> Result<neli::genl::Nlattr<Nl80211Attribute, neli::types::Buffer>, Nl80211Error> {
    let mode = match u32::from(req.mode) {
        1 => 1_u8,
        2 => 2_u8,
        _ => 0_u8,
    };

    let flags = u32::from(u16::from(req.flags));

    let def_uni = flags & virtio_wifi::VIRTIO_WIFI_KEY_F_DEFAULT_UNI > 0;
    let def_multi = flags & virtio_wifi::VIRTIO_WIFI_KEY_F_DEFAULT_MULTI > 0;
    let def_mgmt = flags & virtio_wifi::VIRTIO_WIFI_KEY_F_DEFAULT_MGMT > 0;
    let def = flags & virtio_wifi::VIRTIO_WIFI_KEY_F_DEFAULT > 0;
    let has_idx = flags & virtio_wifi::VIRTIO_WIFI_KEY_F_IDX > 0;
    let has_type = flags & virtio_wifi::VIRTIO_WIFI_KEY_F_TYPE > 0;

    let mut retval = attr!(Nl80211Attribute::Key = Vec::<u8>::new());
    retval = retval
        .nest(&attr!(Nl80211KeyAttribute::Cipher = u32::from(req.cipher)))?
        .nest(&attr!(Nl80211KeyAttribute::Mode = mode))?;
    log::debug!("mode = {mode:?}");
    if has_idx {
        retval = retval.nest(&attr!(Nl80211KeyAttribute::Idx = req.key_index))?;
        log::debug!("key_idx = {:?}", req.key_index);
    }
    if has_type {
        retval = retval.nest(&attr!(
            Nl80211KeyAttribute::Type = u32::from(req.pairwise > 0)
        ))?;
        log::debug!("type = {:?}", u32::from(req.pairwise > 0));
    }
    if let Some(data) = data {
        retval = retval.nest(&attr!(Nl80211KeyAttribute::Data = data))?;
        log::debug!("data = {:?}", data.len());
    }
    if let Some(seq) = seq {
        retval = retval.nest(&attr!(Nl80211KeyAttribute::Seq = seq))?;
        log::debug!("seq = {:?}", seq.len());
        if seq.len() > 16 {
            log::error!("invalid seq len = {:?} seq = {seq:?}", seq.len());
        }
    }
    if def {
        retval = retval.nest(&attr!(Nl80211KeyAttribute::Default = ()))?;
        log::debug!("default = set");
    }
    if def_mgmt {
        retval = retval.nest(&attr!(Nl80211KeyAttribute::DefaultMgmt = ()))?;
        log::debug!("default_mgmt = set");
    }
    if def_uni || def_multi {
        log::debug!("default_types = set");
        let mut default_types = attr!(Nl80211KeyAttribute::DefaultTypes = Vec::<u8>::new());
        if def_uni {
            default_types = default_types.nest(&attr!(Nl80211KeyDefaultTypes::Unicast = ()))?;
        }
        if def_multi {
            default_types = default_types.nest(&attr!(Nl80211KeyDefaultTypes::Multicast = ()))?;
        }
        log::debug!("default_types = {default_types:?}");
        retval = retval.nest(&default_types)?;
    }
    Ok(retval)
}

impl TryFrom<NlMessage> for virtio_wifi::virtio_wifi_key {
    type Error = Nl80211Error;

    fn try_from(msg: NlMessage) -> std::result::Result<Self, Self::Error> {
        // Messages with the NlmF::DUMP flag end with an empty payload message
        // Don't parse message unless receive proper payload (non-error, non-empty,
        // non-ack)
        let NlPayload::Payload(payload) = msg.nl_payload() else {
            return Err(Nl80211Error::LogicError(format!(
                "No payload in msg: {msg:?}"
            )));
        };

        let attr_handle = payload.attrs().get_attr_handle();
        let key_attrs = attr_handle
            .get_nested_attributes::<Nl80211KeyAttribute>(Nl80211Attribute::Key)
            .map_err(|_| {
                Nl80211Error::LogicError(format!(
                    "No {} in Key Nl80211 response",
                    stringify!(Nl80211Attribute::Key)
                ))
            })?;
        let key_attrs = key_attrs.iter().collect::<Vec<_>>();
        let cipher = key_attrs
            .clone()
            .extract_if(.., |attr| {
                matches!(attr.nla_type().nla_type(), Nl80211KeyAttribute::Cipher)
            })
            .next()
            .ok_or_else(|| {
                Nl80211Error::LogicError(format!(
                    "No {} in Key, instead got: {key_attrs:#?}",
                    stringify!(Nl80211KeyAttribute::Cipher)
                ))
            })?
            .get_payload_as::<u32>()?;
        Ok(virtio_wifi::virtio_wifi_key {
            cipher: cipher.into(),
            ..virtio_wifi::virtio_wifi_key::default()
        })
    }
}
