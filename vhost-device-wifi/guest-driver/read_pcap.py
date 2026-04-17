#! /usr/bin/python3

from scapy.all import *
import argparse
import pathlib
import sys


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("pcap", nargs='*', type=pathlib.Path)

    args = parser.parse_args()

    args.pcap = sorted(args.pcap)
    for p in args.pcap:
        print(p)
        p = open(p, "rb")
        scapy_cap = rdpcap(p)
        for packet in scapy_cap:
            print(packet)
