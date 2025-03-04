// Copyright 2019 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use crate::accessibility::types::{AccessibilityInfo, CaptionsSettings, ColorBlindnessType};
use crate::base::{SettingInfo, SettingType};
use crate::handler::base::Request;
use crate::ingress::{request, watch, Scoped};
use crate::job::source::{Error as JobError, ErrorResponder};
use crate::job::Job;
use fidl::prelude::*;
use fidl_fuchsia_settings::{
    AccessibilityRequest, AccessibilitySetResponder, AccessibilitySetResult, AccessibilitySettings,
    AccessibilityWatchResponder,
};

impl ErrorResponder for AccessibilitySetResponder {
    fn id(&self) -> &'static str {
        "Accessibility_Set"
    }

    fn respond(self: Box<Self>, error: fidl_fuchsia_settings::Error) -> Result<(), fidl::Error> {
        self.send(Err(error))
    }
}

impl request::Responder<Scoped<AccessibilitySetResult>> for AccessibilitySetResponder {
    fn respond(self, Scoped(response): Scoped<AccessibilitySetResult>) {
        let _ = self.send(response);
    }
}

impl watch::Responder<AccessibilitySettings, zx::Status> for AccessibilityWatchResponder {
    fn respond(self, response: Result<AccessibilitySettings, zx::Status>) {
        match response {
            Ok(settings) => {
                let _ = self.send(&settings);
            }
            Err(error) => {
                self.control_handle().shutdown_with_epitaph(error);
            }
        }
    }
}

impl TryFrom<AccessibilityRequest> for Job {
    type Error = JobError;

    fn try_from(item: AccessibilityRequest) -> Result<Self, Self::Error> {
        #[allow(unreachable_patterns)]
        match item {
            AccessibilityRequest::Set { settings, responder } => {
                Ok(request::Work::new(SettingType::Accessibility, to_request(settings), responder)
                    .into())
            }
            AccessibilityRequest::Watch { responder } => {
                Ok(watch::Work::new_job(SettingType::Accessibility, responder))
            }
            _ => {
                log::warn!("Received a call to an unsupported API: {:?}", item);
                Err(JobError::Unsupported)
            }
        }
    }
}

impl From<SettingInfo> for AccessibilitySettings {
    fn from(response: SettingInfo) -> Self {
        if let SettingInfo::Accessibility(info) = response {
            return AccessibilitySettings {
                audio_description: info.audio_description,
                screen_reader: info.screen_reader,
                color_inversion: info.color_inversion,
                enable_magnification: info.enable_magnification,
                color_correction: info.color_correction.map(ColorBlindnessType::into),
                captions_settings: info.captions_settings.map(CaptionsSettings::into),
                ..Default::default()
            };
        }

        panic!("incorrect value sent to accessibility");
    }
}

fn to_request(settings: AccessibilitySettings) -> Request {
    Request::SetAccessibilityInfo(AccessibilityInfo {
        audio_description: settings.audio_description,
        screen_reader: settings.screen_reader,
        color_inversion: settings.color_inversion,
        enable_magnification: settings.enable_magnification,
        color_correction: settings
            .color_correction
            .map(fidl_fuchsia_settings::ColorBlindnessType::into),
        captions_settings: settings
            .captions_settings
            .map(fidl_fuchsia_settings::CaptionsSettings::into),
    })
}

#[cfg(test)]
mod tests {
    use crate::accessibility::types::{CaptionFontFamily, CaptionFontStyle, ColorRgba, EdgeStyle};
    use fidl_fuchsia_settings::ColorBlindnessType;

    use super::*;

    #[fuchsia::test]
    fn test_request_try_from_settings_request_empty() {
        let request = to_request(AccessibilitySettings::default());

        const EXPECTED_ACCESSIBILITY_INFO: AccessibilityInfo = AccessibilityInfo {
            audio_description: None,
            screen_reader: None,
            color_inversion: None,
            enable_magnification: None,
            color_correction: None,
            captions_settings: None,
        };

        assert_eq!(request, Request::SetAccessibilityInfo(EXPECTED_ACCESSIBILITY_INFO));
    }

    #[fuchsia::test]
    fn test_try_from_settings_request() {
        const TEST_COLOR: ColorRgba =
            ColorRgba { red: 238.0, green: 23.0, blue: 128.0, alpha: 255.0 };
        const EXPECTED_FONT_STYLE: CaptionFontStyle = CaptionFontStyle {
            family: Some(CaptionFontFamily::Casual),
            color: Some(TEST_COLOR),
            relative_size: Some(1.0),
            char_edge_style: Some(EdgeStyle::Raised),
        };
        const EXPECTED_CAPTION_SETTINGS: CaptionsSettings = CaptionsSettings {
            for_media: Some(true),
            for_tts: Some(true),
            font_style: Some(EXPECTED_FONT_STYLE),
            window_color: Some(TEST_COLOR),
            background_color: Some(TEST_COLOR),
        };
        const EXPECTED_ACCESSIBILITY_INFO: AccessibilityInfo = AccessibilityInfo {
            audio_description: Some(true),
            screen_reader: Some(true),
            color_inversion: Some(true),
            enable_magnification: Some(true),
            color_correction: Some(crate::accessibility::types::ColorBlindnessType::Protanomaly),
            captions_settings: Some(EXPECTED_CAPTION_SETTINGS),
        };

        let accessibility_settings = AccessibilitySettings {
            audio_description: Some(true),
            screen_reader: Some(true),
            color_inversion: Some(true),
            enable_magnification: Some(true),
            color_correction: Some(ColorBlindnessType::Protanomaly),
            captions_settings: Some(EXPECTED_CAPTION_SETTINGS.into()),
            ..Default::default()
        };

        let request = to_request(accessibility_settings);

        assert_eq!(request, Request::SetAccessibilityInfo(EXPECTED_ACCESSIBILITY_INFO));
    }
}
