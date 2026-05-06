/* SPDX-License-Identifier: GPL-2.0+ */
/* Author: Manos Pitsidianakis <manos.pitsidianakis@linaro.org> */

#include <linux/version.h>
#include <net/mac80211.h>
#include "virtio_wifi.h"

static int virtio_wifi_ieee80211_start(struct ieee80211_hw *hw);
static void virtio_wifi_ieee80211_stop(struct ieee80211_hw *hw, bool suspended);
static int virtio_wifi_ieee80211_add_interface(struct ieee80211_hw *hw,
					       struct ieee80211_vif *vif);
static void virtio_wifi_ieee80211_remove_interface(struct ieee80211_hw *hw,
						   struct ieee80211_vif *vif);
static void virtio_wifi_ieee80211_tx(struct ieee80211_hw *hw,
				     struct ieee80211_tx_control *control,
				     struct sk_buff *skb);
static void virtio_wifi_ieee80211_wake_tx_queue(struct ieee80211_hw *hw,
						struct ieee80211_txq *txq);
static int virtio_wifi_ieee80211_config(struct ieee80211_hw *hw,
#if LINUX_VERSION_CODE >= KERNEL_VERSION(6, 17, 0)
					int radio_idx,
#endif
					u32 changed);
static void virtio_wifi_ieee80211_configure_filter(struct ieee80211_hw *hw,
						   unsigned int changed_flags,
						   unsigned int *total_flags,
						   u64 multicast);
static void virtio_wifi_ieee80211_link_info_changed(
	struct ieee80211_hw *hw, struct ieee80211_vif *vif,
	struct ieee80211_bss_conf *info, u64 changed);
static void virtio_wifi_ieee80211_vif_cfg_changed(struct ieee80211_hw *hw,
						  struct ieee80211_vif *vif,
						  u64 changed);

static int virtio_wifi_ieee80211_hw_scan(struct ieee80211_hw *hw,
					 struct ieee80211_vif *vif,
					 struct ieee80211_scan_request *req);

static void virtio_wifi_ieee80211_cancel_hw_scan(struct ieee80211_hw *hw,
						 struct ieee80211_vif *vif);

const struct ieee80211_ops virtio_wifi_ieee80211_ops = {
	.start = virtio_wifi_ieee80211_start,
	.stop = virtio_wifi_ieee80211_stop,
	.add_interface = virtio_wifi_ieee80211_add_interface,
	.remove_interface = virtio_wifi_ieee80211_remove_interface,
	.tx = virtio_wifi_ieee80211_tx,
	.wake_tx_queue = virtio_wifi_ieee80211_wake_tx_queue,
	.config = virtio_wifi_ieee80211_config,
	.link_info_changed = virtio_wifi_ieee80211_link_info_changed,
	.vif_cfg_changed = virtio_wifi_ieee80211_vif_cfg_changed,
	.configure_filter = virtio_wifi_ieee80211_configure_filter,
	.hw_scan = virtio_wifi_ieee80211_hw_scan,
	.cancel_hw_scan = virtio_wifi_ieee80211_cancel_hw_scan,
	.add_chanctx = ieee80211_emulate_add_chanctx,
	.remove_chanctx = ieee80211_emulate_remove_chanctx,
	.change_chanctx = ieee80211_emulate_change_chanctx,
	.switch_vif_chanctx = ieee80211_emulate_switch_vif_chanctx,
};

static int virtio_wifi_ieee80211_start(struct ieee80211_hw *hw)
{
	struct virtio_wifi_wiphy *wiphy = hw->priv;
	u32 index = wiphy->index;
	printk("%s called\n", __func__);
	return virtio_wifi_do_start(wiphy->virtio_wifi, index);
}

static void virtio_wifi_ieee80211_stop(struct ieee80211_hw *hw, bool suspended)
{
	struct virtio_wifi_wiphy *wiphy = hw->priv;
	u32 index = wiphy->index;
	printk("%s called\n", __func__);
	virtio_wifi_do_stop(wiphy->virtio_wifi, index);
}

static int virtio_wifi_ieee80211_add_interface(struct ieee80211_hw *hw,
					       struct ieee80211_vif *vif)
{
	struct virtio_wifi_wiphy *wiphy = hw->priv;
	u32 index = wiphy->index;
	printk("%s called\n", __func__);
	return virtio_wifi_do_add_interface(wiphy->virtio_wifi, index, vif);
}

static void virtio_wifi_ieee80211_remove_interface(struct ieee80211_hw *hw,
						   struct ieee80211_vif *vif)
{
	struct virtio_wifi_wiphy *wiphy = hw->priv;
	u32 index = wiphy->index;
	printk("%s called\n", __func__);
	virtio_wifi_do_remove_interface(wiphy->virtio_wifi, index, vif);
	return;
}

/* Must be atomic, and not sleep */
static void virtio_wifi_ieee80211_tx(struct ieee80211_hw *hw,
				     struct ieee80211_tx_control *control,
				     struct sk_buff *skb)
{
	struct virtio_wifi_wiphy *wiphy = hw->priv;
	struct virtio_wifi_interface *wif = NULL;

	guard(wiphy_mutex)(wiphy);
	printk("%s called\n", __func__);
	struct ieee80211_tx_info *txi = IEEE80211_SKB_CB(skb);
	/* struct ieee80211_hdr *hdr = (void *)skb->data; */
	/* struct ieee80211_channel *channel = wiphy->channel; */
	struct ieee80211_vif *vif = txi->control.vif;
	printk("%s vif=%p\n", __func__, vif);
	if (!vif) {
		goto err;
	}
	wif = (void *)vif->drv_priv;
	printk("%s vif index=%d\n", __func__, wif->index);
	wiphy->tx_wif = wif;
	wiphy->tx_control = control;
	wiphy->tx_skb = skb;
	ieee80211_queue_delayed_work(wiphy->hw, &wiphy->tx_work, 0);
	ieee80211_tx_info_clear_status(txi);

	/* frame was transmitted at most favorable rate at first attempt */
	txi->control.rates[0].count = 1;
	txi->control.rates[1].idx = -1;
	/* if (!(txi->flags & IEEE80211_TX_CTL_NO_ACK) && ack) */
	/* 	txi->flags |= IEEE80211_TX_STAT_ACK; */
	return;
err:
	ieee80211_free_txskb(hw, skb);
	return;
}

static void virtio_wifi_ieee80211_wake_tx_queue(struct ieee80211_hw *hw,
						struct ieee80211_txq *txq)
{
	printk("%s called\n", __func__);
	return;
}

static int virtio_wifi_ieee80211_config(struct ieee80211_hw *hw,
#if LINUX_VERSION_CODE >= KERNEL_VERSION(6, 17, 0)
					int radio_idx,
#endif
					u32 changed)
{
	struct virtio_wifi_wiphy *wiphy = hw->priv;
	struct ieee80211_conf *conf = &hw->conf;

	if (changed & IEEE80211_CONF_CHANGE_MONITOR) {
		if (conf->flags & IEEE80211_CONF_MONITOR) {
			printk("%s: Monitor mode is enabled\n", __func__);
			wiphy->is_monitoring = true;
		} else {
			printk("%s: Monitor mode is disabled\n", __func__);
			wiphy->is_monitoring = false;
		}
	}

	if (changed & IEEE80211_CONF_CHANGE_CHANNEL) {
		/* ctx->offchannel = !!(conf->flags & IEEE80211_CONF_OFFCHANNEL); */
		if (virtio_wifi_do_set_channel(wiphy, &hw->conf.chandef)) {
			return -1;
		}
	}

	return 0;
}

static void virtio_wifi_ieee80211_configure_filter(struct ieee80211_hw *hw,
						   unsigned int changed_flags,
						   unsigned int *total_flags,
						   u64 multicast)
{
	struct virtio_wifi_wiphy *wiphy = hw->priv;

	printk("%s  called with %u\n", __func__, *total_flags);

	wiphy->rx_filter = 0;
	if (virtio_wifi_do_configure_filter(wiphy->virtio_wifi, wiphy->index,
					    changed_flags, total_flags,
					    multicast) != 0) {
		printk("%s early ret\n", __func__);
		return;
	}
	printk("%s  ret with %u\n", __func__, *total_flags);
	if (*total_flags & FIF_ALLMULTI)
		wiphy->rx_filter |= FIF_ALLMULTI;
	if (*total_flags & FIF_MCAST_ACTION)
		wiphy->rx_filter |= FIF_MCAST_ACTION;
	return;
}

static int virtio_wifi_ieee80211_hw_scan(struct ieee80211_hw *hw,
					 struct ieee80211_vif *vif,
					 struct ieee80211_scan_request *req)
{
	struct virtio_wifi_wiphy *wiphy = hw->priv;
	struct virtio_wifi_interface *wif = NULL;
	BUG_ON(!vif);
	wif = (void *)vif->drv_priv;
	struct cfg80211_scan_request *cfg80211_req = &req->req;

	printk("%s called\n", __func__);

	guard(wiphy_mutex)(wiphy);

	if (WARN_ON(wiphy->tmp_chan || wiphy->hw_scan_request)) {
		return -EBUSY;
	}
	int ret = virtio_wifi_do_trigger_scan(wiphy, wif->index, cfg80211_req);
	if (ret) {
		printk("%s do_trigger_scan returned %d\n", __func__, ret);
		goto err;
	}
	wiphy->hw_scan_request = cfg80211_req;
	wiphy->hw_scan_vif = vif;
	wiphy->scan_chan_idx = 0;
	if (cfg80211_req->flags & NL80211_SCAN_FLAG_RANDOM_ADDR)
		get_random_mask_addr(wiphy->scan_addr, cfg80211_req->mac_addr,
				     cfg80211_req->mac_addr_mask);
	else
		memcpy(wiphy->scan_addr, vif->addr, ETH_ALEN);
	memset(&wiphy->survey_data, 0, sizeof(wiphy->survey_data));

	ieee80211_queue_delayed_work(wiphy->hw, &wiphy->hw_scan_work, 0);
err:
	return ret;
}

static void virtio_wifi_ieee80211_cancel_hw_scan(struct ieee80211_hw *hw,
						 struct ieee80211_vif *vif)
{
	printk("%s called\n", __func__);
	return;
}

static void virtio_wifi_ieee80211_link_info_changed(
	struct ieee80211_hw *hw, struct ieee80211_vif *vif,
	struct ieee80211_bss_conf *info, u64 changed)
{
	printk("%s called changed %llu\n", __func__, changed);
	return;
}

static void virtio_wifi_ieee80211_vif_cfg_changed(struct ieee80211_hw *hw,
						  struct ieee80211_vif *vif,
						  u64 changed)
{
	printk("%s called changed %llu\n", __func__, changed);
	return;
}
