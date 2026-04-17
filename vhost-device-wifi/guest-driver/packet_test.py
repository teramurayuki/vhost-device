#! /usr/bin/python3

# trace-cmd record -p function -l '*80211*' -l '*virtio_wifi*' python3 packet_test.py

from scapy.all import *

radiotap = RadioTap(present="TXFlags", TXFlags="NOSEQ+ORDER")

pkt = Dot11(addr1='02:00:00:00:00:00', addr2='02:00:00:00:01:00') / Dot11Deauth(reason=8)
print("Sending the following packet:")
pkt.show()
sendp(radiotap/pkt, count=1, iface='wlan0')
