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
	struct work_struct rxq_work;
	struct virtio_wifi_wiphy *wiphy;

	void **eventq_bufs;
	unsigned int num_eventq_bufs;

	void **rxq_bufs;
	unsigned int num_rxq_bufs;
};

struct virtio_wifi_wiphy {
	struct virtio_wifi *virtio_wifi;
	struct cfg80211_ops *ops;
	u32 index;
	u32 *cipher_suites;
	u32 n_cipher_suites;
	struct ieee80211_supported_band bands[NUM_NL80211_BANDS];
	struct mutex mutex;
	struct list_head node;
	/* Fields for tx delayed work */
	struct delayed_work tx_work;
	struct sk_buff_head tx_list;
	u32 tx_if;
	// Fullmac
	struct cfg80211_scan_request *scan_request;
	bool being_deleted;
	bool wiphy_registered;
	struct wiphy *wiphy;
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
 * virtio_wifi_msg_release() - Decrement message refcnt, potentially free message.
 * @msg: message requested by driver
 *
 * Context: Any context.
 */
void virtio_wifi_msg_release(struct virtio_wifi_msg *msg);

int virtio_wifi_get_band_bitrate(struct virtio_wifi *dev,
				 enum nl80211_band band_kind, u32 bitrate_idx,
				 struct virtio_wifi_band_bitrate *bitrate);

int virtio_wifi_get_band_channel(struct virtio_wifi *dev,
				 enum nl80211_band band_kind, u32 channel_idx,
				 struct virtio_wifi_channel *channel);
int virtio_wifi_get_band(struct virtio_wifi *dev, enum nl80211_band band_kind,
			 struct virtio_wifi_band *band);
int virtio_wifi_get_cipher_suites(struct virtio_wifi *dev, u32 **cipher_suites,
				  int n_cipher_suites);
int virtio_wifi_do_register_wiphy(struct virtio_wifi *dev);
int virtio_wifi_do_add_interface(struct virtio_wifi *dev,
				 enum nl80211_iftype type,
				 struct virtio_wifi_interface *retval);
int virtio_wifi_do_remove_interface(struct virtio_wifi *dev, u32 if_index);
int virtio_wifi_do_change_interface(struct virtio_wifi *dev, u32 if_index,
				    enum nl80211_iftype type);
int virtio_wifi_do_tx(struct virtio_wifi_wiphy *wiphy, u32 if_index,
		      struct sk_buff *skb, bool is_802_3);
int virtio_wifi_do_trigger_scan(struct virtio_wifi_wiphy *wiphy, u32 if_index,
				struct cfg80211_scan_request *req);
int virtio_wifi_do_auth(struct virtio_wifi_wiphy *wiphy, u32 if_index,
			const struct element *ssid_elem,
			struct cfg80211_auth_request *auth_req);
int virtio_wifi_do_assoc(struct virtio_wifi_wiphy *wiphy, u32 if_index,
			 const struct element *ssid_elem,
			 struct cfg80211_assoc_request *auth_req);
void virtio_wifi_do_tx_work(struct work_struct *work);
int virtio_wifi_do_add_key(struct virtio_wifi_wiphy *wiphy, u32 if_index,
			   u8 key_index, bool pairwise, const u8 *mac_addr,
			   struct key_params *params);
int virtio_wifi_do_del_key(struct virtio_wifi_wiphy *wiphy, u32 if_index,
			   u8 key_index, bool pairwise, const u8 *mac_addr);
int virtio_wifi_do_set_default_key(struct virtio_wifi_wiphy *wiphy,
				   u32 if_index, u8 key_index, bool def,
				   bool unicast, bool multicast, bool mgmt);
int virtio_wifi_do_get_key(struct virtio_wifi_wiphy *wiphy, u32 if_index,
			   u8 key_index, bool pairwise, const u8 *mac_addr,
			   struct key_params *params);
int virtio_wifi_do_change_station(struct virtio_wifi_wiphy *wiphy, u32 if_index,
				  const u8 *mac,
				  struct station_parameters *params);
int virtio_wifi_do_connect(struct virtio_wifi_wiphy *wiphy, u32 if_index,
			   struct cfg80211_connect_params *sme);
int virtio_wifi_do_disconnect(struct virtio_wifi_wiphy *wiphy, u32 if_index,
			      u32 cmd, const u8 *mac, const u8 *ie,
			      size_t ie_len, u16 reason_code,
			      bool local_state_change);
int virtio_wifi_do_if_oper(struct virtio_wifi_wiphy *wiphy, u32 if_index,
			   enum virtio_wifi_if_operstate flag, bool value);
int virtio_wifi_do_start_ap(struct virtio_wifi *dev, u32 if_index,
			    struct cfg80211_ap_settings *settings);

/*FIXME:remove*/
static inline const char *cmd_to_s(u32 cmd)
{
	switch (cmd) {
	case VIRTIO_WIFI_CMD_GET_INTERFACES:
		return "VIRTIO_WIFI_CMD_GET_INTERFACES";
	case VIRTIO_WIFI_CMD_NEW_INTERFACE:
		return "VIRTIO_WIFI_CMD_NEW_INTERFACE";
	case VIRTIO_WIFI_CMD_CHANGE_INTERFACE:
		return "VIRTIO_WIFI_CMD_CHANGE_INTERFACE";
	case VIRTIO_WIFI_CMD_DELETE_INTERFACE:
		return "VIRTIO_WIFI_CMD_DELETE_INTERFACE";
	case VIRTIO_WIFI_CMD_GET_BAND:
		return "VIRTIO_WIFI_CMD_GET_BAND";
	case VIRTIO_WIFI_CMD_GET_BAND_CHANNEL:
		return "VIRTIO_WIFI_CMD_GET_BAND_CHANNEL";
	case VIRTIO_WIFI_CMD_GET_BAND_BITRATE:
		return "VIRTIO_WIFI_CMD_GET_BAND_BITRATE";
	case VIRTIO_WIFI_CMD_TRIGGER_SCAN:
		return "VIRTIO_WIFI_CMD_TRIGGER_SCAN";
	case VIRTIO_WIFI_CMD_GET_CIPHERSUITES:
		return "VIRTIO_WIFI_CMD_GET_CIPHERSUITES";
	case VIRTIO_WIFI_CMD_AUTHENTICATE:
		return "VIRTIO_WIFI_CMD_AUTHENTICATE";
	case VIRTIO_WIFI_CMD_ASSOCIATE:
		return "VIRTIO_WIFI_CMD_ASSOCIATE";
	case VIRTIO_WIFI_CMD_ADD_KEY:
		return "VIRTIO_WIFI_CMD_ADD_KEY";
	case VIRTIO_WIFI_CMD_DEL_KEY:
		return "VIRTIO_WIFI_CMD_DEL_KEY";
	case VIRTIO_WIFI_CMD_SET_KEY:
		return "VIRTIO_WIFI_CMD_SET_KEY";
	case VIRTIO_WIFI_CMD_SET_STATION:
		return "VIRTIO_WIFI_CMD_SET_STATION";
	case VIRTIO_WIFI_CMD_START_AP:
		return "VIRTIO_WIFI_CMD_START_AP";
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

int virtio_wifi_cfg80211_init_module(struct virtio_wifi *);
void virtio_wifi_cfg80211_cleanup_module(void);
struct virtio_wifi_wiphy *
virtio_wifi_cfg80211_make_wiphy(struct virtio_wifi *dev);
struct wireless_dev *
virtio_wifi_cfg80211_add_intf(struct wiphy *wiphy,
			      unsigned char name_assign_type,
			      struct virtio_wifi_interface *wif);
void virtio_wifi_cfg80211_destroy_wiphy(struct wiphy *wiphy);
void virtio_wifi_cfg80211_inform_bss(struct wiphy *wiphy,
				     struct virtio_wifi_scan_result *bss,
				     void *ie, size_t ie_len);
void virtio_wifi_cfg80211_scan_done(struct wiphy *wiphy);
void virtio_wifi_cfg80211_cancel_scan(struct wiphy *wiphy);
void virtio_wifi_cfg80211_event_dispatch(struct virtio_wifi_wiphy *w, u32 code,
					 u32 ifindex, void *data, int len);
void virtio_wifi_cfg80211_rx(struct virtio_wifi_wiphy *w, void *data, int len);
#endif // __VIRTIO_WIFI_H
