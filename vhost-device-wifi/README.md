# vhost-device-wifi - WIFI

TODOs:

- Try to avoid needing to run as root, perhaps by using a network namespace.
- Create guest monitor vif
- bottom half locking:
  ```raw
  WARNING: CPU: 0 PID: 0 at kernel/softirq.c:386 __local_bh_enable_ip+0x50/0xa8
  [   27.072208]  __local_bh_enable_ip+0x50/0xa8
  [   27.073287]  vring_interrupt+0x7c/0x118
  [   27.074306]  vm_interrupt+0xa0/0x128 [virtio_mmio]
  [   27.075542]  __handle_irq_event_percpu+0x60/0x1d8
  [   27.076775]  handle_irq_event+0x4c/0x110
  [   27.077789]  handle_fasteoi_irq+0xb4/0x220
  [   27.078843]  handle_irq_desc+0x3c/0x68
  [   27.079825]  generic_handle_domain_irq+0x24/0x40
  [   27.081009]  gic_handle_irq+0x54/0x124
  ````

## Description

## Synopsis

```console
cargo run -- --socket-path /tmp/wifi.sock phy#0
```

## Options

TODO

```text
  -h, --help

  Print help

  -V, --version

  Print version
```

## Examples

The daemon should be started first:

```console
TODO
```

With QEMU command-line options:

```text
  -device vhost-user-test-device,chardev=wifi,id=wifi,virtio-id=10,num_vqs=2,vq_size=1024,config_size=4 \
  -chardev socket,path=/tmp/wifi.sock,id=wifi \
```

## Running as non-root

Since the daemon controls the WiFi hardware through `netlink`, it must have `CAP_NET_ADMIN` capability in the network's namespace. (`GENL_UNS_ADMIN_PERM`).
TODO

<!-- https://www.redhat.com/en/blog/net-namespaces ? -->

## License

This project is licensed under either of

- [Apache License](http://www.apache.org/licenses/LICENSE-2.0), Version 2.0
- [BSD-3-Clause License](https://opensource.org/licenses/BSD-3-Clause)
