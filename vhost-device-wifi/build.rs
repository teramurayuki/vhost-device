use std::process::Command;

fn main() {
    let uname_r =
        String::from_utf8(Command::new("uname").arg("-r").output().unwrap().stdout).unwrap();
    eprintln!("uname -r = {uname_r}");
    let uapi_header = format!("-I/usr/src/linux-headers-{uname_r}/include/uapi/linux");

    // FIXME: use bindgen as a library build-dep
    let status = Command::new("bindgen")
        .args([
            "./guest-driver/virtio_wifi_uapi.h",
            "-o",
            "src/bindings/virtio_wifi_uapi.rs",
            "--no-doc-comments",
            "--no-prepend-enum-name",
            "--explicit-padding",
            "--wrap-unsafe-ops",
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
            "--raw-line",
            "#![allow(nonstandard_style)]",
            "--raw-line",
            "use vm_memory::{Be32 as __be32, Le16 as __le16, Le32 as __le32, Le64 as __le64};",
            "--",
            "--target=aarch64-linux-gnu",
            &uapi_header,
        ])
        .status()
        .expect("failed to execute process");
    eprintln!("process finished with: {status}");

    assert!(status.success());
}
