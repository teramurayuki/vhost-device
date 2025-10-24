# bindgen ./nl80211.h \
#            -o "./nl80211.rs" \
#            --no-doc-comments \
#            --with-derive-default \
#            --with-derive-partialeq \
#            -- \
#            -I/usr/src/linux-headers-`uname -r`/include/uapi/linux

# bindgen ./guest-driver/virtio_wifi_uapi.h \
#            -o "src/bindings/virtio_wifi_uapi.rs" \
#            --no-doc-comments \
#            --no-prepend-enum-name \
#            --explicit-padding \
#            --wrap-unsafe-ops \
#            --rustified-enum 'virtio_wifi_iftype' \
#            --rustified-enum 'virtio_wifi_chan_width' \
#            --rustified-enum 'virtio_wifi_freq_band' \
#            --rustified-enum 'virtio_wifi_interface_flags' \
#            --rustified-enum 'virtio_wifi_event_code' \
#            --allowlist-type "virtio_wifi.*" \
#            --allowlist-item "VIRTIO_WIFI.*" \
#            --blocklist-type "__u32" \
#            --blocklist-type "__le32" \
#            --with-derive-default \
#            --with-derive-partialeq \
#            -- \
#            --target="aarch64-linux-gnu" \
#            -I/usr/src/linux-headers-`uname -r`/include/uapi/linux
bindgen ./ieee80211.h \
           -o "./ieee80211.rs" \
           --no-doc-comments \
           --with-derive-default \
           --with-derive-partialeq \
           -- \
           -I/home/manos/patched_debian_linux_kernels/linux-source-6.12/include/ \
           -I/home/manos/patched_debian_linux_kernels/linux-source-6.12/arch/arm64/include/generated/ \
           -I/home/manos/patched_debian_linux_kernels/linux-source-6.12/./debian/linux-headers-6.12.11+/usr/src/linux-headers-6.12.11+/include/generated/autoconf.h \
           -I/home/manos/patched_debian_linux_kernels/linux-source-6.12/arch/arm64/include/
