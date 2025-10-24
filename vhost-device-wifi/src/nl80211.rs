// SPDX-License-Identifier: Apache-2.0 or BSD-3-Clause
// Copyright 2026 Panasonic Automotive Systems Co., Ltd.
// Author: Manos Pitsidianakis <manos.pitsidianakis@linaro.org>

//! nl80211 communication

use std::{
    collections::BTreeMap,
    error::Error,
    sync::atomic::AtomicBool,
    sync::Arc, //
};

use neli::{
    attr::Attribute,
    consts::{
        nl::{GenlId, NlmF},
        rtnl::{RtAddrFamily, Rtm},
        socket::NlFamily,
    },
    genl::{AttrTypeBuilder, Genlmsghdr, GenlmsghdrBuilder, NlattrBuilder, NoUserHeader},
    nl::{NlPayload, Nlmsghdr},
    router::synchronous::{NlRouter, NlRouterReceiverHandle},
    rtnl::{Ifinfomsg, IfinfomsgBuilder},
    types::GenlBuffer,
    utils::Groups,
};
use nix::{errno::Errno, sys::eventfd::EventFd};
use vm_memory::ByteValued;

use crate::{bindings::virtio_wifi_uapi as virtio_wifi, virtio_wifi::IEEE80211ChannelFlags};

mod bss;
use bss::*;
mod mcast;
use mcast::*;
mod wiphy;
use wiphy::*;
pub use wiphy::{FrameType, Wiphy};

macro_rules! attr {
    ($t:ident::$attr:ident = $val:expr) => {{
        NlattrBuilder::default()
            .nla_type(
                AttrTypeBuilder::default()
                    .nla_type($t::$attr)
                    .build()
                    .unwrap(),
            )
            .nla_payload($val)
            .build()
            .unwrap()
    }};
}

#[neli::neli_enum(serialized_type = "u8")]
pub enum Nl80211Command {
    Unspecified = 0,
    /// `NL80211_CMD_GET_WIPHY`
    GetWiphy = 1,
    /// `NL80211_CMD_SET_WIPHY`
    SetWiphy = 2,
    /// `NL80211_CMD_NEW_WIPHY`
    NewWiphy = 3,
    /// `NL80211_CMD_DEL_WIPHY`
    DelWiphy = 4,
    /// `NL80211_CMD_GET_INTERFACE`
    GetInterface = 5,
    /// `NL80211_CMD_SET_INTERFACE`
    SetInterface = 6,
    /// `NL80211_CMD_NEW_INTERFACE`
    NewInterface = 7,
    /// `NL80211_CMD_DEL_INTERFACE`
    DelInterface = 8,
    /// `NL80211_CMD_GET_KEY`
    GetKey = 9,
    /// `NL80211_CMD_SET_KEY`
    SetKey = 10,
    /// `NL80211_CMD_NEW_KEY`
    NewKey = 11,
    /// `NL80211_CMD_DEL_KEY`
    DelKey = 12,
    /// `NL80211_CMD_GET_BEACON`
    GetBeacon = 13,
    /// `NL80211_CMD_SET_BEACON`
    SetBeacon = 14,
    /// `NL80211_CMD_START_AP`
    StartAp = 15,
    /// `NL80211_CMD_NEW_BEACON`
    NewBeacon = 15,
    /// `NL80211_CMD_STOP_AP`
    StopAp = 16,
    /// `NL80211_CMD_DEL_BEACON`
    DelBeacon = 16,
    /// `NL80211_CMD_GET_STATION`
    GetStation = 17,
    /// `NL80211_CMD_SET_STATION`
    SetStation = 18,
    /// `NL80211_CMD_NEW_STATION`
    NewStation = 19,
    /// `NL80211_CMD_DEL_STATION`
    DelStation = 20,
    /// `NL80211_CMD_GET_MPATH`
    GetMpath = 21,
    /// `NL80211_CMD_SET_MPATH`
    SetMpath = 22,
    /// `NL80211_CMD_NEW_MPATH`
    NewMpath = 23,
    /// `NL80211_CMD_DEL_MPATH`
    DelMpath = 24,
    /// `NL80211_CMD_SET_BSS`
    SetBss = 25,
    /// `NL80211_CMD_SET_REG`
    SetReg = 26,
    /// `NL80211_CMD_REQ_SET_REG`
    ReqSetReg = 27,
    /// `NL80211_CMD_GET_MESH_CONFIG`
    GetMeshConfig = 28,
    /// `NL80211_CMD_SET_MESH_CONFIG`
    SetMeshConfig = 29,
    /// `NL80211_CMD_SET_MGMT_EXTRA_IE`
    SetMgmtExtraIe = 30,
    /// `NL80211_CMD_GET_REG`
    GetReg = 31,
    /// `NL80211_CMD_GET_SCAN`
    GetScan = 32,
    /// `NL80211_CMD_TRIGGER_SCAN`
    TriggerScan = 33,
    /// `NL80211_CMD_NEW_SCAN_RESULTS`
    NewScanResults = 34,
    /// `NL80211_CMD_SCAN_ABORTED`
    ScanAborted = 35,
    /// `NL80211_CMD_REG_CHANGE`
    RegChange = 36,
    /// `NL80211_CMD_AUTHENTICATE`
    Authenticate = 37,
    /// `NL80211_CMD_ASSOCIATE`
    Associate = 38,
    /// `NL80211_CMD_DEAUTHENTICATE`
    Deauthenticate = 39,
    /// `NL80211_CMD_DISASSOCIATE`
    Disassociate = 40,
    /// `NL80211_CMD_MICHAEL_MIC_FAILURE`
    MichaelMicFailure = 41,
    /// `NL80211_CMD_REG_BEACON_HINT`
    RegBeaconHint = 42,
    /// `NL80211_CMD_JOIN_IBSS`
    JoinIbss = 43,
    /// `NL80211_CMD_LEAVE_IBSS`
    LeaveIbss = 44,
    /// `NL80211_CMD_TESTMODE`
    Testmode = 45,
    /// `NL80211_CMD_CONNECT`
    Connect = 46,
    /// `NL80211_CMD_ROAM`
    Roam = 47,
    /// `NL80211_CMD_DISCONNECT`
    Disconnect = 48,
    /// `NL80211_CMD_SET_WIPHY_NETNS`
    SetWiphyNetns = 49,
    /// `NL80211_CMD_GET_SURVEY`
    GetSurvey = 50,
    /// `NL80211_CMD_NEW_SURVEY_RESULTS`
    NewSurveyResults = 51,
    /// `NL80211_CMD_SET_PMKSA`
    SetPmksa = 52,
    /// `NL80211_CMD_DEL_PMKSA`
    DelPmksa = 53,
    /// `NL80211_CMD_FLUSH_PMKSA`
    FlushPmksa = 54,
    /// `NL80211_CMD_REMAIN_ON_CHANNEL`
    RemainOnChannel = 55,
    /// `NL80211_CMD_CANCEL_REMAIN_ON_CHANNEL`
    CancelRemainOnChannel = 56,
    /// `NL80211_CMD_SET_TX_BITRATE_MASK`
    SetTxBitrateMask = 57,
    /// `NL80211_CMD_REGISTER_FRAME`
    RegisterFrame = 58,
    /// `NL80211_CMD_REGISTER_ACTION`
    RegisterAction = 58,
    /// `NL80211_CMD_FRAME`
    Frame = 59,
    /// `NL80211_CMD_ACTION`
    Action = 59,
    /// `NL80211_CMD_FRAME_TX_STATUS`
    FrameTxStatus = 60,
    /// `NL80211_CMD_ACTION_TX_STATUS`
    ActionTxStatus = 60,
    /// `NL80211_CMD_SET_POWER_SAVE`
    SetPowerSave = 61,
    /// `NL80211_CMD_GET_POWER_SAVE`
    GetPowerSave = 62,
    /// `NL80211_CMD_SET_CQM`
    SetCqm = 63,
    /// `NL80211_CMD_NOTIFY_CQM`
    NotifyCqm = 64,
    /// `NL80211_CMD_SET_CHANNEL`
    SetChannel = 65,
    /// `NL80211_CMD_SET_WDS_PEER`
    SetWdsPeer = 66,
    /// `NL80211_CMD_FRAME_WAIT_CANCEL`
    FrameWaitCancel = 67,
    /// `NL80211_CMD_JOIN_MESH`
    JoinMesh = 68,
    /// `NL80211_CMD_LEAVE_MESH`
    LeaveMesh = 69,
    /// `NL80211_CMD_UNPROT_DEAUTHENTICATE`
    UnprotDeauthenticate = 70,
    /// `NL80211_CMD_UNPROT_DISASSOCIATE`
    UnprotDisassociate = 71,
    /// `NL80211_CMD_NEW_PEER_CANDIDATE`
    NewPeerCandidate = 72,
    /// `NL80211_CMD_GET_WOWLAN`
    GetWowlan = 73,
    /// `NL80211_CMD_SET_WOWLAN`
    SetWowlan = 74,
    /// `NL80211_CMD_START_SCHED_SCAN`
    StartSchedScan = 75,
    /// `NL80211_CMD_STOP_SCHED_SCAN`
    StopSchedScan = 76,
    /// `NL80211_CMD_SCHED_SCAN_RESULTS`
    SchedScanResults = 77,
    /// `NL80211_CMD_SCHED_SCAN_STOPPED`
    SchedScanStopped = 78,
    /// `NL80211_CMD_SET_REKEY_OFFLOAD`
    SetRekeyOffload = 79,
    /// `NL80211_CMD_PMKSA_CANDIDATE`
    PmksaCandidate = 80,
    /// `NL80211_CMD_TDLS_OPER`
    TdlsOper = 81,
    /// `NL80211_CMD_TDLS_MGMT`
    TdlsMgmt = 82,
    /// `NL80211_CMD_UNEXPECTED_FRAME`
    UnexpectedFrame = 83,
    /// `NL80211_CMD_PROBE_CLIENT`
    ProbeClient = 84,
    /// `NL80211_CMD_REGISTER_BEACONS`
    RegisterBeacons = 85,
    /// `NL80211_CMD_UNEXPECTED_4ADDR_FRAME`
    Unexpected4addrFrame = 86,
    /// `NL80211_CMD_SET_NOACK_MAP`
    SetNoackMap = 87,
    /// `NL80211_CMD_CH_SWITCH_NOTIFY`
    ChSwitchNotify = 88,
    /// `NL80211_CMD_START_P2P_DEVICE`
    StartP2pDevice = 89,
    /// `NL80211_CMD_STOP_P2P_DEVICE`
    StopP2pDevice = 90,
    /// `NL80211_CMD_CONN_FAILED`
    ConnFailed = 91,
    /// `NL80211_CMD_SET_MCAST_RATE`
    SetMcastRate = 92,
    /// `NL80211_CMD_SET_MAC_ACL`
    SetMacAcl = 93,
    /// `NL80211_CMD_RADAR_DETECT`
    RadarDetect = 94,
    /// `NL80211_CMD_GET_PROTOCOL_FEATURES`
    GetProtocolFeatures = 95,
    /// `NL80211_CMD_UPDATE_FT_IES`
    UpdateFtIes = 96,
    /// `NL80211_CMD_FT_EVENT`
    FtEvent = 97,
    /// `NL80211_CMD_CRIT_PROTOCOL_START`
    CritProtocolStart = 98,
    /// `NL80211_CMD_CRIT_PROTOCOL_STOP`
    CritProtocolStop = 99,
    /// `NL80211_CMD_GET_COALESCE`
    GetCoalesce = 100,
    /// `NL80211_CMD_SET_COALESCE`
    SetCoalesce = 101,
    /// `NL80211_CMD_CHANNEL_SWITCH`
    ChannelSwitch = 102,
    /// `NL80211_CMD_VENDOR`
    Vendor = 103,
    /// `NL80211_CMD_SET_QOS_MAP`
    SetQosMap = 104,
    /// `NL80211_CMD_ADD_TX_TS`
    AddTxTs = 105,
    /// `NL80211_CMD_DEL_TX_TS`
    DelTxTs = 106,
    /// `NL80211_CMD_GET_MPP`
    GetMpp = 107,
    /// `NL80211_CMD_JOIN_OCB`
    JoinOcb = 108,
    /// `NL80211_CMD_LEAVE_OCB`
    LeaveOcb = 109,
    /// `NL80211_CMD_CH_SWITCH_STARTED_NOTIFY`
    ChSwitchStartedNotify = 110,
    /// `NL80211_CMD_TDLS_CHANNEL_SWITCH`
    TdlsChannelSwitch = 111,
    /// `NL80211_CMD_TDLS_CANCEL_CHANNEL_SWITCH`
    TdlsCancelChannelSwitch = 112,
    /// `NL80211_CMD_WIPHY_REG_CHANGE`
    WiphyRegChange = 113,
    /// `NL80211_CMD_ABORT_SCAN`
    AbortScan = 114,
    /// `NL80211_CMD_START_NAN`
    StartNan = 115,
    /// `NL80211_CMD_STOP_NAN`
    StopNan = 116,
    /// `NL80211_CMD_ADD_NAN_FUNCTION`
    AddNanFunction = 117,
    /// `NL80211_CMD_DEL_NAN_FUNCTION`
    DelNanFunction = 118,
    /// `NL80211_CMD_CHANGE_NAN_CONFIG`
    ChangeNanConfig = 119,
    /// `NL80211_CMD_NAN_MATCH`
    NanMatch = 120,
    /// `NL80211_CMD_SET_MULTICAST_TO_UNICAST`
    SetMulticastToUnicast = 121,
    /// `NL80211_CMD_UPDATE_CONNECT_PARAMS`
    UpdateConnectParams = 122,
    /// `NL80211_CMD_SET_PMK`
    SetPmk = 123,
    /// `NL80211_CMD_DEL_PMK`
    DelPmk = 124,
    /// `NL80211_CMD_PORT_AUTHORIZED`
    PortAuthorized = 125,
    /// `NL80211_CMD_RELOAD_REGDB`
    ReloadRegdb = 126,
    /// `NL80211_CMD_EXTERNAL_AUTH`
    ExternalAuth = 127,
    /// `NL80211_CMD_STA_OPMODE_CHANGED`
    StaOpmodeChanged = 128,
    /// `NL80211_CMD_CONTROL_PORT_FRAME`
    ControlPortFrame = 129,
    /// `NL80211_CMD_GET_FTM_RESPONDER_STATS`
    GetFtmResponderStats = 130,
    /// `NL80211_CMD_PEER_MEASUREMENT_START`
    PeerMeasurementStart = 131,
    /// `NL80211_CMD_PEER_MEASUREMENT_RESULT`
    PeerMeasurementResult = 132,
    /// `NL80211_CMD_PEER_MEASUREMENT_COMPLETE`
    PeerMeasurementComplete = 133,
    /// `NL80211_CMD_NOTIFY_RADAR`
    NotifyRadar = 134,
    /// `NL80211_CMD_UPDATE_OWE_INFO`
    UpdateOweInfo = 135,
    /// `NL80211_CMD_PROBE_MESH_LINK`
    ProbeMeshLink = 136,
    /// `NL80211_CMD_SET_TID_CONFIG`
    SetTidConfig = 137,
    /// `NL80211_CMD_UNPROT_BEACON`
    UnprotBeacon = 138,
    /// `NL80211_CMD_CONTROL_PORT_FRAME_TX_STATUS`
    ControlPortFrameTxStatus = 139,
    /// `NL80211_CMD_SET_SAR_SPECS`
    SetSarSpecs = 140,
    /// `NL80211_CMD_OBSS_COLOR_COLLISION`
    ObssColorCollision = 141,
    /// `NL80211_CMD_COLOR_CHANGE_REQUEST`
    ColorChangeRequest = 142,
    /// `NL80211_CMD_COLOR_CHANGE_STARTED`
    ColorChangeStarted = 143,
    /// `NL80211_CMD_COLOR_CHANGE_ABORTED`
    ColorChangeAborted = 144,
    /// `NL80211_CMD_COLOR_CHANGE_COMPLETED`
    ColorChangeCompleted = 145,
    /// `NL80211_CMD_SET_FILS_AAD`
    SetFilsAad = 146,
    /// `NL80211_CMD_ASSOC_COMEBACK`
    AssocComeback = 147,
    /// `NL80211_CMD_ADD_LINK`
    AddLink = 148,
    /// `NL80211_CMD_REMOVE_LINK`
    RemoveLink = 149,
    /// `NL80211_CMD_ADD_LINK_STA`
    AddLinkSta = 150,
    /// `NL80211_CMD_MODIFY_LINK_STA`
    ModifyLinkSta = 151,
    /// `NL80211_CMD_REMOVE_LINK_STA`
    RemoveLinkSta = 152,
    /// `NL80211_CMD_SET_HW_TIMESTAMP`
    SetHwTimestamp = 153,
    /// `NL80211_CMD_LINKS_REMOVED`
    LinksRemoved = 154,
    /// `NL80211_CMD_SET_TID_TO_LINK_MAPPING`
    SetTidToLinkMapping = 155,
    /// `NL80211_CMD_ASSOC_MLO_RECONF`
    AssocMloReconf = 156,
    /// `NL80211_CMD_EPCS_CFG`
    EpcsCfg = 157,
    /// `NL80211_CMD_NAN_NEXT_DW_NOTIFICATION`
    NanNextDwNotification = 158,
    /// `NL80211_CMD_NAN_CLUSTER_JOINED`
    NanClusterJoined = 159,
    /// `NL80211_CMD_INCUMBENT_SIGNAL_DETECT`
    IncumbentSignalDetect = 160,
    /// `NL80211_CMD_NAN_SET_LOCAL_SCHED`
    NanSetLocalSched = 161,
    /// `NL80211_CMD_NAN_SCHED_UPDATE_DONE`
    NanSchedUpdateDone = 162,
    /// `NL80211_CMD_NAN_SET_PEER_SCHED`
    NanSetPeerSched = 163,
    /// `NL80211_CMD_NAN_ULW_UPDATE`
    NanUlwUpdate = 164,
    /// `NL80211_CMD_NAN_CHANNEL_EVAC`
    NanChannelEvac = 165,
    /* Many many more elided */
}
impl neli::consts::genl::Cmd for Nl80211Command {}

#[neli::neli_enum(serialized_type = "u16")]
pub enum Nl80211Attribute {
    Unspecified = 0,
    /// `NL80211_ATTR_WIPHY`
    Wiphy = 1,
    /// `NL80211_ATTR_WIPHY_NAME`
    WiphyName = 2,
    /// `NL80211_ATTR_IFINDEX`
    IfIndex = 3,
    /// `NL80211_ATTR_IFNAME`
    IfName = 4,
    /// `NL80211_ATTR_IFTYPE`
    IfType = 5,
    /// `NL80211_ATTR_MAC`
    Mac = 6,
    /// `NL80211_ATTR_KEY_DATA`
    KeyData = 7,
    /// `NL80211_ATTR_KEY_IDX`
    KeyIdx = 8,
    /// `NL80211_ATTR_KEY_CIPHER`
    KeyCipher = 9,
    /// `NL80211_ATTR_KEY_SEQ`
    KeySeq = 10,
    /// `NL80211_ATTR_KEY_DEFAULT`
    KeyDefault = 11,
    /// `NL80211_ATTR_BEACON_INTERVAL`
    BeaconInterval = 12,
    /// `NL80211_ATTR_DTIM_PERIOD`
    DtimPeriod = 13,
    /// `NL80211_ATTR_BEACON_HEAD`
    BeaconHead = 14,
    /// `NL80211_ATTR_BEACON_TAIL`
    BeaconTail = 15,
    /// `NL80211_ATTR_STA_AID`
    StaAid = 16,
    /// `NL80211_ATTR_STA_FLAGS`
    StaFlags = 17,
    /// `NL80211_ATTR_STA_LISTEN_INTERVAL`
    StaListenInterval = 18,
    /// `NL80211_ATTR_STA_SUPPORTED_RATES`
    StaSupportedRates = 19,
    /// `NL80211_ATTR_STA_VLAN`
    StaVlan = 20,
    /// `NL80211_ATTR_STA_INFO`
    StaInfo = 21,
    /// `NL80211_ATTR_WIPHY_BANDS`
    WiphyBands = 22,
    /// `NL80211_ATTR_MNTR_FLAGS`
    MntrFlags = 23,
    /// `NL80211_ATTR_MESH_ID`
    MeshId = 24,
    /// `NL80211_ATTR_STA_PLINK_ACTION`
    StaPlinkAction = 25,
    /// `NL80211_ATTR_MPATH_NEXT_HOP`
    MpathNextHop = 26,
    /// `NL80211_ATTR_MPATH_INFO`
    MpathInfo = 27,
    /// `NL80211_ATTR_BSS_CTS_PROT`
    BssCtsProt = 28,
    /// `NL80211_ATTR_BSS_SHORT_PREAMBLE`
    BssShortPreamble = 29,
    /// `NL80211_ATTR_BSS_SHORT_SLOT_TIME`
    BssShortSlotTime = 30,
    /// `NL80211_ATTR_HT_CAPABILITY`
    HtCapability = 31,
    /// `NL80211_ATTR_SUPPORTED_IFTYPES`
    SupportedIftypes = 32,
    /// `NL80211_ATTR_REG_ALPHA2`
    RegAlpha2 = 33,
    /// `NL80211_ATTR_REG_RULES`
    RegRules = 34,
    /// `NL80211_ATTR_MESH_CONFIG`
    MeshConfig = 35,
    /// `NL80211_ATTR_BSS_BASIC_RATES`
    BssBasicRates = 36,
    /// `NL80211_ATTR_WIPHY_TXQ_PARAMS`
    WiphyTxqParams = 37,
    /// `NL80211_ATTR_WIPHY_FREQ`
    WiphyFreq = 38,
    /// `NL80211_ATTR_WIPHY_CHANNEL_TYPE`
    WiphyChannelType = 39,
    /// `NL80211_ATTR_KEY_DEFAULT_MGMT`
    KeyDefaultMgmt = 40,
    /// `NL80211_ATTR_MGMT_SUBTYPE`
    MgmtSubtype = 41,
    /// `NL80211_ATTR_IE`
    Ie = 42,
    /// `NL80211_ATTR_MAX_NUM_SCAN_SSIDS`
    MaxNumScanSsids = 43,
    /// `NL80211_ATTR_SCAN_FREQUENCIES`
    ScanFrequencies = 44,
    /// `NL80211_ATTR_SCAN_SSIDS`
    ScanSsids = 45,
    /// `NL80211_ATTR_GENERATION`
    Generation = 46,
    /// `NL80211_ATTR_BSS`
    Bss = 47,
    /// `NL80211_ATTR_REG_INITIATOR`
    RegInitiator = 48,
    /// `NL80211_ATTR_REG_TYPE`
    RegType = 49,
    /// `NL80211_ATTR_SUPPORTED_COMMANDS`
    SupportedCommands = 50,
    /// `NL80211_ATTR_FRAME`
    Frame = 51,
    /// `NL80211_ATTR_SSID`
    Ssid = 52,
    /// `NL80211_ATTR_AUTH_TYPE`
    AuthType = 53,
    /// `NL80211_ATTR_REASON_CODE`
    ReasonCode = 54,
    /// `NL80211_ATTR_KEY_TYPE`
    KeyType = 55,
    /// `NL80211_ATTR_MAX_SCAN_IE_LEN`
    MaxScanIeLen = 56,
    /// `NL80211_ATTR_CIPHER_SUITES`
    CipherSuites = 57,
    /// `NL80211_ATTR_FREQ_BEFORE`
    FreqBefore = 58,
    /// `NL80211_ATTR_FREQ_AFTER`
    FreqAfter = 59,
    /// `NL80211_ATTR_FREQ_FIXED`
    FreqFixed = 60,
    /// `NL80211_ATTR_WIPHY_RETRY_SHORT`
    WiphyRetryShort = 61,
    /// `NL80211_ATTR_WIPHY_RETRY_LONG`
    WiphyRetryLong = 62,
    /// `NL80211_ATTR_WIPHY_FRAG_THRESHOLD`
    WiphyFragThreshold = 63,
    /// `NL80211_ATTR_WIPHY_RTS_THRESHOLD`
    WiphyRtsThreshold = 64,
    /// `NL80211_ATTR_TIMED_OUT`
    TimedOut = 65,
    /// `NL80211_ATTR_USE_MFP`
    UseMfp = 66,
    /// `NL80211_ATTR_STA_FLAGS2`
    StaFlags2 = 67,
    /// `NL80211_ATTR_CONTROL_PORT`
    ControlPort = 68,
    /// `NL80211_ATTR_TESTDATA`
    Testdata = 69,
    /// `NL80211_ATTR_PRIVACY`
    Privacy = 70,
    /// `NL80211_ATTR_DISCONNECTED_BY_AP`
    DisconnectedByAp = 71,
    /// `NL80211_ATTR_STATUS_CODE`
    StatusCode = 72,
    /// `NL80211_ATTR_CIPHER_SUITES_PAIRWISE`
    CipherSuitesPairwise = 73,
    /// `NL80211_ATTR_CIPHER_SUITE_GROUP`
    CipherSuiteGroup = 74,
    /// `NL80211_ATTR_WPA_VERSIONS`
    WpaVersions = 75,
    /// `NL80211_ATTR_AKM_SUITES`
    AkmSuites = 76,
    /// `NL80211_ATTR_REQ_IE`
    ReqIe = 77,
    /// `NL80211_ATTR_RESP_IE`
    RespIe = 78,
    /// `NL80211_ATTR_PREV_BSSID`
    PrevBssid = 79,
    /// `NL80211_ATTR_KEY`
    Key = 80,
    /// `NL80211_ATTR_KEYS`
    Keys = 81,
    /// `NL80211_ATTR_PID`
    Pid = 82,
    /// `NL80211_ATTR_4ADDR`
    _4addr = 83,
    /// `NL80211_ATTR_SURVEY_INFO`
    SurveyInfo = 84,
    /// `NL80211_ATTR_PMKID`
    Pmkid = 85,
    /// `NL80211_ATTR_MAX_NUM_PMKIDS`
    MaxNumPmkids = 86,
    /// `NL80211_ATTR_DURATION`
    Duration = 87,
    /// `NL80211_ATTR_COOKIE`
    Cookie = 88,
    /// `NL80211_ATTR_WIPHY_COVERAGE_CLASS`
    WiphyCoverageClass = 89,
    /// `NL80211_ATTR_TX_RATES`
    TxRates = 90,
    /// `NL80211_ATTR_FRAME_MATCH`
    FrameMatch = 91,
    /// `NL80211_ATTR_ACK`
    Ack = 92,
    /// `NL80211_ATTR_PS_STATE`
    PsState = 93,
    /// `NL80211_ATTR_CQM`
    Cqm = 94,
    /// `NL80211_ATTR_LOCAL_STATE_CHANGE`
    LocalStateChange = 95,
    /// `NL80211_ATTR_AP_ISOLATE`
    ApIsolate = 96,
    /// `NL80211_ATTR_WIPHY_TX_POWER_SETTING`
    WiphyTxPowerSetting = 97,
    /// `NL80211_ATTR_WIPHY_TX_POWER_LEVEL`
    WiphyTxPowerLevel = 98,
    /// `NL80211_ATTR_TX_FRAME_TYPES`
    TxFrameTypes = 99,
    /// `NL80211_ATTR_RX_FRAME_TYPES`
    RxFrameTypes = 100,
    /// `NL80211_ATTR_FRAME_TYPE`
    FrameType = 101,
    /// `NL80211_ATTR_CONTROL_PORT_ETHERTYPE`
    ControlPortEthertype = 102,
    /// `NL80211_ATTR_CONTROL_PORT_NO_ENCRYPT`
    ControlPortNoEncrypt = 103,
    /// `NL80211_ATTR_SUPPORT_IBSS_RSN`
    SupportIbssRsn = 104,
    /// `NL80211_ATTR_WIPHY_ANTENNA_TX`
    WiphyAntennaTx = 105,
    /// `NL80211_ATTR_WIPHY_ANTENNA_RX`
    WiphyAntennaRx = 106,
    /// `NL80211_ATTR_MCAST_RATE`
    McastRate = 107,
    /// `NL80211_ATTR_OFFCHANNEL_TX_OK`
    OffchannelTxOk = 108,
    /// `NL80211_ATTR_BSS_HT_OPMODE`
    BssHtOpmode = 109,
    /// `NL80211_ATTR_KEY_DEFAULT_TYPES`
    KeyDefaultTypes = 110,
    /// `NL80211_ATTR_MAX_REMAIN_ON_CHANNEL_DURATION`
    MaxRemainOnChannelDuration = 111,
    /// `NL80211_ATTR_MESH_SETUP`
    MeshSetup = 112,
    /// `NL80211_ATTR_WIPHY_ANTENNA_AVAIL_TX`
    WiphyAntennaAvailTx = 113,
    /// `NL80211_ATTR_WIPHY_ANTENNA_AVAIL_RX`
    WiphyAntennaAvailRx = 114,
    /// `NL80211_ATTR_SUPPORT_MESH_AUTH`
    SupportMeshAuth = 115,
    /// `NL80211_ATTR_STA_PLINK_STATE`
    StaPlinkState = 116,
    /// `NL80211_ATTR_WOWLAN_TRIGGERS`
    WowlanTriggers = 117,
    /// `NL80211_ATTR_WOWLAN_TRIGGERS_SUPPORTED`
    WowlanTriggersSupported = 118,
    /// `NL80211_ATTR_SCHED_SCAN_INTERVAL`
    SchedScanInterval = 119,
    /// `NL80211_ATTR_INTERFACE_COMBINATIONS`
    InterfaceCombinations = 120,
    /// `NL80211_ATTR_SOFTWARE_IFTYPES`
    SoftwareIftypes = 121,
    /// `NL80211_ATTR_REKEY_DATA`
    RekeyData = 122,
    /// `NL80211_ATTR_MAX_NUM_SCHED_SCAN_SSIDS`
    MaxNumSchedScanSsids = 123,
    /// `NL80211_ATTR_MAX_SCHED_SCAN_IE_LEN`
    MaxSchedScanIeLen = 124,
    /// `NL80211_ATTR_SCAN_SUPP_RATES`
    ScanSuppRates = 125,
    /// `NL80211_ATTR_HIDDEN_SSID`
    HiddenSsid = 126,
    /// `NL80211_ATTR_IE_PROBE_RESP`
    IeProbeResp = 127,
    /// `NL80211_ATTR_IE_ASSOC_RESP`
    IeAssocResp = 128,
    /// `NL80211_ATTR_STA_WME`
    StaWme = 129,
    /// `NL80211_ATTR_SUPPORT_AP_UAPSD`
    SupportApUapsd = 130,
    /// `NL80211_ATTR_ROAM_SUPPORT`
    RoamSupport = 131,
    /// `NL80211_ATTR_SCHED_SCAN_MATCH`
    SchedScanMatch = 132,
    /// `NL80211_ATTR_MAX_MATCH_SETS`
    MaxMatchSets = 133,
    /// `NL80211_ATTR_PMKSA_CANDIDATE`
    PmksaCandidate = 134,
    /// `NL80211_ATTR_TX_NO_CCK_RATE`
    TxNoCckRate = 135,
    /// `NL80211_ATTR_TDLS_ACTION`
    TdlsAction = 136,
    /// `NL80211_ATTR_TDLS_DIALOG_TOKEN`
    TdlsDialogToken = 137,
    /// `NL80211_ATTR_TDLS_OPERATION`
    TdlsOperation = 138,
    /// `NL80211_ATTR_TDLS_SUPPORT`
    TdlsSupport = 139,
    /// `NL80211_ATTR_TDLS_EXTERNAL_SETUP`
    TdlsExternalSetup = 140,
    /// `NL80211_ATTR_DEVICE_AP_SME`
    DeviceApSme = 141,
    /// `NL80211_ATTR_DONT_WAIT_FOR_ACK`
    DontWaitForAck = 142,
    /// `NL80211_ATTR_FEATURE_FLAGS`
    FeatureFlags = 143,
    /// `NL80211_ATTR_PROBE_RESP_OFFLOAD`
    ProbeRespOffload = 144,
    /// `NL80211_ATTR_PROBE_RESP`
    ProbeResp = 145,
    /// `NL80211_ATTR_DFS_REGION`
    DfsRegion = 146,
    /// `NL80211_ATTR_DISABLE_HT`
    DisableHt = 147,
    /// `NL80211_ATTR_HT_CAPABILITY_MASK`
    HtCapabilityMask = 148,
    /// `NL80211_ATTR_NOACK_MAP`
    NoackMap = 149,
    /// `NL80211_ATTR_INACTIVITY_TIMEOUT`
    InactivityTimeout = 150,
    /// `NL80211_ATTR_RX_SIGNAL_DBM`
    RxSignalDbm = 151,
    /// `NL80211_ATTR_BG_SCAN_PERIOD`
    BgScanPeriod = 152,
    /// `NL80211_ATTR_WDEV`
    WDev = 153,
    /// `NL80211_ATTR_USER_REG_HINT_TYPE`
    UserRegHintType = 154,
    /// `NL80211_ATTR_CONN_FAILED_REASON`
    ConnFailedReason = 155,
    /// `NL80211_ATTR_AUTH_DATA`
    AuthData = 156,
    /// `NL80211_ATTR_VHT_CAPABILITY`
    VhtCapability = 157,
    /// `NL80211_ATTR_SCAN_FLAGS`
    ScanFlags = 158,
    /// `NL80211_ATTR_CHANNEL_WIDTH`
    ChannelWidth = 159,
    /// `NL80211_ATTR_CENTER_FREQ1`
    CenterFreq1 = 160,
    /// `NL80211_ATTR_CENTER_FREQ2`
    CenterFreq2 = 161,
    /// `NL80211_ATTR_P2P_CTWINDOW`
    P2pCtwindow = 162,
    /// `NL80211_ATTR_P2P_OPPPS`
    P2pOppps = 163,
    /// `NL80211_ATTR_LOCAL_MESH_POWER_MODE`
    LocalMeshPowerMode = 164,
    /// `NL80211_ATTR_ACL_POLICY`
    AclPolicy = 165,
    /// `NL80211_ATTR_MAC_ADDRS`
    MacAddrs = 166,
    /// `NL80211_ATTR_MAC_ACL_MAX`
    MacAclMax = 167,
    /// `NL80211_ATTR_RADAR_EVENT`
    RadarEvent = 168,
    /// `NL80211_ATTR_EXT_CAPA`
    ExtCapa = 169,
    /// `NL80211_ATTR_EXT_CAPA_MASK`
    ExtCapaMask = 170,
    /// `NL80211_ATTR_STA_CAPABILITY`
    StaCapability = 171,
    /// `NL80211_ATTR_STA_EXT_CAPABILITY`
    StaExtCapability = 172,
    /// `NL80211_ATTR_PROTOCOL_FEATURES`
    ProtocolFeatures = 173,
    /// `NL80211_ATTR_SPLIT_WIPHY_DUMP`
    SplitWiphyDump = 174,
    /// `NL80211_ATTR_DISABLE_VHT`
    DisableVht = 175,
    /// `NL80211_ATTR_VHT_CAPABILITY_MASK`
    VhtCapabilityMask = 176,
    /// `NL80211_ATTR_MDID`
    Mdid = 177,
    /// `NL80211_ATTR_IE_RIC`
    IeRic = 178,
    /// `NL80211_ATTR_CRIT_PROT_ID`
    CritProtId = 179,
    /// `NL80211_ATTR_MAX_CRIT_PROT_DURATION`
    MaxCritProtDuration = 180,
    /// `NL80211_ATTR_PEER_AID`
    PeerAid = 181,
    /// `NL80211_ATTR_COALESCE_RULE`
    CoalesceRule = 182,
    /// `NL80211_ATTR_CH_SWITCH_COUNT`
    ChSwitchCount = 183,
    /// `NL80211_ATTR_CH_SWITCH_BLOCK_TX`
    ChSwitchBlockTx = 184,
    /// `NL80211_ATTR_CSA_IES`
    CsaIes = 185,
    /// `NL80211_ATTR_CNTDWN_OFFS_BEACON`
    CntdwnOffsBeacon = 186,
    /// `NL80211_ATTR_CNTDWN_OFFS_PRESP`
    CntdwnOffsPresp = 187,
    /// `NL80211_ATTR_RXMGMT_FLAGS`
    RxmgmtFlags = 188,
    /// `NL80211_ATTR_STA_SUPPORTED_CHANNELS`
    StaSupportedChannels = 189,
    /// `NL80211_ATTR_STA_SUPPORTED_OPER_CLASSES`
    StaSupportedOperClasses = 190,
    /// `NL80211_ATTR_HANDLE_DFS`
    HandleDfs = 191,
    /// `NL80211_ATTR_SUPPORT_5_MHZ`
    Support5Mhz = 192,
    /// `NL80211_ATTR_SUPPORT_10_MHZ`
    Support10Mhz = 193,
    /// `NL80211_ATTR_OPMODE_NOTIF`
    OpmodeNotif = 194,
    /// `NL80211_ATTR_VENDOR_ID`
    VendorId = 195,
    /// `NL80211_ATTR_VENDOR_SUBCMD`
    VendorSubcmd = 196,
    /// `NL80211_ATTR_VENDOR_DATA`
    VendorData = 197,
    /// `NL80211_ATTR_VENDOR_EVENTS`
    VendorEvents = 198,
    /// `NL80211_ATTR_QOS_MAP`
    QosMap = 199,
    /// `NL80211_ATTR_MAC_HINT`
    MacHint = 200,
    /// `NL80211_ATTR_WIPHY_FREQ_HINT`
    WiphyFreqHint = 201,
    /// `NL80211_ATTR_MAX_AP_ASSOC_STA`
    MaxApAssocSta = 202,
    /// `NL80211_ATTR_TDLS_PEER_CAPABILITY`
    TdlsPeerCapability = 203,
    /// `NL80211_ATTR_SOCKET_OWNER`
    SocketOwner = 204,
    /// `NL80211_ATTR_CSA_C_OFFSETS_TX`
    CsaCOffsetsTx = 205,
    /// `NL80211_ATTR_MAX_CSA_COUNTERS`
    MaxCsaCounters = 206,
    /// `NL80211_ATTR_TDLS_INITIATOR`
    TdlsInitiator = 207,
    /// `NL80211_ATTR_USE_RRM`
    UseRrm = 208,
    /// `NL80211_ATTR_WIPHY_DYN_ACK`
    WiphyDynAck = 209,
    /// `NL80211_ATTR_TSID`
    Tsid = 210,
    /// `NL80211_ATTR_USER_PRIO`
    UserPrio = 211,
    /// `NL80211_ATTR_ADMITTED_TIME`
    AdmittedTime = 212,
    /// `NL80211_ATTR_SMPS_MODE`
    SmpsMode = 213,
    /// `NL80211_ATTR_OPER_CLASS`
    OperClass = 214,
    /// `NL80211_ATTR_MAC_MASK`
    MacMask = 215,
    /// `NL80211_ATTR_WIPHY_SELF_MANAGED_REG`
    WiphySelfManagedReg = 216,
    /// `NL80211_ATTR_EXT_FEATURES`
    ExtFeatures = 217,
    /// `NL80211_ATTR_SURVEY_RADIO_STATS`
    SurveyRadioStats = 218,
    /// `NL80211_ATTR_NETNS_FD`
    NetnsFd = 219,
    /// `NL80211_ATTR_SCHED_SCAN_DELAY`
    SchedScanDelay = 220,
    /// `NL80211_ATTR_REG_INDOOR`
    RegIndoor = 221,
    /// `NL80211_ATTR_MAX_NUM_SCHED_SCAN_PLANS`
    MaxNumSchedScanPlans = 222,
    /// `NL80211_ATTR_MAX_SCAN_PLAN_INTERVAL`
    MaxScanPlanInterval = 223,
    /// `NL80211_ATTR_MAX_SCAN_PLAN_ITERATIONS`
    MaxScanPlanIterations = 224,
    /// `NL80211_ATTR_SCHED_SCAN_PLANS`
    SchedScanPlans = 225,
    /// `NL80211_ATTR_PBSS`
    Pbss = 226,
    /// `NL80211_ATTR_BSS_SELECT`
    BssSelect = 227,
    /// `NL80211_ATTR_STA_SUPPORT_P2P_PS`
    StaSupportP2pPs = 228,
    /// `NL80211_ATTR_PAD`
    Pad = 229,
    /// `NL80211_ATTR_IFTYPE_EXT_CAPA`
    IftypeExtCapa = 230,
    /// `NL80211_ATTR_MU_MIMO_GROUP_DATA`
    MuMimoGroupData = 231,
    /// `NL80211_ATTR_MU_MIMO_FOLLOW_MAC_ADDR`
    MuMimoFollowMacAddr = 232,
    /// `NL80211_ATTR_SCAN_START_TIME_TSF`
    ScanStartTimeTsf = 233,
    /// `NL80211_ATTR_SCAN_START_TIME_TSF_BSSID`
    ScanStartTimeTsfBssid = 234,
    /// `NL80211_ATTR_MEASUREMENT_DURATION`
    MeasurementDuration = 235,
    /// `NL80211_ATTR_MEASUREMENT_DURATION_MANDATORY`
    MeasurementDurationMandatory = 236,
    /// `NL80211_ATTR_MESH_PEER_AID`
    MeshPeerAid = 237,
    /// `NL80211_ATTR_NAN_MASTER_PREF`
    NanMasterPref = 238,
    /// `NL80211_ATTR_BANDS`
    /// This is a u32 bitmask of `BIT(NL80211_BAND_*)` as described in `enum
    /// nl80211_band`.
    Bands = 239,
    /// `NL80211_ATTR_NAN_FUNC`
    NanFunc = 240,
    /// `NL80211_ATTR_NAN_MATCH`
    NanMatch = 241,
    /// `NL80211_ATTR_FILS_KEK`
    FilsKek = 242,
    /// `NL80211_ATTR_FILS_NONCES`
    FilsNonces = 243,
    /// `NL80211_ATTR_MULTICAST_TO_UNICAST_ENABLED`
    MulticastToUnicastEnabled = 244,
    /// `NL80211_ATTR_BSSID`
    Bssid = 245,
    /// `NL80211_ATTR_SCHED_SCAN_RELATIVE_RSSI`
    SchedScanRelativeRssi = 246,
    /// `NL80211_ATTR_SCHED_SCAN_RSSI_ADJUST`
    SchedScanRssiAdjust = 247,
    /// `NL80211_ATTR_TIMEOUT_REASON`
    TimeoutReason = 248,
    /// `NL80211_ATTR_FILS_ERP_USERNAME`
    FilsErpUsername = 249,
    /// `NL80211_ATTR_FILS_ERP_REALM`
    FilsErpRealm = 250,
    /// `NL80211_ATTR_FILS_ERP_NEXT_SEQ_NUM`
    FilsErpNextSeqNum = 251,
    /// `NL80211_ATTR_FILS_ERP_RRK`
    FilsErpRrk = 252,
    /// `NL80211_ATTR_FILS_CACHE_ID`
    FilsCacheId = 253,
    /// `NL80211_ATTR_PMK`
    Pmk = 254,
    /// `NL80211_ATTR_SCHED_SCAN_MULTI`
    SchedScanMulti = 255,
    /// `NL80211_ATTR_SCHED_SCAN_MAX_REQS`
    SchedScanMaxReqs = 256,
    /// `NL80211_ATTR_WANT_1X_4WAY_HS`
    Want1x4wayHs = 257,
    /// `NL80211_ATTR_PMKR0_NAME`
    Pmkr0Name = 258,
    /// `NL80211_ATTR_PORT_AUTHORIZED`
    PortAuthorized = 259,
    /// `NL80211_ATTR_EXTERNAL_AUTH_ACTION`
    ExternalAuthAction = 260,
    /// `NL80211_ATTR_EXTERNAL_AUTH_SUPPORT`
    ExternalAuthSupport = 261,
    /// `NL80211_ATTR_NSS`
    Nss = 262,
    /// `NL80211_ATTR_ACK_SIGNAL`
    AckSignal = 263,
    /// `NL80211_ATTR_CONTROL_PORT_OVER_NL80211`
    ControlPortOverNl80211 = 264,
    /// `NL80211_ATTR_TXQ_STATS`
    TxqStats = 265,
    /// `NL80211_ATTR_TXQ_LIMIT`
    TxqLimit = 266,
    /// `NL80211_ATTR_TXQ_MEMORY_LIMIT`
    TxqMemoryLimit = 267,
    /// `NL80211_ATTR_TXQ_QUANTUM`
    TxqQuantum = 268,
    /// `NL80211_ATTR_HE_CAPABILITY`
    HeCapability = 269,
    /// `NL80211_ATTR_FTM_RESPONDER`
    FtmResponder = 270,
    /// `NL80211_ATTR_FTM_RESPONDER_STATS`
    FtmResponderStats = 271,
    /// `NL80211_ATTR_TIMEOUT`
    Timeout = 272,
    /// `NL80211_ATTR_PEER_MEASUREMENTS`
    PeerMeasurements = 273,
    /// `NL80211_ATTR_AIRTIME_WEIGHT`
    AirtimeWeight = 274,
    /// `NL80211_ATTR_STA_TX_POWER_SETTING`
    StaTxPowerSetting = 275,
    /// `NL80211_ATTR_STA_TX_POWER`
    StaTxPower = 276,
    /// `NL80211_ATTR_SAE_PASSWORD`
    SaePassword = 277,
    /// `NL80211_ATTR_TWT_RESPONDER`
    TwtResponder = 278,
    /// `NL80211_ATTR_HE_OBSS_PD`
    HeObssPd = 279,
    /// `NL80211_ATTR_WIPHY_EDMG_CHANNELS`
    WiphyEdmgChannels = 280,
    /// `NL80211_ATTR_WIPHY_EDMG_BW_CONFIG`
    WiphyEdmgBwConfig = 281,
    /// `NL80211_ATTR_VLAN_ID`
    VlanId = 282,
    /// `NL80211_ATTR_HE_BSS_COLOR`
    HeBssColor = 283,
    /// `NL80211_ATTR_IFTYPE_AKM_SUITES`
    IftypeAkmSuites = 284,
    /// `NL80211_ATTR_TID_CONFIG`
    TidConfig = 285,
    /// `NL80211_ATTR_CONTROL_PORT_NO_PREAUTH`
    ControlPortNoPreauth = 286,
    /// `NL80211_ATTR_PMK_LIFETIME`
    PmkLifetime = 287,
    /// `NL80211_ATTR_PMK_REAUTH_THRESHOLD`
    PmkReauthThreshold = 288,
    /// `NL80211_ATTR_RECEIVE_MULTICAST`
    ReceiveMulticast = 289,
    /// `NL80211_ATTR_WIPHY_FREQ_OFFSET`
    WiphyFreqOffset = 290,
    /// `NL80211_ATTR_CENTER_FREQ1_OFFSET`
    CenterFreq1Offset = 291,
    /// `NL80211_ATTR_SCAN_FREQ_KHZ`
    ScanFreqKhz = 292,
    /// `NL80211_ATTR_HE_6GHZ_CAPABILITY`
    He6ghzCapability = 293,
    /// `NL80211_ATTR_FILS_DISCOVERY`
    FilsDiscovery = 294,
    /// `NL80211_ATTR_UNSOL_BCAST_PROBE_RESP`
    UnsolBcastProbeResp = 295,
    /// `NL80211_ATTR_S1G_CAPABILITY`
    S1gCapability = 296,
    /// `NL80211_ATTR_S1G_CAPABILITY_MASK`
    S1gCapabilityMask = 297,
    /// `NL80211_ATTR_SAE_PWE`
    SaePwe = 298,
    /// `NL80211_ATTR_RECONNECT_REQUESTED`
    ReconnectRequested = 299,
    /// `NL80211_ATTR_SAR_SPEC`
    SarSpec = 300,
    /// `NL80211_ATTR_DISABLE_HE`
    DisableHe = 301,
    /// `NL80211_ATTR_OBSS_COLOR_BITMAP`
    ObssColorBitmap = 302,
    /// `NL80211_ATTR_COLOR_CHANGE_COUNT`
    ColorChangeCount = 303,
    /// `NL80211_ATTR_COLOR_CHANGE_COLOR`
    ColorChangeColor = 304,
    /// `NL80211_ATTR_COLOR_CHANGE_ELEMS`
    ColorChangeElems = 305,
    /// `NL80211_ATTR_MBSSID_CONFIG`
    MbssidConfig = 306,
    /// `NL80211_ATTR_MBSSID_ELEMS`
    MbssidElems = 307,
    /// `NL80211_ATTR_RADAR_BACKGROUND`
    RadarBackground = 308,
    /// `NL80211_ATTR_AP_SETTINGS_FLAGS`
    ApSettingsFlags = 309,
    /// `NL80211_ATTR_EHT_CAPABILITY`
    EhtCapability = 310,
    /// `NL80211_ATTR_DISABLE_EHT`
    DisableEht = 311,
    /// `NL80211_ATTR_MLO_LINKS`
    MloLinks = 312,
    /// `NL80211_ATTR_MLO_LINK_ID`
    MloLinkId = 313,
    /// `NL80211_ATTR_MLD_ADDR`
    MldAddr = 314,
    /// `NL80211_ATTR_MLO_SUPPORT`
    MloSupport = 315,
    /// `NL80211_ATTR_MAX_NUM_AKM_SUITES`
    MaxNumAkmSuites = 316,
    /// `NL80211_ATTR_EML_CAPABILITY`
    EmlCapability = 317,
    /// `NL80211_ATTR_MLD_CAPA_AND_OPS`
    MldCapaAndOps = 318,
    /// `NL80211_ATTR_TX_HW_TIMESTAMP`
    TxHwTimestamp = 319,
    /// `NL80211_ATTR_RX_HW_TIMESTAMP`
    RxHwTimestamp = 320,
    /// `NL80211_ATTR_TD_BITMAP`
    TdBitmap = 321,
    /// `NL80211_ATTR_PUNCT_BITMAP`
    PunctBitmap = 322,
    /// `NL80211_ATTR_MAX_HW_TIMESTAMP_PEERS`
    MaxHwTimestampPeers = 323,
    /// `NL80211_ATTR_HW_TIMESTAMP_ENABLED`
    HwTimestampEnabled = 324,
    /// `NL80211_ATTR_EMA_RNR_ELEMS`
    EmaRnrElems = 325,
    /// `NL80211_ATTR_MLO_LINK_DISABLED`
    MloLinkDisabled = 326,
    /// `NL80211_ATTR_BSS_DUMP_INCLUDE_USE_DATA`
    BssDumpIncludeUseData = 327,
    /// `NL80211_ATTR_MLO_TTLM_DLINK`
    MloTtlmDlink = 328,
    /// `NL80211_ATTR_MLO_TTLM_ULINK`
    MloTtlmUlink = 329,
    /// `NL80211_ATTR_ASSOC_SPP_AMSDU`
    AssocSppAmsdu = 330,
    /// `NL80211_ATTR_WIPHY_RADIOS`
    WiphyRadios = 331,
    /// `NL80211_ATTR_WIPHY_INTERFACE_COMBINATIONS`
    WiphyInterfaceCombinations = 332,
    /// `NL80211_ATTR_VIF_RADIO_MASK`
    VifRadioMask = 333,
    /// `NL80211_ATTR_SUPPORTED_SELECTORS`
    SupportedSelectors = 334,
    /// `NL80211_ATTR_MLO_RECONF_REM_LINKS`
    MloReconfRemLinks = 335,
    /// `NL80211_ATTR_EPCS`
    Epcs = 336,
    /// `NL80211_ATTR_ASSOC_MLD_EXT_CAPA_OPS`
    AssocMldExtCapaOps = 337,
    /// `NL80211_ATTR_WIPHY_RADIO_INDEX`
    WiphyRadioIndex = 338,
    /// `NL80211_ATTR_S1G_LONG_BEACON_PERIOD`
    S1gLongBeaconPeriod = 339,
    /// `NL80211_ATTR_S1G_SHORT_BEACON`
    S1gShortBeacon = 340,
}

impl neli::consts::genl::NlAttrType for Nl80211Attribute {}

/// `enum nl80211_band_attr - band attributes`
///
/// Payload for `Nl80211Attribute::WiphyBands`
#[neli::neli_enum(serialized_type = "u16")]
pub enum Nl80211BandAttr {
    Invalid = 0,
    /// `NL80211_BAND_ATTR_FREQS`
    Freqs = 1,
    /// `NL80211_BAND_ATTR_RATES`
    Rates = 2,
    /// `NL80211_BAND_ATTR_HT_MCS_SET`
    HtMcsSet = 3,
    /// `NL80211_BAND_ATTR_HT_CAPA`
    HtCapa = 4,
    /// `NL80211_BAND_ATTR_HT_AMPDU_FACTOR`
    HtAmpduFactor = 5,
    /// `NL80211_BAND_ATTR_HT_AMPDU_DENSITY`
    HtAmpduDensity = 6,
    /// `NL80211_BAND_ATTR_VHT_MCS_SET`
    VhtMcsSet = 7,
    /// `NL80211_BAND_ATTR_VHT_CAPA`
    VhtCapa = 8,
    /// `NL80211_BAND_ATTR_IFTYPE_DATA`
    IftypeData = 9,
    /// `NL80211_BAND_ATTR_EDMG_CHANNELS`
    EdmgChannels = 10,
    /// `NL80211_BAND_ATTR_EDMG_BW_CONFIG`
    EdmgBwConfig = 11,
    /// `NL80211_BAND_ATTR_S1G_MCS_NSS_SET`
    S1gMcsNssSet = 12,
    /// `NL80211_BAND_ATTR_S1G_CAPA`
    S1gCapa = 13,
}

impl neli::consts::genl::NlAttrType for Nl80211BandAttr {}

/// nl80211Band
///
/// Enumeration from nl80211/nl80211.h:3773
#[neli::neli_enum(serialized_type = "u16")]
pub enum Nl80211Bandc {
    Band2ghz = 0,
    Band5ghz = 1,
    Band60ghz = 2,
}

impl neli::consts::genl::NlAttrType for Nl80211Bandc {}

/// nl80211FreqAttr
///
/// Enumeration from nl80211/nl80211.h:2757
#[neli::neli_enum(serialized_type = "u16")]
pub enum Nl80211FreqAttr {
    /// `NL80211_FREQUENCY_ATTR_INVALID`
    Invalid = 0,
    /// `NL80211_FREQUENCY_ATTR_FREQ`
    Freq = 1,
    /// `NL80211_FREQUENCY_ATTR_DISABLED`
    Disabled = 2,
    /// `NL80211_FREQUENCY_ATTR_NO_IR`
    NoIr = 3,
    /// `NL80211_FREQUENCY_ATTR_NO_IBSS`
    NoIbss = 4,
    /// `NL80211_FREQUENCY_ATTR_RADAR`
    Radar = 5,
    /// `NL80211_FREQUENCY_ATTR_MAX_TX_POWER`
    MaxTxPower = 6,
    /// `NL80211_FREQUENCY_ATTR_DFS_STATE`
    DfsState = 7,
    /// `NL80211_FREQUENCY_ATTR_DFS_TIME`
    DfsTime = 8,
    /// `NL80211_FREQUENCY_ATTR_NO_HT40_MINUS`
    NoHt40Minus = 9,
    /// `NL80211_FREQUENCY_ATTR_NO_HT40_PLUS`
    NoHt40Plus = 10,
    /// `NL80211_FREQUENCY_ATTR_NO_80MHZ`
    No80mhz = 11,
    /// `NL80211_FREQUENCY_ATTR_NO_160MHZ`
    No160mhz = 12,
    /// `NL80211_FREQUENCY_ATTR_DFS_CAC_TIME`
    DfsCacTime = 13,
    /// `NL80211_FREQUENCY_ATTR_INDOOR_ONLY`
    IndoorOnly = 14,
    /// `NL80211_FREQUENCY_ATTR_IR_CONCURRENT`
    IrConcurrent = 15,
    /// `NL80211_FREQUENCY_ATTR_NO_20MHZ`
    No20mhz = 16,
    /// `NL80211_FREQUENCY_ATTR_NO_10MHZ`
    No10mhz = 17,
    /// `NL80211_FREQUENCY_ATTR_WMM`
    Wmm = 18,
    /// `NL80211_FREQUENCY_ATTR_NO_HE`
    NoHe = 19,
    /// `NL80211_FREQUENCY_ATTR_OFFSET`
    Offset = 20,
    /// `NL80211_FREQUENCY_ATTR_1MHZ`
    _1Mhz = 21,
    /// `NL80211_FREQUENCY_ATTR_2MHZ`
    _2Mhz = 22,
    /// `NL80211_FREQUENCY_ATTR_4MHZ`
    _4Mhz = 23,
    /// `NL80211_FREQUENCY_ATTR_8MHZ`
    _8Mhz = 24,
    /// `NL80211_FREQUENCY_ATTR_16MHZ`
    _16Mhz = 25,
    /// `NL80211_FREQUENCY_ATTR_NO_320MHZ`
    No320mhz = 26,
    /// `NL80211_FREQUENCY_ATTR_NO_EHT`
    NoEht = 27,
    /// `NL80211_FREQUENCY_ATTR_PSD`
    Psd = 28,
    /// `NL80211_FREQUENCY_ATTR_DFS_CONCURRENT`
    DfsConcurrent = 29,
    /// `NL80211_FREQUENCY_ATTR_NO_6GHZ_VLP_CLIENT`
    No6ghzVlpClient = 30,
    /// `NL80211_FREQUENCY_ATTR_NO_6GHZ_AFC_CLIENT`
    No6ghzAfcClient = 31,
    /// `NL80211_FREQUENCY_ATTR_CAN_MONITOR`
    CanMonitor = 32,
    /// `NL80211_FREQUENCY_ATTR_ALLOW_6GHZ_VLP_AP`
    Allow6ghzVlpAp = 33,
    /// `NL80211_FREQUENCY_ATTR_ALLOW_20MHZ_ACTIVITY`
    Allow20mhzActivity = 34,
    /// `NL80211_FREQUENCY_ATTR_NO_4MHZ`
    No4mhz = 35,
    /// `NL80211_FREQUENCY_ATTR_NO_8MHZ`
    No8mhz = 36,
    /// `NL80211_FREQUENCY_ATTR_NO_16MHZ`
    No16mhz = 37,
    /// `NL80211_FREQUENCY_ATTR_S1G_NO_PRIMARY`
    S1gNoPrimary = 38,
    /// `NL80211_FREQUENCY_ATTR_NO_UHR`
    NoUhr = 39,
    /// `NL80211_FREQUENCY_ATTR_CAC_START_TIME`
    CacStartTime = 40,
    /// `NL80211_FREQUENCY_ATTR_PAD`
    Pad = 41,
}

impl neli::consts::genl::NlAttrType for Nl80211FreqAttr {}

#[neli::neli_enum(serialized_type = "u16")]
pub enum Nl80211BitrateAttr {
    /// `__NL80211_BITRATE_ATTR_INVALID`
    Invalid = 0,
    /// `NL80211_BITRATE_ATTR_RATE`
    Rate = 1,
    /// `NL80211_BITRATE_ATTR_2GHZ_SHORTPREAMBLE`
    _2ghzShortpreamble = 2,
}

impl neli::consts::genl::NlAttrType for Nl80211BitrateAttr {}

// enum nl80211_key_default_types
#[neli::neli_enum(serialized_type = "u16")]
pub enum Nl80211KeyDefaultTypes {
    Invalid = 0,
    Unicast = 1,
    Multicast = 2,
}

impl neli::consts::genl::NlAttrType for Nl80211KeyDefaultTypes {}

#[neli::neli_enum(serialized_type = "u32")]
pub enum Nl80211IfType {
    Unspecified = 0,
    Station = 2,
    Ap = 3,
    Monitor = 6,
    P2pDevice = 10,
    /* Several more, common ones above */
}

type NlMessage = Nlmsghdr<GenlId, Genlmsghdr<Nl80211Command, Nl80211Attribute>>;

#[derive(Debug)]
pub struct Frequency {
    pub freq: u32,
    pub offset: u32,
    pub max_tx_power: u32,
    pub disabled: bool,
    pub no_ir: bool,
}

impl From<&Frequency> for virtio_wifi::virtio_wifi_channel {
    fn from(freq: &Frequency) -> Self {
        use virtio_wifi::virtio_wifi_freq_band;
        let mut flags = IEEE80211ChannelFlags::empty();
        flags.set(IEEE80211ChannelFlags::DISABLED, freq.disabled);
        flags.set(IEEE80211ChannelFlags::NO_IR, freq.no_ir);
        Self {
            hdr: virtio_wifi::virtio_wifi_hdr {
                code: virtio_wifi::VIRTIO_WIFI_S_OK.into(),
                index: 0.into(),
            },
            // FIXME: do this properly
            band: match freq.freq {
                0..2000 => (virtio_wifi_freq_band::VIRTIO_WIFI_FREQ_BAND_S1GHZ as u32).into(),
                2000..4800 => (virtio_wifi_freq_band::VIRTIO_WIFI_FREQ_BAND_2GHZ as u32).into(),
                4800..5900 => (virtio_wifi_freq_band::VIRTIO_WIFI_FREQ_BAND_5GHZ as u32).into(),
                5900..7200 => (virtio_wifi_freq_band::VIRTIO_WIFI_FREQ_BAND_6GHZ as u32).into(),
                7200.. => (virtio_wifi_freq_band::VIRTIO_WIFI_FREQ_BAND_60GHZ as u32).into(),
            },
            center_freq: freq.freq.into(),
            freq_offset: (freq.offset as u16).into(),
            max_power: freq.max_tx_power.into(),
            flags: flags.bits().into(),
            ..Default::default()
        }
    }
}

#[derive(Debug)]
pub struct Rate {
    pub rate: u32,
    pub short_preamble: bool,
}

impl From<&Rate> for virtio_wifi::virtio_wifi_band_bitrate {
    fn from(rate: &Rate) -> Self {
        Self {
            hdr: virtio_wifi::virtio_wifi_hdr {
                code: virtio_wifi::VIRTIO_WIFI_S_OK.into(),
                index: 0.into(),
            },
            flags: u32::from(rate.short_preamble).into(),
            bitrate: u16::try_from(rate.rate).unwrap().into(),
            hw_value: u16::try_from(rate.rate).unwrap().into(),
            ..Default::default()
        }
    }
}

#[derive(Debug)]
pub struct Band {
    pub index: u32,
    pub rates: Vec<Rate>,
    pub freqs: Vec<Frequency>,
    pub ht_mcs_set: [u8; 16],
    pub ht_capa: u16,
    pub ht_ampdu_factor: u8,
    pub ht_ampdu_density: u8,
}

impl From<&Band> for virtio_wifi::virtio_wifi_band {
    fn from(band: &Band) -> Self {
        Self {
            hdr: virtio_wifi::virtio_wifi_hdr {
                code: virtio_wifi::VIRTIO_WIFI_S_OK.into(),
                index: band.index.into(),
            },
            n_channels: (band.freqs.len() as u32).into(),
            n_bitrates: (band.rates.len() as u32).into(),
            ht_cap: virtio_wifi::virtio_wifi_sta_ht_cap {
                cap: band.ht_capa.into(),
                ht_supported: (band.ht_capa > 0).into(),
                ampdu_factor: band.ht_ampdu_factor,
                ampdu_density: band.ht_ampdu_density,
                padding: 0,
                mcs: Default::default(), // TODO?
            },
            vht_cap: virtio_wifi::virtio_wifi_band_sta_vht_cap::default(),
            s1g_cap: virtio_wifi::virtio_wifi_band_sta_s1g_cap::default(),
            edmg_cap: virtio_wifi::virtio_wifi_band_virtio_wifi_edmg::default(),
            ..Default::default()
        }
    }
}

#[derive(Debug)]
pub enum Nl80211Error {
    LogicError(String),
    RouterError {
        inner: Box<dyn std::error::Error>,
        code: Errno,
    },
}

impl std::fmt::Display for Nl80211Error {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{self:?}")
    }
}

impl From<neli::genl::NlattrBuilderError> for Nl80211Error {
    fn from(err: neli::genl::NlattrBuilderError) -> Self {
        Self::LogicError(err.to_string())
    }
}

impl From<neli::genl::AttrTypeBuilderError> for Nl80211Error {
    fn from(err: neli::genl::AttrTypeBuilderError) -> Self {
        Self::LogicError(err.to_string())
    }
}

impl From<neli::genl::GenlmsghdrBuilderError> for Nl80211Error {
    fn from(err: neli::genl::GenlmsghdrBuilderError) -> Self {
        Self::LogicError(err.to_string())
    }
}

impl From<neli::err::SerError> for Nl80211Error {
    fn from(err: neli::err::SerError) -> Self {
        Self::LogicError(err.to_string())
    }
}

impl<T: std::fmt::Debug + 'static, P: std::fmt::Debug + 'static> From<neli::err::RouterError<T, P>>
    for Nl80211Error
{
    fn from(err: neli::err::RouterError<T, P>) -> Self {
        use neli::err::RouterError;

        match err {
            RouterError::Msg(_)
            | RouterError::Io(_)
            | RouterError::De(_)
            | RouterError::Socket(_)
            | RouterError::BadSeqOrPid(_)
            | RouterError::NoAck
            | RouterError::UnexpectedAck
            | RouterError::ClosedChannel => Self::RouterError {
                inner: err.into(),
                code: Errno::EIO,
            },
            RouterError::Nlmsgerr(ref nlmsgerr) => {
                let code = Errno::from_raw(nlmsgerr.error().abs());

                Self::RouterError {
                    inner: err.into(),
                    code,
                }
            }
        }
    }
}

impl From<Nl80211Error> for u32 {
    fn from(err: Nl80211Error) -> Self {
        match err {
            Nl80211Error::LogicError(_) => virtio_wifi::VIRTIO_WIFI_S_IO_ERR,
            Nl80211Error::RouterError { inner: _, code } => match code {
                Errno::EINVAL => virtio_wifi::VIRTIO_WIFI_S_BAD_MSG,
                Errno::ENOLINK => virtio_wifi::VIRTIO_WIFI_S_NO_LINK,
                Errno::ENETDOWN => virtio_wifi::VIRTIO_WIFI_S_NET_DOWN,
                Errno::ENOTCONN => virtio_wifi::VIRTIO_WIFI_S_NOT_CONN,
                Errno::EOPNOTSUPP => virtio_wifi::VIRTIO_WIFI_S_NOT_SUPP,
                _ => virtio_wifi::VIRTIO_WIFI_S_IO_ERR,
            },
        }
    }
}

/// `enum nl80211_feature_flags` - device/driver features
#[repr(u32)]
pub enum Nl80211FeatureFlags {
    /// `NL80211_FEATURE_SK_TX_STATUS`
    SkTxStatus = 1,
    /// `NL80211_FEATURE_HT_IBSS`
    HtIbss = 2,
    /// `NL80211_FEATURE_INACTIVITY_TIMER`
    InactivityTimer = 4,
    /// `NL80211_FEATURE_CELL_BASE_REG_HINTS`
    CellBaseRegHints = 8,
    /// `NL80211_FEATURE_P2P_DEVICE_NEEDS_CHANNEL`
    P2pDeviceNeedsChannel = 16,
    /// `NL80211_FEATURE_SAE`
    Sae = 32,
    /// `NL80211_FEATURE_LOW_PRIORITY_SCAN`
    LowPriorityScan = 64,
    /// `NL80211_FEATURE_SCAN_FLUSH`
    ScanFlush = 128,
    /// `NL80211_FEATURE_AP_SCAN`
    ApScan = 256,
    /// `NL80211_FEATURE_VIF_TXPOWER`
    VifTxpower = 512,
    /// `NL80211_FEATURE_NEED_OBSS_SCAN`
    NeedObssScan = 1024,
    /// `NL80211_FEATURE_P2P_GO_CTWIN`
    P2pGoCtwin = 2048,
    /// `NL80211_FEATURE_P2P_GO_OPPPS`
    P2pGoOppps = 4096,
    /// `NL80211_FEATURE_ADVERTISE_CHAN_LIMITS`
    AdvertiseChanLimits = 16384,
    /// `NL80211_FEATURE_FULL_AP_CLIENT_STATE`
    FullApClientState = 32768,
    /// `NL80211_FEATURE_USERSPACE_MPM`
    UserspaceMpm = 65536,
    /// `NL80211_FEATURE_ACTIVE_MONITOR`
    ActiveMonitor = 131072,
    /// `NL80211_FEATURE_AP_MODE_CHAN_WIDTH_CHANGE`
    ApModeChanWidthChange = 262144,
    /// `NL80211_FEATURE_DS_PARAM_SET_IE_IN_PROBES`
    DsParamSetIeInProbes = 524288,
    /// `NL80211_FEATURE_WFA_TPC_IE_IN_PROBES`
    WfaTpcIeInProbes = 1048576,
    /// `NL80211_FEATURE_QUIET`
    Quiet = 2097152,
    /// `NL80211_FEATURE_TX_POWER_INSERTION`
    TxPowerInsertion = 4194304,
    /// `NL80211_FEATURE_ACKTO_ESTIMATION`
    AcktoEstimation = 8388608,
    /// `NL80211_FEATURE_STATIC_SMPS`
    StaticSmps = 16777216,
    /// `NL80211_FEATURE_DYNAMIC_SMPS`
    DynamicSmps = 33554432,
    /// `NL80211_FEATURE_SUPPORTS_WMM_ADMISSION`
    SupportsWmmAdmission = 67108864,
    /// `NL80211_FEATURE_MAC_ON_CREATE`: Device supports configuring the vif's
    /// MAC address upon creation.
    MacOnCreate = 134217728,
    /// `NL80211_FEATURE_TDLS_CHANNEL_SWITCH`: Driver supports channel switching
    /// when operating as a TDLS peer.
    TdlsChannelSwitch = 268435456,
    /// `NL80211_FEATURE_SCAN_RANDOM_MAC_ADDR`
    ScanRandomMacAddr = 536870912,
    /// `NL80211_FEATURE_SCHED_SCAN_RANDOM_MAC_ADDR`
    SchedScanRandomMacAddr = 1073741824,
    /// `NL80211_FEATURE_ND_RANDOM_MAC_ADDR`
    NdRandomMacAddr = 2147483648,
}

#[derive(Debug)]
pub struct Interface {
    pub index: u32,
    pub wdev: u64,
    pub name: String,
    pub addr: [u8; 6],
    pub iftype: Result<virtio_wifi::virtio_wifi_iftype, u32>,
    pub response: virtio_wifi::virtio_wifi_interface,
}

impl TryFrom<NlMessage> for Interface {
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
        let mut attr_handles = attr_handle.iter().collect::<Vec<_>>();
        // FIXME: I think NL80211_ATTR_IFINDEX is not included in dump for first element
        // (?), confirm it.
        let index = if let Some(index) = attr_handles
            .extract_if(.., |attr| {
                matches!(attr.nla_type().nla_type(), Nl80211Attribute::IfIndex)
            })
            .next()
        {
            index.get_payload_as::<u32>().unwrap()
        } else {
            0
        };

        let name = if let Some(name) = attr_handles
            .extract_if(.., |attr| {
                matches!(attr.nla_type().nla_type(), Nl80211Attribute::IfName)
            })
            .next()
        {
            name.get_payload_as_with_len::<String>().unwrap()
        } else {
            String::new()
        };
        let iftype = attr_handles
            .extract_if(.., |attr| {
                matches!(attr.nla_type().nla_type(), Nl80211Attribute::IfType)
            })
            .next()
            .ok_or_else(|| {
                Nl80211Error::LogicError(format!(
                    "No {} in Interface Nl80211 response",
                    stringify!(Nl80211Attribute::IfType)
                ))
            })?;
        let iftype = if let Ok(iftype) = iftype.get_payload_as::<Nl80211IfType>() {
            match iftype {
                Nl80211IfType::Unspecified => {
                    Ok(virtio_wifi::virtio_wifi_iftype::VIRTIO_WIFI_IFTYPE_UNSPECIFIED)
                }
                Nl80211IfType::Station => {
                    Ok(virtio_wifi::virtio_wifi_iftype::VIRTIO_WIFI_IFTYPE_STATION)
                }
                Nl80211IfType::Ap => Ok(virtio_wifi::virtio_wifi_iftype::VIRTIO_WIFI_IFTYPE_AP),
                Nl80211IfType::Monitor => {
                    Ok(virtio_wifi::virtio_wifi_iftype::VIRTIO_WIFI_IFTYPE_MONITOR)
                }
                Nl80211IfType::P2pDevice => {
                    Ok(virtio_wifi::virtio_wifi_iftype::VIRTIO_WIFI_IFTYPE_P2P_DEVICE)
                }
                Nl80211IfType::UnrecognizedConst(v) => Err(v),
            }
        } else {
            let payload = iftype.get_payload_as::<u32>().unwrap();
            Err(payload)
        };
        let addr = attr_handles
            .extract_if(.., |attr| {
                matches!(attr.nla_type().nla_type(), Nl80211Attribute::Mac)
            })
            .next()
            .ok_or_else(|| {
                Nl80211Error::LogicError(format!(
                    "No {} in Interface Nl80211 response",
                    stringify!(Nl80211Attribute::Mac)
                ))
            })?
            .get_payload_as_with_len::<Vec<u8>>()
            .unwrap()
            .try_into()
            .unwrap();
        let wdev = attr_handles
            .extract_if(.., |attr| {
                matches!(attr.nla_type().nla_type(), Nl80211Attribute::WDev)
            })
            .next()
            .ok_or_else(|| {
                Nl80211Error::LogicError(format!(
                    "No {} in Interface Nl80211 response",
                    stringify!(Nl80211Attribute::WDev)
                ))
            })?
            .get_payload_as::<u64>()
            .unwrap();
        Ok(Self {
            name,
            index,
            iftype,
            addr,
            wdev,
            response: Default::default(),
        })
    }
}

impl From<&Interface> for virtio_wifi::virtio_wifi_interface {
    fn from(iface: &Interface) -> Self {
        let name = iface
            .name
            .as_bytes()
            .iter()
            .take(16)
            .cloned()
            .collect::<Vec<u8>>();
        let iftype = match iface.iftype {
            Ok(v) => v as u32,
            Err(v) => v,
        };
        let ifname_len = name.len();
        let mut resp = Self {
            hdr: virtio_wifi::virtio_wifi_hdr {
                code: virtio_wifi::VIRTIO_WIFI_S_OK.into(),
                index: iface.index.into(),
            },
            index: iface.index.into(),
            iftype: iftype.into(),
            ..Default::default()
        };
        resp.ifname[..ifname_len].copy_from_slice(&name);
        resp.addr.copy_from_slice(&iface.addr);
        resp
    }
}

/// `enum nl80211_chan_width` - channel width definitions
///
/// These values are used with the `%NL80211_ATTR_CHANNEL_WIDTH`
/// attribute.
#[doc(alias = "nl80211_chan_width")]
#[derive(Copy, Clone, Debug)]
#[repr(u32)]
pub enum Nl80211ChanWidth {
    /// `NL80211_CHAN_WIDTH_20_NOHT`: 20 MHz, non-HT channel
    _20NoHT = 0,
    /// `NL80211_CHAN_WIDTH_20`: 20 MHz HT channel
    _20 = 1,
    /// `NL80211_CHAN_WIDTH_40`: 40 MHz channel, the
    /// `%NL80211_ATTR_CENTER_FREQ1` attribute must be provided as well
    _40 = 2,
    /// `NL80211_CHAN_WIDTH_80`: 80 MHz channel, the
    /// `%NL80211_ATTR_CENTER_FREQ1` attribute must be provided as well
    _80 = 3,
    /// `NL80211_CHAN_WIDTH_80P80`: 80+80 MHz channel, the
    /// `%NL80211_ATTR_CENTER_FREQ1` and `%NL80211_ATTR_CENTER_FREQ2` attributes
    /// must be provided as well
    _80P80 = 4,
    /// `NL80211_CHAN_WIDTH_160` 160 MHz channel, the
    /// `%NL80211_ATTR_CENTER_FREQ1` attribute must be provided as well
    _160 = 5,
    /// `NL80211_CHAN_WIDTH_5`: 5 MHz OFDM channel
    _5 = 6,
    /// `NL80211_CHAN_WIDTH_10`: 10 MHz OFDM channel
    _10 = 7,
    /// `NL80211_CHAN_WIDTH_1`: 1 MHz OFDM channel
    _1 = 8,
    /// `NL80211_CHAN_WIDTH_2`: 2 MHz OFDM channel
    _2 = 9,
    /// `NL80211_CHAN_WIDTH_4`: 4 MHz OFDM channel
    _4 = 10,
    /// `NL80211_CHAN_WIDTH_8`: 8 MHz OFDM channel
    _8 = 11,
    /// `NL80211_CHAN_WIDTH_16`: 16 MHz OFDM channel
    _16 = 12,
    /// `NL80211_CHAN_WIDTH_320`: 320 MHz channel, the
    /// `%NL80211_ATTR_CENTER_FREQ1` attribute must be provided as well
    _320 = 13,
}

impl Nl80211ChanWidth {
    #[inline]
    pub const fn needs_center_freq1(self) -> bool {
        matches!(self, Self::_40 | Self::_80 | Self::_80P80 | Self::_320)
    }

    #[inline]
    pub const fn needs_center_freq2(self) -> bool {
        matches!(self, Self::_80P80)
    }
}

impl TryFrom<u32> for Nl80211ChanWidth {
    type Error = u32;

    fn try_from(val: u32) -> Result<Self, Self::Error> {
        Ok(match val {
            0 => Self::_20NoHT,
            1 => Self::_20,
            2 => Self::_40,
            3 => Self::_80,
            4 => Self::_80P80,
            5 => Self::_160,
            6 => Self::_5,
            7 => Self::_10,
            8 => Self::_1,
            9 => Self::_2,
            10 => Self::_4,
            11 => Self::_8,
            12 => Self::_16,
            13 => Self::_320,
            other => return Err(other),
        })
    }
}

#[repr(C, packed)]
#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub struct Nl80211StaFlagUpdate {
    pub mask: u32,
    pub set: u32,
}

// SAFETY: This struct is plain-old-data.
unsafe impl vm_memory::ByteValued for Nl80211StaFlagUpdate {}

pub type Receiver =
    NlRouterReceiverHandle<GenlId, Genlmsghdr<Nl80211Command, Nl80211Attribute, NoUserHeader>>;

pub struct Nl80211Socket {
    inner: Arc<NlRouter>,
    rtnl: Arc<NlRouter>,
    family_id: u16,
}

pub struct Notifier {
    pub event_receiver: std::sync::mpsc::Receiver<Vec<u8>>,
    pub event_fd: Arc<EventFd>,
    pub rx_event_fd: Arc<EventFd>,
    pub rx_receiver: std::sync::mpsc::Receiver<crate::io::RxWork<'static>>,
    pub exit: Arc<AtomicBool>,
}

impl Nl80211Socket {
    pub fn new() -> Result<(Self, Notifier), Box<dyn Error>> {
        const FAMILY_ID: &str = "nl80211";
        // Initialize NlRouter and resolve 'nl80211' genl family
        let (sock, multicast_receiver) = NlRouter::connect(
            NlFamily::Generic, /* family */
            Some(0),           /* pid */
            Groups::empty(),   /* groups */
        )?;
        sock.enable_ext_ack(true)?;
        let family_id = sock.resolve_genl_family(FAMILY_ID)?;

        // We want mcast notifications for these groups:
        let config_group = sock.resolve_nl_mcast_group(FAMILY_ID, "config")?;
        let scan_group = sock.resolve_nl_mcast_group(FAMILY_ID, "scan")?;
        let mlme_group = sock.resolve_nl_mcast_group(FAMILY_ID, "mlme")?;
        let groups = Groups::new_groups(&[config_group, scan_group, mlme_group]);
        sock.add_mcast_membership(groups)?;
        let inner = sock.into();

        let (event_sender, event_receiver) = std::sync::mpsc::channel();
        let (rx_sender, rx_receiver) = std::sync::mpsc::channel();
        let event_fd = Arc::new(EventFd::new().unwrap());
        let rx_event_fd = Arc::new(EventFd::new().unwrap());
        let exit = Arc::new(Default::default());

        std::thread::spawn({
            let mut mcast_listener = McastListener {
                multicast_receiver,
                sock: Arc::clone(&inner),
                event_fd: Arc::clone(&event_fd),
                rx_event_fd: Arc::clone(&rx_event_fd),
                exit: Arc::clone(&exit),
                family_id,
                event_sender,
                rx_sender,
            };
            move || mcast_listener.listen().unwrap()
        });

        let (rtnl, _) = NlRouter::connect(NlFamily::Route, None, Groups::empty())?;
        rtnl.enable_ext_ack(true)?;
        rtnl.enable_strict_checking(true)?;
        let rtnl = rtnl.into();

        Ok((
            Self {
                inner,
                rtnl,
                family_id,
            },
            Notifier {
                event_receiver,
                event_fd,
                rx_receiver,
                rx_event_fd,
                exit,
            },
        ))
    }

    pub fn list_wiphys(&self) -> Result<BTreeMap<u32, Wiphy>, Nl80211Error> {
        let attrs = vec![attr!(Nl80211Attribute::Wiphy = ())]
            .into_iter()
            .collect::<GenlBuffer<_, _>>();

        let recv = self.inner.send(
            self.family_id,
            NlmF::DUMP | NlmF::ACK,
            NlPayload::Payload(
                GenlmsghdrBuilder::<Nl80211Command, Nl80211Attribute, NoUserHeader>::default()
                    .cmd(Nl80211Command::GetWiphy)
                    .version(1)
                    .attrs(attrs)
                    .build()?,
            ),
        )?;

        let recv = recv.into_iter().collect::<Result<Vec<NlMessage>, _>>()?;
        let mut wiphys = recv
            .into_iter()
            .filter(|msg| !matches!(msg.nl_payload(), NlPayload::DumpExtAck(_)))
            .map(Wiphy::try_from)
            .map(|res| res.map(|w| (w.index, w)))
            .collect::<Result<BTreeMap<u32, Wiphy>, _>>()?;
        for w in wiphys.values_mut() {
            let properties = self.wiphy_properties(w.index)?;
            w.set_properties(properties);
        }
        log::trace!("wiphies = {:?}", &wiphys);
        Ok(wiphys)
    }

    fn wiphy_properties(&self, index: u32) -> Result<Vec<WiphyProperty>, Nl80211Error> {
        let attrs = vec![
            attr!(Nl80211Attribute::Wiphy = index),
            attr!(Nl80211Attribute::SplitWiphyDump = ()),
        ]
        .into_iter()
        .collect::<GenlBuffer<_, _>>();

        let recv = self.inner.send(
            self.family_id,
            NlmF::DUMP | NlmF::ACK,
            NlPayload::Payload(
                GenlmsghdrBuilder::<Nl80211Command, Nl80211Attribute, NoUserHeader>::default()
                    .cmd(Nl80211Command::GetWiphy)
                    .version(1)
                    .attrs(attrs)
                    .build()?,
            ),
        )?;
        let mut properties = vec![];
        for r in recv {
            let r: NlMessage = r?;
            properties.extend(parse_wiphy_properties(&r));
        }
        Ok(properties)
    }

    pub fn list_ifaces(&self, wiphy: &Wiphy) -> Result<BTreeMap<u32, Interface>, Nl80211Error> {
        let index = wiphy.index;

        let attrs = vec![
            attr!(Nl80211Attribute::Wiphy = index),
            attr!(Nl80211Attribute::IfName = ()),
        ]
        .into_iter()
        .collect::<GenlBuffer<_, _>>();

        let recv = self.inner.send(
            self.family_id,
            NlmF::DUMP | NlmF::ACK,
            NlPayload::Payload(
                GenlmsghdrBuilder::<Nl80211Command, Nl80211Attribute, NoUserHeader>::default()
                    .cmd(Nl80211Command::GetInterface)
                    .attrs(attrs)
                    .version(1)
                    .build()?,
            ),
        )?;
        let recv = recv.into_iter().collect::<Result<Vec<NlMessage>, _>>()?;
        recv.into_iter()
            .filter(|msg| !matches!(msg.nl_payload(), NlPayload::DumpExtAck(_)))
            .map(Interface::try_from)
            .map(|res| res.map(|i| (i.index, i)))
            .collect::<Result<BTreeMap<u32, Interface>, _>>()
    }

    pub fn add_iface(
        &self,
        wiphy: &Wiphy,
        mut iface: virtio_wifi::virtio_wifi_interface,
    ) -> Result<Interface, Nl80211Error> {
        let index = wiphy.index;
        log::trace!("Requesting new vif on wiphy {index}");
        let mut attrs = vec![
            attr!(Nl80211Attribute::Wiphy = index),
            attr!(Nl80211Attribute::IfType = Nl80211IfType::Station),
            attr!(Nl80211Attribute::IfName = "wlan0"),
        ];
        if wiphy.feature_flags | Nl80211FeatureFlags::MacOnCreate as u32 > 0 {
            log::trace!("requesting new iface mac to be {:?}", iface.addr);
            attrs.push(attr!(Nl80211Attribute::Mac = iface.addr));
        }

        let attrs = attrs.into_iter().collect::<GenlBuffer<_, _>>();

        let recv = self.inner.send(
            self.family_id,
            NlmF::REQUEST | NlmF::ACK,
            NlPayload::Payload(
                GenlmsghdrBuilder::<Nl80211Command, Nl80211Attribute, NoUserHeader>::default()
                    .cmd(Nl80211Command::NewInterface)
                    .attrs(attrs)
                    .version(1)
                    .build()?,
            ),
        )?;
        let recv = recv.into_iter().collect::<Result<Vec<NlMessage>, _>>()?;
        // dbg!(&recv);
        let mut new: Interface = recv
            .into_iter()
            .filter(|msg| !matches!(msg.nl_payload(), NlPayload::DumpExtAck(_)))
            .map(Interface::try_from)
            .next()
            .unwrap()
            .unwrap();
        log::trace!("new iface mac is {:?}", new.addr);

        iface.hdr.index = wiphy.index.into();
        iface.index = new.index.into();
        new.response = iface;

        log::info!("Created new vif on wiphy {index}: {new:?}");
        log::debug!("Registering frames...");

        for (ft, match_bytes) in &[(FrameType::MGMT_AUTH, b"\x03\x00")] {
            if wiphy.supported_rx_frames.contains(ft) {
                let result = self.register_frame(new.wdev, new.index, *ft, match_bytes.as_slice());
                log::debug!("{ft:?}: {result:?}");
            }
        }
        log::debug!("Registering frames: done");
        Ok(new)
    }

    pub fn del_iface(&self, wiphy: &Wiphy, iface_index: u32) -> Result<(), Nl80211Error> {
        let index = wiphy.index;
        log::info!("Requesting deletion of vif {iface_index} on wiphy {index}");
        let attrs = vec![
            attr!(Nl80211Attribute::Wiphy = index),
            attr!(Nl80211Attribute::IfIndex = iface_index),
        ]
        .into_iter()
        .collect::<GenlBuffer<_, _>>();

        let recv = self.inner.send(
            self.family_id,
            NlmF::REQUEST | NlmF::ACK,
            NlPayload::Payload(
                GenlmsghdrBuilder::<Nl80211Command, Nl80211Attribute, NoUserHeader>::default()
                    .cmd(Nl80211Command::DelInterface)
                    .attrs(attrs)
                    .version(1)
                    .build()?,
            ),
        )?;
        let _recv = recv.into_iter().collect::<Result<Vec<NlMessage>, _>>()?;
        // dbg!(&_recv);
        Ok(())
    }

    pub fn set_channel(
        &self,
        wiphy: &Wiphy,
        iface_index: u32,
        chandef: &virtio_wifi::virtio_wifi_channel,
    ) -> Result<(), Nl80211Error> {
        let index = wiphy.index;
        let center_freq: u32 = chandef.center_freq.into();
        let center_freq1: u32 = chandef.center_freq1.into();
        let center_freq2: u32 = chandef.center_freq2.into();
        let width: u32 = chandef.width.into();
        let width: Nl80211ChanWidth = width.try_into().unwrap();
        log::info!("Requesting setting channel of wiphy {index} iface {iface_index} to {width:?}");
        let mut attrs = vec![
            attr!(Nl80211Attribute::IfIndex = iface_index),
            attr!(Nl80211Attribute::WiphyFreq = center_freq),
            attr!(Nl80211Attribute::ChannelWidth = width as u32),
        ];

        if width.needs_center_freq1() {
            attrs.push(attr!(Nl80211Attribute::CenterFreq1 = center_freq1));
        }
        if width.needs_center_freq2() {
            attrs.push(attr!(Nl80211Attribute::CenterFreq2 = center_freq2));
        }

        let attrs = attrs.into_iter().collect::<GenlBuffer<_, _>>();

        let recv = self.inner.send(
            self.family_id,
            NlmF::REQUEST | NlmF::ACK,
            NlPayload::Payload(
                GenlmsghdrBuilder::<Nl80211Command, Nl80211Attribute, NoUserHeader>::default()
                    .cmd(Nl80211Command::SetChannel)
                    .attrs(attrs)
                    .version(1)
                    .build()?,
            ),
        )?;
        let _recv = recv.into_iter().collect::<Result<Vec<NlMessage>, _>>()?;
        // dbg!(&_recv);
        Ok(())
    }

    pub fn trigger_scan(&self, iface_index: u32) -> Result<(), Nl80211Error> {
        // TODO: allow selecting frequencies with NL80211_ATTR_SCAN_FREQUENCIES
        // TODO: allow selecting SSIDs with NL80211_ATTR_SCAN_SSIDS
        log::debug!("trigger scan vif {iface_index}");
        let attrs = vec![
            attr!(Nl80211Attribute::IfIndex = iface_index),
            // Wildcard SSID search
            attr!(Nl80211Attribute::Ssid = &b""[..]),
        ]
        .into_iter()
        .collect::<GenlBuffer<_, _>>();

        let recv = self.inner.send(
            self.family_id,
            NlmF::ACK,
            NlPayload::Payload(
                GenlmsghdrBuilder::<Nl80211Command, Nl80211Attribute, NoUserHeader>::default()
                    .cmd(Nl80211Command::TriggerScan)
                    .attrs(attrs)
                    .version(1)
                    .build()?,
            ),
        )?;
        let _recv = recv.into_iter().collect::<Result<Vec<NlMessage>, _>>()?;
        // dbg!(&_recv);
        Ok(())
    }

    pub fn register_frame(
        &self,
        wdev: u64,
        iface_index: u32,
        ft: FrameType,
        match_bytes: &[u8],
    ) -> Result<(), Nl80211Error> {
        log::trace!("registering {ft:?}");
        let attrs = vec![
            attr!(Nl80211Attribute::WDev = wdev),
            attr!(Nl80211Attribute::IfIndex = iface_index),
            attr!(Nl80211Attribute::FrameType = ft.as_u16()),
            attr!(Nl80211Attribute::FrameMatch = match_bytes),
        ]
        .into_iter()
        .collect::<GenlBuffer<_, _>>();

        let recv = self.inner.send(
            self.family_id,
            NlmF::ACK,
            NlPayload::Payload(
                GenlmsghdrBuilder::<Nl80211Command, Nl80211Attribute, NoUserHeader>::default()
                    .cmd(Nl80211Command::RegisterFrame)
                    .attrs(attrs)
                    .version(1)
                    .build()?,
            ),
        )?;
        let _recv = recv
            .into_iter()
            .collect::<Result<Vec<NlMessage>, _>>()
            .map_err(Nl80211Error::from);
        if matches!(_recv, Err(Nl80211Error::RouterError {inner: _, ref code}) if *code == Errno::EALREADY)
        {
            return Ok(());
        }
        _recv?;
        // dbg!(&_recv);
        Ok(())
    }

    pub fn authenticate(
        &self,
        iface_index: u32,
        req: &virtio_wifi::virtio_wifi_authentication_request,
        ie: &[u8],
    ) -> Result<(), Nl80211Error> {
        log::debug!("authenticate vif {iface_index} req={req:#?}");

        let bssid = req.mac.as_slice();
        let ssid = &req.ssid[..u16::from(req.ssid_len).into()];
        let freq = u32::from(req.freq);
        let freq_offset = u32::from(req.freq_offset);
        let auth_type = u32::from(req.auth_type);

        // Warning: if ssid is empty, the response cannot be deserialized; there is some
        // kind of error response that neli cannot parse maybe.
        let mut attrs = vec![
            attr!(Nl80211Attribute::IfIndex = iface_index),
            attr!(Nl80211Attribute::Mac = bssid),
            attr!(Nl80211Attribute::Ssid = ssid),
            attr!(Nl80211Attribute::WiphyFreq = freq),
            attr!(Nl80211Attribute::WiphyFreqOffset = freq_offset),
            attr!(Nl80211Attribute::AuthType = auth_type),
        ];

        if !ie.is_empty() {
            attrs.push(attr!(Nl80211Attribute::Ie = ie));
        }
        let attrs = attrs.into_iter().collect::<GenlBuffer<_, _>>();
        let recv = self.inner.send(
            self.family_id,
            NlmF::ACK,
            NlPayload::Payload(
                GenlmsghdrBuilder::<Nl80211Command, Nl80211Attribute, NoUserHeader>::default()
                    .cmd(Nl80211Command::Authenticate)
                    .attrs(attrs)
                    .version(1)
                    .build()?,
            ),
        )?;
        let _recv = recv.into_iter().collect::<Result<Vec<NlMessage>, _>>()?;
        // dbg!(&_recv);
        Ok(())
    }

    pub fn associate(
        &self,
        iface_index: u32,
        req: &virtio_wifi::virtio_wifi_association_request,
        ie: &[u8],
    ) -> Result<(), Nl80211Error> {
        log::debug!("associate vif {iface_index}");

        let bssid = req.mac.as_slice();
        let ssid = &req.ssid[..u16::from(req.ssid_len).into()];
        let freq = u32::from(req.freq);
        let wpa_versions = u32::from(req.wpa_versions);
        let cipher_group = u32::from(req.cipher_group);

        let mut cipher_suites_pairwise = [0_u32; 5];
        let n_ciphers_pairwise = u32::from(req.n_ciphers_pairwise) as usize;
        for i in 0..n_ciphers_pairwise {
            cipher_suites_pairwise[i] = req.ciphers_pairwise[i].into();
        }

        let mut akm_suites = [0_u32; 10];
        let n_akm_suites = u32::from(req.n_akm_suites) as usize;
        for i in 0..n_akm_suites {
            akm_suites[i] = req.akm_suites[i].into();
        }
        let mut attrs = vec![
            attr!(Nl80211Attribute::IfIndex = iface_index),
            attr!(Nl80211Attribute::Mac = bssid),
            attr!(Nl80211Attribute::Ssid = ssid),
            attr!(Nl80211Attribute::WiphyFreq = freq),
        ];
        if n_akm_suites > 0 {
            attrs.push(attr!(
                Nl80211Attribute::AkmSuites = akm_suites[..n_akm_suites].to_vec()
            ));
        }
        if n_ciphers_pairwise > 0 {
            attrs.push(attr!(
                Nl80211Attribute::CipherSuitesPairwise =
                    cipher_suites_pairwise[..n_ciphers_pairwise].to_vec()
            ));
        }
        if wpa_versions > 0 {
            attrs.push(attr!(Nl80211Attribute::WpaVersions = wpa_versions));
        }
        if cipher_group > 0 {
            attrs.push(attr!(Nl80211Attribute::CipherSuiteGroup = cipher_group));
        }

        if !ie.is_empty() {
            attrs.push(attr!(Nl80211Attribute::Ie = ie));
        }
        let attrs = attrs.into_iter().collect::<GenlBuffer<_, _>>();
        let recv = self.inner.send(
            self.family_id,
            NlmF::ACK,
            NlPayload::Payload(
                GenlmsghdrBuilder::<Nl80211Command, Nl80211Attribute, NoUserHeader>::default()
                    .cmd(Nl80211Command::Associate)
                    .attrs(attrs)
                    .version(1)
                    .build()?,
            ),
        )?;
        let _recv = recv.into_iter().collect::<Result<Vec<NlMessage>, _>>()?;
        // dbg!(&_recv);
        Ok(())
    }

    pub fn add_key(
        &self,
        iface_index: u32,
        req: &virtio_wifi::virtio_wifi_key,
        key: &[u8],
        seq: &[u8],
    ) -> Result<(), Nl80211Error> {
        log::debug!("add_key to vif {iface_index}");

        let mac = req.mac.as_slice();
        let cipher: u32 = req.cipher.into();
        let mut attrs = vec![
            attr!(Nl80211Attribute::IfIndex = iface_index),
            attr!(Nl80211Attribute::KeyIdx = req.key_index),
            attr!(Nl80211Attribute::KeyCipher = cipher),
            attr!(Nl80211Attribute::KeyData = key),
            attr!(Nl80211Attribute::KeySeq = seq),
        ];
        if req.pairwise > 0 {
            attrs.push(attr!(Nl80211Attribute::Mac = mac));
            attrs.push(attr!(Nl80211Attribute::KeyType = 1_u32));
        } else {
            attrs.push(attr!(Nl80211Attribute::KeyType = 0_u32));
            debug_assert_eq!(req.mac, [0; 6], "{mac:?}");
        }
        let attrs = attrs.into_iter().collect::<GenlBuffer<_, _>>();

        let recv = self.inner.send(
            self.family_id,
            NlmF::ACK,
            NlPayload::Payload(
                GenlmsghdrBuilder::<Nl80211Command, Nl80211Attribute, NoUserHeader>::default()
                    .cmd(Nl80211Command::NewKey)
                    .attrs(attrs)
                    .version(1)
                    .build()?,
            ),
        )?;
        let _recv = recv.into_iter().collect::<Result<Vec<NlMessage>, _>>()?;
        // dbg!(&_recv);
        Ok(())
    }

    pub fn del_key(
        &self,
        iface_index: u32,
        req: &virtio_wifi::virtio_wifi_key,
    ) -> Result<(), Nl80211Error> {
        log::debug!("del_key to vif {iface_index}");
        let mac = req.mac.as_slice();
        let mut attrs = vec![
            attr!(Nl80211Attribute::IfIndex = iface_index),
            attr!(Nl80211Attribute::KeyIdx = req.key_index),
        ];

        if req.pairwise > 0 {
            attrs.push(attr!(Nl80211Attribute::Mac = mac));
            // attrs.push(attr!(Nl80211Attribute::KeyType = 1_u32));
        } else {
            //attrs.push(attr!(Nl80211Attribute::KeyType = 0_u32));
            debug_assert_eq!(req.mac, [0; 6], "{mac:?}");
        }

        let attrs = attrs.into_iter().collect::<GenlBuffer<_, _>>();

        let recv = self.inner.send(
            self.family_id,
            NlmF::ACK,
            NlPayload::Payload(
                GenlmsghdrBuilder::<Nl80211Command, Nl80211Attribute, NoUserHeader>::default()
                    .cmd(Nl80211Command::DelKey)
                    .attrs(attrs)
                    .version(1)
                    .build()?,
            ),
        )?;
        let _recv = recv.into_iter().collect::<Result<Vec<NlMessage>, _>>()?;
        // dbg!(&_recv);
        Ok(())
    }

    pub fn set_key(
        &self,
        iface_index: u32,
        req: &virtio_wifi::virtio_wifi_key,
    ) -> Result<(), Nl80211Error> {
        log::debug!("set_key to vif {iface_index}");
        let mut key_default_types = attr!(Nl80211Attribute::KeyDefaultTypes = Vec::<u8>::new());
        if req.unicast > 0 {
            key_default_types =
                key_default_types.nest(&attr!(Nl80211KeyDefaultTypes::Unicast = ()))?;
        }
        if req.multicast > 0 {
            key_default_types =
                key_default_types.nest(&attr!(Nl80211KeyDefaultTypes::Multicast = ()))?;
        }
        let attrs = vec![
            attr!(Nl80211Attribute::IfIndex = iface_index),
            attr!(Nl80211Attribute::KeyIdx = req.key_index),
            attr!(Nl80211Attribute::KeyDefault = ()),
            key_default_types,
        ]
        .into_iter()
        .collect::<GenlBuffer<_, _>>();

        let recv = self.inner.send(
            self.family_id,
            NlmF::ACK,
            NlPayload::Payload(
                GenlmsghdrBuilder::<Nl80211Command, Nl80211Attribute, NoUserHeader>::default()
                    .cmd(Nl80211Command::SetKey)
                    .attrs(attrs)
                    .version(1)
                    .build()?,
            ),
        )?;
        let _recv = recv.into_iter().collect::<Result<Vec<NlMessage>, _>>()?;
        // dbg!(&_recv);
        Ok(())
    }

    pub fn set_station(
        &self,
        iface_index: u32,
        req: &virtio_wifi::virtio_wifi_change_station,
    ) -> Result<(), Nl80211Error> {
        log::debug!("set STA flags for vif {iface_index}");

        let mac = req.mac.as_slice();

        let flags = Nl80211StaFlagUpdate {
            mask: req.mask.into(),
            set: req.set.into(),
        };

        let attrs = vec![
            attr!(Nl80211Attribute::IfIndex = iface_index),
            attr!(Nl80211Attribute::Mac = mac),
            attr!(Nl80211Attribute::StaFlags2 = flags.as_slice()),
        ]
        .into_iter()
        .collect::<GenlBuffer<_, _>>();

        let recv = self.inner.send(
            self.family_id,
            NlmF::ACK,
            NlPayload::Payload(
                GenlmsghdrBuilder::<Nl80211Command, Nl80211Attribute, NoUserHeader>::default()
                    .cmd(Nl80211Command::SetStation)
                    .attrs(attrs)
                    .version(1)
                    .build()?,
            ),
        )?;
        let _recv = recv.into_iter().collect::<Result<Vec<NlMessage>, _>>()?;
        // dbg!(&_recv);
        Ok(())
    }

    pub fn connect(
        &self,
        _wiphy: &Wiphy,
        iface_index: u32,
        req: &virtio_wifi::virtio_wifi_authentication_request,
        ie: &[u8],
    ) -> Result<(), Nl80211Error> {
        log::debug!("connect vif {iface_index}");

        let bssid = req.mac.as_slice();
        let ssid = &req.ssid[..u16::from(req.ssid_len).into()];
        let freq = u32::from(req.freq);
        let freq_offset = u32::from(req.freq_offset);
        let auth_type = u32::from(req.auth_type);
        let privacy = req.privacy > 0;

        // Warning: if ssid is empty, the response cannot be deserialized; there is some
        // kind of error response that neli cannot parse maybe.
        let mut attrs = vec![
            attr!(Nl80211Attribute::IfIndex = iface_index),
            attr!(Nl80211Attribute::Ssid = ssid),
            attr!(Nl80211Attribute::AuthType = auth_type),
        ];
        if !ie.is_empty() {
            attrs.push(attr!(Nl80211Attribute::Ie = ie));
        }
        if bssid != [0; 6] {
            attrs.push(attr!(Nl80211Attribute::Mac = bssid));
        }
        if freq > 0 {
            attrs.push(attr!(Nl80211Attribute::WiphyFreq = freq));
            attrs.push(attr!(Nl80211Attribute::WiphyFreqOffset = freq_offset));
        }

        if privacy {
            let wpa_versions = u32::from(req.wpa_versions);
            let cipher_group = u32::from(req.cipher_group);

            let mut cipher_suites_pairwise = [0_u32; 5];
            let n_ciphers_pairwise = u32::from(req.n_ciphers_pairwise) as usize;
            for i in 0..n_ciphers_pairwise {
                cipher_suites_pairwise[i] = req.ciphers_pairwise[i].into();
            }

            let mut akm_suites = [0_u32; 10];
            let n_akm_suites = u32::from(req.n_akm_suites) as usize;
            for i in 0..n_akm_suites {
                akm_suites[i] = req.akm_suites[i].into();
            }
            if n_akm_suites > 0 {
                attrs.push(attr!(
                    Nl80211Attribute::AkmSuites = akm_suites[..n_akm_suites].to_vec()
                ));
            }
            if n_ciphers_pairwise > 0 {
                attrs.push(attr!(
                    Nl80211Attribute::CipherSuitesPairwise =
                        cipher_suites_pairwise[..n_ciphers_pairwise].to_vec()
                ));
            }
            if wpa_versions > 0 {
                attrs.push(attr!(Nl80211Attribute::WpaVersions = wpa_versions));
            }
            if cipher_group > 0 {
                attrs.push(attr!(Nl80211Attribute::CipherSuiteGroup = cipher_group));
            }
        }

        let attrs = attrs.into_iter().collect::<GenlBuffer<_, _>>();

        let recv = self.inner.send(
            self.family_id,
            NlmF::ACK,
            NlPayload::Payload(
                GenlmsghdrBuilder::<Nl80211Command, Nl80211Attribute, NoUserHeader>::default()
                    .cmd(Nl80211Command::Connect)
                    .attrs(attrs)
                    .version(1)
                    .build()?,
            ),
        )?;
        let _recv = recv.into_iter().collect::<Result<Vec<NlMessage>, _>>()?;
        // dbg!(&_recv);
        Ok(())
    }

    pub fn disconnect(
        &self,
        iface_index: u32,
        req: &virtio_wifi::virtio_wifi_disconnect_request,
        ie: &[u8],
    ) -> Result<(), Nl80211Error> {
        log::debug!("disconnect vif {iface_index} {req:?}");

        let cmd = u32::from(req.hdr.code);
        let cmd = if cmd == virtio_wifi::VIRTIO_WIFI_CMD_DEAUTHENTICATE {
            Nl80211Command::Deauthenticate
        } else {
            assert_eq!(cmd, virtio_wifi::VIRTIO_WIFI_CMD_DISASSOCIATE);
            Nl80211Command::Disassociate
        };
        let mac = req.mac.as_slice();

        let reason_code: u16 = req.reason_code.into();
        let local_state_change = req.local_state_change > 0;

        let mut attrs = vec![
            attr!(Nl80211Attribute::IfIndex = iface_index),
            attr!(Nl80211Attribute::ReasonCode = reason_code),
        ];
        if !ie.is_empty() {
            attrs.push(attr!(Nl80211Attribute::Ie = ie));
        }
        if mac != [0; 6] {
            attrs.push(attr!(Nl80211Attribute::Mac = mac));
        }
        if local_state_change {
            attrs.push(attr!(Nl80211Attribute::LocalStateChange = ()));
        }

        let attrs = attrs.into_iter().collect::<GenlBuffer<_, _>>();

        let recv = self.inner.send(
            self.family_id,
            NlmF::ACK,
            NlPayload::Payload(
                GenlmsghdrBuilder::<Nl80211Command, Nl80211Attribute, NoUserHeader>::default()
                    .cmd(cmd)
                    .attrs(attrs)
                    .version(1)
                    .build()?,
            ),
        )?;
        let _recv = recv.into_iter().collect::<Result<Vec<NlMessage>, _>>()?;
        // dbg!(&_recv);
        Ok(())
    }

    pub fn if_operstate(
        &self,
        iface_index: u32,
        req: &virtio_wifi::virtio_wifi_set_if_operstate,
    ) -> Result<(), Box<dyn Error>> {
        log::debug!("set if_operstate vif {iface_index} {req:?}");
        const IFF_UP: u32 =
            1 << (virtio_wifi::virtio_wifi_if_operstate::VIRTIO_WIFI_IF_OPER_UP as u32);

        let flags: u32 = req.flags.into();
        let change_mask: u32 = req.change_mask.into();
        // let mut rtbuffer = neli::types::RtBuffer::new();
        // rtbuffer.push(
        //     neli::rtnl::RtattrBuilder::default()
        //         .rta_type(Ifla::Operstate)
        //         .rta_payload(state)
        //         .build()
        //         .unwrap(),
        // );
        let ifinfomsg = IfinfomsgBuilder::default()
            .ifi_index(iface_index.cast_signed())
            .ifi_family(RtAddrFamily::Unspecified);
        // .rtattrs(rtbuffer);

        let ifinfomsg = if flags & IFF_UP > 0 && change_mask & IFF_UP > 0 {
            ifinfomsg.up().build()?
        } else if flags & IFF_UP == 0 && change_mask & IFF_UP > 0 {
            ifinfomsg.down().build()?
        } else {
            ifinfomsg.build()?
        };

        let recv = self.rtnl.send::<_, _, Rtm, Ifinfomsg>(
            Rtm::Setlink,
            NlmF::REQUEST | NlmF::ACK,
            NlPayload::Payload(ifinfomsg),
        )?;
        for response in recv {
            if let Some(payload) = response?.get_payload() {
                log::trace!("{:?}", payload)
            }
        }
        Ok(())
    }
}
