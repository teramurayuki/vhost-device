#! /usr/bin/python3

from scapy.all import *
import argparse
import pathlib
import sys


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--needle", action="append", default=[])
    parser.add_argument("packet_path", nargs='*', type=pathlib.Path)

    args = parser.parse_args()

    args.packet_path = sorted(args.packet_path, key=lambda x: int(x.name))
    pkt = Dot11()
    stats = {
            "addr1": set(),
            "addr2": set(),
            "addr3": set(),
            "type": set(),
            "subtype": set(),
            }

    for p in args.packet_path:
        data = open(p, "rb").read()
        pkt2= pkt.__class__(data)
        pkt2.show()
        stats["type"].add(pkt2.type)
        stats["subtype"].add(pkt2.subtype)
        stats["addr1"].add(pkt2.addr1)
        stats["addr2"].add(pkt2.addr2)
        stats["addr3"].add(pkt2.addr3)

    print("Statistics:")
    for k in stats:
        if len(stats[k]) > 20:
            val = f"{len(stats[k])} items"
        else:
            val = f"{stats[k]}"

        print(f"{k}: {val}")
        for n in args.needle:
            if n in stats[k]:
                print("Contains", n)
            else:
                print("Doesn't contain", n)
