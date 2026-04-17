#ifndef VIRTIO_WIFI_IF_H
#define VIRTIO_WIFI_IF_H

#pragma GCC diagnostic push
#pragma GCC diagnostic error "-Wpadded"

#include <linux/virtio_types.h>

/*******************************************************************************
 * FEATURE BITS
 */
/* enum { */
/* 	/1* device supports control elements *1/ */
/* 	VIRTIO_SND_F_CTLS = 0 */
/* }; */

/*******************************************************************************
 * CONFIGURATION SPACE
 */
struct virtio_wifi_config {
	/* # of available phys */
	__le32 phys;
};

enum {
	/* device virtqueue indexes */
	VIRTIO_WIFI_VQ_CONTROL = 0,
	VIRTIO_WIFI_VQ_EVENT,
	VIRTIO_WIFI_VQ_TX,
	VIRTIO_WIFI_VQ_RX,
	/* # of device virtqueues */
	VIRTIO_WIFI_VQ_MAX
};

/* Commands: */

enum {
	/* @NL80211_CMD_GET_WIPHY */
	VIRTIO_WIFI_CMD_GET_WIPHY = 1,
	/* @NL80211_CMD_GET_WIPHY dump */
	VIRTIO_WIFI_CMD_GET_WIPHYS,
	/* @NL80211_CMD_SET_WIPHY */
	VIRTIO_WIFI_CMD_SET_WIPHY,
	/* @NL80211_CMD_GET_INTERFACE */
	VIRTIO_WIFI_CMD_GET_INTERFACE,
	/* @NL80211_CMD_GET_INTERFACE dump */
	VIRTIO_WIFI_CMD_GET_INTERFACES,
	/* @NL80211_CMD_SET_INTERFACE */
	VIRTIO_WIFI_CMD_SET_INTERFACE,
	/* @NL80211_CMD_NEW_INTERFACE */
	VIRTIO_WIFI_CMD_NEW_INTERFACE,
	/* @NL80211_CMD_DEL_INTERFACE */
	VIRTIO_WIFI_CMD_DELETE_INTERFACE,
	VIRTIO_WIFI_CMD_GET_BAND,
	VIRTIO_WIFI_CMD_GET_BAND_CHANNEL,
	VIRTIO_WIFI_CMD_GET_BAND_BITRATE,
	VIRTIO_WIFI_CMD_START_WIPHY,
	VIRTIO_WIFI_CMD_STOP_WIPHY,
	VIRTIO_WIFI_CMD_ADD_INTERFACE,
	VIRTIO_WIFI_CMD_REMOVE_INTERFACE,
	VIRTIO_WIFI_CMD_CHANGE_CONFIGURATION,
	VIRTIO_WIFI_CMD_CONFIGURE_RX_FILTER,
	VIRTIO_WIFI_CMD_SET_CHANNEL,

	/* common status codes */
	VIRTIO_WIFI_S_OK = 0x8000,
	VIRTIO_WIFI_S_BAD_MSG,
	VIRTIO_WIFI_S_NOT_SUPP,
	VIRTIO_WIFI_S_IO_ERR
};

/* common header */
struct virtio_wifi_hdr {
	__le32 code;
	__le32 index;
};

struct virtio_wifi_cmd_request {
	/* VIRTIO_WIFI_CMD_XXX */
	struct virtio_wifi_hdr hdr;
};

static const int _is_padded = sizeof(struct virtio_wifi_cmd_request) != 0;

struct virtio_wifi_cmd_get_iface_request {
	/* VIRTIO_WIFI_CMD_GET_INTERFACE */
	struct virtio_wifi_hdr hdr;
	__le32 phy_index;
};

static const int _is_padded__ =
	sizeof(struct virtio_wifi_cmd_get_iface_request) != 0;

struct virtio_wifi_radio {
	struct virtio_wifi_hdr hdr;
	__le32 ifaces;
	/* const char *reg_alpha2; */
	/* const struct ieee80211_regdomain *regd; */
	__le32 flags;
	__le32 reserved;
	/* const char *hwname; */
	__le32 hwname_len;
	char hwname[64];
	/* const u8 *perm_addr; */
	__le32 iftypes;
	__le32 available_antennas_tx;
	__le32 available_antennas_rx;

	/* u32 *ciphers; */
	/* u8 n_ciphers; */
	/* const struct cfg80211_pmsr_capabilities *pmsr_capa; */
};

static const int _is_padded_ = sizeof(struct virtio_wifi_radio) != 0;

#define VIRTIO_WIFI_RADIO_F_REG_STRICT (1 << 0)
#define VIRTIO_WIFI_RADIO_F_P2P_DEVICE (1 << 1)
#define VIRTIO_WIFI_RADIO_F_USE_CHANCTX (1 << 2)
#define VIRTIO_WIFI_RADIO_F_MULTI_RADIO (1 << 3)
#define VIRTIO_WIFI_RADIO_F_DESTROY_ON_CLOSE (1 << 4)
#define VIRTIO_WIFI_RADIO_F_NO_VIF (1 << 5)
#define VIRTIO_WIFI_RADIO_F_MLO (1 << 6)
#define VIRTIO_WIFI_RADIO_F_NAN_DEVICE (1 << 7)

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

struct virtio_wifi_interface_data {
	struct virtio_wifi_hdr hdr;
	/* const char *reg_alpha2; */
	/* const struct ieee80211_regdomain *regd; */
	__le32 ifname_len;
	char ifname[64];
	/* const u8 *perm_addr; */
	__le32 iftype;
	__le32 n_bands;
	/* u32 *ciphers; */
	/* u8 n_ciphers; */
	/* const struct cfg80211_pmsr_capabilities *pmsr_capa; */
};

static const int _is_padded___ = sizeof(struct virtio_wifi_interface_data) != 0;

struct virtio_wifi_cmd_get_band_request {
	/* VIRTIO_WIFI_CMD_GET_BAND */
	struct virtio_wifi_hdr hdr;
	__le32 phy_index;
};

static const int _is_padded____ =
	sizeof(struct virtio_wifi_cmd_get_band_request) != 0;

/* IEEE80211_HT_MCS_MASK_LEN */
#define VIRTIO_WIFI_HT_MCS_MASK_LEN 10

/* struct ieee80211_mcs_info */
struct virtio_wifi_mcs_info {
	__le16 rx_highest;
	__u8 rx_mask[VIRTIO_WIFI_HT_MCS_MASK_LEN];
	__u8 tx_params;
	__u8 reserved;
};

/* struct ieee80211_vht_mcs_info */
struct virtio_wifi_vht_mcs_info {
	__le16 rx_mcs_map;
	__le16 rx_highest;
	__le16 tx_mcs_map;
	__le16 tx_highest;
};
/* struct ieee80211_vht_cap */
struct virtio_wifi_vht_cap {
	__le32 vht_cap_info;
	struct virtio_wifi_vht_mcs_info supp_mcs;
};

/* struct ieee80211_sta_vht_cap */
struct virtio_wifi_sta_vht_cap {
	__le32 vht_supported;
	__le32 cap; /* use IEEE80211_VHT_CAP_ */
	struct virtio_wifi_vht_mcs_info vht_mcs;
};

static const int _is_padded______ = sizeof(struct virtio_wifi_sta_vht_cap) != 0;

/* enum ieee80211_edmg_bw_config */
enum virtio_wifi_edmg_bw_config {
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

/* struct ieee80211_edmg */
struct virtio_wifi_edmg {
	__u8 channels;
	__u8 padding[3];
	enum virtio_wifi_edmg_bw_config bw_config;
};

/* struct ieee80211_sta_ht_cap */
struct virtio_wifi_sta_ht_cap {
	__le16 cap; /* use IEEE80211_HT_CAP_ */
	__u8 ht_supported;
	__u8 ampdu_factor;
	__u8 ampdu_density;
	__u8 padding;
	struct virtio_wifi_mcs_info mcs;
};

/* struct ieee80211_sta_s1g_cap */
struct virtio_wifi_sta_s1g_cap {
	// FIXME: is this safe to read if cpu endianness is big?
	__u8 s1g;
	__u8 cap[10]; /* use S1G_CAPAB_ */
	__u8 nss_mcs[5];
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
	struct virtio_wifi_sta_vht_cap vht_cap;
	struct virtio_wifi_sta_s1g_cap s1g_cap;
	struct virtio_wifi_edmg edmg_cap;
};

static const int _is_padded_____ = sizeof(struct virtio_wifi_band) != 0;

struct virtio_wifi_cmd_get_band_channel_request {
	/* VIRTIO_WIFI_CMD_GET_BAND_CHANNEL */
	struct virtio_wifi_hdr hdr;
	__le32 phy_index;
	__le32 band_index;
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
	/* TODO: is this valid for LE -> native BE? */
	__u8 beacon_found;
	__u8 psd;
	__le16 reserved;
};

static const int _is_padded_______ = sizeof(struct virtio_wifi_channel) != 0;

struct virtio_wifi_cmd_get_band_bitrate_request {
	/* VIRTIO_WIFI_CMD_GET_BAND_BITRATE */
	struct virtio_wifi_hdr hdr;
	__le32 phy_index;
	__le32 band_index;
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
};

static const int _is_padded_________ =
	sizeof(struct virtio_wifi_interface_cfg) != 0;

/* struct ieee80211_vif - per-interface data */
struct virtio_wifi_interface {
	struct virtio_wifi_hdr hdr;
	/* enum virtio_wifi_iftype */
	__le32 iftype;
	__le32 index;
	struct virtio_wifi_interface_cfg cfg;
#define ETH_ALEN 6 /* Octets in one ethernet addr	 */
	__u8 addr[ETH_ALEN];
	__le16 reserved;
	__le32 driver_flags;
};

static const int _is_padded__________ = sizeof(struct virtio_wifi_interface) !=
					0;

struct virtio_wifi_configure_filter {
	struct virtio_wifi_hdr hdr;
	__le32 changed_flags;
	__le32 total_flags;
	__le64 multicast;
};

static const int _is_padded___________ =
	sizeof(struct virtio_wifi_configure_filter) != 0;

#pragma GCC diagnostic pop

#endif /* VIRTIO_WIFI_IF_H */
