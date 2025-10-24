# vhost-device-wifi - WIFI

TODOs:

- Try to avoid needing to run as root, perhaps by using a network namespace?
- Create guest monitor vif?
- `get_tx_power`?
- `get_channel`?

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
