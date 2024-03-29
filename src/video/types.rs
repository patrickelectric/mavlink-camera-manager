use std::fmt;

use super::video_source::VideoSource;
use super::video_source_file::VideoSourceFile;
use super::video_source_gst::VideoSourceGst;
use super::video_source_local::VideoSourceLocal;
use super::video_source_redirect::VideoSourceRedirect;
use gst;
use paperclip::actix::Apiv2Schema;
use serde::{de::{self, Visitor}, Deserialize, Deserializer, Serialize};

#[derive(Apiv2Schema, Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum VideoSourceType {
    Gst(VideoSourceGst),
    Local(VideoSourceLocal),
    File(VideoSourceFile),
    Redirect(VideoSourceRedirect),
}

#[derive(Apiv2Schema, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum VideoEncodeType {
    Unknown(String),
    H265,
    H264,
    Mjpg,
    Yuyv,
}

#[derive(Apiv2Schema, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
pub struct Format {
    pub encode: VideoEncodeType,
    pub sizes: Vec<Size>,
}

#[derive(Apiv2Schema, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
pub struct Size {
    pub width: u32,
    pub height: u32,
    pub intervals: Vec<FrameInterval>,
}

#[derive(Apiv2Schema, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
pub struct FrameInterval {
    pub numerator: u32,
    pub denominator: u32,
}

impl From<gst::Fraction> for FrameInterval {
    fn from(fraction: gst::Fraction) -> Self {
        FrameInterval {
            numerator: fraction.numer() as u32,
            denominator: fraction.denom() as u32,
        }
    }
}

#[derive(Apiv2Schema, Clone, Debug, Default, Serialize)]
pub struct Control {
    pub name: String,
    pub cpp_type: String,
    pub id: u64,
    pub state: ControlState,
    pub configuration: ControlType,
}

#[derive(Apiv2Schema, Clone, Debug, Serialize)]
pub enum ControlType {
    Bool(ControlBool),
    Slider(ControlSlider),
    Menu(ControlMenu),
}

#[derive(Apiv2Schema, Clone, Debug, Default, Serialize)]
pub struct ControlState {
    pub is_disabled: bool,
    pub is_inactive: bool,
}

#[derive(Apiv2Schema, Clone, Debug, Serialize)]
pub struct ControlBool {
    pub default: i64,
    pub value: i64,
}

#[derive(Apiv2Schema, Clone, Debug, Serialize)]
pub struct ControlSlider {
    pub default: i64,
    pub value: i64,
    pub step: u64,
    pub max: i64,
    pub min: i64,
}

#[derive(Apiv2Schema, Clone, Debug, Serialize)]
pub struct ControlMenu {
    pub default: i64,
    pub value: i64,
    pub options: Vec<ControlOption>,
}

#[derive(Apiv2Schema, Clone, Debug, Serialize)]
pub struct ControlOption {
    pub name: String,
    pub value: i64,
}

impl VideoSourceType {
    pub fn inner(&self) -> &(dyn VideoSource + '_) {
        match self {
            VideoSourceType::File(file) => file,
            VideoSourceType::Local(local) => local,
            VideoSourceType::Gst(gst) => gst,
            VideoSourceType::Redirect(redirect) => redirect,
        }
    }
}

impl VideoEncodeType {
    //TODO: use trait fromstr, check others places
    pub fn from_str(fourcc: &str) -> VideoEncodeType {
        let fourcc = fourcc.to_uppercase();
        match fourcc.as_str() {
            "H264" => VideoEncodeType::H264,
            "MJPG" => VideoEncodeType::Mjpg,
            "YUYV" => VideoEncodeType::Yuyv,
            _ => VideoEncodeType::Unknown(fourcc),
        }
    }

    pub fn to_codec(self) -> String {
        match self {
            VideoEncodeType::H264 => "video/x-h264",
            VideoEncodeType::H265 => "video/x-h265",
            // TODO: We need to handle the mpeg version one day, but not today
            VideoEncodeType::Mjpg => "video/mpeg",
            VideoEncodeType::Yuyv => "video/x-raw,format=I420",
            VideoEncodeType::Unknown(codec) => {
                return codec;
            }
        }.to_string()
    }

    pub fn from_codec(codec: &str) -> VideoEncodeType {
        match codec {
            "video/x-h264" => VideoEncodeType::H264,
            "video/x-h265" => VideoEncodeType::H265,
            // TODO: We need to handle the mpeg version one day, but not today
            "video/mpeg" => VideoEncodeType::Mjpg,
            "video/x-raw,format=I420" => VideoEncodeType::Yuyv,
            codec => VideoEncodeType::Unknown(codec.to_string()),
        }
    }
}

impl Default for ControlType {
    fn default() -> Self {
        ControlType::Bool(ControlBool {
            default: 0,
            value: 0,
        })
    }
}

struct VideoEncodeTypeVisitor;

impl<'de> Visitor<'de> for VideoEncodeTypeVisitor {
    type Value = VideoEncodeType;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a string representing a video encoding type")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let variant = match value.to_uppercase().as_str() {
            "H265" => VideoEncodeType::H265,
            "H264" => VideoEncodeType::H264,
            "MJPG" => VideoEncodeType::Mjpg,
            "YUYV" => VideoEncodeType::Yuyv,
            _ => VideoEncodeType::Unknown(value.to_owned()),
        };
        Ok(variant)
    }
}

// Implementing the Deserialize trait for VideoEncodeType
impl<'de> Deserialize<'de> for VideoEncodeType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(VideoEncodeTypeVisitor)
    }
}

pub static STANDARD_SIZES: &[(u32, u32); 16] = &[
    (7680, 4320),
    (7200, 3060),
    (3840, 2160),
    (2560, 1440),
    (1920, 1080),
    (1600, 1200),
    (1440, 1080),
    (1280, 1080),
    (1280, 720),
    (1024, 768),
    (960, 720),
    (800, 600),
    (640, 480),
    (640, 360),
    (320, 240),
    (256, 144),
];
