#ifndef VIRTIO_WIFI_IF_H
#define VIRTIO_WIFI_IF_H

#pragma GCC diagnostic push
#pragma GCC diagnostic error "-Wpadded"

#include <linux/virtio_types.h>

#define VIRTIO_WIFI_F_AUTHENTICATE 0
#define VIRTIO_WIFI_F_CONNECT 1
#define VIRTIO_WIFI_F_CHANGE_INTERFACE 2
#define VIRTIO_WIFI_F_START_AP 3
#define VIRTIO_WIFI_F_REGISTER_FRAME 4
#define VIRTIO_WIFI_F_AP_SME 5

/*
 * Configuration Space
 */
struct virtio_wifi_config {
	__le32 num_of_ifaces;
	/* TODO: add regulatory domain */
#define VIRTIO_WIFI_FLAG_AP_SME (1 << 0)
/* #define VIRTIO_WIFI_FLAG_REG_STRICT (1 << 0) */
/* #define VIRTIO_WIFI_FLAG_P2P_DEVICE (1 << 1) */
/* #define VIRTIO_WIFI_FLAG_USE_CHANCTX (1 << 2) */
/* #define VIRTIO_WIFI_FLAG_MULTI_RADIO (1 << 3) */
/* #define VIRTIO_WIFI_FLAG_DESTROY_ON_CLOSE (1 << 4) */
/* #define VIRTIO_WIFI_FLAG_NO_VIF (1 << 5) */
/* #define VIRTIO_WIFI_FLAG_MLO (1 << 6) */
	__le32 flags;
	__le32 iftypes;
	__le32 available_antennas_tx;
	__le32 available_antennas_rx;
	__le32 n_cipher_suites;
} __attribute__((packed));

static const int _is_padded_ = sizeof(struct virtio_wifi_config) != 0;

/* device virtqueue indexes */
enum {
	VIRTIO_WIFI_VQ_CONTROL = 0,
	VIRTIO_WIFI_VQ_EVENT,
	VIRTIO_WIFI_VQ_TX,
	VIRTIO_WIFI_VQ_RX,
	/* # of device virtqueues */
	VIRTIO_WIFI_VQ_MAX
};

/* Commands: */

enum {
	VIRTIO_WIFI_CMD_INVALID = 0,
	VIRTIO_WIFI_CMD_GET_INTERFACES,
	VIRTIO_WIFI_CMD_NEW_INTERFACE,
	VIRTIO_WIFI_CMD_CHANGE_INTERFACE,
	VIRTIO_WIFI_CMD_DELETE_INTERFACE,
	VIRTIO_WIFI_CMD_GET_BAND,
	VIRTIO_WIFI_CMD_GET_BAND_CHANNEL,
	VIRTIO_WIFI_CMD_GET_BAND_BITRATE,
	VIRTIO_WIFI_CMD_GET_CIPHERSUITES,
	// TODO: allow selecting frequencies/ssids in request
	VIRTIO_WIFI_CMD_TRIGGER_SCAN,
	VIRTIO_WIFI_CMD_CONNECT,
	VIRTIO_WIFI_CMD_AUTHENTICATE,
	VIRTIO_WIFI_CMD_ASSOCIATE,
	VIRTIO_WIFI_CMD_DEAUTHENTICATE,
	VIRTIO_WIFI_CMD_DISASSOCIATE,
	VIRTIO_WIFI_CMD_ADD_KEY,
	VIRTIO_WIFI_CMD_SET_KEY,
	VIRTIO_WIFI_CMD_GET_KEY,
	VIRTIO_WIFI_CMD_DEL_KEY,
	VIRTIO_WIFI_CMD_SET_STATION,
	VIRTIO_WIFI_CMD_IF_OPERSTATE,
	VIRTIO_WIFI_CMD_START_AP,
	VIRTIO_WIFI_CMD_STOP_AP,
	VIRTIO_WIFI_CMD_REGISTER_FRAME,

	/* common status codes */
	VIRTIO_WIFI_S_OK = 0x8000,
	VIRTIO_WIFI_S_BAD_MSG,
	VIRTIO_WIFI_S_NOT_SUPP,
	VIRTIO_WIFI_S_IO_ERR,
	VIRTIO_WIFI_S_NET_DOWN,
	VIRTIO_WIFI_S_NOT_CONN,
	VIRTIO_WIFI_S_NO_LINK
};

enum {
	VIRTIO_WIFI_802_3,
	VIRTIO_WIFI_802_11,
};

/* common header */
struct virtio_wifi_hdr {
	__le32 code;
	__le32 index;
};

struct virtio_wifi_tlv {
	__le32 type;
	__le32 length;
	__le32 value[];
};

/* wiphy data */

/* Used in struct virtio_wifi_interface */
enum virtio_wifi_iftype {
	VIRTIO_WIFI_IFTYPE_UNSPECIFIED = 0,
	VIRTIO_WIFI_IFTYPE_ADHOC,
	VIRTIO_WIFI_IFTYPE_STATION,
	VIRTIO_WIFI_IFTYPE_AP,
	VIRTIO_WIFI_IFTYPE_AP_VLAN,
	VIRTIO_WIFI_IFTYPE_WDS,
	VIRTIO_WIFI_IFTYPE_MONITOR,
	VIRTIO_WIFI_IFTYPE_MESH_POINT,
	VIRTIO_WIFI_IFTYPE_P2P_CLIENT,
	VIRTIO_WIFI_IFTYPE_P2P_GO,
	VIRTIO_WIFI_IFTYPE_P2P_DEVICE,
	VIRTIO_WIFI_IFTYPE_OCB,
	VIRTIO_WIFI_IFTYPE_NAN,
	VIRTIO_WIFI_IFTYPE_MAX,
};

/* IEEE80211_HT_MCS_MASK_LEN */
#define VIRTIO_WIFI_HT_MCS_MASK_LEN 10

/* enum ieee80211_edmg_bw_config */
enum virtio_wifi_edmg_bw_config {
	VIRTIO_WIFI_EDMG_BW_CONFIG_INVALID = 0,
	VIRTIO_WIFI_EDMG_BW_CONFIG_4 = 4,
	VIRTIO_WIFI_EDMG_BW_CONFIG_5 = 5,
	VIRTIO_WIFI_EDMG_BW_CONFIG_6 = 6,
	VIRTIO_WIFI_EDMG_BW_CONFIG_7 = 7,
	VIRTIO_WIFI_EDMG_BW_CONFIG_8 = 8,
	VIRTIO_WIFI_EDMG_BW_CONFIG_9 = 9,
	VIRTIO_WIFI_EDMG_BW_CONFIG_10 = 10,
	VIRTIO_WIFI_EDMG_BW_CONFIG_11 = 11,
	VIRTIO_WIFI_EDMG_BW_CONFIG_12 = 12,
	VIRTIO_WIFI_EDMG_BW_CONFIG_13 = 13,
	VIRTIO_WIFI_EDMG_BW_CONFIG_14 = 14,
	VIRTIO_WIFI_EDMG_BW_CONFIG_15 = 15,
};

/* struct ieee80211_sta_ht_cap */
struct virtio_wifi_sta_ht_cap {
	__le16 cap; /* use IEEE80211_HT_CAP_ */
	__u8 ht_supported;
	__u8 ampdu_factor;
	__u8 ampdu_density;
	__u8 padding;
	/* struct ieee80211_mcs_info */
	struct mcs_info {
		__le16 rx_highest;
		__u8 rx_mask[VIRTIO_WIFI_HT_MCS_MASK_LEN];
		__u8 tx_params;
		__u8 reserved;
	} mcs;
};

struct virtio_wifi_band {
	struct virtio_wifi_hdr hdr;
	/* enum virtio_wifi_freq_band */
	__le32 band;
	__le32 n_channels;
	__le32 n_bitrates;
	__le16 n_iftype_data;
	struct virtio_wifi_sta_ht_cap ht_cap;
	__le16 padding;
	/* struct ieee80211_sta_vht_cap */
	struct sta_vht_cap {
		__le32 vht_supported;
		__le32 cap; /* use IEEE80211_VHT_CAP_ */
		/* struct ieee80211_vht_mcs_info */
		struct virtio_wifi_vht_mcs_info {
			__le16 rx_mcs_map;
			__le16 rx_highest;
			__le16 tx_mcs_map;
			__le16 tx_highest;
		} vht_mcs;
	} vht_cap;
	/* struct ieee80211_sta_s1g_cap */
	struct sta_s1g_cap {
		// FIXME: is this safe to read if cpu endianness is big?
		__u8 s1g;
		__u8 cap[10]; /* use S1G_CAPAB_ */
		__u8 nss_mcs[5];
	} s1g_cap;
	/* struct ieee80211_edmg */
	struct virtio_wifi_edmg {
		__u8 channels;
		__u8 padding[3];
		enum virtio_wifi_edmg_bw_config bw_config;
	} edmg_cap;
};

static const int _is_padded_____ = sizeof(struct virtio_wifi_band) != 0;

struct virtio_wifi_cmd_get_band_channel_request {
	/* VIRTIO_WIFI_CMD_GET_BAND_CHANNEL */
	struct virtio_wifi_hdr hdr;
	__le32 channel_index;
};

/**
 * enum nl80211_band - Frequency band
 * @NL80211_BAND_2GHZ: 2.4 GHz ISM band
 * @NL80211_BAND_5GHZ: around 5 GHz band (4.9 - 5.7 GHz)
 * @NL80211_BAND_60GHZ: around 60 GHz band (58.32 - 69.12 GHz)
 * @NL80211_BAND_6GHZ: around 6 GHz band (5.9 - 7.2 GHz)
 * @NL80211_BAND_S1GHZ: around 900MHz, supported by S1G PHYs
 * @NL80211_BAND_LC: light communication band (placeholder)
 * @NUM_NL80211_BANDS: number of bands, avoid using this in userspace
 *	since newer kernel versions may support more bands
 */
enum virtio_wifi_freq_band {
	VIRTIO_WIFI_FREQ_BAND_2GHZ = 0,
	VIRTIO_WIFI_FREQ_BAND_5GHZ,
	VIRTIO_WIFI_FREQ_BAND_60GHZ,
	VIRTIO_WIFI_FREQ_BAND_6GHZ,
	VIRTIO_WIFI_FREQ_BAND_S1GHZ,
	VIRTIO_WIFI_FREQ_BAND_LC,
};

/* enum nl80211_chan_width */
enum virtio_wifi_chan_width {
	VIRTIO_WIFI_CHAN_WIDTH_20_NOHT = 0,
	VIRTIO_WIFI_CHAN_WIDTH_20,
	VIRTIO_WIFI_CHAN_WIDTH_40,
	VIRTIO_WIFI_CHAN_WIDTH_80,
	VIRTIO_WIFI_CHAN_WIDTH_80P80,
	VIRTIO_WIFI_CHAN_WIDTH_160,
	VIRTIO_WIFI_CHAN_WIDTH_5,
	VIRTIO_WIFI_CHAN_WIDTH_10,
	VIRTIO_WIFI_CHAN_WIDTH_1,
	VIRTIO_WIFI_CHAN_WIDTH_2,
	VIRTIO_WIFI_CHAN_WIDTH_4,
	VIRTIO_WIFI_CHAN_WIDTH_8,
	VIRTIO_WIFI_CHAN_WIDTH_16,
	VIRTIO_WIFI_CHAN_WIDTH_320,
};

/* struct ieee80211_channel { */
struct virtio_wifi_channel {
	struct virtio_wifi_hdr hdr;
	/* enum virtio_wifi_freq_band */
	__le32 band;
	__le32 center_freq;
	__le32 center_freq1;
	__le32 center_freq2;
	__le16 freq_offset;
	__le16 hw_value;
	/* enum virtio_wifi_chan_width */
	__le32 width;
	__le32 flags;
	__le32 max_antenna_gain;
	__le32 max_power;
	__le32 max_reg_power;
	/* enum nl80211_dfs_state dfs_state; */
	__le32 dfs_state_entered;
	__le32 dfs_cac_ms;
	__u8 beacon_found;
	__u8 psd;
	__le16 reserved;
};

static const int _is_padded_______ = sizeof(struct virtio_wifi_channel) != 0;

struct virtio_wifi_cmd_get_band_bitrate_request {
	/* VIRTIO_WIFI_CMD_GET_BAND_BITRATE */
	struct virtio_wifi_hdr hdr;
	__le32 bitrate_index;
};

/* struct ieee80211_rate */
struct virtio_wifi_band_bitrate {
	struct virtio_wifi_hdr hdr;
	__le32 flags;
	__le16 bitrate;
	__le16 hw_value;
	__le16 hw_value_short;
	__le16 reserved;
};

static const int _is_padded________ = sizeof(struct virtio_wifi_band_bitrate) !=
				      0;

enum virtio_wifi_interface_flags {
	VIRTIO_WIFI_INTERFACE_F_ASSOC = 0,
	VIRTIO_WIFI_INTERFACE_F_IBSS_JOINED,
	VIRTIO_WIFI_INTERFACE_F_IBSS_CREATOR,
	VIRTIO_WIFI_INTERFACE_F_PS,
	VIRTIO_WIFI_INTERFACE_F_S1G,
	VIRTIO_WIFI_INTERFACE_F_IDLE,
};

/* struct ieee80211_vif - per-interface data */
struct virtio_wifi_interface {
	struct virtio_wifi_hdr hdr;
	/* enum virtio_wifi_iftype */
	__le32 iftype;
	__le32 index;
	__u8 ifname[16];
	/* struct ieee80211_vif_cfg */
	struct virtio_wifi_interface_cfg {
		/* VIRTIO_WIFI_INTERFACE_F_X */
		__le32 flags;
		__le16 aid;
		__le16 eml_cap;
		__le16 eml_med_sync_delay;
		__le16 mld_capa_op;
#define IEEE80211_BSS_ARP_ADDR_LIST_LEN 4
		__be32 arp_addr_list[IEEE80211_BSS_ARP_ADDR_LIST_LEN];
		__le32 arp_addr_cnt;
#define IEEE80211_MAX_SSID_LEN 32
		__u8 ssid[IEEE80211_MAX_SSID_LEN];
		__le16 ssid_len;
#define ETH_ALEN 6 /* Octets in one ethernet addr	 */
		__u8 ap_addr[ETH_ALEN];
	} cfg;
#define ETH_ALEN 6 /* Octets in one ethernet addr	 */
	__u8 addr[ETH_ALEN];
	__le16 reserved;
	__le32 driver_flags;
};

static const int _is_padded__________ = sizeof(struct virtio_wifi_interface) !=
					0;
enum virtio_wifi_event_code {
	VIRTIO_WIFI_EVENT_CODE_INVALID = 0,
	VIRTIO_WIFI_EVENT_CODE_SCAN_RESULT,
	VIRTIO_WIFI_EVENT_CODE_END_SCAN,
	VIRTIO_WIFI_EVENT_CODE_CANCEL_SCAN,
	VIRTIO_WIFI_EVENT_CODE_AUTHENTICATE,
	VIRTIO_WIFI_EVENT_CODE_AUTHENTICATE_TIMED_OUT,
	VIRTIO_WIFI_EVENT_CODE_ASSOCIATE,
	VIRTIO_WIFI_EVENT_CODE_ASSOCIATE_TIMED_OUT,
	VIRTIO_WIFI_EVENT_CODE_CONNECT,
	VIRTIO_WIFI_EVENT_CODE_CONNECT_TIMED_OUT,
	VIRTIO_WIFI_EVENT_CODE_DISCONNECT,
	VIRTIO_WIFI_EVENT_CODE_DISASSOCIATE,
	VIRTIO_WIFI_EVENT_CODE_DEAUTHENTICATE,
	VIRTIO_WIFI_EVENT_CODE_NEW_STATION,
	VIRTIO_WIFI_EVENT_CODE_DELETE_STATION,
};

struct virtio_wifi_event {
	__le32 code;
	__le32 ifindex;
};

struct virtio_wifi_scan_result {
	struct virtio_wifi_event hdr;
	/* struct cfg80211_inform_bss *data, */
	/* frame type (if known) enum cfg80211_bss_frame_type */
	/* @tsf: the TSF sent by the peer in the beacon/probe response (or 0) */
	__le64 tsf;
	__le32 ftype;
	/* @capability: the capability field sent by the peer */
	__le16 capability;
	/* the beacon interval announced by the peer */
	__le16 beacon_interval;
	__le32 band;
	__le32 center_freq;
	__le32 freq_offset;
	__le16 hw_value;
#define ETH_ALEN 6 /* Octets in one ethernet addr	 */
	__u8 bssid[ETH_ALEN];
	__le32 signal;
	/* ielen: length of the additional IEs */
	__le64 ielen;
};

static const int _is_padded____________ =
	sizeof(struct virtio_wifi_scan_result) != 0;

struct virtio_wifi_authentication_request {
	struct virtio_wifi_event hdr;
#define ETH_ALEN 6 /* Octets in one ethernet addr	 */
	__u8 mac[ETH_ALEN];
	__le16 ssid_len;
#define IEEE80211_MAX_SSID_LEN 32
	__u8 ssid[IEEE80211_MAX_SSID_LEN];
	__le32 freq;
	__le32 freq_offset;
	__le32 wpa_versions;
	__le32 cipher_group;
	__le32 n_ciphers_pairwise;
#define NL80211_MAX_NR_CIPHER_SUITES 5
	__le32 ciphers_pairwise[NL80211_MAX_NR_CIPHER_SUITES];
	__le32 n_akm_suites;
#define CFG80211_MAX_NUM_AKM_SUITES 10
	__le32 akm_suites[CFG80211_MAX_NUM_AKM_SUITES];
	__u8 privacy;
	__u8 padding[3];
};

static const int _is_padded_____________ =
	sizeof(struct virtio_wifi_authentication_request) != 0;

enum virtio_wifi_authentication_request_field {
	VIRTIO_WIFI_AUTH_REQ_AUTH_TYPE = 0,
	VIRTIO_WIFI_AUTH_REQ_IE,
};

struct virtio_wifi_association_request {
	struct virtio_wifi_event hdr;
#define ETH_ALEN 6 /* Octets in one ethernet addr	 */
	__u8 mac[ETH_ALEN];
	__le16 ssid_len;
#define IEEE80211_MAX_SSID_LEN 32
	__u8 ssid[IEEE80211_MAX_SSID_LEN];
	__le32 freq;
	__le32 freq_offset;
	__le32 wpa_versions;
	__le32 cipher_group;
	__le32 n_ciphers_pairwise;
#define NL80211_MAX_NR_CIPHER_SUITES 5
	__le32 ciphers_pairwise[NL80211_MAX_NR_CIPHER_SUITES];
	__le32 n_akm_suites;
#define CFG80211_MAX_NUM_AKM_SUITES 10
	__le32 akm_suites[CFG80211_MAX_NUM_AKM_SUITES];
	__le32 padding0;
};

static const int _is_padded______________ =
	sizeof(struct virtio_wifi_association_request) != 0;

enum virtio_wifi_association_request_field {
	VIRTIO_WIFI_ASSOC_REQ_IE = 0,
};

struct virtio_wifi_association_response {
	struct virtio_wifi_event hdr;
	__le64 frame_len;
	__le64 req_ies_len;
	__le32 uapsd_queues;
	__le32 padding0;
};

static const int _is_padded_______________ =
	sizeof(struct virtio_wifi_association_response) != 0;

enum virtio_wifi_association_response_field {
	VIRTIO_WIFI_ASSOC_RESP_FRAME = 0,
	VIRTIO_WIFI_ASSOC_RESP_IES,
};

struct virtio_wifi_connection_response {
	struct virtio_wifi_event hdr;
#define ETH_ALEN 6 /* Octets in one ethernet addr	 */
	__u8 mac[ETH_ALEN];
	__le16 status_code;
	__le64 req_ie_len;
	__le64 resp_ie_len;
};

static const int _is_padded________________ =
	sizeof(struct virtio_wifi_connection_response) != 0;

enum virtio_wifi_connection_response_field {
	VIRTIO_WIFI_CONN_RESP_REQ_IE = 0,
	VIRTIO_WIFI_CONN_RESP_RESP_IE,
};

struct virtio_wifi_disconnection_response {
	struct virtio_wifi_event hdr;
	__le16 reason_code;
	__u8 disconnected_by_ap;
	__u8 padding[5];
	__le64 ie_len;
};

static const int _is_padded_________________ =
	sizeof(struct virtio_wifi_disconnection_response) != 0;

enum virtio_wifi_disconnection_response_field {
	VIRTIO_WIFI_DISCONN_RESP_IE = 0,
};

struct virtio_wifi_station_change {
	struct virtio_wifi_event hdr;
#define ETH_ALEN 6 /* Octets in one ethernet addr	 */
	__u8 mac[ETH_ALEN];
	__u8 padding[2];
	__le64 ie_len;
};

static const int _is_padded__________________f___ =
	sizeof(struct virtio_wifi_station_change) != 0;

struct virtio_wifi_key {
	struct virtio_wifi_hdr hdr;
#define ETH_ALEN 6 /* Octets in one ethernet addr	 */
	__u8 mac[ETH_ALEN];
	__le16 vlan_id;
	__le32 cipher;
	/* enum nl80211_key_mode */
	__le32 mode;
#define VIRTIO_WIFI_KEY_F_DEFAULT_UNI (1 << 0)
#define VIRTIO_WIFI_KEY_F_DEFAULT_MULTI (1 << 1)
#define VIRTIO_WIFI_KEY_F_DEFAULT_MGMT (1 << 2)
#define VIRTIO_WIFI_KEY_F_DEFAULT (1 << 3)
#define VIRTIO_WIFI_KEY_F_IDX (1 << 4)
#define VIRTIO_WIFI_KEY_F_TYPE (1 << 5)
	__le16 flags;
	__u8 pairwise;
	__u8 key_index;
};

static const int _is_padded__________________ =
	sizeof(struct virtio_wifi_key) != 0;

enum virtio_wifi_key_field {
	VIRTIO_WIFI_KEY_DATA = 0,
	VIRTIO_WIFI_KEY_SEQ,
};

struct virtio_wifi_change_station {
	struct virtio_wifi_hdr hdr;
#define ETH_ALEN 6 /* Octets in one ethernet addr	 */
	__u8 mac[ETH_ALEN];
	__le16 padding;
	__le32 mask;
	__le32 set;
};

static const int _is_padded___________________ =
	sizeof(struct virtio_wifi_change_station) != 0;

/* Used both for deauth and disassoc */
struct virtio_wifi_disconnect_request {
	struct virtio_wifi_hdr hdr;
#define ETH_ALEN 6 /* Octets in one ethernet addr	 */
	__u8 mac[ETH_ALEN];
	__le16 reason_code;
	__u8 local_state_change;
	__u8 padding[7];
};

static const int _is_padded____________________ =
	sizeof(struct virtio_wifi_disconnect_request) != 0;

enum virtio_wifi_disconnect_request_field {
	VIRTIO_WIFI_DISCONN_REQ_IE = 0,
};

enum virtio_wifi_if_operstate {
	VIRTIO_WIFI_IF_OPER_UNKNOWN = 0,
	VIRTIO_WIFI_IF_OPER_NOTPRESENT = 1,
	VIRTIO_WIFI_IF_OPER_DOWN = 2,
	VIRTIO_WIFI_IF_OPER_LOWERLAYERDOWN = 3,
	VIRTIO_WIFI_IF_OPER_TESTING = 4,
	VIRTIO_WIFI_IF_OPER_DORMANT = 5,
	VIRTIO_WIFI_IF_OPER_UP = 6,
};

struct virtio_wifi_set_if_operstate {
	struct virtio_wifi_hdr hdr;
	__le32 flags;
	__le32 change_mask;
};

static const int _is_padded_____________________ =
	sizeof(struct virtio_wifi_set_if_operstate) != 0;

// Values used in wpa_driver_nl80211_set_ap()
struct virtio_wifi_beacon {
	struct virtio_wifi_hdr hdr;
	struct virtio_wifi_channel channel;
	__le32 beacon_interval;
	__le32 beacon_rate;
	__le32 wpa_versions;
	/* enum virtio_wifi_freq_band */
	__le32 band;
	__le32 dtim_period;
	__le32 cipher_group;
	__le32 n_ciphers_pairwise;
#define NL80211_MAX_NR_CIPHER_SUITES 5
	__le32 ciphers_pairwise[NL80211_MAX_NR_CIPHER_SUITES];
	__le32 n_akm_suites;
#define CFG80211_MAX_NUM_AKM_SUITES 10
	__le32 akm_suites[CFG80211_MAX_NUM_AKM_SUITES];
	__u8 privacy;
	__u8 padding[3];
	// pbss?
	// ftm_responder?
};

static const int _is_padded_______________________ =
	sizeof(struct virtio_wifi_beacon) != 0;

enum virtio_wifi_beacon_field {
	VIRTIO_WIFI_BEACON_HEAD = 0,
	VIRTIO_WIFI_BEACON_TAIL,
	VIRTIO_WIFI_BEACON_IES,
	VIRTIO_WIFI_BEACON_IES_PROBE_RESP,
	VIRTIO_WIFI_BEACON_IES_ASSOC_RESP,
	VIRTIO_WIFI_BEACON_PROBE_RESP,
	VIRTIO_WIFI_BEACON_SSID,
	VIRTIO_WIFI_BEACON_AUTH_TYPE,
};

// VIRTIO_WIFI_CMD_REGISTER_FRAME
struct virtio_wifi_register_frame {
	struct virtio_wifi_hdr hdr;
	__le16 ftype;
	__le16 stype;
};

static const int _is_padded________________________ =
	sizeof(struct virtio_wifi_register_frame) != 0;

#pragma GCC diagnostic pop

#endif /* VIRTIO_WIFI_IF_H */
