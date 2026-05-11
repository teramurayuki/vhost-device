use std::{
    collections::BTreeMap,
    error::Error,
    sync::Arc, //
};

use neli::{
    attr::Attribute,
    consts::{
        nl::{GenlId, NlmF},
        socket::NlFamily,
    },
    genl::{AttrTypeBuilder, Genlmsghdr, GenlmsghdrBuilder, NlattrBuilder, NoUserHeader},
    nl::{NlPayload, Nlmsghdr},
    router::synchronous::{NlRouter, NlRouterReceiverHandle},
    types::GenlBuffer,
    utils::Groups,
};

use nix::sys::eventfd::EventFd;
use vm_memory::ByteValued;

use crate::virtio_wifi;

mod bss;
use bss::*;

#[neli::neli_enum(serialized_type = "u8")]
pub enum Nl80211Command {
    Unspecified = 0,
    GetWiPhy = 1,
    GetInterface = 5,
    /// `NL80211_CMD_NEW_INTERFACE`
    NewInterface = 7,
    /// `NL80211_CMD_DEL_INTERFACE`
    DelInterface = 8,
    /// `NL80211_CMD_GET_SCAN`
    GetScan = 32,
    /// `NL80211_CMD_TRIGGER_SCAN`
    TriggerScan = 33,
    /// `NL80211_CMD_SET_CHANNEL`
    SetChannel = 65,
    /// `NL80211_CMD_NEW_SCAN_RESULTS`
    NewScanResults = 34,
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

impl From<&Frequency> for virtio_wifi::VirtioWifiChannel {
    fn from(freq: &Frequency) -> Self {
        use virtio_wifi::{IEEE80211ChannelFlags, VirtioWifiFreqBand};
        let mut flags = IEEE80211ChannelFlags::empty();
        flags.set(IEEE80211ChannelFlags::DISABLED, freq.disabled);
        flags.set(IEEE80211ChannelFlags::NO_IR, freq.no_ir);
        Self {
            hdr: virtio_wifi::VirtioWifiHdr {
                code: virtio_wifi::VIRTIO_WIFI_S_OK.into(),
                index: 0.into(),
            },
            // FIXME: do this properly
            band: match freq.freq {
                0..2000 => (VirtioWifiFreqBand::S1GHZ as u32).into(),
                2000..4800 => (VirtioWifiFreqBand::_2GHZ as u32).into(),
                4800..5900 => (VirtioWifiFreqBand::_5GHZ as u32).into(),
                5900..7200 => (VirtioWifiFreqBand::_6GHZ as u32).into(),
                7200.. => (VirtioWifiFreqBand::_60GHZ as u32).into(),
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

impl From<&Rate> for virtio_wifi::VirtioWifiBandBitrate {
    fn from(rate: &Rate) -> Self {
        Self {
            hdr: virtio_wifi::VirtioWifiHdr {
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

impl From<&Band> for virtio_wifi::VirtioWifiBand {
    fn from(band: &Band) -> Self {
        Self {
            hdr: virtio_wifi::VirtioWifiHdr {
                code: virtio_wifi::VIRTIO_WIFI_S_OK.into(),
                index: band.index.into(),
            },
            n_channels: (band.freqs.len() as u32).into(),
            n_bitrates: (band.rates.len() as u32).into(),
            ht_cap: virtio_wifi::VirtioWifiStaHtCap {
                cap: band.ht_capa.into(),
                ht_supported: (band.ht_capa > 0).into(),
                ampdu_factor: band.ht_ampdu_factor,
                ampdu_density: band.ht_ampdu_density,
                padding: 0,
                mcs: Default::default(), // TODO?
            },
            vht_cap: virtio_wifi::VirtioWifiStaVhtCap::default(),
            s1g_cap: virtio_wifi::VirtioWifiStaS1gCap::default(),
            edmg_cap: virtio_wifi::VirtioWifiEdmg::default(),
            ..Default::default()
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
    pub cipher_suites: u32,
    pub max_num_pmkids: u8,
    // Protocols other than PAE are supported.
    pub control_port_ethertype: bool,
    pub wiphy_antenna_avail_tx: u32,
    pub wiphy_antenna_avail_rx: u32,
    pub wiphy_antenna_tx: u32,
    pub wiphy_antenna_rx: u32,
    pub supported_iftypes: u32,
    pub wiphy_bands: u32,
    pub supported_commands: u32,
    pub max_remain_on_channel_duration: u32,
    pub offchannel_tx_ok: bool,
    pub software_iftypes: u32,
    pub interface_combinations: u32,
    pub ht_capability_mask: u32,
}

impl TryFrom<NlMessage> for Wiphy {
    type Error = ();

    fn try_from(msg: NlMessage) -> std::result::Result<Self, Self::Error> {
        // Messages with the NlmF::DUMP flag end with an empty payload message
        // Don't parse message unless receive proper payload (non-error, non-empty,
        // non-ack)
        let NlPayload::Payload(payload) = msg.nl_payload() else {
            return Err(());
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
                    .ok_or(())?;
                temp.get_payload_as_with_len::<String>().unwrap()
            }};
            ($attr:ident : $t:ty) => {{
                let temp = attr_handles
                    .extract_if(.., |attr| {
                        matches!(attr.nla_type().nla_type(), Nl80211Attribute::$attr)
                    })
                    .next()
                    .ok_or(())?;
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
        let cipher_suites = extract!(CipherSuites: u32);
        let max_num_pmkids = extract!(MaxNumPmkids: u8);
        let control_port_ethertype = extract!(ControlPortEthertype: bool);
        let wiphy_antenna_avail_tx = extract!(WiphyAntennaAvailTx: u32);
        let wiphy_antenna_avail_rx = extract!(WiphyAntennaAvailRx: u32);
        let wiphy_antenna_tx = extract!(WiphyAntennaTx: u32);
        let wiphy_antenna_rx = extract!(WiphyAntennaRx: u32);
        let supported_iftypes = extract!(SupportedIftypes: u32);
        let wiphy_bands = extract!(WiphyBands: u32);
        let supported_commands = extract!(SupportedCommands: u32);
        let max_remain_on_channel_duration = extract!(MaxRemainOnChannelDuration: u32);
        let offchannel_tx_ok = extract!(OffchannelTxOk: bool);
        let software_iftypes = extract!(SoftwareIftypes: u32);
        let interface_combinations = extract!(InterfaceCombinations: u32);
        let ht_capability_mask = extract!(HtCapabilityMask: u32);
        Ok(Self {
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
            supported_commands,
            max_remain_on_channel_duration,
            offchannel_tx_ok,
            software_iftypes,
            interface_combinations,
            ht_capability_mask,
        })
    }
}

impl From<&Wiphy> for virtio_wifi::VirtioWifiRadio {
    fn from(wiphy: &Wiphy) -> Self {
        let name = wiphy
            .name
            .as_bytes()
            .iter()
            .take(64)
            .cloned()
            .collect::<Vec<u8>>();
        let hwname_len = name.len();
        let mut resp = Self {
            hdr: virtio_wifi::VirtioWifiHdr {
                code: virtio_wifi::VIRTIO_WIFI_S_OK.into(),
                index: wiphy.index.into(),
            },
            available_antennas_tx: wiphy.wiphy_antenna_avail_tx.into(),
            available_antennas_rx: wiphy.wiphy_antenna_avail_rx.into(),
            ..Default::default()
        };
        resp.hwname[..hwname_len].copy_from_slice(&name);
        resp.hwname_len = (hwname_len as u32).into();
        resp
    }
}

#[derive(Debug)]
pub struct Interface {
    pub index: u32,
    pub name: String,
    pub addr: [u8; 6],
    pub iftype: Result<crate::virtio_wifi::IfType, u32>,
    pub response: virtio_wifi::VirtioWifiInterface,
}

impl TryFrom<NlMessage> for Interface {
    type Error = ();

    fn try_from(msg: NlMessage) -> std::result::Result<Self, Self::Error> {
        // Messages with the NlmF::DUMP flag end with an empty payload message
        // Don't parse message unless receive proper payload (non-error, non-empty,
        // non-ack)
        let NlPayload::Payload(payload) = msg.nl_payload() else {
            return Err(());
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
            .ok_or(())?;
        use crate::virtio_wifi::IfType;
        let iftype = if let Ok(iftype) = iftype.get_payload_as::<Nl80211IfType>() {
            match iftype {
                Nl80211IfType::Unspecified => Ok(IfType::Unspecified),
                Nl80211IfType::Station => Ok(IfType::Station),
                Nl80211IfType::Ap => Ok(IfType::Ap),
                Nl80211IfType::Monitor => Ok(IfType::Monitor),
                Nl80211IfType::P2pDevice => Ok(IfType::P2pDevice),
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
            .ok_or(())?
            .get_payload_as_with_len::<Vec<u8>>()
            .unwrap()
            .try_into()
            .unwrap();
        Ok(Self {
            name,
            index,
            iftype,
            addr,
            response: Default::default(),
        })
    }
}

impl From<&Interface> for virtio_wifi::VirtioWifiInterfaceData {
    fn from(iface: &Interface) -> Self {
        let name = iface
            .name
            .as_bytes()
            .iter()
            .take(64)
            .cloned()
            .collect::<Vec<u8>>();
        let iftype = match iface.iftype {
            Ok(v) => v as u32,
            Err(v) => v,
        };
        let ifname_len = name.len();
        let mut resp = Self {
            hdr: virtio_wifi::VirtioWifiHdr {
                code: virtio_wifi::VIRTIO_WIFI_S_OK.into(),
                index: iface.index.into(),
            },
            iftype: iftype.into(),
            ..Default::default()
        };
        resp.ifname[..ifname_len].copy_from_slice(&name);
        resp.ifname_len = (ifname_len as u32).into();
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

pub type Receiver =
    NlRouterReceiverHandle<GenlId, Genlmsghdr<Nl80211Command, Nl80211Attribute, NoUserHeader>>;

pub struct Nl80211Socket {
    inner: Arc<NlRouter>,
    family_id: u16,
}

pub struct EventNotifier {
    pub receiver: std::sync::mpsc::Receiver<Vec<u8>>,
    pub event_fd: Arc<EventFd>,
}

impl Nl80211Socket {
    pub fn new() -> Result<(Self, EventNotifier), Box<dyn Error>> {
        const FAMILY_ID: &str = "nl80211";
        // Initialize NlRouter and resolve 'nl80211' genl family
        let (sock, mut multicast_receiver) = NlRouter::connect(
            NlFamily::Generic, /* family */
            Some(0),           /* pid */
            Groups::empty(),   /* groups */
        )?;
        sock.enable_ext_ack(true)?;
        let family_id = sock.resolve_genl_family(FAMILY_ID)?;
        let config_group = sock.resolve_nl_mcast_group(FAMILY_ID, "config")?;
        let scan_group = sock.resolve_nl_mcast_group(FAMILY_ID, "scan")?;
        let groups = Groups::new_groups(&[config_group, scan_group]);
        sock.add_mcast_membership(groups)?;
        let inner = sock.into();
        let (event_sender, receiver) = std::sync::mpsc::channel();
        let event_fd = Arc::new(EventFd::new().unwrap());
        std::thread::spawn({
            let sock = Arc::clone(&inner);
            let event_fd = Arc::clone(&event_fd);
            let family_id = family_id;
            move || {
                while let Some(msg) = multicast_receiver.next_typed::<GenlId, Genlmsghdr<
                    Nl80211Command,
                    Nl80211Attribute,
                    NoUserHeader,
                >>() {
                    // log::trace!("mcast {msg:?}");
                    let Ok(msg) = msg else {
                        break;
                    };
                    let NlPayload::Payload(payload) = msg.nl_payload() else {
                        continue;
                    };
                    if matches!(payload.cmd(), Nl80211Command::NewScanResults) && *msg.nl_pid() == 0
                    {
                        let index = payload
                            .attrs()
                            .get_attr_handle()
                            .iter()
                            .find(|a| matches!(a.nla_type().nla_type(), Nl80211Attribute::IfIndex))
                            .unwrap()
                            .get_payload_as::<u32>()
                            .unwrap();

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

                        let recv = sock
                            .send(
                                family_id,
                                NlmF::REQUEST
                                    | NlmF::ACK
                                    | NlmF::REPLACE
                                    | NlmF::ROOT
                                    | NlmF::MATCH,
                                NlPayload::Payload(
                                    GenlmsghdrBuilder::<
                                        Nl80211Command,
                                        Nl80211Attribute,
                                        NoUserHeader,
                                    >::default()
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
                                    code: (virtio_wifi::VirtioWifiEventCode::ScanResult as u32)
                                        .into(),
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
                            while let Err(std::sync::mpsc::SendError(again)) =
                                event_sender.send(result)
                            {
                                result = again;
                                log::error!("Send error!");
                            }
                            event_fd.write(1).unwrap();
                        }
                        event_sender
                            .send(
                                virtio_wifi::VirtioWifiEvent {
                                    code: (virtio_wifi::VirtioWifiEventCode::EndScan as u32).into(),
                                    if_index: _if_index.unwrap().into(),
                                }
                                .as_slice()
                                .to_vec(),
                            )
                            .unwrap();
                        event_fd.write(1).unwrap();
                    } else {
                        // let attr_handle = payload.attrs().get_attr_handle();
                        // let mut attr_handles = attr_handle.iter().collect::<Vec<_>>();
                        //dbg!(&attr_handles);
                    }
                }
            }
        });
        Ok((
            Self { inner, family_id },
            EventNotifier { receiver, event_fd },
        ))
    }

    pub fn list_wiphys(&self) -> Result<BTreeMap<u32, Wiphy>, Box<dyn Error>> {
        // let index: libc::c_int = 1;
        let attrs = vec![NlattrBuilder::default()
            .nla_type(
                AttrTypeBuilder::default()
                    .nla_type(Nl80211Attribute::Wiphy)
                    .build()
                    .unwrap(),
            )
            .nla_payload(())
            // .nla_payload(index)
            .build()
            .unwrap()]
        .into_iter()
        .collect::<GenlBuffer<_, _>>();
        // Query system for WiFi radios using the 'GetWiphy' command
        let recv = self.inner.send(
            self.family_id,
            NlmF::DUMP | NlmF::ACK,
            NlPayload::Payload(
                GenlmsghdrBuilder::<Nl80211Command, Nl80211Attribute, NoUserHeader>::default()
                    .cmd(Nl80211Command::GetWiPhy)
                    .version(1)
                    .attrs(attrs)
                    .build()?,
            ),
        )?;
        let recv = recv.into_iter().collect::<Result<Vec<NlMessage>, _>>()?;
        Ok(recv
            .into_iter()
            .filter(|msg| !matches!(msg.nl_payload(), NlPayload::DumpExtAck(_)))
            .map(Wiphy::try_from)
            .map(|res| res.map(|w| (w.index, w)))
            .collect::<Result<BTreeMap<u32, Wiphy>, _>>()
            .map_err(|()| "GetWiphy error")?)
    }

    pub fn list_ifaces(&self, wiphy: &Wiphy) -> Result<BTreeMap<u32, Interface>, Box<dyn Error>> {
        let index = wiphy.index;
        // Query system for WiFi interfaces using the 'GetInterface' command
        // Empty payload for 'Ifname' attribute will dump all WiFi interfaces
        let attrs = vec![
            NlattrBuilder::default()
                .nla_type(
                    AttrTypeBuilder::default()
                        .nla_type(Nl80211Attribute::Wiphy)
                        .build()
                        .unwrap(),
                )
                .nla_payload(index)
                .build()
                .unwrap(),
            NlattrBuilder::<_, ()>::default()
                .nla_type(
                    AttrTypeBuilder::default()
                        .nla_type(Nl80211Attribute::IfName)
                        .build()
                        .unwrap(),
                )
                .nla_payload(())
                .build()
                .unwrap(),
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
        Ok(recv
            .into_iter()
            .filter(|msg| !matches!(msg.nl_payload(), NlPayload::DumpExtAck(_)))
            .map(Interface::try_from)
            .map(|res| res.map(|i| (i.index, i)))
            .collect::<Result<BTreeMap<u32, Interface>, _>>()
            .map_err(|()| "GetInterface Error")?)
    }

    pub fn add_iface(
        &self,
        wiphy: &Wiphy,
        mut iface: virtio_wifi::VirtioWifiInterface,
    ) -> Result<Interface, Box<dyn Error>> {
        // NL80211_CMD_NEW_INTERFACE
        //  * @NL80211_CMD_NEW_INTERFACE: Newly created virtual interface or response
        //    Can also be sent from userspace to request creation of a new virtual
        //    interface, then requires attributes %NL80211_ATTR_WIPHY,
        //    %NL80211_ATTR_IFTYPE and %NL80211_ATTR_IFNAME.
        let index = wiphy.index;
        log::info!("Requesting new vif on wiphy {index}");
        // Query system for WiFi interfaces using the 'GetInterface' command
        // Empty payload for 'Ifname' attribute will dump all WiFi interfaces
        let mut attrs = vec![
            NlattrBuilder::default()
                .nla_type(
                    AttrTypeBuilder::default()
                        .nla_type(Nl80211Attribute::Wiphy)
                        .build()
                        .unwrap(),
                )
                .nla_payload(index)
                .build()
                .unwrap(),
            NlattrBuilder::default()
                .nla_type(
                    AttrTypeBuilder::default()
                        .nla_type(Nl80211Attribute::IfType)
                        .build()
                        .unwrap(),
                )
                .nla_payload(Nl80211IfType::Station)
                .build()
                .unwrap(),
            NlattrBuilder::default()
                .nla_type(
                    AttrTypeBuilder::default()
                        .nla_type(Nl80211Attribute::IfName)
                        .build()
                        .unwrap(),
                )
                .nla_payload("wlan0")
                .build()
                .unwrap(),
        ];
        if wiphy.feature_flags | Nl80211FeatureFlags::MacOnCreate as u32 > 0 {
            log::debug!("requesting new iface mac to be {:?}", iface.addr);
            attrs.push(
                NlattrBuilder::default()
                    .nla_type(
                        AttrTypeBuilder::default()
                            .nla_type(Nl80211Attribute::Mac)
                            .build()
                            .unwrap(),
                    )
                    .nla_payload(iface.addr)
                    .build()
                    .unwrap(),
            );
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
        log::debug!("new iface mac is {:?}", new.addr);
        iface.hdr.index = wiphy.index.into();
        iface.index = new.index.into();
        new.response = iface;
        // iface.iftype = match new.iftype {
        //     Ok(f) => (f as u32).into(),
        //     Err(other) => other.into(),
        // };
        log::info!("Created new vif on wiphy {index}: {new:?}");
        Ok(new)
    }

    pub fn del_iface(&self, wiphy: &Wiphy, iface_index: u32) -> Result<(), Box<dyn Error>> {
        let index = wiphy.index;
        log::info!("Requesting deletion of vif {iface_index} on wiphy {index}");
        let attrs = vec![
            NlattrBuilder::default()
                .nla_type(
                    AttrTypeBuilder::default()
                        .nla_type(Nl80211Attribute::Wiphy)
                        .build()
                        .unwrap(),
                )
                .nla_payload(index)
                .build()
                .unwrap(),
            NlattrBuilder::default()
                .nla_type(
                    AttrTypeBuilder::default()
                        .nla_type(Nl80211Attribute::IfIndex)
                        .build()
                        .unwrap(),
                )
                .nla_payload(iface_index)
                .build()
                .unwrap(),
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
        chandef: &virtio_wifi::VirtioWifiChannel,
    ) -> Result<(), Box<dyn Error>> {
        let index = wiphy.index;
        let center_freq: u32 = chandef.center_freq.into();
        let center_freq1: u32 = chandef.center_freq1.into();
        let center_freq2: u32 = chandef.center_freq2.into();
        let width: u32 = chandef.width.into();
        let width: Nl80211ChanWidth = width.try_into().unwrap();
        log::info!("Requesting setting channel of wiphy {index} iface {iface_index} to {width:?}");
        let mut attrs = vec![
            NlattrBuilder::default()
                .nla_type(
                    AttrTypeBuilder::default()
                        .nla_type(Nl80211Attribute::IfIndex)
                        .build()
                        .unwrap(),
                )
                .nla_payload(iface_index)
                .build()
                .unwrap(),
            NlattrBuilder::default()
                .nla_type(
                    AttrTypeBuilder::default()
                        .nla_type(Nl80211Attribute::WiphyFreq)
                        .build()
                        .unwrap(),
                )
                .nla_payload(center_freq)
                .build()
                .unwrap(),
            NlattrBuilder::default()
                .nla_type(
                    AttrTypeBuilder::default()
                        .nla_type(Nl80211Attribute::ChannelWidth)
                        .build()
                        .unwrap(),
                )
                .nla_payload(width as u32)
                .build()
                .unwrap(),
        ];

        if width.needs_center_freq1() {
            attrs.push(
                NlattrBuilder::default()
                    .nla_type(
                        AttrTypeBuilder::default()
                            .nla_type(Nl80211Attribute::CenterFreq1)
                            .build()
                            .unwrap(),
                    )
                    .nla_payload(center_freq1)
                    .build()
                    .unwrap(),
            );
        }
        if width.needs_center_freq2() {
            attrs.push(
                NlattrBuilder::default()
                    .nla_type(
                        AttrTypeBuilder::default()
                            .nla_type(Nl80211Attribute::CenterFreq2)
                            .build()
                            .unwrap(),
                    )
                    .nla_payload(center_freq2)
                    .build()
                    .unwrap(),
            );
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

    pub fn trigger_scan(&self, iface_index: u32) -> Result<(), Box<dyn Error>> {
        log::info!("trigger scan vif {iface_index}");
        let attrs = vec![
            NlattrBuilder::default()
                .nla_type(
                    AttrTypeBuilder::default()
                        .nla_type(Nl80211Attribute::IfIndex)
                        .build()
                        .unwrap(),
                )
                .nla_payload(iface_index)
                .build()
                .unwrap(),
            NlattrBuilder::default()
                .nla_type(
                    AttrTypeBuilder::default()
                        .nla_type(Nl80211Attribute::Ssid)
                        .build()
                        .unwrap(),
                )
                // Wildcard SSID search
                .nla_payload(&b""[..])
                .build()
                .unwrap(),
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
}
