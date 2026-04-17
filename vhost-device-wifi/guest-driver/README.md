
```text
make
modprobe mac80211
modprobe ./virtio-wifi.ko
```

```text
iw phy phy0 interface add wlan0 type station
ip link set dev wlan0 up
```

```text
iw dev wlan0 scan
```

```text
nmcli device set wlan0 managed no
```


```text
$ cat /etc/NetworkManager/conf.d/99-unmanaged-devices.conf
[device-wifi-unmanaged]
match-device=type:wifi
managed=0
```

```text
trace-cmd record -p function -l '*80211*' -l '*virtio_wifi*' ...
trace-cmd report | less
```
