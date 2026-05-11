/* SPDX-License-Identifier: GPL-2.0 */
/* Author: Manos Pitsidianakis <manos.pitsidianakis@linaro.org> */

#include <net/cfg80211.h>
#include <net/rtnetlink.h>
#include <linux/etherdevice.h>
#include <linux/math64.h>
#include <linux/module.h>
#include <linux/version.h>
#include <linux/skbuff.h>

#include "virtio_wifi.h"

static struct rtnl_link_ops virtio_wifi_cfg80211_link_ops;
static const struct net_device_ops virtio_wifi_netdev_ops;
static struct virtio_wifi *common_virtio_wifi;
static void virtio_wifi_cfg80211_connect_complete(struct work_struct *work);
static void virtio_wifi_cfg80211_net_device_destructor(struct net_device *dev);
static rx_handler_result_t virtio_wifi_cfg80211_rx_handler(struct sk_buff **pskb);

struct virtio_wifi_vif {
	struct list_head list;
	u32 index;
	struct wireless_dev wdev;
	struct net_device *ndev;
	struct virtio_wifi *virtio_wifi;
	/* Lock to protect vif specific net_stats and flags */
	spinlock_t if_lock;
	unsigned long flags;
	int ssid_len;
	u8 ssid[IEEE80211_MAX_SSID_LEN];
	u8 dot11_auth_mode;
	u8 auth_mode;
	u8 prwise_crypto;
	u8 prwise_crypto_len;
	u8 grp_crypto;
	u8 grp_crypto_len;
	u8 def_txkey_index;
	u8 bssid[ETH_ALEN];
	u8 req_bssid[ETH_ALEN];
	u16 ch_hint;
	u16 bss_ch;
	struct aggr_info *aggr_cntxt;

	struct timer_list disconnect_timer;
	struct timer_list sched_scan_timer;

	struct cfg80211_scan_request *scan_req;
	int reconnect_flag;
	u32 last_roc_id;
	u32 last_cancel_roc_id;
	u32 send_action_id;
	bool probe_req_report;
	u16 assoc_bss_beacon_int;
	u32 txe_intvl;
	u8 assoc_bss_dtim_period;
	u16 rsn_capab;

	struct list_head mc_filter;

	bool is_up;
	bool is_connected;
	bool being_deleted;
	u32 tx_packets;
	u32 tx_failed;
	u32 connect_requested_ssid_len;
	u8 connect_requested_ssid[IEEE80211_MAX_SSID_LEN];
	u8 connect_requested_bss[ETH_ALEN];
	struct delayed_work connect;
};

static struct ieee80211_channel channel_2ghz = {
	.band = NL80211_BAND_2GHZ,
	.center_freq = 2432,
	.hw_value = 2432,
	.max_power = 20,
};

static struct ieee80211_rate bitrates_2ghz[] = {
	{ .bitrate = 10 },  { .bitrate = 20 }, { .bitrate = 55 },
	{ .bitrate = 110 }, { .bitrate = 60 }, { .bitrate = 120 },
	{ .bitrate = 240 },
};

static struct ieee80211_supported_band band_2ghz = {
	.channels = &channel_2ghz,
	.bitrates = bitrates_2ghz,
	.band = NL80211_BAND_2GHZ,
	.n_channels = 1,
	.n_bitrates = ARRAY_SIZE(bitrates_2ghz),
	.ht_cap = {
		.ht_supported = true,
		.cap = IEEE80211_HT_CAP_SUP_WIDTH_20_40 |
		       IEEE80211_HT_CAP_GRN_FLD |
		       IEEE80211_HT_CAP_SGI_20 |
		       IEEE80211_HT_CAP_SGI_40 |
		       IEEE80211_HT_CAP_DSSSCCK40,
		.ampdu_factor = 0x3,
		.ampdu_density = 0x6,
		.mcs = {
			.rx_mask = {0xff, 0xff},
			.tx_params = IEEE80211_HT_MCS_TX_DEFINED,
		},
	},
};

static struct ieee80211_channel channel_5ghz = {
	.band = NL80211_BAND_5GHZ,
	.center_freq = 5240,
	.hw_value = 5240,
	.max_power = 20,
};

static struct ieee80211_rate bitrates_5ghz[] = {
	{ .bitrate = 60 },
	{ .bitrate = 120 },
	{ .bitrate = 240 },
};

#define RX_MCS_MAP                             \
	(IEEE80211_VHT_MCS_SUPPORT_0_9 << 0 |  \
	 IEEE80211_VHT_MCS_SUPPORT_0_9 << 2 |  \
	 IEEE80211_VHT_MCS_SUPPORT_0_9 << 4 |  \
	 IEEE80211_VHT_MCS_SUPPORT_0_9 << 6 |  \
	 IEEE80211_VHT_MCS_SUPPORT_0_9 << 8 |  \
	 IEEE80211_VHT_MCS_SUPPORT_0_9 << 10 | \
	 IEEE80211_VHT_MCS_SUPPORT_0_9 << 12 | \
	 IEEE80211_VHT_MCS_SUPPORT_0_9 << 14)

#define TX_MCS_MAP                             \
	(IEEE80211_VHT_MCS_SUPPORT_0_9 << 0 |  \
	 IEEE80211_VHT_MCS_SUPPORT_0_9 << 2 |  \
	 IEEE80211_VHT_MCS_SUPPORT_0_9 << 4 |  \
	 IEEE80211_VHT_MCS_SUPPORT_0_9 << 6 |  \
	 IEEE80211_VHT_MCS_SUPPORT_0_9 << 8 |  \
	 IEEE80211_VHT_MCS_SUPPORT_0_9 << 10 | \
	 IEEE80211_VHT_MCS_SUPPORT_0_9 << 12 | \
	 IEEE80211_VHT_MCS_SUPPORT_0_9 << 14)

static struct ieee80211_supported_band band_5ghz = {
	.channels = &channel_5ghz,
	.bitrates = bitrates_5ghz,
	.band = NL80211_BAND_5GHZ,
	.n_channels = 1,
	.n_bitrates = ARRAY_SIZE(bitrates_5ghz),
	.ht_cap = {
		.ht_supported = true,
		.cap = IEEE80211_HT_CAP_SUP_WIDTH_20_40 |
		       IEEE80211_HT_CAP_GRN_FLD |
		       IEEE80211_HT_CAP_SGI_20 |
		       IEEE80211_HT_CAP_SGI_40 |
		       IEEE80211_HT_CAP_DSSSCCK40,
		.ampdu_factor = 0x3,
		.ampdu_density = 0x6,
		.mcs = {
			.rx_mask = {0xff, 0xff},
			.tx_params = IEEE80211_HT_MCS_TX_DEFINED,
		},
	},
	.vht_cap = {
		.vht_supported = true,
		.cap = IEEE80211_VHT_CAP_MAX_MPDU_LENGTH_11454 |
		       IEEE80211_VHT_CAP_SUPP_CHAN_WIDTH_160_80PLUS80MHZ |
		       IEEE80211_VHT_CAP_RXLDPC |
		       IEEE80211_VHT_CAP_SHORT_GI_80 |
		       IEEE80211_VHT_CAP_SHORT_GI_160 |
		       IEEE80211_VHT_CAP_TXSTBC |
		       IEEE80211_VHT_CAP_RXSTBC_1 |
		       IEEE80211_VHT_CAP_RXSTBC_2 |
		       IEEE80211_VHT_CAP_RXSTBC_3 |
		       IEEE80211_VHT_CAP_RXSTBC_4 |
		       IEEE80211_VHT_CAP_MAX_A_MPDU_LENGTH_EXPONENT_MASK,
		.vht_mcs = {
			.rx_mcs_map = cpu_to_le16(RX_MCS_MAP),
			.tx_mcs_map = cpu_to_le16(TX_MCS_MAP),
		}
	},
};


void virtio_wifi_cfg80211_inform_bss(struct wiphy *wiphy, struct virtio_wifi_scan_result *bss, void *ie, size_t ie_len)
{
	u64 tsf = virtio_le_to_cpu(bss->tsf);
	u32 signal = virtio_le_to_cpu(bss->signal);
	struct cfg80211_bss *informed_bss;

	/* printk("%s called\n", __func__); */
	informed_bss = cfg80211_inform_bss(
		wiphy, &channel_2ghz, CFG80211_BSS_FTYPE_PRESP,
		bss->bssid, tsf, WLAN_CAPABILITY_ESS, 0, ie,
		ie_len, signal, GFP_KERNEL);
	cfg80211_put_bss(wiphy, informed_bss);
}

void virtio_wifi_cfg80211_scan_done(struct wiphy *wiphy)
{
	struct virtio_wifi_wiphy *priv = wiphy_priv(wiphy);
	struct cfg80211_scan_info scan_info = { .aborted = false };
	/* printk("%s called\n", __func__); */

	if (!priv->scan_request || priv->being_deleted)
		return;
	cfg80211_scan_done(priv->scan_request, &scan_info);
	priv->scan_request = NULL;
}

/* Called with the rtnl lock held. */
static int virtio_wifi_cfg80211_scan(struct wiphy *wiphy,
				     struct cfg80211_scan_request *request)
{
	struct virtio_wifi_wiphy *priv = wiphy_priv(wiphy);
	struct wireless_dev *wdev = request->wdev;
	struct virtio_wifi_vif *vif = container_of(wdev, struct virtio_wifi_vif, wdev);

	/* printk("%s called\n", __func__); */
	wiphy_debug(wiphy, "scan\n");

	if (priv->scan_request || priv->being_deleted)
		return -EBUSY;

	priv->scan_request = request;

	return virtio_wifi_do_trigger_scan(priv, vif->index, request);
}

;
static struct wireless_dev *virtio_wifi_cfg80211_add_virtual_intf(
	struct wiphy *wiphy, const char *name, unsigned char name_assign_type,
	enum nl80211_iftype type, struct vif_params *params)
{
	struct virtio_wifi_wiphy *priv = wiphy_priv(wiphy);
	struct virtio_wifi *virtio_wifi = priv->virtio_wifi;;
	struct virtio_wifi_vif *vif;
	struct net_device *ndev;
	struct wireless_dev *wdev;
	int ret = 0;

	u32 if_index = 0;

	printk("%s called\n", __func__);

	/* if (ar->num_vif == ar->vif_max) { */
	/* 	ath6kl_err("Reached maximum number of supported vif\n"); */
	/* 	return ERR_PTR(-EINVAL); */
	/* } */

	if (type != NL80211_IFTYPE_STATION) {
		printk("%s: %d is not a supported interface type\n", __func__, type);
		return ERR_PTR(-EINVAL);
	}

	// https://elixir.bootlin.com/linux/v6.12.83/source/drivers/net/wireless/ath/ath6kl/cfg80211.c#L3779

	
	/* ether_addr_copy(addr, priv->virtio_wifi->mac_addr); */
	ndev = alloc_netdev(sizeof(*vif), name, name_assign_type, ether_setup);
	if (!ndev) {
		WARN_ON(!ndev);
		return NULL;
	}
	ret = virtio_wifi_do_add_interface(virtio_wifi, priv->index, type, ndev, &if_index);
	if (ret) {
		WARN_ON(ret);
		kfree(ndev);
		return ERR_PTR(ret);
	}

	vif = netdev_priv(ndev);
	ret = netdev_rx_handler_register(ndev,
					 virtio_wifi_cfg80211_rx_handler, vif);
	if (ret) {
		WARN_ON(ret);
		kfree(ndev);
		return ERR_PTR(ret);
	}

	vif->being_deleted = false;
	vif->is_connected = false;
	vif->is_up = false;
	vif->virtio_wifi = virtio_wifi;
	vif->ndev = ndev;

	INIT_DELAYED_WORK(&vif->connect,
			  virtio_wifi_cfg80211_connect_complete);
	vif->index = if_index;

	wdev = &vif->wdev;
	wdev->wiphy = wiphy;
	wdev->netdev = ndev;
	wdev->iftype = type;
	SET_NETDEV_DEV(ndev, wiphy_dev(wdev->wiphy));
	ndev->ieee80211_ptr = wdev;

	ndev->rtnl_link_ops = &virtio_wifi_cfg80211_link_ops;
	ndev->netdev_ops = &virtio_wifi_netdev_ops;
	ndev->needs_free_netdev = true;
	ndev->priv_destructor = virtio_wifi_cfg80211_net_device_destructor;
	ndev->needed_headroom = ETH_HLEN;
	
	if (cfg80211_register_netdevice(ndev)) {
		WARN_ON(1);
		kfree(ndev);
		netdev_rx_handler_unregister(ndev);
		return ERR_PTR(-ENOMEM);
	}

	return &vif->wdev;
}

static int virtio_wifi_cfg80211_del_virtual_intf(struct wiphy *wiphy,
						 struct wireless_dev *wdev)
{
	// TODO
	struct virtio_wifi_wiphy *priv = wiphy_priv(wiphy);
	struct virtio_wifi *virtio_wifi = priv->virtio_wifi;;
	struct virtio_wifi_vif *vif = container_of(wdev, struct virtio_wifi_vif, wdev);
	int ret = 0;

	printk("%s called\n", __func__);
	ret = virtio_wifi_do_remove_interface(virtio_wifi, priv->index, vif->index);
	if (ret) {
		return ret;
	}
	
	cfg80211_unregister_netdevice(vif->ndev);
	return 0;
}

/* May acquire and release the rdev BSS lock. */
void virtio_wifi_cfg80211_cancel_scan(struct wiphy *wiphy)
{
	struct virtio_wifi_wiphy *priv = wiphy_priv(wiphy);

	/* printk("%s called\n", __func__); */
	/* Clean up dangling callbacks if necessary. */
	if (priv->scan_request) {
		struct cfg80211_scan_info scan_info = { .aborted = true };
		/* Schedules work which acquires and releases the rtnl lock. */
		cfg80211_scan_done(priv->scan_request, &scan_info);
		priv->scan_request = NULL;
	}
}

/* Called with the rtnl lock held. */
static int virtio_wifi_cfg80211_connect(struct wiphy *wiphy,
					struct net_device *netdev,
					struct cfg80211_connect_params *sme)
{
	struct virtio_wifi_vif *vif = netdev_priv(netdev);
	bool could_schedule;

	printk("%s called\n", __func__);

	if (vif->being_deleted || !vif->is_up)
		return -EBUSY;

	if (!sme->ssid)
		return -EINVAL;

	vif->connect_requested_ssid_len = sme->ssid_len;
	memcpy(vif->connect_requested_ssid, sme->ssid, sme->ssid_len);

	could_schedule = schedule_delayed_work(&vif->connect, HZ * 2);
	if (!could_schedule)
		return -EBUSY;

	if (sme->bssid) {
		ether_addr_copy(vif->connect_requested_bss, sme->bssid);
	} else {
		/* virtio_wifi_cfg80211_inform_bss2(wiphy); */
		eth_zero_addr(vif->connect_requested_bss);
	}

	wiphy_debug(wiphy, "connect\n");

	return 0;
}

/* Acquires and releases the rdev event lock. */
static void virtio_wifi_cfg80211_connect_complete(struct work_struct *work)
{
	struct virtio_wifi_vif *vif = container_of(
		work, struct virtio_wifi_vif, connect.work);
	u8 *requested_bss = vif->connect_requested_bss;
	u16 status = WLAN_STATUS_SUCCESS;
	printk("%s called\n", __func__);

	if (is_zero_ether_addr(requested_bss))
		requested_bss = NULL;

	if (!vif->is_up)
		status = WLAN_STATUS_UNSPECIFIED_FAILURE;
	else
		vif->is_connected = true;

	/* Schedules an event that acquires the rtnl lock. */
	cfg80211_connect_result(vif->ndev,
				vif->is_connected ? requested_bss : NULL,
				NULL, 0, NULL, 0, status, GFP_KERNEL);
}

/* May acquire and release the rdev event lock. */
static void virtio_wifi_cfg80211_cancel_connect(struct net_device *netdev)
{
	struct virtio_wifi_vif *vif = netdev_priv(netdev);

	if (!vif->is_connected)
		return;

	/* If there is work pending, clean up dangling callbacks. */
	if (cancel_delayed_work_sync(&vif->connect)) {
		/* Schedules an event that acquires the rtnl lock. */
		cfg80211_connect_result(
			vif->ndev, vif->connect_requested_bss, NULL, 0,
			NULL, 0, WLAN_STATUS_UNSPECIFIED_FAILURE, GFP_KERNEL);
	}
}

/* Called with the rtnl lock held. Acquires the rdev event lock. */
static int virtio_wifi_cfg80211_disconnect(struct wiphy *wiphy,
					   struct net_device *netdev,
					   u16 reason_code)
{
	struct virtio_wifi_vif *vif = netdev_priv(netdev);

	printk("%s called\n", __func__);
	if (vif->being_deleted)
		return -EBUSY;

	wiphy_debug(wiphy, "disconnect\n");
	virtio_wifi_cfg80211_cancel_connect(netdev);

	cfg80211_disconnected(netdev, reason_code, NULL, 0, true, GFP_KERNEL);
	vif->is_connected = false;
	netif_carrier_off(netdev);

	return 0;
}

/* Called with the rtnl lock held. */
static int virtio_wifi_cfg80211_get_station(struct wiphy *wiphy,
					    struct net_device *dev,
					    const u8 *mac,
					    struct station_info *sinfo)
{
	struct virtio_wifi_vif *vif = netdev_priv(dev);

	wiphy_debug(wiphy, "get_station\n");

	if (!vif->is_connected)
		return -ENOENT;

	sinfo->filled = BIT_ULL(NL80211_STA_INFO_TX_PACKETS) |
			BIT_ULL(NL80211_STA_INFO_TX_FAILED) |
			BIT_ULL(NL80211_STA_INFO_SIGNAL) |
			BIT_ULL(NL80211_STA_INFO_TX_BITRATE);
	sinfo->tx_packets = vif->tx_packets;
	sinfo->tx_failed = vif->tx_failed;
	/* For CFG80211_SIGNAL_TYPE_MBM, value is expressed in _dBm_ */
	sinfo->signal = -50;
	sinfo->txrate = (struct rate_info){
		.legacy = 10, /* units are 100kbit/s */
	};
	return 0;
}

/* Called with the rtnl lock held. */
static int virtio_wifi_cfg80211_dump_station(struct wiphy *wiphy,
					     struct net_device *dev, int idx,
					     u8 *mac,
					     struct station_info *sinfo)
{
	struct virtio_wifi_vif *vif = netdev_priv(dev);

	wiphy_debug(wiphy, "dump_station\n");

	if (idx != 0 || !vif->is_connected)
		return -ENOENT;

	/* ether_addr_copy(mac, fake_router_bssid); */

	//TODO:
	return -ENOENT;
	/* return virtio_wifi_cfg80211_get_station(wiphy, dev, fake_router_bssid, */
	/* 					sinfo); */
}

// struct cfg80211_ops
static const struct cfg80211_ops virtio_wifi_cfg80211_cfg80211_ops = {
	.add_virtual_intf = virtio_wifi_cfg80211_add_virtual_intf,
	.del_virtual_intf = virtio_wifi_cfg80211_del_virtual_intf,
	.scan = virtio_wifi_cfg80211_scan,

	.connect = virtio_wifi_cfg80211_connect,
	.disconnect = virtio_wifi_cfg80211_disconnect,

	.get_station = virtio_wifi_cfg80211_get_station,
	.dump_station = virtio_wifi_cfg80211_dump_station,
	/* int (*add_key)(struct wiphy *wiphy, struct net_device *netdev, int link_id, u8 key_index, bool pairwise, */
	/*                const u8 *mac_addr, struct key_params *params); */
	/* int (*del_key)(struct wiphy *wiphy, struct net_device *netdev, int link_id, u8 key_index, bool pairwise, */
	/*                const u8 *mac_addr); */
	/* int (*set_default_key)(struct wiphy *wiphy, struct net_device *netdev, int link_id, u8 key_index, */
	/*                        bool unicast, bool multicast); */
};

static const u32 cipher_suites[] = {
	WLAN_CIPHER_SUITE_WEP40,
	WLAN_CIPHER_SUITE_WEP104,
	WLAN_CIPHER_SUITE_TKIP,
	WLAN_CIPHER_SUITE_CCMP,
	WLAN_CIPHER_SUITE_SMS4,
};

/* Acquires and releases the rtnl lock. */
struct virtio_wifi_wiphy *
virtio_wifi_cfg80211_make_wiphy(struct virtio_wifi *virtio_wifi,
				struct virtio_wifi_radio *radio)
{
	struct wiphy *wiphy;
	struct virtio_wifi_wiphy *priv;
	int err;

	wiphy = wiphy_new(&virtio_wifi_cfg80211_cfg80211_ops, sizeof(*priv));

	if (!wiphy)
		return NULL;

	set_wiphy_dev(wiphy, &virtio_wifi->virtio_dev->dev);

	priv = wiphy_priv(wiphy);
	priv->wiphy =wiphy;
	priv->virtio_wifi = virtio_wifi;

	wiphy->max_scan_ssids = 4;
	wiphy->max_scan_ie_len = 1000;
	wiphy->signal_type = CFG80211_SIGNAL_TYPE_MBM;

	wiphy->bands[NL80211_BAND_2GHZ] = &band_2ghz;
	wiphy->bands[NL80211_BAND_5GHZ] = &band_5ghz;
	wiphy->bands[NL80211_BAND_60GHZ] = NULL;
	for (int band_kind = NL80211_BAND_2GHZ; band_kind < NUM_NL80211_BANDS;
	     band_kind++) {
		struct virtio_wifi_band band = { 0 };

		struct ieee80211_supported_band *sband =
			&priv->bands[band_kind];
		sband->band = band_kind;

		switch (band_kind) {
		case NL80211_BAND_2GHZ:
			break;
		default:
			continue;
		}
		err = virtio_wifi_get_band(virtio_wifi, 0, band_kind, &band);
		/*FIXME*/ BUG_ON(err);
		sband->ht_cap.cap = virtio_le_to_cpu(band.ht_cap.cap);
		sband->ht_cap.ht_supported =
			virtio_le_to_cpu(band.ht_cap.ht_supported);
		sband->ht_cap.ampdu_factor =
			virtio_le_to_cpu(band.ht_cap.ampdu_factor);
		sband->ht_cap.ampdu_density =
			virtio_le_to_cpu(band.ht_cap.ampdu_density);
		sband->ht_cap.mcs.rx_highest =
			virtio_le_to_cpu(band.ht_cap.mcs.rx_highest);
		memcpy(sband->ht_cap.mcs.rx_mask, band.ht_cap.mcs.rx_mask,
		       ARRAY_SIZE(band.ht_cap.mcs.rx_mask));
		sband->ht_cap.mcs.tx_params =
			virtio_le_to_cpu(band.ht_cap.mcs.tx_params);
		sband->n_channels = virtio_le_to_cpu(band.n_channels);
		sband->n_bitrates = virtio_le_to_cpu(band.n_bitrates);
		if (sband->n_channels == 0) {
			continue;
		}
		sband->channels = devm_kzalloc(
			&virtio_wifi->virtio_dev->dev,
			sizeof(struct ieee80211_channel) * sband->n_channels,
			GFP_KERNEL);

		for (int ch = 0; ch < sband->n_channels; ch++) {
			struct virtio_wifi_channel channel = { 0 };
			err = virtio_wifi_get_band_channel(virtio_wifi, 0, band_kind,
							   ch, &channel);
			/*FIXME*/ BUG_ON(err);
			/* check that channel.band== band_kind ? */
			sband->channels[ch].band =
				virtio_le_to_cpu(channel.band);
			sband->channels[ch].center_freq =
				virtio_le_to_cpu(channel.center_freq);
			sband->channels[ch].freq_offset =
				virtio_le_to_cpu(channel.freq_offset);
			sband->channels[ch].hw_value =
				virtio_le_to_cpu(channel.hw_value);
			sband->channels[ch].flags =
				virtio_le_to_cpu(channel.flags);
			sband->channels[ch].max_antenna_gain =
				virtio_le_to_cpu(channel.max_antenna_gain);
			sband->channels[ch].max_power =
				virtio_le_to_cpu(channel.max_power);
			sband->channels[ch].max_reg_power =
				virtio_le_to_cpu(channel.max_reg_power);
			sband->channels[ch].dfs_state_entered =
				virtio_le_to_cpu(channel.dfs_state_entered);
			sband->channels[ch].dfs_cac_ms =
				virtio_le_to_cpu(channel.dfs_cac_ms);
			sband->channels[ch].beacon_found = channel.beacon_found;
			sband->channels[ch].psd = channel.psd;
		}

		sband->bitrates = devm_kzalloc(&virtio_wifi->virtio_dev->dev,
					       sizeof(struct ieee80211_rate) *
						       sband->n_bitrates,
					       GFP_KERNEL);

		for (int br = 0; br < sband->n_bitrates; br++) {
			struct virtio_wifi_band_bitrate bitrate = { 0 };
			err = virtio_wifi_get_band_bitrate(virtio_wifi, 0, band_kind,
							   br, &bitrate);
			/*FIXME*/ BUG_ON(err);
			sband->bitrates[br].bitrate =
				virtio_le_to_cpu(bitrate.bitrate);
			sband->bitrates[br].flags =
				virtio_le_to_cpu(bitrate.flags);
			sband->bitrates[br].hw_value =
				virtio_le_to_cpu(bitrate.hw_value);
			sband->bitrates[br].hw_value_short =
				virtio_le_to_cpu(bitrate.hw_value_short);
		}
		wiphy->bands[band_kind] = sband;
	}

	wiphy->interface_modes = BIT(NL80211_IFTYPE_STATION);
	wiphy->available_antennas_tx =
		virtio_le_to_cpu(radio->available_antennas_tx);
	wiphy->available_antennas_rx =
		virtio_le_to_cpu(radio->available_antennas_rx);

	wiphy->cipher_suites = cipher_suites;
	wiphy->n_cipher_suites = ARRAY_SIZE(cipher_suites);

	priv->flags = virtio_le_to_cpu(radio->flags);
	priv->hwname_len = virtio_le_to_cpu(radio->hwname_len);
	strscpy(priv->hwname, radio->hwname, sizeof(radio->hwname));

	priv->being_deleted = false;
	priv->scan_request = NULL;
	INIT_DELAYED_WORK(&priv->tx_work, virtio_wifi_do_tx_work);
	skb_queue_head_init(&priv->tx_list);

	err = wiphy_register(wiphy);
	if (err < 0) {
		wiphy_free(wiphy);
		return NULL;
	}

	return priv;
}

/* Acquires and releases the rtnl lock. */
void virtio_wifi_cfg80211_destroy_wiphy(struct wiphy *wiphy)
{
	struct virtio_wifi_wiphy *priv;

	printk("%s called %p\n", __func__, wiphy);
	WARN(!wiphy, "%s called with null wiphy", __func__);
	if (!wiphy)
		return;

	priv = wiphy_priv(wiphy);
	priv->being_deleted = true;
	virtio_wifi_cfg80211_cancel_scan(wiphy);

	if (wiphy->registered)
		wiphy_unregister(wiphy);
	wiphy_free(wiphy);
}

/* Enters and exits a RCU-bh critical section. */
static netdev_tx_t virtio_wifi_cfg80211_start_xmit(struct sk_buff *skb,
						   struct net_device *dev)
{
	struct virtio_wifi_vif *vif = netdev_priv(dev);
	struct virtio_wifi_wiphy *priv = wiphy_priv(vif->wdev.wiphy);
	struct ieee80211_tx_info *txi = IEEE80211_SKB_CB(skb);

	printk("%s called\n", __func__);
	vif->tx_packets++;
	/* if (!vif->is_connected) { */
	/* 	vif->tx_failed++; */
	/* 	return NET_XMIT_DROP; */
	/* } */
	priv->tx_if = vif->index;
	skb_queue_tail(&priv->tx_list, skb);
	skb->dev = vif->ndev;

	schedule_delayed_work(&priv->tx_work, 0);
	ieee80211_tx_info_clear_status(txi);

	/* frame was transmitted at most favorable rate at first attempt */
	txi->control.rates[0].count = 1;
	txi->control.rates[1].idx = -1;

	return NETDEV_TX_OK;
}

/* Called with rtnl lock held. */
static int virtio_wifi_cfg80211_net_device_open(struct net_device *dev)
{
	struct virtio_wifi_vif *vif = netdev_priv(dev);

	printk("%s called\n", __func__);
	vif->is_up = true;
	return 0;
}

/* Called with rtnl lock held. */
static int virtio_wifi_cfg80211_net_device_stop(struct net_device *dev)
{
	struct virtio_wifi_vif *vif = netdev_priv(dev);

	printk("%s called\n", __func__);
	vif->is_up = false;

	if (!dev->ieee80211_ptr)
		return 0;

	virtio_wifi_cfg80211_cancel_scan(dev->ieee80211_ptr->wiphy);
	virtio_wifi_cfg80211_cancel_connect(dev);
	netif_carrier_off(dev);

	return 0;
}

/* static int */
/* virtio_wifi_cfg80211_net_device_get_iflink(const struct net_device *dev) */
/* { */
/* 	struct virtio_wifi_cfg80211_netdev_priv *priv = netdev_priv(dev); */

/* 	return READ_ONCE(priv->lowerdev->ifindex); */
/* } */

static const struct net_device_ops virtio_wifi_netdev_ops = {
	.ndo_start_xmit = virtio_wifi_cfg80211_start_xmit,
	.ndo_open = virtio_wifi_cfg80211_net_device_open,
	.ndo_stop = virtio_wifi_cfg80211_net_device_stop,
	/* .ndo_get_iflink = virtio_wifi_cfg80211_net_device_get_iflink, */
};

/* Invoked as part of rtnl lock release. */
static void virtio_wifi_cfg80211_net_device_destructor(struct net_device *dev)
{
	/* Delayed past dellink to allow nl80211 to react to the device being
	 * deleted.
	 */
	kfree(dev->ieee80211_ptr);
	dev->ieee80211_ptr = NULL;
	printk("%s called\n", __func__);
}

/* No lock interaction. */
static void virtio_wifi_cfg80211_setup(struct net_device *dev)
{
	ether_setup(dev);
	dev->netdev_ops = &virtio_wifi_netdev_ops;
	dev->needs_free_netdev = true;
	printk("%s called\n", __func__);
}

/* Called in a RCU read critical section from netif_receive_skb */
static rx_handler_result_t
virtio_wifi_cfg80211_rx_handler(struct sk_buff **pskb)
{
	struct sk_buff *skb = *pskb;
	struct virtio_wifi_vif *vif =
		rcu_dereference(skb->dev->rx_handler_data);

	printk("%s called\n", __func__);
	if (!vif->is_connected)
		return RX_HANDLER_PASS;

	/* GFP_ATOMIC because this is a packet interrupt handler. */
	skb = skb_share_check(skb, GFP_ATOMIC);
	if (!skb) {
		dev_err(&vif->virtio_wifi->virtio_dev->dev, "can't skb_share_check\n");
		return RX_HANDLER_CONSUMED;
	}

	*pskb = skb;
	skb->dev = vif->ndev;
	skb->pkt_type = PACKET_HOST;
	return RX_HANDLER_ANOTHER;
}

static int register_ndev(struct net_device *dev) {
	int err;
	struct list_head *node;
	struct virtio_wifi_wiphy *virtio_wifi_wiphy;
	struct virtio_wifi_vif *vif = netdev_priv(dev);

	netif_carrier_off(dev);

	dev_err(&vif->virtio_wifi->virtio_dev->dev,
			"can't netdev_rx_handler_register: %d\n", -ENODEV);
	return -ENODEV;
	/* if (!tb[IFLA_MTU]) */
	/* 	dev->mtu = priv->lowerdev->mtu; */
	/* else if (dev->mtu > priv->lowerdev->mtu) */
	/* 	return -EINVAL; */

	err = netdev_rx_handler_register(dev,
					 virtio_wifi_cfg80211_rx_handler, vif);
	if (err) {
		dev_err(&vif->virtio_wifi->virtio_dev->dev,
			"can't netdev_rx_handler_register: %d\n", err);
		return err;
	}

	dev->ieee80211_ptr =

#if LINUX_VERSION_CODE >= KERNEL_VERSION(6, 13, 0)
		kzalloc_obj(*dev->ieee80211_ptr);
#else
		kzalloc(sizeof(*dev->ieee80211_ptr), GFP_KERNEL);
#endif

	if (!dev->ieee80211_ptr) {
		err = -ENOMEM;
		goto remove_handler;
	}

	dev->ieee80211_ptr->iftype = NL80211_IFTYPE_STATION;

	spin_lock_bh(&common_virtio_wifi->wiphys_lock);
	BUG_ON(list_empty(&common_virtio_wifi->wiphys));
	node = &common_virtio_wifi->wiphys;
	virtio_wifi_wiphy = list_entry(node, struct virtio_wifi_wiphy, node);
	dev->ieee80211_ptr->wiphy = virtio_wifi_wiphy->wiphy;
	spin_unlock_bh(&common_virtio_wifi->wiphys_lock);

	err = register_netdevice(dev);
	if (err) {
		printk("can't register_netdevice: %d\n",
			err);
		goto free_wireless_dev;
	}

	dev->rtnl_link_ops = &virtio_wifi_cfg80211_link_ops;

	/* err = netdev_upper_dev_link(dev, dev, extack); */
	/* if (err) { */
	/* 	printk( */
	/* 		"can't netdev_upper_dev_link: %d\n", err); */
	/* 	goto unregister_netdev; */
	/* } */

	dev->priv_destructor = virtio_wifi_cfg80211_net_device_destructor;
	vif->being_deleted = false;
	vif->is_connected = false;
	vif->is_up = false;
	INIT_DELAYED_WORK(&vif->connect,
			  virtio_wifi_cfg80211_connect_complete);
	__module_get(THIS_MODULE);

	return 0;
/* unregister_netdev: */
/* 	unregister_netdevice(dev); */
free_wireless_dev:
	kfree(dev->ieee80211_ptr);
	dev->ieee80211_ptr = NULL;
remove_handler:
	netdev_rx_handler_unregister(dev);
	return err;
}


static struct rtnl_link_ops virtio_wifi_cfg80211_link_ops = {
	.kind = "virtio_wifi",
	.setup = virtio_wifi_cfg80211_setup,
	.priv_size = sizeof(struct virtio_wifi_vif),
};

static bool netif_is_virtio_wifi_cfg80211_dev(const struct net_device *dev)
{
	return rcu_access_pointer(dev->rx_handler) ==
	       virtio_wifi_cfg80211_rx_handler;
}

static int virtio_wifi_cfg80211_event(struct notifier_block *this,
				      unsigned long event, void *ptr)
{
	struct net_device *lower_dev = netdev_notifier_info_to_dev(ptr);
	LIST_HEAD(list_kill);

	if (!netif_is_virtio_wifi_cfg80211_dev(lower_dev))
		return NOTIFY_DONE;

	switch (event) {
	case NETDEV_UNREGISTER:
		/* priv = rtnl_dereference(lower_dev->rx_handler_data); */
		/* if (!priv) */
		/* 	return NOTIFY_DONE; */

		/* upper_dev = priv->upperdev; */

		/* upper_dev->rtnl_link_ops->dellink(upper_dev, &list_kill); */
		/* unregister_netdevice_many(&list_kill); */
		break;
	}

	return NOTIFY_DONE;
}

static struct notifier_block virtio_wifi_cfg80211_notifier = {
	.notifier_call = virtio_wifi_cfg80211_event,
};

/* Acquires and releases the rtnl lock. */
int virtio_wifi_cfg80211_init_module(struct virtio_wifi *virtio_wifi)
{
	int err;

	err = register_netdevice_notifier(&virtio_wifi_cfg80211_notifier);
	if (err)
		return err;

	err = -ENOMEM;

	err = rtnl_link_register(&virtio_wifi_cfg80211_link_ops);
	if (err)
		goto notifier;
	common_virtio_wifi = virtio_wifi;
	return 0;

notifier:
	unregister_netdevice_notifier(&virtio_wifi_cfg80211_notifier);
	return err;
}

/* Acquires and releases the rtnl lock. */
void virtio_wifi_cfg80211_cleanup_module(void)
{
	common_virtio_wifi = NULL;
	/* Will delete any devices that depend on the wiphy. */
	rtnl_link_unregister(&virtio_wifi_cfg80211_link_ops);
	unregister_netdevice_notifier(&virtio_wifi_cfg80211_notifier);
}

MODULE_ALIAS_RTNL_LINK("virtio_wifi");
