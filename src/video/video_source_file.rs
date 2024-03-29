use super::types::*;
use super::video_source::{VideoSource, VideoSourceAvailable};

use gst::glib::object::ObjectExt;
use paperclip::actix::Apiv2Schema;
use serde::{Deserialize, Serialize};

use gst_sys;
use gst_pbutils;
use gst_pbutils::Discoverer;
use gst_pbutils::DiscovererInfo;
use gst_pbutils::prelude::DiscovererStreamInfoExt;

use resolve_path::PathResolveExt;

use tracing::*;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct VideoCaptureConfiguration {
    pub encode: VideoEncodeType,
    pub height: u32,
    pub width: u32,
    pub frame_interval: FrameInterval,
}


#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VideoSourceFile {
    pub name: String,
    pub source: std::path::PathBuf,
    pub configuration: VideoCaptureConfiguration,

}

/*
impl VideoSourceFile {
    pub fn new(name: String, source: std::path::PathBuf) -> Self {
        let discoverer = Discoverer::new(gst::ClockTime::from_seconds(1)).expect("Failed to create discoverer");

        /*
        discoverer.connect_discovered(|_discoverer, info, _error| {
            handle_discovered(info);
        });

        discoverer.start().expect("Failed to start discoverer");
        discoverer.discover_uri(&format!("file://{file_path}")).expect("Failed to discover URI");
        discoverer.stop();
        */

        let file_path = source.to_str().expect("Path to string conversion failed.");
        let info = discoverer.discover_uri(&format!("file://{file_path}")).expect("Failed to discover URI");

        dbg!(info.result());

        let streams = info.stream_info();
        for stream in streams {
            if let Some(caps) = stream.caps() {
                println!("Caps: {}", caps.to_string());
                if let Some(structure) = caps.structure(0) {
                    println!("Codec: {}", structure.name());
                }
            }
        }

        Self { name, source }
    }
}
*/

impl VideoSource for VideoSourceFile {
    fn name(&self) -> &String {
        &self.name
    }

    fn source_string(&self) -> &str {
        self.source.to_str().expect("Path to string conversion failed.")
    }

    fn formats(&self) -> Vec<Format> {
        vec![Format {
            encode: self.configuration.encode.clone(),
            sizes: vec![Size {
                width: self.configuration.width,
                height: self.configuration.height,
                intervals: vec![self.configuration.frame_interval.clone()],
            }],
        }]
    }

    fn set_control_by_name(&self, _control_name: &str, _value: i64) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Redirect source doesn't have controls.",
        ))
    }

    fn set_control_by_id(&self, _control_id: u64, _value: i64) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Redirect source doesn't have controls.",
        ))
    }

    fn control_value_by_name(&self, _control_name: &str) -> std::io::Result<i64> {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Redirect source doesn't have controls.",
        ))
    }

    fn control_value_by_id(&self, _control_id: u64) -> std::io::Result<i64> {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Redirect source doesn't have controls.",
        ))
    }

    fn controls(&self) -> Vec<Control> {
        vec![]
    }

    fn is_valid(&self) -> bool {
        /*
        match &self.source {
            VideoSourceFileType::Redirect(_) => true,
        }
         */
        true
    }

    fn is_shareable(&self) -> bool {
        true
    }
}

// Based on the original result implementation: https://stackoverflow.com/a/49806368
macro_rules! skip_error {
    ($res:expr) => {
        match $res {
            Ok(val) => val,
            Err(e) => {
                warn!("Got an error: {} Continuing..", e);
                continue;
            },
        }
    };
}

macro_rules! skip_none {
    ($opt:expr) => {
        match $opt {
            Some(val) => val,
            None => {
                warn!("Got none. Continuing..");
                continue;
            },
        }
    };
}


impl VideoSourceAvailable for VideoSourceFile {
    fn cameras_available() -> Vec<VideoSourceType> {
        let mut sources: Vec<VideoSourceType> = vec![];
        let root_path = "/home/patrick/git/blue/mavlink-camera-manager/videos".resolve();
        // let root_path = "/home/patrick/Downloads".resolve();
        let discoverer = Discoverer::new(gst::ClockTime::from_seconds(1)).expect("Failed to create discoverer");

        for path in std::fs::read_dir(root_path).unwrap() {
            // let file_path = path.unwrap().path();
            let file_path = path.unwrap().path();
            let file_path_string = file_path.to_str().unwrap();

            info!("Looking at file: {file_path_string}");
            let info = skip_error!(discoverer.discover_uri(&format!("file://{file_path_string}")));
            let stream = skip_none!(info.stream_info());
            let caps = skip_none!(stream.caps());

            for video_info in info.video_streams() {
                dbg!(video_info.bitrate());
                dbg!(video_info.caps());
                dbg!(video_info.depth());
                dbg!(video_info.framerate());
                dbg!(video_info.height());
                dbg!(video_info.width());
                dbg!(video_info.is_image());
                dbg!(video_info.stream_type_nick());
                dbg!(video_info.stream_id());

                let caps = skip_none!(video_info.caps());
                let codec = skip_none!(caps.structure(0)).name();

                sources.push(VideoSourceType::File(VideoSourceFile {
                    name: file_path.file_name().unwrap_or_default().to_str().unwrap_or("no-name").to_string(),
                    source: file_path,
                    configuration: VideoCaptureConfiguration {
                        encode: VideoEncodeType::from_codec(codec),
                        height: video_info.height() as u32,
                        width: video_info.width() as u32,
                        frame_interval: FrameInterval::from(video_info.framerate()),
                    },
                }));
                break;
            }

            /*
            let stream = skip_none!(info.stream_info());
            let caps = skip_none!(stream.caps());
            let structure = skip_none!(caps.structure(0));

            let description: String;
            if caps.is_fixed() {
                description = gst_pbutils::functions::pb_utils_get_codec_description(&caps).into();
            } else {
                description = caps.to_string();
            }

            dbg!(stream.next());

            let codec = structure.name();
            println!("Caps: {}", caps.to_string());
            println!("Codec: {}", codec);
            dbg!(description);
            dbg!(info.duration());
            dbg!(info.tags());
            dbg!(info.stream_list());
            dbg!(info.toc());
            dbg!(stream.stream_type_nick());
            // dbg!(stream.());
            let width = skip_error!(structure.get::<i32>("width"));
            let height = skip_error!(structure.get::<i32>("height"));

            println!("Codec: {}", codec);
            println!("Width: {}, Height: {}", width, height);

            if let Ok(framerate) = structure.get::<gst::Fraction>("framerate") {
                // Frame interval is the inverse of framerate
                let frame_interval = gst::Fraction::new(framerate.denom(), framerate.numer());
                println!("Framerate: {}/{}", framerate.numer(), framerate.denom());
                println!("Frame interval (as a fraction of seconds): {}/{}",
                         frame_interval.numer(), frame_interval.denom());
            }
            */

            /*
            let a = VideoSourceFile {
                name: "file".into(),//file_path.file_name().unwrap_or("no-name".into()).to_str().unwrap_or("no-name").to_string(),
                source: file_path,
                configuration: VideoCaptureConfiguration {
                    encode: VideoEncodeType::Unknown(codec.to_string()),
                    height: height as u32,
                    width: width as u32,
                    frame_interval: FrameInterval {
                        numerator: 1,
                        denominator: 30,
                    }
                },
            };
             */
        }

        sources
        /*
        vec![VideoSourceType::Redirect(VideoSourceFile {
            name: "Redirect source".into(),
            source: VideoSourceFileType::Redirect("Redirect".into()),
        })]*/
    }
}

/*
fn handle_discovered(info: DiscovererInfo) {
    match info.result() {
        Ok(_) => println!("Discovery succeeded"),
        Err(err) => println!("Discovery failed: {}", err),
    }

    let streams = info.stream_info();
    for stream in streams {
        println!("Stream type: {}", stream.stream_type_nick());
        if let Some(caps) = stream.caps() {
            println!("Caps: {}", caps.to_string());
            if let Some(structure) = caps.structure(0) {
                println!("Codec: {}", structure.name());
            }
        }
    }
}*/