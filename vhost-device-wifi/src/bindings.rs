// SPDX-License-Identifier: Apache-2.0 or BSD-3-Clause
// Copyright 2026 Panasonic Automotive Systems Co., Ltd.
// Author: Manos Pitsidianakis <manos.pitsidianakis@linaro.org>

#[rustfmt::skip]
pub mod virtio_wifi_uapi;

macro_rules! impl_byte_valued {
    ($($i:ident),*$(,)?) => {
        $(
            // SAFETY: This struct is plain-old-data.
            unsafe impl vm_memory::ByteValued for virtio_wifi_uapi::$i {}
         )*
    };
}

impl_byte_valued! {
    virtio_wifi_association_request,
    virtio_wifi_association_response,
    virtio_wifi_authentication_request,
    virtio_wifi_band,
    virtio_wifi_band_bitrate,
    virtio_wifi_beacon,
    virtio_wifi_change_station,
    virtio_wifi_channel,
    virtio_wifi_cmd_get_band_bitrate_request,
    virtio_wifi_cmd_get_band_channel_request,
    virtio_wifi_config,
    virtio_wifi_connection_response,
    virtio_wifi_disconnect_request,
    virtio_wifi_disconnection_response,
    virtio_wifi_event,
    virtio_wifi_hdr,
    virtio_wifi_interface,
    virtio_wifi_key,
    virtio_wifi_register_frame,
    virtio_wifi_scan_result,
    virtio_wifi_set_if_operstate,
    virtio_wifi_station_change,
}
