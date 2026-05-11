/* SPDX-License-Identifier: GPL-2.0+ */
/* Author: Manos Pitsidianakis <manos.pitsidianakis@linaro.org> */

/*
 * Virtio-wifi driver.
 */

#include <linux/delay.h>
#include <linux/device.h>
#include <linux/dev_printk.h>
#include <linux/mm.h>
#include <linux/mutex.h>
#include <linux/list.h>
#include <linux/scatterlist.h>
#include <linux/types.h>
#include <linux/videodev2.h>
#include <linux/vmalloc.h>
#include <linux/wait.h>
#include <linux/workqueue.h>
#include <linux/module.h>
#include <linux/moduleparam.h>
#include <linux/version.h>
#include <linux/virtio.h>
#include <linux/virtio_config.h>
#include <linux/virtio_ids.h>

#include "virtio_wifi.h"

// TODO: how much is enough?
#define BUF_CAP sizeof(struct virtio_wifi_hdr) + 10 * PAGE_SIZE

struct class *virtio_wifi_class;

struct platform_driver virtio_wifi_platform_driver = {
	.driver = {
		.name = "virtio_wifi",
	},
};

/**
 * virtio_wifi_feed_vq() - Make a device write-only buffer available.
 * @virtio_wifi: device data
 * @vq: notification virtqueue
 * @buf: buffer
 * @buf_len: buffer capacity in bytes
 * @data: token, identifying buffer
 *
 * Context: Caller must prevent concurrent access to vq.
 * Return: Zero on success, negative error code otherwise.
 */
static int virtio_wifi_feed_vq(struct virtio_wifi *virtio_wifi,
			       struct virtqueue *vq, void *buf,
			       unsigned int buf_len, void *data)
{
	struct scatterlist sg;

	sg_init_one(&sg, buf, buf_len);

	return virtqueue_add_inbuf(vq, &sg, 1, data, GFP_ATOMIC);
}

static int virtio_wifi_populate_vq(struct virtio_wifi *virtio_wifi, u32 buf_cap,
				   bool lock)
{
	unsigned int num_elems, i;
	struct virtio_wifi_vq *virtio_wifi_vq = &virtio_wifi->rxq;
	struct virtqueue *vq;
	unsigned long flags;
	void *buf;
	int ret;

	num_elems = virtio_wifi->num_rxq_bufs;
	vq = virtio_wifi_vq->vq;

	for (i = 0; i < num_elems; i++) {
		buf = virtio_wifi->rxq_bufs[i];

		if (lock) {
			spin_lock_irqsave(&virtio_wifi_vq->lock, flags);

			ret = virtio_wifi_feed_vq(virtio_wifi, vq, buf, buf_cap,
						  buf);

			spin_unlock_irqrestore(&virtio_wifi_vq->lock, flags);
		} else {
			ret = virtio_wifi_feed_vq(virtio_wifi, vq, buf, buf_cap,
						  buf);
		}

		if (ret)
			return ret;
	}

	return 0;
}

static int virtio_wifi_alloc_vq_bufs(struct virtio_wifi *virtio_wifi,
				     unsigned int num_elems, u32 buf_cap)
{
	struct device *dev = &virtio_wifi->virtio_dev->dev;
	void **buf_list;
	unsigned int i;
	void *buf;

	buf_list = devm_kcalloc(dev, num_elems, sizeof(*buf_list), GFP_KERNEL);
	if (!buf_list)
		return -ENOMEM;

	virtio_wifi->rxq_bufs = buf_list;
	virtio_wifi->num_rxq_bufs = num_elems;

	for (i = 0; i < num_elems; i++) {
		buf = devm_kzalloc(dev, buf_cap, GFP_KERNEL);
		if (!buf)
			return -ENOMEM;

		buf_list[i] = buf;
	}

	return 0;
}

static int virtio_wifi_init_rxq(struct virtio_wifi *virtio_wifi)
{
	int ret = 0;
	struct virtio_wifi_vq *vq = &virtio_wifi->rxq;
	unsigned int num_elems = virtqueue_get_vring_size(vq->vq);

	if (num_elems == 0)
		return -ENOSPC;

	if (!virtio_wifi->rxq_bufs) {
		ret = virtio_wifi_alloc_vq_bufs(virtio_wifi, num_elems,
						BUF_CAP);
		if (ret)
			return ret;
	} else {
		virtio_wifi->num_rxq_bufs =
			min(num_elems, virtio_wifi->num_rxq_bufs);
	}

	return 0;
}

void virtio_wifi_do_tx_work(struct work_struct *work)
{
	struct virtio_wifi_wiphy *wiphy =
		container_of(work, struct virtio_wifi_wiphy, tx_work.work);
	u32 if_index = 0;
	struct sk_buff *skb = NULL;

	printk("%s called\n", __func__);

	guard(wiphy_mutex)(wiphy);

	if_index = wiphy->tx_if;
	skb = skb_dequeue(&wiphy->tx_list);

	if (skb == NULL) {
		return;
	}

	wiphy->tx_if = 0;
	wiphy->tx_control = NULL;

	BUG_ON(skb == NULL);
	int ret = virtio_wifi_do_tx(wiphy, if_index, skb);
	if (ret) {
		printk("%s do_tx returned %d\n", __func__, ret);
		goto err;
	}
	/* TODO: ieee80211_tx_status_skb(wiphy->hw, skb); */

	return;
err:
	/* TODO: ieee80211_free_txskb(wiphy->hw, skb); */
	return;
}

/**
 * commandq_callback: Callback for the command queue.
 * @queue: command virtqueue.
 *
 * This just wakes up the thread that was waiting on the command to complete.
 */
static void commandq_callback(struct virtqueue *queue)
{
	unsigned int len;
	struct virtio_wifi_msg *msg;

	/* printk("%s called\n", __func__); */
process_bufs:
	while ((msg = virtqueue_get_buf(queue, &len))) {
		/* printk("%s waking up msg\n", __func__); */
		msg->resp_actual_size = len;

		complete(&msg->responded);
		virtio_wifi_msg_release(msg);
		/* wake_up(&msg->vv->wq); */
	}

	if (!virtqueue_enable_cb(queue)) {
		virtqueue_disable_cb(queue);
		goto process_bufs;
	}
}

/**
 * eventq_callback() - Callback for the event queue.
 * @queue: event virtqueue.
 *
 * This just schedules for event work to be run.
 */
static void eventq_callback(struct virtqueue *queue)
{
	struct virtio_wifi *vv = queue->vdev->priv;

	/* printk("%s called\n", __func__); */
	schedule_work(&vv->eventq_work);
}

static void virtio_wifi_process_event_buf(struct virtio_wifi *virtio_wifi,
				       void *data, int len)
{
	struct virtio_wifi_event *event = data;
	u32 code = virtio_le_to_cpu(event->code);
	struct virtio_wifi_wiphy *w, *n;

	switch (code) {
		case VIRTIO_WIFI_EVENT_CODE_SCAN_RESULT:
	case VIRTIO_WIFI_EVENT_CODE_END_SCAN:
	case VIRTIO_WIFI_EVENT_CODE_CANCEL_SCAN:
			break;
	default:
			// TODO print err
			return;
	}

	spin_lock_bh(&virtio_wifi->wiphys_lock);
	// step 1 find wiphy based on index
	list_for_each_entry_safe(w, n, &virtio_wifi->wiphys, node) {
	switch (code) {
		case VIRTIO_WIFI_EVENT_CODE_SCAN_RESULT:
			struct virtio_wifi_scan_result *bss = data;
			size_t ie_len = bss->ielen;
			virtio_wifi_cfg80211_inform_bss(w->wiphy, data, (data + sizeof(struct virtio_wifi_scan_result)), ie_len);

			break;
	case VIRTIO_WIFI_EVENT_CODE_END_SCAN:
			virtio_wifi_cfg80211_scan_done(w->wiphy);
			break;
	case VIRTIO_WIFI_EVENT_CODE_CANCEL_SCAN:
			virtio_wifi_cfg80211_cancel_scan(w->wiphy);
			break;
	default:
			// TODO print err
			return;
	}
		break;
	}
	spin_unlock_bh(&virtio_wifi->wiphys_lock);

	return;
}

static void virtio_wifi_process_events(struct virtio_wifi *virtio_wifi)
{
	unsigned int len;
	void *token;
	struct virtio_wifi_vq *eventq = &virtio_wifi->eventq;
	unsigned long flags;
	bool notify;

	/* printk("%s called\n", __func__); */
process_bufs:
	while ((token = virtqueue_get_buf(eventq->vq, &len))) {
		/* printk("%s got event buf %u bytes\n", __func__, len); */
		if (len >= sizeof(struct virtio_wifi_event)) {
			virtio_wifi_process_event_buf(virtio_wifi, token, len);
		} else {
			printk("%s got event buf %u bytes < sizeof(struct virtio_wifi_event)\n",
			       __func__, len);
		}

		spin_lock_irqsave(&eventq->lock, flags);
		notify = virtqueue_kick_prepare(eventq->vq);
		/* spin_unlock_irqrestore(&rxq->lock, flags); */

		if (virtio_wifi_feed_vq(virtio_wifi, eventq->vq, token, BUF_CAP,
					token)) {
			printk("%s: failed to re-expose event buffer\n",
			       __func__);
			dev_warn(&virtio_wifi->virtio_dev->dev,
				 "%s: failed to re-expose event buffer\n",
				 __func__);
		}

		notify = virtqueue_kick_prepare(eventq->vq);

		spin_unlock_irqrestore(&eventq->lock, flags);

		if (notify)
			virtqueue_notify(eventq->vq);
	}

	if (!virtqueue_enable_cb(eventq->vq)) {
		virtqueue_disable_cb(eventq->vq);
		goto process_bufs;
	}

}

static void virtio_wifi_event_work(struct work_struct *work)
{
	struct virtio_wifi *vv =
		container_of(work, struct virtio_wifi, eventq_work);

	/* printk("%s called\n", __func__); */
	virtio_wifi_process_events(vv);
}

/**
 * tx_callback: Callback for the TX queue.
 * @queue: tx virtqueue.
 *
 * This just wakes up the thread that was waiting on the command to complete.
 */
static void tx_callback(struct virtqueue *queue)
{
	unsigned int len;
	struct virtio_wifi_msg *msg;

	/* printk("%s called\n", __func__); */
process_bufs:
	while ((msg = virtqueue_get_buf(queue, &len))) {
		/* printk("%s waking up msg\n", __func__); */
		msg->resp_actual_size = len;

		complete(&msg->responded);
		virtio_wifi_msg_release(msg);
		/* wake_up(&msg->vv->wq); */
	}

	if (!virtqueue_enable_cb(queue)) {
		virtqueue_disable_cb(queue);
		goto process_bufs;
	}
}

static void virtio_wifi_process_rx_buf(struct virtio_wifi *virtio_wifi,
				       void *data, int len)
{
	struct virtio_wifi_hdr *hdr = data;
	struct sk_buff *skb = NULL;
	int frame_data_len;
	void *frame_data;
	u32 index = virtio_le_to_cpu(hdr->index);

	/* printk("%s for iface = %u\n", __func__, index); */
	struct virtio_wifi_wiphy *w, *n;

	spin_lock_bh(&virtio_wifi->wiphys_lock);
	// step 1 find wiphy based on index
	list_for_each_entry_safe(w, n, &virtio_wifi->wiphys, node) {
		frame_data_len = len - sizeof(struct virtio_wifi_hdr);
		frame_data = data + sizeof(struct virtio_wifi_hdr);

		/* Allocate new skb here */
		skb = alloc_skb(frame_data_len, GFP_KERNEL);
		if (skb == NULL) {
			printk("%s alloc_skb frame_data_len = %d failed\n",
			       __func__, frame_data_len);
			break;
		}

		/* Copy the data */
		skb_put_data(skb, frame_data, frame_data_len);
		// step 2 ieee80211_rx_irqsafe(data->hw, skb); (is radiotap needed?)
		/* TODO ieee80211_rx_irqsafe(w->hw, skb); */
		break;
	}
	spin_unlock_bh(&virtio_wifi->wiphys_lock);

	return;
}

/**
 * rx_callback: Callback for the RX queue.
 * @queue: rx virtqueue.
 *
 * This just wakes up the thread that was waiting on the command to complete.
 */
static void rx_callback(struct virtqueue *queue)
{
	unsigned int len;
	void *token;
	struct virtio_wifi *virtio_wifi = queue->vdev->priv;
	struct virtio_wifi_vq *rxq = &virtio_wifi->rxq;
	unsigned long flags;
	bool notify;

	/* printk("%s called\n", __func__); */
process_bufs:
	while ((token = virtqueue_get_buf(queue, &len))) {
		/* printk("%s got RX buf %u bytes\n", __func__, len); */
		if (len >= sizeof(struct virtio_wifi_hdr)) {
			virtio_wifi_process_rx_buf(virtio_wifi, token, len);
		} else {
			printk("%s got RX buf %u bytes < sizeof(struct virtio_wifi_hdr)\n",
			       __func__, len);
		}

		spin_lock_irqsave(&rxq->lock, flags);
		notify = virtqueue_kick_prepare(rxq->vq);
		/* spin_unlock_irqrestore(&rxq->lock, flags); */

		if (virtio_wifi_feed_vq(virtio_wifi, queue, token, BUF_CAP,
					token)) {
			printk("%s: failed to re-expose input buffer\n",
			       __func__);
			dev_warn(&virtio_wifi->virtio_dev->dev,
				 "%s: failed to re-expose input buffer\n",
				 __func__);
		}

		notify = virtqueue_kick_prepare(queue);

		spin_unlock_irqrestore(&rxq->lock, flags);

		if (notify)
			virtqueue_notify(queue);
	}

	if (!virtqueue_enable_cb(queue)) {
		virtqueue_disable_cb(queue);
		goto process_bufs;
	}
}

static int virtio_wifi_init_eventq(struct virtio_wifi *virtio_wifi, u32 num_elems, size_t buf_cap)
{
	void **buf_list;
	struct device *dev = &virtio_wifi->virtio_dev->dev;
	void *buf;
	int ret = 0;
	size_t bytes = 0;
size_t i = 0;

	if (num_elems == 0)
		return -ENOSPC;

	if (unlikely(check_mul_overflow(num_elems, sizeof(buf), &bytes)))
		return -ENOMEM;

	buf_list = devm_kmalloc(dev, bytes, GFP_KERNEL);
	if (!buf_list)
		return -ENOMEM;

	virtio_wifi->eventq_bufs = buf_list;
	virtio_wifi->num_eventq_bufs = num_elems;

	for (i = 0; i < num_elems; i++) {
		buf = devm_kzalloc(dev, buf_cap, GFP_KERNEL);
		if (!buf)
		{
			ret = -ENOMEM;
			goto err;
		}

		buf_list[i] = buf;
	}
	goto finish;

err:
	for (size_t n = 0; n < i; n++) {
		if (buf_list[n]) {
			devm_kfree(dev, buf_list[n]);
		}
	}

finish:
	return ret;
}

static int virtio_wifi_populate_eventqvq(struct virtio_wifi *virtio_wifi, u32 buf_cap,
				   bool lock)
{
	unsigned int num_elems, i;
	struct virtio_wifi_vq *virtio_wifi_vq = &virtio_wifi->eventq;
	struct virtqueue *vq;
	unsigned long flags;
	void *buf;
	int ret;

	num_elems = virtio_wifi->num_eventq_bufs;
	vq = virtio_wifi_vq->vq;

	for (i = 0; i < num_elems; i++) {
		buf = virtio_wifi->eventq_bufs[i];

		if (lock) {
			spin_lock_irqsave(&virtio_wifi_vq->lock, flags);

			ret = virtio_wifi_feed_vq(virtio_wifi, vq, buf, buf_cap,
						  buf);

			spin_unlock_irqrestore(&virtio_wifi_vq->lock, flags);
		} else {
			ret = virtio_wifi_feed_vq(virtio_wifi, vq, buf, buf_cap,
						  buf);
		}

		if (ret)
			return ret;
	}

	return 0;
}


static int virtio_wifi_probe(struct virtio_device *virtio_dev)
{
	struct device *dev = &virtio_dev->dev;
	struct virtqueue *vqs[VIRTIO_WIFI_VQ_MAX];
	struct virtio_wifi_vq *rxq, *eventq;
	struct virtio_wifi *vv;
	int ret;
	unsigned long flags;
	bool notify;

#if LINUX_VERSION_CODE >= KERNEL_VERSION(6, 11, 0)
	static struct virtqueue_info vq_info[VIRTIO_WIFI_VQ_MAX] = {
		{
			.name = "command",
			.callback = commandq_callback,
		},
		{
			.name = "event",
			.callback = eventq_callback,
		},
		{
			.name = "tx",
			.callback = tx_callback,
		},
		{
			.name = "rx",
			.callback = rx_callback,
		},
	};
#else
	static vq_callback_t *vq_callbacks[] = {
		commandq_callback,
		eventq_callback,
		tx_callback,
		rx_callback,
	};
	static const char *const vq_names[] = { "command", "event", "tx",
						"rx" };
#endif

	printk("%s called\n", __func__);

	ret = platform_driver_register(&virtio_wifi_platform_driver);
	if (ret) {
		printk("%s platform_driver_register = %d\n", __func__, ret);
		goto err_platform_driver;
	}

	printk("%s: kzalloc \n", __func__);
	vv = devm_kzalloc(dev, sizeof(*vv), GFP_KERNEL);
	if (!vv)
		return -ENOMEM;

	vv->virtio_dev = virtio_dev;
	virtio_dev->priv = vv;

	/* init_waitqueue_head(&vv->wq); */

	printk("%s: find vqs:\n", __func__);
#if LINUX_VERSION_CODE >= KERNEL_VERSION(6, 11, 0)
	ret = virtio_find_vqs(virtio_dev, VIRTIO_WIFI_VQ_MAX, vqs, vq_info,
			      NULL);
#else
	ret = virtio_find_vqs(virtio_dev, VIRTIO_WIFI_VQ_MAX, vqs, vq_callbacks,
			      vq_names, NULL);
#endif
	printk("%s: find vqs ret=%d:\n", __func__, ret);
	if (ret)
		goto err_find_vqs;

	vv->controlq.vq = vqs[0];
	spin_lock_init(&vv->controlq.lock);
	vv->eventq.vq = vqs[1];
	spin_lock_init(&vv->eventq.lock);
	vv->txq.vq = vqs[2];
	spin_lock_init(&vv->txq.lock);
	vv->rxq.vq = vqs[3];
	spin_lock_init(&vv->rxq.lock);
	printk("%s: init work:\n", __func__);
	INIT_WORK(&vv->eventq_work, virtio_wifi_event_work);

	ret = virtio_wifi_init_rxq(vv);
	if (ret)
		goto err_find_vqs;

	virtio_device_ready(virtio_dev);
	virtio_wifi_class = class_create("virtio_wifi");
	if (IS_ERR(virtio_wifi_class)) {
		ret = PTR_ERR(virtio_wifi_class);
		goto err_class;
	}
	INIT_LIST_HEAD(&vv->wiphys);
	spin_lock_init(&vv->wiphys_lock);

	printk("%s: register wiphys\n", __func__);
	virtio_wifi_do_register_wiphys(vv);

	rxq = &vv->rxq;
	eventq = &vv->eventq;
	ret = virtio_wifi_populate_vq(vv, BUF_CAP, true);
	if (ret)
		goto err_after_ready;

	spin_lock_irqsave(&rxq->lock, flags);
	notify = virtqueue_kick_prepare(rxq->vq);
	spin_unlock_irqrestore(&rxq->lock, flags);

	if (notify && !virtqueue_notify(rxq->vq)) {
		ret = -EIO;
		goto err_after_ready;
	}

	ret = virtio_wifi_init_eventq(vv, virtqueue_get_vring_size(vv->eventq.vq), BUF_CAP);
	if (ret) {
		goto err_after_ready;
	}
	ret = virtio_wifi_populate_eventqvq(vv, BUF_CAP, true);

	spin_lock_irqsave(&eventq->lock, flags);
	notify = virtqueue_kick_prepare(eventq->vq);
	spin_unlock_irqrestore(&eventq->lock, flags);

	if (notify && !virtqueue_notify(eventq->vq)) {
		ret = -EIO;
		goto err_after_ready;
	}

	ret = virtio_wifi_cfg80211_init_module(vv);
	if (ret) {
		goto err_after_ready;
	}


	return 0;

err_after_ready:
	// FIXME is this enough?
	virtio_reset_device(virtio_dev);
	virtio_dev->config->del_vqs(virtio_dev);
	class_destroy(virtio_wifi_class);
err_class:
err_find_vqs:
	platform_driver_unregister(&virtio_wifi_platform_driver);
err_platform_driver:

	return ret;
}

static void virtio_wifi_remove(struct virtio_device *virtio_dev)
{
	struct virtio_wifi *vv = virtio_dev->priv;

	cancel_work_sync(&vv->eventq_work);

	struct virtio_wifi_wiphy *wiphy;

	spin_lock_bh(&vv->wiphys_lock);
	while ((wiphy = list_first_entry_or_null(
			&vv->wiphys, struct virtio_wifi_wiphy, node))) {
		list_del(&wiphy->node);
		spin_unlock_bh(&vv->wiphys_lock);
		virtio_wifi_cfg80211_destroy_wiphy(wiphy->wiphy);
		spin_lock_bh(&vv->wiphys_lock);
	}
	spin_unlock_bh(&vv->wiphys_lock);

	virtio_wifi_cfg80211_cleanup_module();
	virtio_reset_device(virtio_dev);

	virtio_dev->config->del_vqs(virtio_dev);
	class_destroy(virtio_wifi_class);
	platform_driver_unregister(&virtio_wifi_platform_driver);
}

static struct virtio_device_id id_table[] = {
	{ VIRTIO_ID_MAC80211_WLAN, VIRTIO_DEV_ANY_ID },
	{ 0 },
};

static unsigned int features[] = {};

static struct virtio_driver virtio_wifi_driver = {
	.feature_table = features,
	.feature_table_size = ARRAY_SIZE(features),
	.driver.name = "virtio-wifi",
	.driver.owner = THIS_MODULE,
	.id_table = id_table,
	.probe = virtio_wifi_probe,
	.remove = virtio_wifi_remove,
};

module_virtio_driver(virtio_wifi_driver);

MODULE_DEVICE_TABLE(virtio, id_table);
MODULE_DESCRIPTION("virtio wifi driver");
MODULE_AUTHOR("Manos Pitsidianakis <manos.pitsidianakis@linaro.org>");
MODULE_LICENSE("GPL");
