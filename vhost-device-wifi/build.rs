// SPDX-License-Identifier: Apache-2.0 or BSD-3-Clause
// Copyright 2026 Panasonic Automotive Systems Co., Ltd.
// Author: Manos Pitsidianakis <manos.pitsidianakis@linaro.org>

use std::process::Command;

fn main() {
    println!("cargo::rerun-if-changed=guest-driver/virtio_wifi_uapi.h");

    let uname_r =
        String::from_utf8(Command::new("uname").arg("-r").output().unwrap().stdout).unwrap();
    eprintln!("uname -r = {uname_r}");
    let uapi_header = format!("-I/usr/src/linux-headers-{uname_r}/include/uapi/linux");

    macro_rules! bindgen_args {
        ($($s:ident),*$(,)?) => {
            [
                "./guest-driver/virtio_wifi_uapi.h",
                "-o",
                "src/bindings/virtio_wifi_uapi.rs",
                "--no-doc-comments",
                "--no-prepend-enum-name",
                "--explicit-padding",
                "--wrap-unsafe-ops",
                "--rustified-enum",
                "virtio_wifi_edmg_bw_config",
                "--rustified-enum",
                "virtio_wifi_iftype",
                "--rustified-enum",
                "virtio_wifi_chan_width",
                "--rustified-enum",
                "virtio_wifi_freq_band",
                "--rustified-enum",
                "virtio_wifi_interface_flags",
                "--rustified-enum",
                "virtio_wifi_event_code",
                "--rustified-enum",
                "virtio_wifi_if_operstate",
                "--allowlist-type",
                "virtio_wifi.*",
                "--allowlist-item",
                "VIRTIO_WIFI.*",
                "--blocklist-type",
                "__u32",
                "--blocklist-type",
                "__le32",
                "--blocklist-type",
                "__be32",
                "--blocklist-type",
                "__le16",
                "--blocklist-type",
                "__le64",
                "--with-derive-default",
                "--with-derive-partialeq",
                $(
                    "--with-derive-custom-struct",
                    concat!(stringify!($s), "=Copy,Clone,Debug"),
                )*
                    "--raw-line",
                    "#![allow(nonstandard_style, clippy::undocumented_unsafe_blocks, clippy::ptr_as_ptr, clippy::missing_safety_doc)]",
                    "--raw-line",
                    "use vm_memory::{Be32 as __be32, Le16 as __le16, Le32 as __le32, Le64 as __le64};",
                    "--",
                    "--target=aarch64-linux-gnu",
                    &uapi_header,
                    ]
        };
    }
    // FIXME: use bindgen as a library build-dep
    let status = Command::new("bindgen")
        .args(bindgen_args! {
            virtio_wifi_config,
                virtio_wifi_association_request,
                virtio_wifi_association_response,
                virtio_wifi_authentication_request,
                virtio_wifi_band,
                virtio_wifi_band_bitrate,
                virtio_wifi_band_sta_vht_cap,
                virtio_wifi_band_sta_vht_cap_virtio_wifi_vht_mcs_info,
                virtio_wifi_beacon,
                virtio_wifi_change_station,
                virtio_wifi_channel,
                virtio_wifi_cmd_get_band_bitrate_request,
                virtio_wifi_cmd_get_band_channel_request,
                virtio_wifi_connection_response,
                virtio_wifi_disconnect_request,
                virtio_wifi_disconnection_response,
                virtio_wifi_event,
                virtio_wifi_hdr,
                virtio_wifi_interface,
                virtio_wifi_interface_virtio_wifi_interface_cfg,
                virtio_wifi_key,
                virtio_wifi_register_frame,
                virtio_wifi_scan_result,
                virtio_wifi_set_if_operstate,
                virtio_wifi_sta_ht_cap,
                virtio_wifi_sta_ht_cap_mcs_info,
                virtio_wifi_station_change,
        })
        .status()
        .expect("failed to execute process");
    eprintln!("process finished with: {status}");

    assert!(status.success());
}
