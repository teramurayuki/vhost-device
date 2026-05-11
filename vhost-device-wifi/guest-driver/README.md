
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

Capture: `airodump-ng -w captures/CAP moni0`
Decode: `python3 read_pcap.py captures/CAP-04.cap`

```
$ wpa_supplicant -Dnl80211 -iwlan0 -c wpa_supplicant.conf &

$ wpa_cli
> add_network 0
> set_network 0 ssid "HONEYPOT"
> set_network 0 psk "password"
> enable_network 0
```
