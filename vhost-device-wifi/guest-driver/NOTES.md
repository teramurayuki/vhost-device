
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

netlink debugging:

```
# create the network interface
sudo ip link add  nlmon0 type nlmon
sudo ip link set dev nlmon0 up
sudo tcpdump -i nlmon0 -w netlink.pcap # capture your packets
wireshark netlink.pcap # look at the results with wireshark
```

```
ip addr add 192.168.1.153/24 dev wlan0
ip route add default via 192.168.1.1 dev wlan0
```


```
/usr/sbin/hostapd -B -P /run/hostapd.pid /etc/hostapd/hostapd.conf
wpa_cli -i wlan0 log_level debug
journalctl  -f -u wpa_supplicant -u NetworkManager -u systemd-networkd
```
