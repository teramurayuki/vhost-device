// SPDX-License-Identifier: Apache-2.0 or BSD-3-Clause
// Copyright 2026 Panasonic Automotive Systems Co., Ltd.
// Author: Manos Pitsidianakis <manos.pitsidianakis@linaro.org>

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum FrameType {
    Management(ManagementFrameType),
    Control(ControlFrameType),
    Data,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ManagementFrameType {
    AssociationRequest,
    AssociationResponse,
    ReassociationRequest,
    ReassociationResponse,
    ProbeRequest,
    ProbeResponse,
    TimingAdvertisement,
    Beacon,
    ATIM,
    Disassociation,
    Authentication,
    Deauthentication,
    Action,
    ActionNoAck,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ControlFrameType {
    Trigger,
    TACK,
    BeamformingReportPoll,
    VhtheNdpAnnouncement,
    ControlFrameExtension,
    ControlWrapper,
    BlockAckRequest,
    BlockAck,
    PSPoll,
    RTS,
    CTS,
    ACK,
    CFEnd,
    CFEndCFACK,
}
