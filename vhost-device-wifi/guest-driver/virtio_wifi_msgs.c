/* SPDX-License-Identifier: GPL-2.0+ */
/* Author: Manos Pitsidianakis <manos.pitsidianakis@linaro.org> */

#include <linux/virtio.h>
#include <linux/string.h>
#include "virtio_wifi.h"

/**
 * @resp_head: message response header
 *
 * Return: negative system errno, or 0
 */
static inline int
virtio_wifi_msg_get_resp_errno(struct virtio_wifi_hdr *resp_head)
{
	switch (virtio_le_to_cpu(resp_head->code)) {
	case VIRTIO_WIFI_S_OK:
		return 0;
	case VIRTIO_WIFI_S_NOT_SUPP:
		return -EOPNOTSUPP;
	case VIRTIO_WIFI_S_BAD_MSG:
		return -EINVAL;
	case VIRTIO_WIFI_S_IO_ERR:
	default:
		return -EIO;
	}
}

struct virtio_wifi_msg *virtio_wifi_msg_init(struct virtio_wifi *virtio_wifi,
					     u32 msg_type,
					     unsigned int req_size,
					     unsigned int resp_cap)
{
	struct device *dev = &virtio_wifi->virtio_dev->dev;
	struct virtio_wifi_cmd_request *req_head;
	struct virtio_wifi_msg *msg;

	msg = devm_kzalloc(dev, sizeof(*msg), GFP_KERNEL);
	if (!msg)
		return NULL;

	init_completion(&msg->responded);

	msg->req = devm_kzalloc(dev, req_size, GFP_KERNEL);
	if (!msg->req)
		goto err_free_msg;

	req_head = msg->req;

	msg->resp = devm_kzalloc(dev, resp_cap, GFP_KERNEL);
	if (!msg->resp)
		goto err_free_msg_req;

	msg->virtio_wifi = virtio_wifi;
	msg->req_size = req_size;
	msg->resp_cap = resp_cap;

	refcount_set(&msg->refcnt, 2);

	req_head->hdr.code = virtio_cpu_to_le(msg_type, req_head->hdr.code);

	return msg;

err_free_msg_req:
	devm_kfree(dev, msg->req);

err_free_msg:
	devm_kfree(dev, msg);

	return NULL;
}

void virtio_wifi_msg_release(struct virtio_wifi_msg *msg)
{
	struct device *dev;

	if (refcount_dec_and_test(&msg->refcnt)) {
		dev = &msg->virtio_wifi->virtio_dev->dev;

		devm_kfree(dev, msg->req);
		devm_kfree(dev, msg->resp);
		devm_kfree(dev, msg);
	}
}

/**
 * virtio_wifi_msg_xfer() - send message request, wait until message response
 * @vq: virtqueue
 * @msg: message with driver request
 * @timeout_jiffies: message response timeout, 0 for no timeout
 *
 * Context: Process context. Takes and releases vq.lock. May sleep.
 * Return: Zero on success, negative error code otherwise.
 */
static int virtio_wifi_msg_xfer(struct virtio_wifi_vq *vq,
				struct virtio_wifi_msg *msg,
				unsigned long timeout_jiffies)
{
	struct scatterlist out_sg[1];
	struct scatterlist in_sg[1];
	struct scatterlist *sgs[2];
	unsigned long flags;
	long timeout_ret;
	bool notify;
	int ret;

	sgs[0] = out_sg;
	sgs[1] = in_sg;

	sg_init_one(out_sg, msg->req, msg->req_size);
	sg_init_one(in_sg, msg->resp, msg->resp_cap);

	spin_lock_irqsave(&vq->lock, flags);

	ret = virtqueue_add_sgs(vq->vq, sgs, 1, 1, msg, GFP_ATOMIC);
	if (ret) {
		spin_unlock_irqrestore(&vq->lock, flags);
		/*
		 * Release in place of the response callback, which will never
		 * come.
		 */
		virtio_wifi_msg_release(msg);
		return ret;
	}

	notify = virtqueue_kick_prepare(vq->vq);

	spin_unlock_irqrestore(&vq->lock, flags);

	if (notify)
		virtqueue_notify(vq->vq);

	if (timeout_jiffies) {
		timeout_ret = wait_for_completion_interruptible_timeout(
			&msg->responded, timeout_jiffies);

		if (!timeout_ret)
			return -ETIMEDOUT;
		else if (timeout_ret < 0)
			return (int)timeout_ret;
	} else {
		ret = wait_for_completion_interruptible(&msg->responded);
		if (ret)
			return ret;
	}

	printk("%s: msg->resp_actual_size=%d: sizeof(struct virtio_wifi_hdr)=%ld\n",
	       __func__, msg->resp_actual_size, sizeof(struct virtio_wifi_hdr));
	if (msg->resp_actual_size < sizeof(struct virtio_wifi_hdr))
		return -EINVAL;

	ret = virtio_wifi_msg_get_resp_errno(msg->resp);
	printk("%s: cmd = %s ret= %d\n", __func__,
	       cmd_to_s(virtio_le_to_cpu(
		       (*(struct virtio_wifi_hdr *)msg->req).code)),
	       ret);
	if (ret)
		return ret;

	printk("%s: msg->resp_actual_size=%d: msg->resp_cap=%d\n", __func__,
	       msg->resp_actual_size, msg->resp_cap);
	/*
	 * There is not yet a case where returning a short message would make
	 * sense, so consider any deviation an error.
	 */
	if (msg->resp_actual_size != msg->resp_cap)
		return -EINVAL;

	return 0;
}

static int virtio_wifi_get_wiphy(struct virtio_wifi *dev, u32 wiphy_idx,
				 struct virtio_wifi_radio *radio)
{
	struct virtio_wifi_msg *msg;
	struct virtio_wifi_cmd_request *req;
	size_t resp_cap = 0;

	int ret = 0;

	BUG_ON(!dev || !radio);

	printk("%s: getting wiphy %u\n", __func__, wiphy_idx);

	resp_cap = sizeof(struct virtio_wifi_radio);

	msg = virtio_wifi_msg_init(dev, VIRTIO_WIFI_CMD_GET_WIPHY,
				   sizeof(struct virtio_wifi_cmd_request),
				   resp_cap);

	req = msg->req;
	req->hdr.index = virtio_cpu_to_le(wiphy_idx, req->hdr.index);

	ret = virtio_wifi_msg_xfer(&dev->controlq, msg, 0);
	if (ret) {
		printk("%s: xfer returned %d\n", __func__, ret);
		goto finish;
	}

	memcpy(radio, msg->resp, sizeof(struct virtio_wifi_radio));

finish:
	virtio_wifi_msg_release(msg);

	return ret;
}

static int virtio_wifi_get_band(struct virtio_wifi *dev, u32 wiphy_idx,
				enum nl80211_band band_kind,
				struct virtio_wifi_band *band)
{
	struct virtio_wifi_msg *msg;
	struct virtio_wifi_cmd_get_band_request *req;
	size_t resp_cap = 0;

	int ret = 0;

	BUG_ON(!dev || !band);

	printk("%s: getting wiphy %u band %u\n", __func__, wiphy_idx,
	       band_kind);

	resp_cap = sizeof(struct virtio_wifi_band);

	msg = virtio_wifi_msg_init(
		dev, VIRTIO_WIFI_CMD_GET_BAND,
		sizeof(struct virtio_wifi_cmd_get_band_request), resp_cap);

	req = msg->req;
	req->hdr.index = virtio_cpu_to_le(band_kind, req->hdr.index);
	req->phy_index = virtio_cpu_to_le(wiphy_idx, req->phy_index);

	ret = virtio_wifi_msg_xfer(&dev->controlq, msg, 0);
	if (ret) {
		printk("%s: xfer returned %d\n", __func__, ret);
		goto finish;
	}

	memcpy(band, msg->resp, sizeof(struct virtio_wifi_band));

finish:
	virtio_wifi_msg_release(msg);

	return ret;
}

static int
virtio_wifi_get_band_bitrate(struct virtio_wifi *dev, u32 wiphy_idx,
			     enum nl80211_band band_kind, u32 bitrate_idx,
			     struct virtio_wifi_band_bitrate *bitrate)
{
	struct virtio_wifi_msg *msg;
	struct virtio_wifi_cmd_get_band_bitrate_request *req;
	size_t resp_cap = 0;

	int ret = 0;

	BUG_ON(!dev || !bitrate);

	printk("%s: getting wiphy %u band %u\n", __func__, wiphy_idx,
	       band_kind);

	resp_cap = sizeof(struct virtio_wifi_band_bitrate);

	msg = virtio_wifi_msg_init(
		dev, VIRTIO_WIFI_CMD_GET_BAND_BITRATE,
		sizeof(struct virtio_wifi_cmd_get_band_bitrate_request),
		resp_cap);

	req = msg->req;
	req->hdr.index = virtio_cpu_to_le(bitrate_idx, req->hdr.index);
	req->phy_index = virtio_cpu_to_le(wiphy_idx, req->phy_index);
	req->band_index = virtio_cpu_to_le(band_kind, req->band_index);

	ret = virtio_wifi_msg_xfer(&dev->controlq, msg, 0);
	if (ret) {
		printk("%s: xfer returned %d\n", __func__, ret);
		goto finish;
	}

	memcpy(bitrate, msg->resp, sizeof(struct virtio_wifi_band_bitrate));

finish:
	virtio_wifi_msg_release(msg);

	return ret;
}

static int virtio_wifi_get_band_channel(struct virtio_wifi *dev, u32 wiphy_idx,
					enum nl80211_band band_kind,
					u32 channel_idx,
					struct virtio_wifi_channel *channel)
{
	struct virtio_wifi_msg *msg;
	struct virtio_wifi_cmd_get_band_channel_request *req;
	size_t resp_cap = 0;

	int ret = 0;

	BUG_ON(!dev || !channel);

	printk("%s: getting wiphy %u band %u\n", __func__, wiphy_idx,
	       band_kind);

	resp_cap = sizeof(struct virtio_wifi_channel);

	msg = virtio_wifi_msg_init(
		dev, VIRTIO_WIFI_CMD_GET_BAND_CHANNEL,
		sizeof(struct virtio_wifi_cmd_get_band_channel_request),
		resp_cap);

	req = msg->req;
	req->hdr.index = virtio_cpu_to_le(channel_idx, req->hdr.index);
	req->phy_index = virtio_cpu_to_le(wiphy_idx, req->phy_index);
	req->band_index = virtio_cpu_to_le(band_kind, req->band_index);

	ret = virtio_wifi_msg_xfer(&dev->controlq, msg, 0);
	if (ret) {
		printk("%s: xfer returned %d\n", __func__, ret);
		goto finish;
	}

	memcpy(channel, msg->resp, sizeof(struct virtio_wifi_channel));

finish:
	virtio_wifi_msg_release(msg);

	return ret;
}

/**
 *
 * Context: Process context.
 * Return: Zero on success, negative error code otherwise.
 */
int virtio_wifi_do_register_wiphys(struct virtio_wifi *dev)
{
	struct virtio_wifi_wiphy *wiphy = NULL;
	struct virtio_wifi_radio radio = { 0 };
	struct ieee80211_hw *hw = NULL;
	u32 num_of_wiphys = 0;

	int ret;

	virtio_cread_le(dev->virtio_dev, struct virtio_wifi_config, phys,
			&num_of_wiphys);

	printk("%s: registering %u wiphys\n", __func__, num_of_wiphys);

	if (num_of_wiphys == 0) {
		return 0;
	}

	ret = virtio_wifi_get_wiphy(dev, 0, &radio);
	if (ret) {
		goto finish;
	}

	u8 rtlmac1[] = { 0x00, 0xe0, 0x4c, 0x81, 0x92, 0x00 };

	hw = ieee80211_alloc_hw(sizeof(struct virtio_wifi_wiphy),
				&virtio_wifi_ieee80211_ops);

	BUG_ON(!hw);
	wiphy = hw->priv;
	wiphy->hw = hw;
	wiphy->virtio_wifi = dev;

	mutex_init(&wiphy->mutex);
	wiphy->index = virtio_le_to_cpu(radio.hdr.index);
	wiphy->ifaces = virtio_le_to_cpu(radio.ifaces);
	for (int band_kind = NL80211_BAND_2GHZ; band_kind < NUM_NL80211_BANDS;
	     band_kind++) {
		struct virtio_wifi_band band = { 0 };

		struct ieee80211_supported_band *sband =
			&wiphy->bands[band_kind];

		sband->band = band_kind;

		switch (band_kind) {
		case NL80211_BAND_2GHZ:
			break;
		default:
			continue;
		}
		ret = virtio_wifi_get_band(dev, 0, band_kind, &band);
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
		/*FIXME*/ BUG_ON(ret);
		sband->n_channels = virtio_le_to_cpu(band.n_channels);
		sband->n_bitrates = virtio_le_to_cpu(band.n_bitrates);
		if (sband->n_channels == 0) {
			continue;
		}
		sband->channels = devm_kzalloc(
			&dev->virtio_dev->dev,
			sizeof(struct ieee80211_channel) * sband->n_channels,
			GFP_KERNEL);

		for (int ch = 0; ch < sband->n_channels; ch++) {
			struct virtio_wifi_channel channel = { 0 };
			ret = virtio_wifi_get_band_channel(dev, 0, band_kind,
							   ch, &channel);
			/*FIXME*/ BUG_ON(ret);
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

		sband->bitrates = devm_kzalloc(&dev->virtio_dev->dev,
					       sizeof(struct ieee80211_rate) *
						       sband->n_bitrates,
					       GFP_KERNEL);

		for (int br = 0; br < sband->n_bitrates; br++) {
			struct virtio_wifi_band_bitrate bitrate = { 0 };
			ret = virtio_wifi_get_band_bitrate(dev, 0, band_kind,
							   br, &bitrate);
			/*FIXME*/ BUG_ON(ret);
			sband->bitrates[br].bitrate =
				virtio_le_to_cpu(bitrate.bitrate);
			sband->bitrates[br].flags =
				virtio_le_to_cpu(bitrate.flags);
			sband->bitrates[br].hw_value =
				virtio_le_to_cpu(bitrate.hw_value);
			sband->bitrates[br].hw_value_short =
				virtio_le_to_cpu(bitrate.hw_value_short);
		}

		hw->wiphy->bands[band_kind] = sband;
	}
	if (ret) {
		goto finish;
	}
	wiphy->flags = virtio_le_to_cpu(radio.flags);
	wiphy->hwname_len = virtio_le_to_cpu(radio.hwname_len);
	strscpy(wiphy->hwname, radio.hwname, sizeof(radio.hwname));

	wiphy->hw->vif_data_size = sizeof(struct virtio_wifi_interface);

	get_random_bytes((rtlmac1 + (ETH_ALEN - 1)), 1);
	SET_IEEE80211_PERM_ADDR(wiphy->hw, rtlmac1);
	ieee80211_hw_set(hw, NO_AUTO_VIF);

	/* INIT_DELAYED_WORK(&wiphy->hw_scan_work, virtio_wifi_hw_scan_work); */
	INIT_DELAYED_WORK(&wiphy->tx_work, virtio_wifi_do_tx_work);
	wiphy->hw->queues = 5;
	wiphy_net_set(wiphy->hw->wiphy, &init_net);
	wiphy->hw->wiphy->interface_modes = BIT(NL80211_IFTYPE_STATION);
	wiphy->hw->wiphy->available_antennas_tx = virtio_le_to_cpu(radio.available_antennas_tx);
	wiphy->hw->wiphy->available_antennas_rx = virtio_le_to_cpu(radio.available_antennas_rx);

	wiphy->dev = device_create(virtio_wifi_class, NULL, 0, hw,
				   "virtio_wifi%d", wiphy->index);
	if (IS_ERR(wiphy->dev)) {
		printk("%s: device_create failed (%ld)\n", __func__,
		       PTR_ERR(wiphy->dev));
		ret = -ENOMEM;
		goto finish;
	}
	wiphy->dev->driver = &virtio_wifi_platform_driver.driver;
	ret = device_bind_driver(wiphy->dev);
	if (ret != 0) {
		printk("%s: device_bind_driver failed (%d)\n", __func__, ret);
		goto finish;
	}
	SET_IEEE80211_DEV(hw, wiphy->dev);
	ret = ieee80211_register_hw(hw);
	if (ret < 0) {
		printk("%s: ieee80211_register_hw failed (%d)\n", __func__,
		       ret);
		goto finish;
	}
	spin_lock_bh(&dev->wiphys_lock);
	list_add_tail(&wiphy->node, &dev->wiphys);
	spin_unlock_bh(&dev->wiphys_lock);

// FIXME cleanup sequence
finish:
	return ret;
}

int virtio_wifi_do_start(struct virtio_wifi *dev, u32 wiphy_idx)
{
	struct virtio_wifi_msg *msg;
	struct virtio_wifi_hdr *req;

	int ret = 0;

	BUG_ON(!dev);

	printk("%s: starting wiphy %u\n", __func__, wiphy_idx);

	msg = virtio_wifi_msg_init(dev, VIRTIO_WIFI_CMD_START_WIPHY,
				   sizeof(struct virtio_wifi_hdr),
				   sizeof(struct virtio_wifi_hdr));

	req = msg->req;
	req->index = virtio_cpu_to_le(wiphy_idx, req->index);

	ret = virtio_wifi_msg_xfer(&dev->controlq, msg, 0);
	virtio_wifi_msg_release(msg);

	return ret;
}

int virtio_wifi_do_stop(struct virtio_wifi *dev, u32 wiphy_idx)
{
	struct virtio_wifi_msg *msg;
	struct virtio_wifi_hdr *req;

	int ret = 0;

	BUG_ON(!dev);

	printk("%s: stopping wiphy %u\n", __func__, wiphy_idx);

	msg = virtio_wifi_msg_init(dev, VIRTIO_WIFI_CMD_STOP_WIPHY,
				   sizeof(struct virtio_wifi_hdr),
				   sizeof(struct virtio_wifi_hdr));

	req = msg->req;
	req->index = virtio_cpu_to_le(wiphy_idx, req->index);

	ret = virtio_wifi_msg_xfer(&dev->controlq, msg, 0);
	virtio_wifi_msg_release(msg);

	return ret;
}

static inline void iface_from_vif(struct virtio_wifi_interface *iface,
				  struct ieee80211_vif *vif)
{
	iface->iftype = virtio_cpu_to_le(vif->type, iface->iftype);

	iface->cfg.aid = virtio_cpu_to_le(vif->cfg.aid, iface->cfg.aid);
	iface->cfg.eml_cap =
		virtio_cpu_to_le(vif->cfg.eml_cap, iface->cfg.eml_cap);
	iface->cfg.eml_med_sync_delay = virtio_cpu_to_le(
		vif->cfg.eml_med_sync_delay, iface->cfg.eml_med_sync_delay);
	iface->cfg.mld_capa_op =
		virtio_cpu_to_le(vif->cfg.mld_capa_op, iface->cfg.mld_capa_op);
	memcpy(iface->cfg.arp_addr_list, vif->cfg.arp_addr_list,
	       ARRAY_SIZE(vif->cfg.arp_addr_list));
	iface->cfg.arp_addr_cnt = virtio_cpu_to_le(vif->cfg.arp_addr_cnt,
						   iface->cfg.arp_addr_cnt);
	memcpy(iface->cfg.ssid, vif->cfg.ssid, ARRAY_SIZE(vif->cfg.ssid));
	iface->cfg.ssid_len =
		virtio_cpu_to_le(vif->cfg.ssid_len, iface->cfg.ssid_len);
	memcpy(iface->cfg.ap_addr, vif->cfg.ap_addr,
	       ARRAY_SIZE(vif->cfg.ap_addr));

	memcpy(iface->addr, vif->addr, ARRAY_SIZE(vif->addr));
}

int virtio_wifi_do_add_interface(struct virtio_wifi *dev, u32 wiphy_idx,
				 struct ieee80211_vif *vif)
{
	struct virtio_wifi_msg *msg;
	struct virtio_wifi_hdr *req;
	struct virtio_wifi_interface *wif = (void *)vif->drv_priv;
	iface_from_vif(wif, vif);

	int ret = 0;

	BUG_ON(!dev || !vif);

	printk("%s: adding interface to wiphy %u\n", __func__, wiphy_idx);

	msg = virtio_wifi_msg_init(dev, VIRTIO_WIFI_CMD_ADD_INTERFACE,
				   sizeof(struct virtio_wifi_interface),
				   sizeof(struct virtio_wifi_interface));

	req = msg->req;
	memcpy(msg->req, wif, sizeof(struct virtio_wifi_interface));
	req->index = virtio_cpu_to_le(wiphy_idx, req->index);
	// FIXME: prepopulate msg->req?
	req->code = virtio_cpu_to_le(VIRTIO_WIFI_CMD_ADD_INTERFACE, req->code);
	ret = virtio_wifi_msg_xfer(&dev->controlq, msg, 0);
	if (ret) {
		goto finish;
	}

	memcpy(wif, msg->resp, sizeof(struct virtio_wifi_interface));
	printk("%s: wiphy %u created vif %u type %d\n", __func__, wiphy_idx,
	       wif->index, vif->type);
	memcpy(vif->addr, wif->addr, ARRAY_SIZE(wif->addr));
	vif->driver_flags = virtio_le_to_cpu(wif->driver_flags);
	/* vif_from_iface(vif, wif); */
finish:
	virtio_wifi_msg_release(msg);
	return ret;
}

int virtio_wifi_do_remove_interface(struct virtio_wifi *dev, u32 wiphy_idx,
				    struct ieee80211_vif *vif)
{
	struct virtio_wifi_msg *msg;
	struct virtio_wifi_hdr *req;
	struct virtio_wifi_interface *wif =
		(struct virtio_wifi_interface *)vif->drv_priv;
	int ret = 0;

	BUG_ON(!dev || !vif);

	printk("%s: removing interface %u to wiphy %u\n", __func__, wif->index,
	       wiphy_idx);

	msg = virtio_wifi_msg_init(dev, VIRTIO_WIFI_CMD_REMOVE_INTERFACE,
				   sizeof(struct virtio_wifi_interface),
				   sizeof(struct virtio_wifi_hdr));

	memcpy(msg->req, vif->drv_priv, sizeof(struct virtio_wifi_interface));
	req = msg->req;
	req->index = virtio_cpu_to_le(wiphy_idx, req->index);
	// FIXME: prepopulate msg->req?
	req->code =
		virtio_cpu_to_le(VIRTIO_WIFI_CMD_REMOVE_INTERFACE, req->code);

	ret = virtio_wifi_msg_xfer(&dev->controlq, msg, 0);
	if (ret) {
		goto finish;
	}

	printk("%s: wiphy %u removed vif %u\n", __func__, wiphy_idx,
	       wif->index);
finish:
	virtio_wifi_msg_release(msg);
	return ret;
}

#define SUPPORTED_FILTERS			\
	(FIF_ALLMULTI |				\
	FIF_CONTROL |				\
	FIF_PSPOLL |				\
	FIF_OTHER_BSS |				\
	FIF_BCN_PRBRESP_PROMISC |		\
	FIF_PROBE_REQ |				\
	FIF_MCAST_ACTION |			\
	FIF_FCSFAIL)

int virtio_wifi_do_configure_filter(struct virtio_wifi *dev, u32 wiphy_idx,
				    unsigned int changed_flags,
				    unsigned int *total_flags, u64 multicast)
{
	struct virtio_wifi_msg *msg;
	struct virtio_wifi_configure_filter *req;
	int ret = 0;

	BUG_ON(!dev);

	/* printk("%s: wiphy %u\n", __func__, wiphy_idx); */

	*total_flags &= SUPPORTED_FILTERS;

	msg = virtio_wifi_msg_init(dev, VIRTIO_WIFI_CMD_CONFIGURE_RX_FILTER,
				   sizeof(struct virtio_wifi_configure_filter),
				   sizeof(struct virtio_wifi_configure_filter));

	req = msg->req;
	req->hdr.index = virtio_cpu_to_le(wiphy_idx, req->hdr.index);
	req->changed_flags =
		virtio_cpu_to_le(changed_flags, req->changed_flags);
	req->total_flags = virtio_cpu_to_le(*total_flags, req->total_flags);
	req->multicast = virtio_cpu_to_le(multicast, req->multicast);

	ret = virtio_wifi_msg_xfer(&dev->controlq, msg, 0);
	if (ret) {
		goto finish;
	}

	*total_flags = virtio_le_to_cpu(
		(*(struct virtio_wifi_configure_filter *)msg->resp).total_flags);
finish:
	virtio_wifi_msg_release(msg);
	return ret;
}

int virtio_wifi_do_tx(struct virtio_wifi_wiphy *wiphy, u32 if_index,
		      struct sk_buff *skb)
{
	struct virtio_wifi *dev = wiphy->virtio_wifi;
	struct virtio_wifi_msg *msg;
	struct virtio_wifi_cmd_request *req;
	struct virtio_wifi_vq *vq = &dev->txq;

	int num_sg;

	unsigned long timeout_jiffies = msecs_to_jiffies(500);

	int ret = 0;

	BUG_ON(!dev || !skb);

	printk("%s: bytes=%d datalen=%d truesize=%d skb_end_offset=%d\n",
	       __func__, skb->len, skb->data_len, skb->truesize,
	       skb_end_offset(skb));

	msg = virtio_wifi_msg_init(dev, 0, sizeof(struct virtio_wifi_hdr),
				   sizeof(struct virtio_wifi_hdr));

	req = msg->req;
	req->hdr.index = virtio_cpu_to_le(if_index, req->hdr.index);
	req->hdr.code = virtio_cpu_to_le((u32)skb->len, req->hdr.code);

	struct scatterlist out_sg[1];
	/* struct scatterlist data_out_sg[1]; */
	struct scatterlist in_sg[1];
	struct scatterlist *sgs[3];
	unsigned long flags;
	long timeout_ret;
	bool notify;

	sgs[0] = out_sg;
	sgs[1] = wiphy->sg;
	sgs[2] = in_sg;

	sg_init_one(out_sg, msg->req, msg->req_size);
	sg_init_table(wiphy->sg, skb_shinfo(skb)->nr_frags + 1);
	/* sg_set_buf(wiphy->sg, &req->hdr, sizeof(struct virtio_wifi_hdr)); */
	num_sg = skb_to_sgvec(skb, wiphy->sg, 0, skb->len);
	if (unlikely(num_sg < 0)) {
		printk("%s: num_sg == %d < 0\n", __func__, num_sg);
		virtio_wifi_msg_release(msg);
		return num_sg;
	}
	num_sg++;
	/* sg_init_one(data_out_sg, skb->head, skb_end_offset(skb)); */
	/* num_sg = skb_to_sgvec(skb, &data_out_sg[0], 0, skb->len); */
	sg_init_one(in_sg, msg->resp, msg->resp_cap);

	spin_lock_irqsave(&vq->lock, flags);

	ret = virtqueue_add_sgs(vq->vq, sgs, num_sg, 1, msg, GFP_ATOMIC);
	if (ret) {
		spin_unlock_irqrestore(&vq->lock, flags);
		printk("%s: virtqueue_add_sgs ret == %d \n", __func__, ret);
		/*
		 * Release in place of the response callback, which will never
		 * come.
		 */
		virtio_wifi_msg_release(msg);
		return ret;
	}

	notify = virtqueue_kick_prepare(vq->vq);

	spin_unlock_irqrestore(&vq->lock, flags);

	if (notify)
		virtqueue_notify(vq->vq);

	if (timeout_jiffies) {
		timeout_ret = wait_for_completion_interruptible_timeout(
			&msg->responded, timeout_jiffies);
		if (!timeout_ret || timeout_ret < 0)
			printk("%s: ETIMEDOUT timeout %ld\n", __func__,
			       timeout_ret);

		if (!timeout_ret)
			return -ETIMEDOUT;
		else if (timeout_ret < 0)
			return (int)timeout_ret;
	} else {
		ret = wait_for_completion_interruptible(&msg->responded);
		if (ret)
			return ret;
	}

	/* printk("%s: msg->resp_actual_size=%d: sizeof(struct virtio_wifi_hdr)=%ld\n", */
	/*        __func__, msg->resp_actual_size, sizeof(struct virtio_wifi_hdr)); */
	if (msg->resp_actual_size < sizeof(struct virtio_wifi_hdr))
		return -EINVAL;

	ret = virtio_wifi_msg_get_resp_errno(msg->resp);
	printk("%s: cmd = %s ret= %d\n", __func__,
	       cmd_to_s(virtio_le_to_cpu(
		       (*(struct virtio_wifi_hdr *)msg->req).code)),
	       ret);
	if (ret) {
		printk("%s: cmd = %s ret= %d\n", __func__,
		       cmd_to_s(virtio_le_to_cpu(
			       (*(struct virtio_wifi_hdr *)msg->req).code)),
		       ret);
		return ret;
	}

	/*
	 * There is not yet a case where returning a short message would make
	 * sense, so consider any deviation an error.
	 */
	if (msg->resp_actual_size != msg->resp_cap) {
		printk("%s: msg->resp_actual_size=%d: msg->resp_cap=%d\n",
		       __func__, msg->resp_actual_size, msg->resp_cap);
		return -EINVAL;
	}

	return ret;
}

static inline const char *chan_width_s(enum nl80211_chan_width w)
{
	switch (w) {
	case NL80211_CHAN_WIDTH_20_NOHT:
		return "20MHz_NOHT";
	case NL80211_CHAN_WIDTH_20:
		return "20MHz";
	case NL80211_CHAN_WIDTH_40:
		return "40MHz";
	case NL80211_CHAN_WIDTH_80:
		return "80MHz";
	case NL80211_CHAN_WIDTH_80P80:
		return "80+80MHz";
	case NL80211_CHAN_WIDTH_160:
		return "160MHz";
	case NL80211_CHAN_WIDTH_5:
		return "5MHz";
	case NL80211_CHAN_WIDTH_10:
		return "10MHz";
	case NL80211_CHAN_WIDTH_1:
		return "1MHz";
	case NL80211_CHAN_WIDTH_2:
		return "2MHz";
	case NL80211_CHAN_WIDTH_4:
		return "4MHz";
	case NL80211_CHAN_WIDTH_8:
		return "8MHz";
	case NL80211_CHAN_WIDTH_16:
		return "16MHz";
	case NL80211_CHAN_WIDTH_320:
		return "320MHz";
	default:
		return "invalid?";
	}
};

int virtio_wifi_do_set_channel(struct virtio_wifi_wiphy *wiphy,
				struct cfg80211_chan_def *chandef)
{
	struct virtio_wifi *dev;
	struct virtio_wifi_msg *msg;
	struct virtio_wifi_channel *req;
	int ret = 0;

	BUG_ON(!wiphy);

	dev = wiphy->virtio_wifi;
	printk("%s: Requesting to set channel: %d MHz width: %s(%d)\n",
	       __func__, chandef->chan->center_freq,
	       chan_width_s(chandef->width), chandef->width);

	msg = virtio_wifi_msg_init(dev, VIRTIO_WIFI_CMD_SET_CHANNEL,
				   sizeof(struct virtio_wifi_channel),
				   sizeof(struct virtio_wifi_hdr));

	req = msg->req;
	req->hdr.index = virtio_cpu_to_le(wiphy->index, req->hdr.index);
	req->width = virtio_cpu_to_le(chandef->width, req->width);
	req->band = virtio_cpu_to_le(chandef->chan->band, req->band);
	req->center_freq =
		virtio_cpu_to_le(chandef->chan->center_freq, req->center_freq);
	req->center_freq1 =
		virtio_cpu_to_le(chandef->center_freq1, req->center_freq1);
	req->center_freq2 =
		virtio_cpu_to_le(chandef->center_freq2, req->center_freq2);

	ret = virtio_wifi_msg_xfer(&dev->controlq, msg, 0);
	if (ret) {
		goto finish;
	}
	printk("%s: Successfully set channel: %d MHz width: %d\n", __func__,
	       chandef->chan->center_freq, chandef->width);

finish:
	virtio_wifi_msg_release(msg);
	return ret;
}
