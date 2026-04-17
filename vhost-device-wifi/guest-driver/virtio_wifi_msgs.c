/* SPDX-License-Identifier: GPL-2.0+ */
/* Author: Manos Pitsidianakis <manos.pitsidianakis@linaro.org> */

#include <linux/virtio.h>
#include <linux/string.h>
#include <linux/rtnetlink.h>
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
	case VIRTIO_WIFI_S_NO_LINK:
		return -ENOLINK;
	case VIRTIO_WIFI_S_NET_DOWN:
		return -ENETDOWN;
	case VIRTIO_WIFI_S_NOT_CONN:
		return -ENOTCONN;
	case VIRTIO_WIFI_S_IO_ERR:
	default:
		return -EIO;
	}
}

/**
 * virtio_wifi_msg_init() - Allocate and initialize requestq message.
 * @virtio_wifi: device data
 * @msg_type: virtio_wifi message type
 * @req_size: size of request buffer to be allocated
 * @resp_cap: size of response buffer to be allocated
 *
 * Initializes the message refcnt to 2. The refcnt will be decremented once in
 * the virtqueue callback, and once in the thread waiting on the message (on
 * completion or timeout).
 *
 * Context: Process context.
 * Return: non-NULL on success.
 */
static struct virtio_wifi_msg *
virtio_wifi_msg_init(struct virtio_wifi *virtio_wifi, u32 msg_type,
		     unsigned int req_size, unsigned int resp_cap)
{
	struct virtio_wifi_hdr *req_head;
	struct virtio_wifi_msg *msg;

	msg = kzalloc(sizeof(*msg), GFP_KERNEL);
	if (!msg)
		return ERR_PTR(-ENOMEM);

	init_completion(&msg->responded);

	msg->req = kzalloc(req_size, GFP_KERNEL);
	if (!msg->req)
		goto err_free_msg;

	req_head = msg->req;

	msg->resp = kzalloc(resp_cap, GFP_KERNEL);
	if (!msg->resp)
		goto err_free_msg_req;

	msg->virtio_wifi = virtio_wifi;
	msg->req_size = req_size;
	msg->resp_cap = resp_cap;

	refcount_set(&msg->refcnt, 2);

	req_head->code = virtio_cpu_to_le(msg_type, req_head->code);

	return msg;

err_free_msg_req:
	kfree(msg->req);

err_free_msg:
	kfree(msg);

	return ERR_PTR(-ENOMEM);
}

void virtio_wifi_msg_release(struct virtio_wifi_msg *msg)
{
	struct device *dev;

	if (refcount_dec_and_test(&msg->refcnt)) {
		dev = &msg->virtio_wifi->virtio_dev->dev;

		kfree(msg->req);
		kfree(msg->resp);
		kfree(msg);
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

	/* printk("%s: msg->resp_actual_size=%d: sizeof(struct virtio_wifi_hdr)=%ld\n", */
	/*        __func__, msg->resp_actual_size, sizeof(struct virtio_wifi_hdr)); */
	if (msg->resp_actual_size < sizeof(struct virtio_wifi_hdr))
		return -EINVAL;

	ret = virtio_wifi_msg_get_resp_errno(msg->resp);
	/* printk("%s: cmd = %s ret= %d\n", __func__, */
	/*        cmd_to_s(virtio_le_to_cpu( */
	/* 	       (*(struct virtio_wifi_hdr *)msg->req).code)), */
	/*        ret); */
	if (ret)
		return ret;

	/*
	 * There is not yet a case where returning a short message would make
	 * sense, so consider any deviation an error.
	 */
	if (msg->resp_actual_size != msg->resp_cap) {
		printk("%s: msg->resp_actual_size=%d: msg->resp_cap=%d\n",
		       __func__, msg->resp_actual_size, msg->resp_cap);
		return -EINVAL;
	}

	return 0;
}

static int virtio_wifi_get_interfaces(struct virtio_wifi *dev,
				      struct virtio_wifi_interface *ifaces,
				      u32 num_of_ifaces)
{
	struct virtio_wifi_msg *msg;

	int ret = 0;

	BUG_ON(!dev || !ifaces);

	msg = virtio_wifi_msg_init(dev, VIRTIO_WIFI_CMD_GET_INTERFACES,
				   sizeof(struct virtio_wifi_hdr),
				   sizeof(struct virtio_wifi_interface) *
					   num_of_ifaces);
	if (IS_ERR(msg))
		return PTR_ERR(msg);

	ret = virtio_wifi_msg_xfer(&dev->controlq, msg, 0);

	if (ret) {
		printk("%s: xfer returned %d\n", __func__, ret);
		goto finish;
	}

	memcpy(ifaces, msg->resp,
	       sizeof(struct virtio_wifi_interface) * num_of_ifaces);

finish:
	virtio_wifi_msg_release(msg);

	return ret;
}

int virtio_wifi_get_band(struct virtio_wifi *dev, enum nl80211_band band_kind,
			 struct virtio_wifi_band *band)
{
	struct virtio_wifi_msg *msg;
	struct virtio_wifi_hdr *req;

	int ret = 0;

	BUG_ON(!dev || !band);

	msg = virtio_wifi_msg_init(dev, VIRTIO_WIFI_CMD_GET_BAND,
				   sizeof(struct virtio_wifi_hdr),
				   sizeof(struct virtio_wifi_band));
	if (IS_ERR(msg))
		return PTR_ERR(msg);

	req = msg->req;
	req->index = virtio_cpu_to_le(band_kind, req->index);

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

int virtio_wifi_get_band_bitrate(struct virtio_wifi *dev,
				 enum nl80211_band band_kind, u32 bitrate_idx,
				 struct virtio_wifi_band_bitrate *bitrate)
{
	struct virtio_wifi_msg *msg;
	struct virtio_wifi_cmd_get_band_bitrate_request *req;

	int ret = 0;

	BUG_ON(!dev || !bitrate);

	msg = virtio_wifi_msg_init(
		dev, VIRTIO_WIFI_CMD_GET_BAND_BITRATE,
		sizeof(struct virtio_wifi_cmd_get_band_bitrate_request),
		sizeof(struct virtio_wifi_band_bitrate));
	if (IS_ERR(msg))
		return PTR_ERR(msg);

	req = msg->req;
	req->hdr.index = virtio_cpu_to_le(band_kind, req->hdr.index);
	req->bitrate_index = virtio_cpu_to_le(bitrate_idx, req->bitrate_index);

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

int virtio_wifi_get_band_channel(struct virtio_wifi *dev,
				 enum nl80211_band band_kind, u32 channel_idx,
				 struct virtio_wifi_channel *channel)
{
	struct virtio_wifi_msg *msg;
	struct virtio_wifi_cmd_get_band_channel_request *req;

	int ret = 0;

	BUG_ON(!dev || !channel);

	msg = virtio_wifi_msg_init(
		dev, VIRTIO_WIFI_CMD_GET_BAND_CHANNEL,
		sizeof(struct virtio_wifi_cmd_get_band_channel_request),
		sizeof(struct virtio_wifi_channel));
	if (IS_ERR(msg))
		return PTR_ERR(msg);

	req = msg->req;
	req->hdr.index = virtio_cpu_to_le(band_kind, req->hdr.index);
	req->channel_index = virtio_cpu_to_le(channel_idx, req->channel_index);

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

int virtio_wifi_get_cipher_suites(struct virtio_wifi *dev, u32 **cipher_suites,
				  int n_cipher_suites)
{
	struct virtio_wifi_msg *msg;
	struct virtio_wifi_hdr *resp;

	int ret = 0;

	BUG_ON(!dev || !cipher_suites);

	msg = virtio_wifi_msg_init(dev, VIRTIO_WIFI_CMD_GET_CIPHERSUITES,
				   sizeof(struct virtio_wifi_hdr),
				   sizeof(struct virtio_wifi_hdr) +
					   n_cipher_suites * sizeof(u32));
	if (IS_ERR(msg))
		return PTR_ERR(msg);

	ret = virtio_wifi_msg_xfer(&dev->controlq, msg, 0);
	if (ret) {
		printk("%s: xfer returned %d\n", __func__, ret);
		goto finish;
	}
	resp = msg->resp;
	*cipher_suites = devm_kzalloc(&dev->virtio_dev->dev,
				      n_cipher_suites * sizeof(u32),
				      GFP_KERNEL);
	if (!*cipher_suites) {
		ret = -ENOMEM;
		goto finish;
	}
	memcpy(*cipher_suites, msg->resp + sizeof(struct virtio_wifi_hdr),
	       n_cipher_suites * sizeof(u32));

finish:
	virtio_wifi_msg_release(msg);

	return ret;
}

int virtio_wifi_do_register_wiphy(struct virtio_wifi *dev)
{
	struct virtio_wifi_wiphy *wiphy = NULL;
	struct virtio_wifi_interface *ifaces = NULL;
	struct wireless_dev *wdev;
	u32 num_of_ifaces = 0;

	int ret = 0;

	printk("%s: registering wiphy\n", __func__);

	wiphy = virtio_wifi_cfg80211_make_wiphy(dev);
	if (wiphy == NULL) {
		ret = -ENODEV;
		goto finish;
	}

	dev->wiphy = wiphy;

	virtio_cread_le(dev->virtio_dev, struct virtio_wifi_config,
			num_of_ifaces, &num_of_ifaces);

	if (num_of_ifaces > 0) {
		ifaces = kzalloc(sizeof(*ifaces) * num_of_ifaces, GFP_KERNEL);
		ret = virtio_wifi_get_interfaces(dev, ifaces, num_of_ifaces);
		if (ret) {
			kfree(ifaces);
			goto finish;
		}
		for (u32 i = 0; i < num_of_ifaces; i++) {
			rtnl_lock();
			wdev = virtio_wifi_cfg80211_add_intf(
				wiphy->wiphy, NET_NAME_PREDICTABLE, &ifaces[i]);
			rtnl_unlock();
			if (IS_ERR(wdev)) {
				ret = PTR_ERR(wdev);
				goto finish;
			}
		}
	}
// FIXME cleanup sequence
finish:
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

int virtio_wifi_do_add_interface(struct virtio_wifi *dev,
				 enum nl80211_iftype type,
				 struct virtio_wifi_interface *retval)
{
	struct virtio_wifi_msg *msg;
	struct virtio_wifi_interface *wif;
	u8 addr[ETH_ALEN] = { 0 };
	int ret = 0;

	BUG_ON(!dev || !retval);

	printk("%s: adding interface to wiphy\n", __func__);

	msg = virtio_wifi_msg_init(dev, VIRTIO_WIFI_CMD_NEW_INTERFACE,
				   sizeof(struct virtio_wifi_interface),
				   sizeof(struct virtio_wifi_interface));
	if (IS_ERR(msg))
		return PTR_ERR(msg);

	wif = msg->req;
	wif->iftype = virtio_cpu_to_le(type, wif->iftype);
	eth_random_addr(addr);
	ether_addr_copy(wif->addr, addr);

	ret = virtio_wifi_msg_xfer(&dev->controlq, msg, 0);
	if (ret) {
		goto finish;
	}

	wif = msg->resp;
	printk("%s: wiphy created vif %u type %d\n", __func__, wif->index,
	       type);
	memcpy(retval, wif, sizeof(*wif));
finish:
	virtio_wifi_msg_release(msg);
	return ret;
}

int virtio_wifi_do_remove_interface(struct virtio_wifi *dev, u32 if_index)
{
	struct virtio_wifi_msg *msg;
	struct virtio_wifi_interface *ifreq;
	int ret = 0;

	BUG_ON(!dev);

	printk("%s: removing interface %u to wiphy\n", __func__, if_index);

	msg = virtio_wifi_msg_init(dev, VIRTIO_WIFI_CMD_DELETE_INTERFACE,
				   sizeof(struct virtio_wifi_interface),
				   sizeof(struct virtio_wifi_hdr));
	if (IS_ERR(msg))
		return PTR_ERR(msg);

	ifreq = msg->req;

	ifreq->index = virtio_cpu_to_le(if_index, ifreq->index);

	ret = virtio_wifi_msg_xfer(&dev->controlq, msg, 0);
	if (ret) {
		goto finish;
	}

	printk("%s: wiphy removed vif %u\n", __func__, if_index);
finish:
	virtio_wifi_msg_release(msg);
	return ret;
}

int virtio_wifi_do_tx(struct virtio_wifi_wiphy *wiphy, u32 if_index,
		      struct sk_buff *skb)
{
	struct virtio_wifi *dev = wiphy->virtio_wifi;
	struct virtio_wifi_msg *msg;
	struct virtio_wifi_hdr *req;
	struct virtio_wifi_vq *vq = &dev->txq;

	unsigned long timeout_jiffies = msecs_to_jiffies(500);

	int ret = 0;

	BUG_ON(!dev || !skb);

	msg = virtio_wifi_msg_init(dev, VIRTIO_WIFI_TX_802_3,
				   sizeof(struct virtio_wifi_hdr) + skb->len,
				   sizeof(struct virtio_wifi_hdr));
	if (IS_ERR(msg))
		return PTR_ERR(msg);

	req = msg->req;
	req->index = virtio_cpu_to_le(if_index, req->index);

	struct scatterlist out_sg[1];
	struct scatterlist in_sg[1];
	struct scatterlist *sgs[2];
	unsigned long flags;
	long timeout_ret;
	bool notify;

	sgs[0] = out_sg;
	sgs[1] = in_sg;

	sg_init_one(out_sg, msg->req, msg->req_size);
	memcpy(msg->req + sizeof(struct virtio_wifi_hdr), skb->data, skb->len);
	dev_kfree_skb(skb);
	/* sg_set_buf(wiphy->sg, &req->hdr, sizeof(struct virtio_wifi_hdr)); */
	/* sg_init_one(data_out_sg, skb->head, skb_end_offset(skb)); */
	/* num_sg = skb_to_sgvec(skb, &data_out_sg[0], 0, skb->len); */
	sg_init_one(in_sg, msg->resp, msg->resp_cap);

	/* printk("%s: bytes=%d datalen=%d truesize=%d skb_end_offset=%d num_sg=%d\n", */
	/*        __func__, skb->len, skb->data_len, skb->truesize, */
	/*        skb_end_offset(skb), num_sg); */

	spin_lock_irqsave(&vq->lock, flags);

	ret = virtqueue_add_sgs(vq->vq, sgs, 1, 1, msg, GFP_ATOMIC);
	if (ret) {
		spin_unlock_irqrestore(&vq->lock, flags);
		printk("%s: virtqueue_add_sgs ret == %d \n", __func__, ret);
		/*
		 * Release in place of the response callback, which will never
		 * come.
		 */
		goto finish;
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

		if (!timeout_ret) {
			ret = -ETIMEDOUT;
			goto finish;
		} else if (timeout_ret < 0) {
			ret = (int)timeout_ret;
			goto finish;
		}
	} else {
		ret = wait_for_completion_interruptible(&msg->responded);
		if (ret) {
			goto finish;
		}
	}

	/* printk("%s: msg->resp_actual_size=%d: sizeof(struct virtio_wifi_hdr)=%ld\n", */
	/*        __func__, msg->resp_actual_size, sizeof(struct virtio_wifi_hdr)); */
	if (msg->resp_actual_size < sizeof(struct virtio_wifi_hdr)) {
		ret = -EINVAL;
		goto finish;
	}

	ret = virtio_wifi_msg_get_resp_errno(msg->resp);
	/* printk("%s: cmd = %s ret= %d\n", __func__, */
	/*        cmd_to_s(virtio_le_to_cpu( */
	/* 	       (*(struct virtio_wifi_hdr *)msg->req).code)), */
	/*        ret); */
	if (ret) {
		printk("%s: cmd = %s ret= %d\n", __func__,
		       cmd_to_s(virtio_le_to_cpu(
			       (*(struct virtio_wifi_hdr *)msg->req).code)),
		       ret);
		goto finish;
	}

	/*
	 * There is not yet a case where returning a short message would make
	 * sense, so consider any deviation an error.
	 */
	if (msg->resp_actual_size != msg->resp_cap) {
		printk("%s: msg->resp_actual_size=%d: msg->resp_cap=%d\n",
		       __func__, msg->resp_actual_size, msg->resp_cap);
		ret = -EINVAL;
		goto finish;
	}

finish:
	virtio_wifi_msg_release(msg);
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

int virtio_wifi_do_trigger_scan(struct virtio_wifi_wiphy *wiphy, u32 if_index,
				struct cfg80211_scan_request *req)

{
	struct virtio_wifi *dev;
	struct virtio_wifi_msg *msg;
	struct virtio_wifi_hdr *hdr;
	int ret = 0;

	BUG_ON(!wiphy);

	dev = wiphy->virtio_wifi;
	/* printk("%s: Triggering scan on vif %d\n", __func__, if_index); */

	msg = virtio_wifi_msg_init(dev, VIRTIO_WIFI_CMD_TRIGGER_SCAN,
				   sizeof(struct virtio_wifi_hdr),
				   sizeof(struct virtio_wifi_hdr));
	if (IS_ERR(msg))
		return PTR_ERR(msg);

	hdr = msg->req;
	hdr->index = virtio_cpu_to_le(if_index, hdr->index);

	ret = virtio_wifi_msg_xfer(&dev->controlq, msg, 0);
	if (ret) {
		goto finish;
	}

finish:
	virtio_wifi_msg_release(msg);
	return ret;
}

int virtio_wifi_do_auth(struct virtio_wifi_wiphy *wiphy, u32 if_index,
			const struct element *ssid_elem,
			struct cfg80211_auth_request *auth_req)
{
	struct virtio_wifi *dev;
	struct virtio_wifi_msg *msg;
	struct virtio_wifi_hdr *hdr;
	struct virtio_wifi_authentication_request *req;
	int ret = 0;

	BUG_ON(!wiphy);

	dev = wiphy->virtio_wifi;

	msg = virtio_wifi_msg_init(
		dev, VIRTIO_WIFI_CMD_AUTHENTICATE,
		sizeof(struct virtio_wifi_authentication_request) +
			auth_req->ie_len,
		sizeof(struct virtio_wifi_hdr));
	if (IS_ERR(msg))
		return PTR_ERR(msg);

	hdr = msg->req;
	hdr->index = virtio_cpu_to_le(if_index, hdr->index);
	req = msg->req;
	req->auth_type = virtio_cpu_to_le(auth_req->auth_type, req->auth_type);
	ether_addr_copy(req->mac, auth_req->bss->bssid);
	req->freq = virtio_cpu_to_le(auth_req->bss->channel->center_freq,
				     req->freq);
	req->freq_offset = virtio_cpu_to_le(auth_req->bss->channel->freq_offset,
					    req->freq_offset);
	req->ielen = virtio_cpu_to_le(auth_req->ie_len, req->ielen);

	memcpy(req->ssid, ssid_elem->data,
	       MIN(ssid_elem->datalen, IEEE80211_MAX_SSID_LEN));
	req->ssid_len = virtio_cpu_to_le(
		MIN(ssid_elem->datalen, IEEE80211_MAX_SSID_LEN), req->ssid_len);
	if (auth_req->ie_len > 0) {
		memcpy(msg->req +
			       sizeof(struct virtio_wifi_authentication_request),
		       auth_req->ie, auth_req->ie_len);
	}

	ret = virtio_wifi_msg_xfer(&dev->controlq, msg, 0);
	if (ret) {
		goto finish;
	}

finish:
	virtio_wifi_msg_release(msg);
	return ret;
}

int virtio_wifi_do_assoc(struct virtio_wifi_wiphy *wiphy, u32 if_index,
			 const struct element *ssid_elem,
			 struct cfg80211_assoc_request *assoc_req)
{
	struct virtio_wifi *dev;
	struct virtio_wifi_msg *msg;
	struct virtio_wifi_hdr *hdr;
	struct virtio_wifi_association_request *req;
	int ret = 0;

	BUG_ON(!wiphy);

	dev = wiphy->virtio_wifi;

	msg = virtio_wifi_msg_init(
		dev, VIRTIO_WIFI_CMD_ASSOCIATE,
		sizeof(struct virtio_wifi_association_request) +
			assoc_req->ie_len,
		sizeof(struct virtio_wifi_hdr));
	if (IS_ERR(msg))
		return PTR_ERR(msg);

	hdr = msg->req;
	hdr->index = virtio_cpu_to_le(if_index, hdr->index);
	req = msg->req;
	/* ether_addr_copy(req->mac, assoc_req->bss->bssid); */
	req->ielen = virtio_cpu_to_le(assoc_req->ie_len, req->ielen);
	ether_addr_copy(req->mac, assoc_req->bss->bssid);
	req->freq = virtio_cpu_to_le(assoc_req->bss->channel->center_freq,
				     req->freq);
	req->freq_offset = virtio_cpu_to_le(
		assoc_req->bss->channel->freq_offset, req->freq_offset);

	memcpy(req->ssid, ssid_elem->data,
	       MIN(ssid_elem->datalen, IEEE80211_MAX_SSID_LEN));
	req->ssid_len = virtio_cpu_to_le(
		MIN(ssid_elem->datalen, IEEE80211_MAX_SSID_LEN), req->ssid_len);
	if (assoc_req->ie_len > 0) {
		memcpy(msg->req +
			       sizeof(struct virtio_wifi_association_request),
		       assoc_req->ie, assoc_req->ie_len);
	}
	req->wpa_versions = virtio_cpu_to_le(assoc_req->crypto.wpa_versions,
					     req->wpa_versions);
	req->cipher_group = virtio_cpu_to_le(assoc_req->crypto.cipher_group,
					     req->cipher_group);
	req->n_akm_suites = virtio_cpu_to_le(assoc_req->crypto.n_akm_suites,
					     req->n_akm_suites);
	memcpy(req->akm_suites, assoc_req->crypto.akm_suites,
	       sizeof(u32) * assoc_req->crypto.n_akm_suites);
	req->n_ciphers_pairwise = virtio_cpu_to_le(
		assoc_req->crypto.n_ciphers_pairwise, req->n_ciphers_pairwise);
	memcpy(req->ciphers_pairwise, assoc_req->crypto.ciphers_pairwise,
	       sizeof(u32) * assoc_req->crypto.n_ciphers_pairwise);

	ret = virtio_wifi_msg_xfer(&dev->controlq, msg, 0);
	if (ret) {
		goto finish;
	}

finish:
	virtio_wifi_msg_release(msg);
	return ret;
}

int virtio_wifi_do_add_key(struct virtio_wifi_wiphy *wiphy, u32 if_index,
			   u8 key_index, bool pairwise, const u8 *mac_addr,
			   struct key_params *params)
{
	struct virtio_wifi *dev;
	struct virtio_wifi_msg *msg;
	struct virtio_wifi_hdr *hdr;
	struct virtio_wifi_key *key;
	int ret = 0;

	BUG_ON(!wiphy);

	dev = wiphy->virtio_wifi;
	msg = virtio_wifi_msg_init(dev, VIRTIO_WIFI_CMD_ADD_KEY,
				   sizeof(struct virtio_wifi_key) +
					   params->key_len + params->seq_len,
				   sizeof(struct virtio_wifi_hdr));
	if (IS_ERR(msg))
		return PTR_ERR(msg);

	hdr = msg->req;
	hdr->index = virtio_cpu_to_le(if_index, hdr->index);

	key = msg->req;
	key->key_index = key_index;
	key->pairwise = pairwise;
	if (mac_addr)
		ether_addr_copy(key->mac, mac_addr);
	key->vlan_id = virtio_cpu_to_le(params->vlan_id, key->vlan_id);
	key->cipher = virtio_cpu_to_le(params->cipher, key->cipher);
	key->mode = virtio_cpu_to_le(params->mode, key->mode);
	key->key_len = virtio_cpu_to_le(params->key_len, key->key_len);
	key->seq_len = virtio_cpu_to_le(params->seq_len, key->seq_len);
	void *key_ptr = msg->req + sizeof(struct virtio_wifi_key);
	memcpy(key_ptr, params->key, params->key_len);
	void *seq_ptr = key_ptr + params->key_len;
	memcpy(seq_ptr, params->seq, params->seq_len);

	ret = virtio_wifi_msg_xfer(&dev->controlq, msg, 0);
	if (ret) {
		goto finish;
	}

finish:
	virtio_wifi_msg_release(msg);
	return ret;
}

int virtio_wifi_do_del_key(struct virtio_wifi_wiphy *wiphy, u32 if_index,
			   u8 key_index, bool pairwise, const u8 *mac_addr)
{
	struct virtio_wifi *dev;
	struct virtio_wifi_msg *msg;
	struct virtio_wifi_hdr *hdr;
	struct virtio_wifi_key *key;
	int ret = 0;

	BUG_ON(!wiphy);

	dev = wiphy->virtio_wifi;
	msg = virtio_wifi_msg_init(dev, VIRTIO_WIFI_CMD_DEL_KEY,
				   sizeof(struct virtio_wifi_key),
				   sizeof(struct virtio_wifi_hdr));
	if (IS_ERR(msg))
		return PTR_ERR(msg);

	hdr = msg->req;
	hdr->index = virtio_cpu_to_le(if_index, hdr->index);

	key = msg->req;
	key->key_index = key_index;
	key->pairwise = pairwise;
	if (pairwise && mac_addr)
		ether_addr_copy(key->mac, mac_addr);

	ret = virtio_wifi_msg_xfer(&dev->controlq, msg, 0);
	if (ret) {
		goto finish;
	}

finish:
	virtio_wifi_msg_release(msg);
	return ret;
}

int virtio_wifi_do_set_default_key(struct virtio_wifi_wiphy *wiphy,
				   u32 if_index, u8 key_index, bool unicast,
				   bool multicast)
{
	struct virtio_wifi *dev;
	struct virtio_wifi_msg *msg;
	struct virtio_wifi_hdr *hdr;
	struct virtio_wifi_key *key;
	int ret = 0;

	BUG_ON(!wiphy);

	dev = wiphy->virtio_wifi;
	msg = virtio_wifi_msg_init(dev, VIRTIO_WIFI_CMD_SET_KEY,
				   sizeof(struct virtio_wifi_key),
				   sizeof(struct virtio_wifi_hdr));
	if (IS_ERR(msg))
		return PTR_ERR(msg);

	hdr = msg->req;
	hdr->index = virtio_cpu_to_le(if_index, hdr->index);

	key = msg->req;
	key->key_index = key_index;
	key->unicast = unicast;
	key->multicast = multicast;

	ret = virtio_wifi_msg_xfer(&dev->controlq, msg, 0);
	if (ret) {
		goto finish;
	}

finish:
	virtio_wifi_msg_release(msg);
	return ret;
}
int virtio_wifi_do_change_station(struct virtio_wifi_wiphy *wiphy, u32 if_index,
				  const u8 *mac,
				  struct station_parameters *params)
{
	struct virtio_wifi *dev;
	struct virtio_wifi_msg *msg;
	struct virtio_wifi_hdr *hdr;
	struct virtio_wifi_change_station *sta;
	int ret = 0;

	BUG_ON(!wiphy);

	dev = wiphy->virtio_wifi;
	msg = virtio_wifi_msg_init(dev, VIRTIO_WIFI_CMD_SET_STATION,
				   sizeof(struct virtio_wifi_change_station),
				   sizeof(struct virtio_wifi_hdr));
	if (IS_ERR(msg))
		return PTR_ERR(msg);

	hdr = msg->req;
	hdr->index = virtio_cpu_to_le(if_index, hdr->index);

	sta = msg->req;
	sta->mask = virtio_cpu_to_le(params->sta_flags_mask, sta->mask);
	sta->set = virtio_cpu_to_le(params->sta_flags_set, sta->set);
	if (mac)
		ether_addr_copy(sta->mac, mac);

	ret = virtio_wifi_msg_xfer(&dev->controlq, msg, 0);
	if (ret) {
		goto finish;
	}

finish:
	virtio_wifi_msg_release(msg);
	return ret;
}

int virtio_wifi_do_connect(struct virtio_wifi_wiphy *wiphy, u32 if_index,
			   struct cfg80211_connect_params *sme)
{
	struct virtio_wifi *dev;
	struct virtio_wifi_msg *msg;
	struct virtio_wifi_hdr *hdr;
	struct virtio_wifi_authentication_request *req;
	int ret = 0;

	BUG_ON(!wiphy);

	dev = wiphy->virtio_wifi;

	msg = virtio_wifi_msg_init(
		dev, VIRTIO_WIFI_CMD_CONNECT,
		sizeof(struct virtio_wifi_authentication_request) + sme->ie_len,
		sizeof(struct virtio_wifi_hdr));
	if (IS_ERR(msg))
		return PTR_ERR(msg);

	hdr = msg->req;
	hdr->index = virtio_cpu_to_le(if_index, hdr->index);
	req = msg->req;
	req->auth_type = virtio_cpu_to_le(sme->auth_type, req->auth_type);
	if (sme->bssid) {
		ether_addr_copy(req->mac, sme->bssid);
	} else if (sme->bssid_hint) {
		ether_addr_copy(req->mac, sme->bssid_hint);
	}
	if (sme->channel) {
		req->freq =
			virtio_cpu_to_le(sme->channel->center_freq, req->freq);
		req->freq_offset = virtio_cpu_to_le(sme->channel->freq_offset,
						    req->freq_offset);
	}
	req->ielen = virtio_cpu_to_le(sme->ie_len, req->ielen);
	if (sme->ie_len > 0) {
		memcpy(msg->req +
			       sizeof(struct virtio_wifi_authentication_request),
		       sme->ie, sme->ie_len);
	}

	memcpy(req->ssid, sme->ssid,
	       MIN(sme->ssid_len, IEEE80211_MAX_SSID_LEN));
	req->ssid_len = virtio_cpu_to_le(
		MIN(sme->ssid_len, IEEE80211_MAX_SSID_LEN), req->ssid_len);

	req->privacy = sme->privacy;
	req->wpa_versions =
		virtio_cpu_to_le(sme->crypto.wpa_versions, req->wpa_versions);
	req->cipher_group =
		virtio_cpu_to_le(sme->crypto.cipher_group, req->cipher_group);
	req->n_akm_suites =
		virtio_cpu_to_le(sme->crypto.n_akm_suites, req->n_akm_suites);
	memcpy(req->akm_suites, sme->crypto.akm_suites,
	       sizeof(u32) * sme->crypto.n_akm_suites);
	req->n_ciphers_pairwise = virtio_cpu_to_le(
		sme->crypto.n_ciphers_pairwise, req->n_ciphers_pairwise);
	memcpy(req->ciphers_pairwise, sme->crypto.ciphers_pairwise,
	       sizeof(u32) * sme->crypto.n_ciphers_pairwise);

	ret = virtio_wifi_msg_xfer(&dev->controlq, msg, 0);
	if (ret) {
		goto finish;
	}

finish:
	virtio_wifi_msg_release(msg);
	return ret;
}

int virtio_wifi_do_disconnect(struct virtio_wifi_wiphy *wiphy, u32 if_index,
			      u32 cmd, const u8 *mac, const u8 *ie,
			      size_t ie_len, u16 reason_code,
			      bool local_state_change)
{
	struct virtio_wifi *dev;
	struct virtio_wifi_msg *msg;
	struct virtio_wifi_hdr *hdr;
	struct virtio_wifi_disconnect_request *req;
	int ret = 0;

	BUG_ON(!wiphy || !(cmd == VIRTIO_WIFI_CMD_DEAUTHENTICATE ||
			   cmd == VIRTIO_WIFI_CMD_DISASSOCIATE));

	dev = wiphy->virtio_wifi;

	msg = virtio_wifi_msg_init(
		dev, cmd,
		sizeof(struct virtio_wifi_disconnect_request) + ie_len,
		sizeof(struct virtio_wifi_hdr));
	if (IS_ERR(msg))
		return PTR_ERR(msg);

	hdr = msg->req;
	hdr->index = virtio_cpu_to_le(if_index, hdr->index);
	req = msg->req;
	req->reason_code = virtio_cpu_to_le(reason_code, req->reason_code);

	if (mac) {
		ether_addr_copy(req->mac, mac);
	}

	if (ie && ie_len > 0) {
		req->ie_len = virtio_cpu_to_le(ie_len, req->ie_len);
		memcpy(msg->req + sizeof(struct virtio_wifi_disconnect_request),
		       ie, ie_len);
	}

	ret = virtio_wifi_msg_xfer(&dev->controlq, msg, 0);
	if (ret) {
		goto finish;
	}

finish:
	virtio_wifi_msg_release(msg);
	return ret;
}

int virtio_wifi_do_if_oper(struct virtio_wifi_wiphy *wiphy, u32 if_index,
			   enum virtio_wifi_if_operstate flag, bool value)
{
	struct virtio_wifi *dev;
	struct virtio_wifi_msg *msg;
	struct virtio_wifi_hdr *hdr;
	struct virtio_wifi_set_if_operstate *req;

	int ret = 0;

	BUG_ON(!wiphy);

	dev = wiphy->virtio_wifi;

	msg = virtio_wifi_msg_init(dev, VIRTIO_WIFI_CMD_IF_OPERSTATE,
				   sizeof(struct virtio_wifi_set_if_operstate),
				   sizeof(struct virtio_wifi_hdr));
	if (IS_ERR(msg))
		return PTR_ERR(msg);

	hdr = msg->req;
	hdr->index = virtio_cpu_to_le(if_index, hdr->index);
	req = msg->req;

	req->flags = virtio_cpu_to_le(value ? 1 << flag : 0, req->flags);
	req->change_mask = virtio_cpu_to_le(1 << flag, req->change_mask);

	ret = virtio_wifi_msg_xfer(&dev->controlq, msg, 0);
	if (ret) {
		goto finish;
	}

finish:
	virtio_wifi_msg_release(msg);
	return ret;
}
