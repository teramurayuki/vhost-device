// SPDX-License-Identifier: Apache-2.0 or BSD-3-Clause
// Copyright 2026 Panasonic Automotive Systems Co., Ltd.
// Author: Manos Pitsidianakis <manos.pitsidianakis@linaro.org>

use nix::time::{clock_gettime, ClockId};
pub use nix::{sys::time::TimeSpec, Result};

pub fn get_utc() -> Result<TimeSpec> {
    clock_gettime(ClockId::CLOCK_REALTIME)
}

pub fn get_tai() -> Result<TimeSpec> {
    clock_gettime(ClockId::CLOCK_TAI)
}

pub fn get_monotonic() -> Result<TimeSpec> {
    clock_gettime(ClockId::CLOCK_MONOTONIC)
}

#[derive(Debug)]
pub enum Alarm {
    Unset { enabled: bool },
    Set { alarm_time: u64, enabled: bool },
    Expired { alarm_time: u64, enabled: bool },
}

impl Alarm {
    pub fn set(&mut self, alarm_time: u64, enabled: bool) {
        *self = Alarm::Set {
            alarm_time,
            enabled,
        };
    }

    pub fn set_enabled(&mut self, value: bool) {
        let (Self::Unset { ref mut enabled }
        | Self::Set {
            ref mut enabled, ..
        }
        | Self::Expired {
            ref mut enabled, ..
        }) = self;
        *enabled = value;
    }

    pub fn enabled(&self) -> bool {
        let (Self::Unset { enabled } | Self::Set { enabled, .. } | Self::Expired { enabled, .. }) =
            self;
        *enabled
    }
}

#[derive(Debug)]
pub struct Clock {
    pub r#type: u8,
    pub leap_second_smearing: u8,
    pub alarm: Option<Alarm>,
    pub read_fn: fn() -> Result<TimeSpec>,
}

impl Clock {
    pub fn new_utc() -> Self {
        Self {
            r#type: crate::virtio_rtc::VIRTIO_RTC_CLOCK_UTC_SMEARED,
            leap_second_smearing: crate::virtio_rtc::VIRTIO_RTC_SMEAR_UNSPECIFIED,
            alarm: Some(Alarm::Unset { enabled: false }),
            read_fn: get_utc,
        }
    }

    pub fn new_tai() -> Self {
        Self {
            r#type: crate::virtio_rtc::VIRTIO_RTC_CLOCK_TAI,
            leap_second_smearing: crate::virtio_rtc::VIRTIO_RTC_SMEAR_UNSPECIFIED,
            alarm: None,
            read_fn: get_tai,
        }
    }

    pub fn new_monotonic() -> Self {
        Self {
            r#type: crate::virtio_rtc::VIRTIO_RTC_CLOCK_MONOTONIC,
            leap_second_smearing: crate::virtio_rtc::VIRTIO_RTC_SMEAR_UNSPECIFIED,
            alarm: None,
            read_fn: get_monotonic,
        }
    }

    pub fn read(&self) -> Result<u64> {
        let spec = (self.read_fn)()?;
        Ok((spec.tv_nsec() + spec.tv_sec() * 1000000000) as u64)
    }
}
