# vhost-device-rtc - Real time clock/timer device

This crate provides both a library and a binary for implementing a VIRTIO RTC through `vhost-user` protocol.
The RTC (Real Time Clock) device provides information about current time.

## Description

## Synopsis

```console
vhost-device-rtc [OPTIONS] --socket-path <SOCKET>
```

## Options

```text
 vhost-device-rtc

  -s, --socket-path <SOCKET>

  Location of vhost-user Unix domain socket

  --no-offer-alarm

  Don't offer alarm functionality for smeared UTC clocks. Turns `VIRTIO_RTC_F_ALARM` feature off

  -h, --help

  Print help

  -V, --version

  Print version
```

## Examples

The daemon should be started first:

```console
vhost-device-rtc --socket-path /path/to/rtc.sock
```

With QEMU command-line options:

```text
-chardev socket,path=/path/to/rtc.sock,id=rtc \
-device vhost-user-rtc-pci,chardev=rtc,id=rtc \
```

Or similar.

For usage with QEMU, a `memfd` memory backend is required, e.g.:

```text
  -m 8192 \
  -object memory-backend-memfd,id=mem,size=8G,share=on \
  -numa node,memdev=mem \
```

## Usage in a Linux guest

If the `virtio_rtc` driver is in your kernel (versions 6.17 and up), these or similar lines should appear on your kernel log:

```text
[    0.353545] virtio_rtc virtio2: registered as rtc0
[    0.354654] virtio_rtc virtio2: setting system clock to 2025-11-19T10:44:38 UTC (1763549078)
```

The clocks will be visible as `/dev/ptp*` devices.

```console
$ cat /sys/class/ptp/ptp0/clock_name
Virtio PTP type 0/variant 0
$ $ phc_ctl /dev/ptp0 get
phc_ctl[449.603]: clock time is ....
```

Smeared UTC clocks will also be visible as `/dev/rtc*` devices which allow for alarm functionality.

```console
# ls -halt /sys/class/rtc/
total 0
drwxr-xr-x  2 root root 0 Nov 19 10:45 .
lrwxrwxrwx  1 root root 0 Nov 19 10:44 rtc0 -> ../../devices/platform/4010000000.pcie/pci0000:00/0000:00:03.0/virtio2/rtc/rtc0
# cat /sys/class/rtc/rtc0/name
virtio_rtc virtio2
```

Set the alarm to wake up the guest:

```console
# date +%s -d'+10 seconds' > /sys/class/rtc/rtc0/wakealarm
```

## License

This project is licensed under either of

- [Apache License](http://www.apache.org/licenses/LICENSE-2.0), Version 2.0
- [BSD-3-Clause License](https://opensource.org/licenses/BSD-3-Clause)
