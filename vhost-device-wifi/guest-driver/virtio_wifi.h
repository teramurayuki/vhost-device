/* SPDX-License-Identifier: GPL-2.0+ */
/* Author: Manos Pitsidianakis <manos.pitsidianakis@linaro.org> */

/*
 * Virtio-wifi structures & functions declarations.
 */

#ifndef __VIRTIO_WIFI_H
#define __VIRTIO_WIFI_H

#include <linux/virtio_config.h>
#include <linux/platform_device.h>
#include <net/mac80211.h>

#include "virtio_wifi_uapi.h"

extern struct class *virtio_wifi_class;

extern struct platform_driver virtio_wifi_platform_driver;

/**
 * struct virtio_wifi_vq - virtqueue abstraction
 * @vq: virtqueue
 * @lock: protects access to vq
 */
struct virtio_wifi_vq {
	struct virtqueue *vq;
	spinlock_t lock;
};

/**
 * struct virtio_wifi - Virtio-wifi device.
 * @virtio_dev: virtio device for the wifi device.
 * @controlq: virtio command queue.
 * @eventq: virtio event queue.
 * @eventq_work: work to run when events are received on @eventq.
 */
struct virtio_wifi {
	struct virtio_device *virtio_dev;
	struct virtio_wifi_vq controlq;
	struct virtio_wifi_vq eventq;
	struct virtio_wifi_vq txq;
	struct virtio_wifi_vq rxq;
	struct work_struct eventq_work;
	spinlock_t wiphys_lock;
	struct list_head wiphys;

	void **rxq_bufs;
	unsigned int num_rxq_bufs;
};

struct virtio_wifi_wiphy {
	struct virtio_wifi *virtio_wifi;
	u32 index;
	u32 ifaces;
	u32 flags;
	u32 rx_filter;
	u32 hwname_len;
	char hwname[64];
	bool is_monitoring;
	struct ieee80211_supported_band bands[NUM_NL80211_BANDS];
	struct ieee80211_hw *hw;
	struct device *dev;
	struct mutex mutex;
	struct list_head node;
	/* Fields for rx delayed work */
	struct scatterlist sg[MAX_SKB_FRAGS + 2];
	spinlock_t rx_work_lock;
	struct delayed_work rx_work;
	struct list_head rx_work_list;
	/* Fields for tx delayed work */
	struct delayed_work tx_work;
	struct ieee80211_tx_control *tx_control;
	struct sk_buff *tx_skb;
	struct virtio_wifi_interface *tx_wif;
	/* Fields used for hw scanning */
	/*
	struct delayed_work hw_scan_work;
	struct ieee80211_channel *tmp_chan;
	struct cfg80211_scan_request *hw_scan_request;
	struct ieee80211_vif *hw_scan_vif;
	int scan_chan_idx;
	u8 scan_addr[ETH_ALEN];
	struct {
		struct ieee80211_channel *channel;
		unsigned long next_start, start, end;
	} survey_data;
	*/
};

DEFINE_GUARD(wiphy_mutex, struct virtio_wifi_wiphy *, mutex_lock(&_T->mutex),
	     mutex_unlock(&_T->mutex));

/**
 * struct virtio_wifi_msg - Message requested by driver, responded by device.
 * @virtio_wifi: device data
 * @req: request buffer
 * @resp: response buffer
 * @responded: vqueue callback signals response reception
 * @refcnt: Message reference count, message and buffers will be deallocated
 *	    once 0. refcnt is decremented in the vqueue callback and in the
 *	    thread waiting on the responded completion.
 *          If a message response wait function times out, the message will be
 *          freed upon late reception (refcnt will reach 0 in the callback), or
 *          device removal.
 * @req_size: size of request in bytes
 * @resp_cap: maximum size of response in bytes
 * @resp_actual_size: actual size of response
 */
struct virtio_wifi_msg {
	struct virtio_wifi *virtio_wifi;
	void *req;
	void *resp;
	struct completion responded;
	refcount_t refcnt;
	unsigned int req_size;
	unsigned int resp_cap;
	unsigned int resp_actual_size;
};

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
struct virtio_wifi_msg *virtio_wifi_msg_init(struct virtio_wifi *virtio_wifi,
					     u32 msg_type,
					     unsigned int req_size,
					     unsigned int resp_cap);

/**
 * virtio_wifi_msg_release() - Decrement message refcnt, potentially free message.
 * @msg: message requested by driver
 *
 * Context: Any context.
 */
void virtio_wifi_msg_release(struct virtio_wifi_msg *msg);

extern const struct ieee80211_ops virtio_wifi_ieee80211_ops;
int virtio_wifi_do_register_wiphys(struct virtio_wifi *dev);
int virtio_wifi_do_start(struct virtio_wifi *dev, u32 wiphy_idx);
int virtio_wifi_do_stop(struct virtio_wifi *dev, u32 wiphy_idx);
int virtio_wifi_do_add_interface(struct virtio_wifi *dev, u32 wiphy_idx,
				 struct ieee80211_vif *vif);
int virtio_wifi_do_remove_interface(struct virtio_wifi *dev, u32 wiphy_idx,
				    struct ieee80211_vif *vif);
int virtio_wifi_do_configure_filter(struct virtio_wifi *dev, u32 wiphy_idx,
				    unsigned int changed_flags,
				    unsigned int *total_flags, u64 multicast);
int virtio_wifi_do_tx(struct virtio_wifi_wiphy *wiphy, u32 if_index,
		      struct sk_buff *skb);
void virtio_wifi_hw_scan_work(struct work_struct *work);
void virtio_wifi_do_tx_work(struct work_struct *work);
int virtio_wifi_do_set_channel(struct virtio_wifi_wiphy *wiphy,
			       struct cfg80211_chan_def *chandef);

/*FIXME:remove*/
static inline const char *cmd_to_s(u32 cmd)
{
	switch (cmd) {
	case VIRTIO_WIFI_CMD_GET_WIPHY:
		return "VIRTIO_WIFI_CMD_GET_WIPHY";
	case VIRTIO_WIFI_CMD_GET_WIPHYS:
		return "VIRTIO_WIFI_CMD_GET_WIPHYS";
	case VIRTIO_WIFI_CMD_SET_WIPHY:
		return "VIRTIO_WIFI_CMD_SET_WIPHY";
	case VIRTIO_WIFI_CMD_GET_INTERFACE:
		return "VIRTIO_WIFI_CMD_GET_INTERFACE";
	case VIRTIO_WIFI_CMD_GET_INTERFACES:
		return "VIRTIO_WIFI_CMD_GET_INTERFACES";
	case VIRTIO_WIFI_CMD_SET_INTERFACE:
		return "VIRTIO_WIFI_CMD_SET_INTERFACE";
	case VIRTIO_WIFI_CMD_NEW_INTERFACE:
		return "VIRTIO_WIFI_CMD_NEW_INTERFACE";
	case VIRTIO_WIFI_CMD_DELETE_INTERFACE:
		return "VIRTIO_WIFI_CMD_DELETE_INTERFACE";
	case VIRTIO_WIFI_CMD_GET_BAND:
		return "VIRTIO_WIFI_CMD_GET_BAND";
	case VIRTIO_WIFI_CMD_GET_BAND_CHANNEL:
		return "VIRTIO_WIFI_CMD_GET_BAND_CHANNEL";
	case VIRTIO_WIFI_CMD_GET_BAND_BITRATE:
		return "VIRTIO_WIFI_CMD_GET_BAND_BITRATE";
	case VIRTIO_WIFI_CMD_START_WIPHY:
		return "VIRTIO_WIFI_CMD_START_WIPHY";
	case VIRTIO_WIFI_CMD_STOP_WIPHY:
		return "VIRTIO_WIFI_CMD_STOP_WIPHY";
	case VIRTIO_WIFI_CMD_ADD_INTERFACE:
		return "VIRTIO_WIFI_CMD_ADD_INTERFACE";
	case VIRTIO_WIFI_CMD_REMOVE_INTERFACE:
		return "VIRTIO_WIFI_CMD_REMOVE_INTERFACE";
	case VIRTIO_WIFI_CMD_CHANGE_CONFIGURATION:
		return "VIRTIO_WIFI_CMD_CHANGE_CONFIGURATION";
	case VIRTIO_WIFI_CMD_CONFIGURE_RX_FILTER:
		return "VIRTIO_WIFI_CMD_CONFIGURE_RX_FILTER";
	case VIRTIO_WIFI_S_OK:
		return "VIRTIO_WIFI_S_OK";
	case VIRTIO_WIFI_S_BAD_MSG:
		return "VIRTIO_WIFI_S_BAD_MSG";
	case VIRTIO_WIFI_S_NOT_SUPP:
		return "VIRTIO_WIFI_S_NOT_SUPP";
	case VIRTIO_WIFI_S_IO_ERR:
		return "VIRTIO_WIFI_S_IO_ERR";
	default:
		return "invalid";
	};
}

#endif // __VIRTIO_WIFI_H
