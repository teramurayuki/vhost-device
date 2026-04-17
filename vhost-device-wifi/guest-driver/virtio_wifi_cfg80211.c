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
static void virtio_wifi_cfg80211_net_device_destructor(struct net_device *dev);

struct virtio_wifi_vif {
	struct list_head list;
	u32 index;
	struct wireless_dev wdev;
	struct net_device *ndev;
	struct virtio_wifi *virtio_wifi;

	bool is_up;
	bool is_connected;
	bool being_deleted;
	u32 tx_packets;
	u32 tx_failed;
	u8 connect_requested_bss[ETH_ALEN];
	struct cfg80211_bss *associated_bss;
};

void virtio_wifi_cfg80211_rx(struct virtio_wifi_wiphy *w, u32 ifindex,
			     struct sk_buff *skb)
{
	struct wireless_dev *tmp;
	struct virtio_wifi_vif *vif;
	wiphy_lock(w->wiphy);
	rcu_read_lock();
	list_for_each_entry_rcu(tmp, &w->wiphy->wdev_list, list) {
		vif = container_of(tmp, struct virtio_wifi_vif, wdev);
		if (vif->index != ifindex) {
			vif = NULL;
			continue;
		}
		break;
	}
	rcu_read_unlock();
	wiphy_unlock(w->wiphy);

	if (vif == NULL) {
		WARN_ON(vif == NULL);
		return;
	}
	skb->protocol = eth_type_trans(skb, vif->ndev);
	netif_receive_skb(skb);
}

void virtio_wifi_cfg80211_event_dispatch(struct virtio_wifi_wiphy *w, u32 code,
					 u32 ifindex, void *data, int len)
{
	struct virtio_wifi_scan_result *bss;
	struct virtio_wifi_association_response *assoc_resp;
	struct virtio_wifi_connection_response *conn_resp;
	struct virtio_wifi_disconnection_response *disconn_resp;
	struct wireless_dev *tmp;
	struct virtio_wifi_vif *vif = NULL;
	size_t ie_len;

	wiphy_lock(w->wiphy);
	rcu_read_lock();
	list_for_each_entry_rcu(tmp, &w->wiphy->wdev_list, list) {
		vif = container_of(tmp, struct virtio_wifi_vif, wdev);
		if (vif->index != ifindex) {
			vif = NULL;
			continue;
		}
		break;
	}
	rcu_read_unlock();
	wiphy_unlock(w->wiphy);

	if (vif == NULL) {
		return;
	}

	// TODO check `len` when casting `data` to each response type.
	switch (code) {
	case VIRTIO_WIFI_EVENT_CODE_SCAN_RESULT:
		/* printk("VIRTIO_WIFI_EVENT_CODE_SCAN_RESULT\n"); */
		bss = data;
		ie_len = virtio_le_to_cpu(bss->ielen);
		wiphy_lock(w->wiphy);
		virtio_wifi_cfg80211_inform_bss(
			w->wiphy, data,
			(data + sizeof(struct virtio_wifi_scan_result)),
			ie_len);
		wiphy_unlock(w->wiphy);
		break;
	case VIRTIO_WIFI_EVENT_CODE_END_SCAN:
		/* printk("VIRTIO_WIFI_EVENT_CODE_END_SCAN\n"); */
		wiphy_lock(w->wiphy);
		virtio_wifi_cfg80211_scan_done(w->wiphy);
		wiphy_unlock(w->wiphy);
		break;
	case VIRTIO_WIFI_EVENT_CODE_CANCEL_SCAN:
		printk("VIRTIO_WIFI_EVENT_CODE_CANCEL_SCAN\n");
		wiphy_lock(w->wiphy);
		virtio_wifi_cfg80211_cancel_scan(w->wiphy);
		wiphy_unlock(w->wiphy);
		break;
	case VIRTIO_WIFI_EVENT_CODE_AUTHENTICATE:
		printk("VIRTIO_WIFI_EVENT_CODE_AUTHENTICATE\n");
		wiphy_lock(w->wiphy);
		cfg80211_rx_mlme_mgmt(vif->ndev,
				      (data + sizeof(struct virtio_wifi_event)),
				      len - sizeof(struct virtio_wifi_event));
		wiphy_unlock(w->wiphy);
		break;
	case VIRTIO_WIFI_EVENT_CODE_AUTHENTICATE_TIMED_OUT:
		printk("VIRTIO_WIFI_EVENT_CODE_AUTHENTICATE_TIMED_OUT\n");
		// TODO: call cfg80211_auth_timeout with mac
		WARN_ON(VIRTIO_WIFI_EVENT_CODE_AUTHENTICATE_TIMED_OUT);
		break;
	case VIRTIO_WIFI_EVENT_CODE_ASSOCIATE:
		printk("VIRTIO_WIFI_EVENT_CODE_ASSOCIATE\n");
		assoc_resp = data;
		u32 uapsd_queues = virtio_le_to_cpu(assoc_resp->uapsd_queues);
		size_t frame_len = virtio_le_to_cpu(assoc_resp->frame_len);
		size_t req_ies_len = virtio_le_to_cpu(assoc_resp->req_ies_len);
		// TODO: boundary checking
		u8 *frame_data =
			data + sizeof(struct virtio_wifi_association_response);
		const u8 *req_ies =
			data + sizeof(struct virtio_wifi_association_response) +
			frame_len;
		struct cfg80211_rx_assoc_resp_data rx_assoc_resp_data = { 0 };
		rx_assoc_resp_data.buf = frame_data;
		rx_assoc_resp_data.len = frame_len;
		rx_assoc_resp_data.uapsd_queues = uapsd_queues;
		if (req_ies_len > 0) {
			rx_assoc_resp_data.req_ies = req_ies;
			rx_assoc_resp_data.req_ies_len = req_ies_len;
		}

		wiphy_lock(w->wiphy);
		cfg80211_ref_bss(w->wiphy, vif->associated_bss);
		rx_assoc_resp_data.links[0].bss = vif->associated_bss;
		cfg80211_rx_assoc_resp(vif->ndev, &rx_assoc_resp_data);
		wiphy_unlock(w->wiphy);
		break;
	case VIRTIO_WIFI_EVENT_CODE_ASSOCIATE_TIMED_OUT:
		printk("VIRTIO_WIFI_EVENT_CODE_ASSOCIATE_TIMED_OUT\n");
		// TODO: call cfg80211_auth_timeout
		WARN_ON(VIRTIO_WIFI_EVENT_CODE_ASSOCIATE_TIMED_OUT);
		break;
	case VIRTIO_WIFI_EVENT_CODE_CONNECT:
		printk("VIRTIO_WIFI_EVENT_CODE_CONNECT\n");
		conn_resp = data;
		u8 *requested_bss = conn_resp->mac;
		u16 status = virtio_le_to_cpu(conn_resp->status_code);
		if (!vif->is_up)
			status = WLAN_STATUS_UNSPECIFIED_FAILURE;
		else
			vif->is_connected = (status == WLAN_STATUS_SUCCESS);

		size_t req_ie_len = virtio_le_to_cpu(conn_resp->req_ie_len);
		size_t resp_ie_len = virtio_le_to_cpu(conn_resp->resp_ie_len);

		// TODO: boundary checking
		const u8 *req_ie = data + sizeof(*conn_resp);
		const u8 *resp_ie = req_ie + req_ie_len;

		wiphy_lock(w->wiphy);
		ether_addr_copy(vif->connect_requested_bss, requested_bss);
		cfg80211_ref_bss(w->wiphy, vif->associated_bss);
		cfg80211_connect_bss(vif->ndev, requested_bss,
				     vif->associated_bss, req_ie, req_ie_len,
				     resp_ie, resp_ie_len, status, GFP_KERNEL,
				     NL80211_TIMEOUT_UNSPECIFIED);
		if (vif->is_connected) {
			netif_carrier_on(vif->ndev);
			netif_wake_queue(vif->ndev);
		}
		wiphy_unlock(w->wiphy);
		break;
	case VIRTIO_WIFI_EVENT_CODE_DISCONNECT:
		printk("VIRTIO_WIFI_EVENT_CODE_DISCONNECT\n");
		disconn_resp = data;
		u16 reason_code = virtio_le_to_cpu(disconn_resp->reason_code);
		size_t ie_len = virtio_le_to_cpu(disconn_resp->ie_len);
		const u8 *ie = ie_len > 0 ? (data + sizeof(*disconn_resp)) :
					    NULL;
		wiphy_lock(w->wiphy);
		virtio_wifi_cfg80211_cancel_scan(w->wiphy);
		cfg80211_disconnected(vif->ndev, reason_code, ie, ie_len, false,
				      GFP_KERNEL);
		netif_stop_queue(vif->ndev);
		netif_carrier_off(vif->ndev);
		eth_zero_addr(vif->connect_requested_bss);
		wiphy_unlock(w->wiphy);
		break;
	case VIRTIO_WIFI_EVENT_CODE_DISASSOCIATE:
		printk("VIRTIO_WIFI_EVENT_CODE_DISASSOCIATE\n");
		break;
	case VIRTIO_WIFI_EVENT_CODE_DEAUTHENTICATE:
		printk("VIRTIO_WIFI_EVENT_CODE_DEAUTHENTICATE\n");
		wiphy_lock(w->wiphy);
		cfg80211_rx_mlme_mgmt(vif->ndev,
				      (data + sizeof(struct virtio_wifi_event)),
				      len - sizeof(struct virtio_wifi_event));
		wiphy_unlock(w->wiphy);
		break;
	default:
		printk("%s UNHANDLED CODE = %u\n\n\n", __func__, code);
		// TODO print err
		return;
	}
}

void virtio_wifi_cfg80211_inform_bss(struct wiphy *wiphy,
				     struct virtio_wifi_scan_result *bss,
				     void *ie, size_t ie_len)
{
	struct virtio_wifi_wiphy *priv = wiphy_priv(wiphy);
	struct cfg80211_bss *informed_bss;

	u64 tsf = virtio_le_to_cpu(bss->tsf);
	u32 signal = virtio_le_to_cpu(bss->signal);
	u16 capability = virtio_le_to_cpu(bss->capability);
	u16 beacon_interval = virtio_le_to_cpu(bss->beacon_interval);

	u32 center_freq = virtio_le_to_cpu(bss->center_freq);

	if (!priv->scan_request || priv->being_deleted) {
		return;
	}
	/* printk("%s called center_freq = %u\n", __func__, center_freq); */

	struct ieee80211_channel *channel = NULL;

	for (u32 i = 0; i < priv->scan_request->n_channels; i++) {
		if (priv->scan_request->channels[i]->center_freq ==
		    center_freq) {
			channel = priv->scan_request->channels[i];
			break;
		}
	}
	if (channel == NULL) {
		// Ignore result, the scan request was not looking for this
		// frequency
		return;
	}

	/* printk("%s called\n", __func__); */
	informed_bss = cfg80211_inform_bss(
		wiphy, channel, CFG80211_BSS_FTYPE_UNKNOWN, bss->bssid, tsf,
		capability, beacon_interval, ie, ie_len, signal, GFP_KERNEL);
	if (!informed_bss) {
		return;
	}
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
	struct virtio_wifi_vif *vif =
		container_of(wdev, struct virtio_wifi_vif, wdev);
	int ret;

	/* printk("%s called\n", __func__); */
	wiphy_debug(wiphy, "scan\n");

	if (priv->scan_request || priv->being_deleted)
		return -EBUSY;

	priv->scan_request = request;

	ret = virtio_wifi_do_trigger_scan(priv, vif->index, request);
	if (ret) {
		priv->scan_request = NULL;
	}

	return ret;
}

struct wireless_dev *
virtio_wifi_cfg80211_add_intf(struct wiphy *wiphy,
			      unsigned char name_assign_type,
			      struct virtio_wifi_interface *wif)
{
	struct virtio_wifi_wiphy *priv = wiphy_priv(wiphy);
	struct virtio_wifi *virtio_wifi = priv->virtio_wifi;
	struct virtio_wifi_vif *vif;
	struct net_device *ndev;
	struct wireless_dev *wdev;
	u8 addr[ETH_ALEN] = { 0 };
	char name[IFNAMSIZ] = { 0 };

	printk("%s called\n", __func__);

	strscpy(name, wif->ifname, IFNAMSIZ);

	ndev = alloc_netdev(sizeof(struct virtio_wifi_vif), name,
			    name_assign_type, ether_setup);
	if (!ndev) {
		WARN_ON(!ndev);
		return ERR_PTR(-ENOMEM);
	}

	ether_addr_copy(addr, wif->addr);
	eth_hw_addr_set(ndev, addr);

	vif = netdev_priv(ndev);

	vif->being_deleted = false;
	vif->is_connected = false;
	vif->is_up = false;
	vif->virtio_wifi = virtio_wifi;
	vif->ndev = ndev;

	vif->index = virtio_le_to_cpu(wif->index);

	wdev = &vif->wdev;
	wdev->wiphy = wiphy;
	wdev->netdev = ndev;
	wdev->iftype = virtio_le_to_cpu(wif->iftype);
	SET_NETDEV_DEV(ndev, wiphy_dev(wdev->wiphy));
	ndev->ieee80211_ptr = wdev;

	ndev->rtnl_link_ops = &virtio_wifi_cfg80211_link_ops;
	ndev->netdev_ops = &virtio_wifi_netdev_ops;
	ndev->needs_free_netdev = true;
	ndev->priv_destructor = virtio_wifi_cfg80211_net_device_destructor;
	ndev->needed_headroom = ETH_HLEN;

	if (cfg80211_register_netdevice(ndev)) {
		WARN_ON(1);
		free_netdev(ndev);
		/* netdev_rx_handler_unregister(ndev); */
		return ERR_PTR(-ENOMEM);
	}

	return &vif->wdev;
}

static struct wireless_dev *virtio_wifi_cfg80211_add_virtual_intf(
	struct wiphy *wiphy, const char *name, unsigned char name_assign_type,
	enum nl80211_iftype type, struct vif_params *params)
{
	struct virtio_wifi_wiphy *priv = wiphy_priv(wiphy);
	struct virtio_wifi *virtio_wifi = priv->virtio_wifi;
	struct virtio_wifi_interface wif = { 0 };
	struct wireless_dev *wdev;
	int ret = 0;

	printk("%s called\n", __func__);

	/* if (ar->num_vif == ar->vif_max) { */
	/* 	ath6kl_err("Reached maximum number of supported vif\n"); */
	/* 	return ERR_PTR(-EINVAL); */
	/* } */

	if (type != NL80211_IFTYPE_STATION) {
		printk("%s: %d is not a supported interface type\n", __func__,
		       type);
		return ERR_PTR(-EINVAL);
	}

	// https://elixir.bootlin.com/linux/v6.12.83/source/drivers/net/wireless/ath/ath6kl/cfg80211.c#L3779

	/* ether_addr_copy(addr, priv->virtio_wifi->mac_addr); */
	ret = virtio_wifi_do_add_interface(virtio_wifi, type, &wif);
	if (ret) {
		WARN_ON(ret);
		return ERR_PTR(ret);
	}
	strscpy((char *)&wif.ifname, name, IFNAMSIZ);
	wdev = virtio_wifi_cfg80211_add_intf(wiphy, name_assign_type, &wif);

	if (IS_ERR_VALUE(wdev)) {
		virtio_wifi_do_remove_interface(virtio_wifi, wif.index);
	}
	return wdev;
}

static int virtio_wifi_cfg80211_del_virtual_intf(struct wiphy *wiphy,
						 struct wireless_dev *wdev)
{
	struct virtio_wifi_wiphy *priv = wiphy_priv(wiphy);
	struct virtio_wifi *virtio_wifi = priv->virtio_wifi;
	struct virtio_wifi_vif *vif =
		container_of(wdev, struct virtio_wifi_vif, wdev);
	int ret = 0;

	printk("%s called\n", __func__);
	ret = virtio_wifi_do_remove_interface(virtio_wifi, vif->index);
	if (ret) {
		return ret;
	}

	cfg80211_stop_iface(wiphy, wdev, GFP_KERNEL);
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
	struct virtio_wifi_wiphy *priv = wiphy_priv(wiphy);

	printk("%s called\n", __func__);

	if (vif->being_deleted || !vif->is_up)
		return -EBUSY;

	if (!sme->ssid)
		return -EINVAL;

	return virtio_wifi_do_connect(priv, vif->index, sme);
}

/* May acquire and release the rdev event lock. */
static void virtio_wifi_cfg80211_cancel_connect(struct net_device *netdev)
{
	struct virtio_wifi_vif *vif = netdev_priv(netdev);

	printk("%s called\n", __func__);
	if (!vif->is_connected)
		return;

	/* /1* If there is work pending, clean up dangling callbacks. *1/ */
	/* if (cancel_delayed_work_sync(&vif->connect)) { */
	/* 	/1* Schedules an event that acquires the rtnl lock. *1/ */
	/* 	cfg80211_connect_result(vif->ndev, vif->connect_requested_bss, */
	/* 				NULL, 0, NULL, 0, */
	/* 				WLAN_STATUS_UNSPECIFIED_FAILURE, */
	/* 				GFP_KERNEL); */
	/* } */
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
	netif_stop_queue(vif->ndev);
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

	printk("%s called\n", __func__);
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

	printk("%s called\n", __func__);
	wiphy_debug(wiphy, "dump_station\n");

	if (idx != 0 || !vif->is_connected)
		return -ENOENT;

	/* ether_addr_copy(mac, fake_router_bssid); */

	//TODO:
	return -ENOENT;
	/* return virtio_wifi_cfg80211_get_station(wiphy, dev, fake_router_bssid, */
	/* 					sinfo); */
}

static int virtio_wifi_cfg80211_auth(struct wiphy *wiphy,
				     struct net_device *dev,
				     struct cfg80211_auth_request *req)
{
	struct virtio_wifi_vif *vif = netdev_priv(dev);
	struct virtio_wifi_wiphy *priv = wiphy_priv(wiphy);
	struct cfg80211_bss *bss = req->bss;
	const struct cfg80211_bss_ies *ies = rcu_access_pointer(bss->ies);

	if (ies == NULL) {
		return -EINVAL;
	}
	const struct element *ssid_elem =
		cfg80211_find_elem(WLAN_EID_SSID, ies->data, ies->len);
	if (ssid_elem == NULL) {
		return -EINVAL;
	}

	/* printk("%s called\n", __func__); */
	return virtio_wifi_do_auth(priv, vif->index, ssid_elem, req);
}

static int virtio_wifi_cfg80211_assoc(struct wiphy *wiphy,
				      struct net_device *dev,
				      struct cfg80211_assoc_request *req)
{
	struct virtio_wifi_vif *vif = netdev_priv(dev);
	struct virtio_wifi_wiphy *priv = wiphy_priv(wiphy);
	struct cfg80211_bss *bss = req->bss;

	const struct cfg80211_bss_ies *ies = rcu_access_pointer(bss->ies);

	if (!ies) {
		return -EINVAL;
	}
	const struct element *ssid_elem =
		cfg80211_find_elem(WLAN_EID_SSID, ies->data, ies->len);

	cfg80211_ref_bss(wiphy, req->bss);

	vif->associated_bss = bss;
	/* printk("%s called\n", __func__); */
	return virtio_wifi_do_assoc(priv, vif->index, ssid_elem, req);
}

static int virtio_wifi_cfg80211_deauth(struct wiphy *wiphy,
				       struct net_device *dev,
				       struct cfg80211_deauth_request *req)
{
	struct virtio_wifi_vif *vif = netdev_priv(dev);
	struct virtio_wifi_wiphy *priv = wiphy_priv(wiphy);

	printk("%s called\n", __func__);

	return virtio_wifi_do_disconnect(priv, vif->index,
					 VIRTIO_WIFI_CMD_DEAUTHENTICATE,
					 req->bssid, req->ie, req->ie_len,
					 req->reason_code,
					 req->local_state_change);
}

static int virtio_wifi_cfg80211_disassoc(struct wiphy *wiphy,
					 struct net_device *dev,
					 struct cfg80211_disassoc_request *req)
{
	struct virtio_wifi_vif *vif = netdev_priv(dev);
	struct virtio_wifi_wiphy *priv = wiphy_priv(wiphy);

	printk("%s called\n", __func__);
	return virtio_wifi_do_disconnect(priv, vif->index,
					 VIRTIO_WIFI_CMD_DISASSOCIATE,
					 req->ap_addr, req->ie, req->ie_len,
					 req->reason_code,
					 req->local_state_change);
}

static int virtio_wifi_cfg80211_add_key(struct wiphy *wiphy,
					struct net_device *netdev, int link_id,
					u8 key_index, bool pairwise,
					const u8 *mac_addr,
					struct key_params *params)
{
	struct virtio_wifi_vif *vif = netdev_priv(netdev);
	struct virtio_wifi_wiphy *priv = wiphy_priv(wiphy);
	printk("%s called\n", __func__);

	return virtio_wifi_do_add_key(priv, vif->index, key_index, pairwise,
				      mac_addr, params);
}

static int virtio_wifi_cfg80211_del_key(struct wiphy *wiphy,
					struct net_device *netdev, int link_id,
					u8 key_index, bool pairwise,
					const u8 *mac_addr)
{
	struct virtio_wifi_vif *vif = netdev_priv(netdev);
	struct virtio_wifi_wiphy *priv = wiphy_priv(wiphy);
	printk("%s called\n", __func__);

	return virtio_wifi_do_del_key(priv, vif->index, key_index, pairwise,
				      mac_addr);
}

static int virtio_wifi_cfg80211_set_default_key(struct wiphy *wiphy,
						struct net_device *netdev,
						int link_id, u8 key_index,
						bool unicast, bool multicast)
{
	struct virtio_wifi_vif *vif = netdev_priv(netdev);
	struct virtio_wifi_wiphy *priv = wiphy_priv(wiphy);
	printk("%s called\n", __func__);

	return virtio_wifi_do_set_default_key(priv, vif->index, key_index,
					      unicast, multicast);
}

static int
virtio_wifi_cfg80211_change_station(struct wiphy *wiphy, struct net_device *dev,
				    const u8 *mac,
				    struct station_parameters *params)
{
	struct virtio_wifi_vif *vif = netdev_priv(dev);
	struct virtio_wifi_wiphy *priv = wiphy_priv(wiphy);
	printk("%s called\n", __func__);

	return virtio_wifi_do_change_station(priv, vif->index, mac, params);
}

static const struct cfg80211_ops virtio_wifi_cfg80211_cfg80211_ops = {
	.add_virtual_intf = virtio_wifi_cfg80211_add_virtual_intf,
	.del_virtual_intf = virtio_wifi_cfg80211_del_virtual_intf,
	.scan = virtio_wifi_cfg80211_scan,

	.connect = virtio_wifi_cfg80211_connect,
	.disconnect = virtio_wifi_cfg80211_disconnect,

	/* .get_station = virtio_wifi_cfg80211_get_station, */
	/* .dump_station = virtio_wifi_cfg80211_dump_station, */
	.change_station = virtio_wifi_cfg80211_change_station,
	.auth = virtio_wifi_cfg80211_auth,
	.assoc = virtio_wifi_cfg80211_assoc,
	.deauth = virtio_wifi_cfg80211_deauth,
	.disassoc = virtio_wifi_cfg80211_disassoc,
	.add_key = virtio_wifi_cfg80211_add_key,
	.del_key = virtio_wifi_cfg80211_del_key,
	.set_default_key = virtio_wifi_cfg80211_set_default_key,
};

/* Acquires and releases the rtnl lock. */
struct virtio_wifi_wiphy *
virtio_wifi_cfg80211_make_wiphy(struct virtio_wifi *virtio_wifi)
{
	struct wiphy *wiphy;
	struct virtio_wifi_wiphy *priv;
	int err;
	struct cfg80211_ops *ops =
		kzalloc(sizeof(struct cfg80211_ops), GFP_KERNEL);

	*ops = virtio_wifi_cfg80211_cfg80211_ops;
	u32 flags = 0;
	virtio_cread_le(virtio_wifi->virtio_dev, struct virtio_wifi_config,
			flags, &flags);

	if ((flags & VIRTIO_WIFI_RADIO_F_AUTHENTICATE) == 0) {
		ops->auth = NULL;
		ops->deauth = NULL;
	}
	if ((flags & VIRTIO_WIFI_RADIO_F_CONNECT) == 0) {
		ops->connect = NULL;
		ops->disconnect = NULL;
	}

	wiphy = wiphy_new(ops, sizeof(*priv));

	if (!wiphy) {
		kfree(ops);
		return NULL;
	}

	set_wiphy_dev(wiphy, &virtio_wifi->virtio_dev->dev);

	priv = wiphy_priv(wiphy);
	priv->wiphy = wiphy;
	mutex_init(&priv->mutex);
	priv->virtio_wifi = virtio_wifi;

	wiphy->max_scan_ssids = 4;
	wiphy->max_scan_ie_len = 1000;
	wiphy->signal_type = CFG80211_SIGNAL_TYPE_MBM;

	for (int band_kind = NL80211_BAND_2GHZ; band_kind < NUM_NL80211_BANDS;
	     band_kind++) {
		struct virtio_wifi_band band = { 0 };

		wiphy->bands[band_kind] = NULL;

		struct ieee80211_supported_band *sband =
			&priv->bands[band_kind];
		sband->band = band_kind;

		switch (band_kind) {
		case NL80211_BAND_2GHZ:
		case NL80211_BAND_5GHZ:
			break;
		default:
			continue;
		}
		err = virtio_wifi_get_band(virtio_wifi, band_kind, &band);
		if (err)
			continue;

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
			err = virtio_wifi_get_band_channel(
				virtio_wifi, band_kind, ch, &channel);
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
			err = virtio_wifi_get_band_bitrate(
				virtio_wifi, band_kind, br, &bitrate);
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
	virtio_cread_le(virtio_wifi->virtio_dev, struct virtio_wifi_config,
			available_antennas_tx, &wiphy->available_antennas_tx);
	virtio_cread_le(virtio_wifi->virtio_dev, struct virtio_wifi_config,
			available_antennas_rx, &wiphy->available_antennas_rx);

	virtio_cread_le(virtio_wifi->virtio_dev, struct virtio_wifi_config,
			n_cipher_suites, &priv->n_cipher_suites);
	printk("%s n_cipher_suites=%d\n", __func__, priv->n_cipher_suites);
	err = virtio_wifi_get_cipher_suites(virtio_wifi, &priv->cipher_suites,
					    priv->n_cipher_suites);
	/*FIXME*/ BUG_ON(err);
	wiphy->cipher_suites = priv->cipher_suites;
	wiphy->n_cipher_suites = priv->n_cipher_suites;

	priv->being_deleted = false;
	priv->scan_request = NULL;
	INIT_DELAYED_WORK(&priv->tx_work, virtio_wifi_do_tx_work);
	skb_queue_head_init(&priv->tx_list);

	err = wiphy_register(wiphy);
	if (err < 0) {
		virtio_wifi_cfg80211_destroy_wiphy(wiphy);
		return NULL;
	}
	priv->wiphy_registered = true;

	return priv;
}

/* Acquires and releases the rtnl lock. */
void virtio_wifi_cfg80211_destroy_wiphy(struct wiphy *wiphy)
{
	struct virtio_wifi_wiphy *priv;

	printk("%s called %p\n", __func__, wiphy);
	if (!wiphy)
		return;

	priv = wiphy_priv(wiphy);
	priv->being_deleted = true;

	priv->n_cipher_suites = 0;
	wiphy->cipher_suites = priv->cipher_suites;
	wiphy->n_cipher_suites = priv->n_cipher_suites;

	cancel_delayed_work_sync(&priv->tx_work);
	if (priv->wiphy_registered) {
		virtio_wifi_cfg80211_cancel_scan(wiphy);
		wiphy_unregister(wiphy);
		priv->wiphy_registered = false;
	}
	kfree(priv->ops);
	wiphy_free(wiphy);
}

/* Enters and exits a RCU-bh critical section. */
static netdev_tx_t virtio_wifi_cfg80211_start_xmit(struct sk_buff *skb,
						   struct net_device *dev)
{
	struct virtio_wifi_vif *vif = netdev_priv(dev);
	struct virtio_wifi_wiphy *priv = wiphy_priv(vif->wdev.wiphy);

	/* printk("%s called\n", __func__); */
	vif->tx_packets++;
	if (!vif->is_connected) {
		vif->tx_failed++;
		return NET_XMIT_DROP;
	}
	priv->tx_if = vif->index;
	skb->dev = vif->ndev;
	skb_queue_tail(&priv->tx_list, skb);

	schedule_delayed_work(&priv->tx_work, 0);

	return NETDEV_TX_OK;
}

/* Called with rtnl lock held. */
static int virtio_wifi_cfg80211_net_device_open(struct net_device *dev)
{
	struct virtio_wifi_vif *vif = netdev_priv(dev);
	struct virtio_wifi_wiphy *priv = wiphy_priv(vif->wdev.wiphy);
	int ret;

	printk("%s called\n", __func__);
	ret = virtio_wifi_do_if_oper(priv, vif->index, VIRTIO_WIFI_IF_OPER_UP,
				     true);
	if (ret) {
		return ret;
	}
	vif->is_up = true;
	netif_carrier_on(dev);
	netif_wake_queue(dev);
	return ret;
}

/* Called with rtnl lock held. */
static int virtio_wifi_cfg80211_net_device_stop(struct net_device *dev)
{
	struct virtio_wifi_vif *vif = netdev_priv(dev);
	struct virtio_wifi_wiphy *priv = wiphy_priv(vif->wdev.wiphy);
	int ret;

	printk("%s called\n", __func__);
	ret = virtio_wifi_do_if_oper(priv, vif->index, VIRTIO_WIFI_IF_OPER_UP,
				     false);

	if (ret) {
		return ret;
	}
	vif->is_up = false;

	if (!dev->ieee80211_ptr)
		return 0;

	virtio_wifi_cfg80211_cancel_scan(dev->ieee80211_ptr->wiphy);
	virtio_wifi_cfg80211_cancel_connect(dev);
	netif_stop_queue(dev);
	netif_carrier_off(dev);

	return 0;
}

static const struct net_device_ops virtio_wifi_netdev_ops = {
	.ndo_start_xmit = virtio_wifi_cfg80211_start_xmit,
	.ndo_open = virtio_wifi_cfg80211_net_device_open,
	.ndo_stop = virtio_wifi_cfg80211_net_device_stop,
};

/* Invoked as part of rtnl lock release. */
static void virtio_wifi_cfg80211_net_device_destructor(struct net_device *dev)
{
	/* Delayed past dellink to allow nl80211 to react to the device being
	 * deleted.
	 */
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

/* /1* Called in a RCU read critical section from netif_receive_skb *1/ */
/* static rx_handler_result_t */
/* virtio_wifi_cfg80211_rx_handler(struct sk_buff **pskb) */
/* { */
/* 	struct sk_buff *skb = *pskb; */
/* 	struct virtio_wifi_vif *vif = */
/* 		rcu_dereference(skb->dev->rx_handler_data); */

/* 	/1* printk("%s called\n", __func__); *1/ */
/* 	if (!vif->is_connected) */
/* 		return RX_HANDLER_PASS; */

/* 	/1* GFP_ATOMIC because this is a packet interrupt handler. *1/ */
/* 	skb = skb_share_check(skb, GFP_ATOMIC); */
/* 	if (!skb) { */
/* 		dev_err(&vif->virtio_wifi->virtio_dev->dev, */
/* 			"can't skb_share_check\n"); */
/* 		return RX_HANDLER_CONSUMED; */
/* 	} */

/* 	*pskb = skb; */
/* 	skb->dev = vif->ndev; */
/* 	skb->pkt_type = PACKET_HOST; */
/* 	return RX_HANDLER_ANOTHER; */
/* } */

static struct rtnl_link_ops virtio_wifi_cfg80211_link_ops = {
	.kind = "virtio_wifi",
	.setup = virtio_wifi_cfg80211_setup,
	.priv_size = sizeof(struct virtio_wifi_vif),
};

/* static bool netif_is_virtio_wifi_cfg80211_dev(const struct net_device *dev) */
/* { */
/* 	return rcu_access_pointer(dev->rx_handler) == */
/* 	       virtio_wifi_cfg80211_rx_handler; */
/* } */

static int virtio_wifi_cfg80211_event(struct notifier_block *this,
				      unsigned long event, void *ptr)
{
	/* struct net_device *lower_dev = netdev_notifier_info_to_dev(ptr); */
	LIST_HEAD(list_kill);

	/* if (!netif_is_virtio_wifi_cfg80211_dev(lower_dev)) */
	/* 	return NOTIFY_DONE; */

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
