// SPDX-License-Identifier: BSD-3-Clause OR GPL-2.0+

/*
 * Vhost-wifi driver.
 */

#include <linux/delay.h>
#include <linux/device.h>
#include <linux/dev_printk.h>
#include <linux/mm.h>
#include <linux/mutex.h>
#include <linux/scatterlist.h>
#include <linux/types.h>
#include <linux/vmalloc.h>
#include <linux/wait.h>
#include <linux/workqueue.h>
#include <linux/module.h>
#include <linux/moduleparam.h>
#include <linux/version.h>
#include <linux/virtio.h>
#include <linux/virtio_config.h>
#include <linux/virtio_ids.h>


#include <linux/compat.h>
#include <linux/eventfd.h>
#include <linux/vhost.h>
#include <linux/virtio_net.h>
#include <linux/miscdevice.h>
#include <linux/module.h>
#include <linux/moduleparam.h>
#include <linux/mutex.h>
#include <linux/workqueue.h>
#include <linux/file.h>
#include <linux/slab.h>
#include <linux/sched/clock.h>
#include <linux/sched/signal.h>
#include <linux/vmalloc.h>

#include <linux/net.h>
#include <linux/if_packet.h>
#include <linux/if_arp.h>
#include <linux/if_tun.h>
#include <linux/if_macvlan.h>
#include <linux/if_tap.h>
#include <linux/if_vlan.h>
#include <linux/skb_array.h>
#include <linux/skbuff.h>

#include <net/sock.h>
#include <net/xdp.h>

#include "vhost.h"

/* #include "protocol.h" */
/* #include "session.h" */
/* #include "virtio_media.h" */

#ifndef VIRTIO_ID_WIFI
#define VIRTIO_ID_WIFI 10
#endif

#define VHOST_WIFI_MINOR		243


static const struct file_operations vhost_net_fops = {
	.owner          = THIS_MODULE,
	.release        = vhost_net_release,
	.read_iter      = vhost_net_chr_read_iter,
	.write_iter     = vhost_net_chr_write_iter,
	.poll           = vhost_net_chr_poll,
	.unlocked_ioctl = vhost_net_ioctl,
	.compat_ioctl   = compat_ptr_ioctl,
	.open           = vhost_net_open,
	.llseek		= noop_llseek,
};

static struct miscdevice vhost_wifi_misc = {
	.minor = MISC_DYNAMIC_MINOR,
	.name = "vhost-wifi",
	.fops = &vhost_wifi_fops,
};

static int __init vhost_wifi_init(void)
{
	return misc_register(&vhost_wifi_misc);
}
module_init(vhost_wifi_init);

static void __exit vhost_wifi_exit(void)
{
	misc_deregister(&vhost_wifi_misc);
}
module_exit(vhost_wifi_exit);

MODULE_VERSION("0.0.1");
MODULE_LICENSE("GPL v2");
MODULE_AUTHOR("Manos Pitsidianakis <manos.pitsidianakis@linaro.org>");
MODULE_DESCRIPTION("vhost-wifi driver");
//MODULE_ALIAS_MISCDEV(MISC_DYNAMIC_MINOR);
MODULE_ALIAS("devname:vhost-wifi");
