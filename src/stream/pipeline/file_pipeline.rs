use crate::{
    stream::types::CaptureConfiguration,
    video::{
        types::{VideoEncodeType, VideoSourceType},
        video_source_gst::VideoSourceGstType,
    },
    video_stream::types::VideoAndStreamInformation,
};

use super::{
    PipelineGstreamerInterface, PipelineState, PIPELINE_FILTER_NAME, PIPELINE_RTP_TEE_NAME,
    PIPELINE_VIDEO_TEE_NAME,
};

use anyhow::{anyhow, Result};

use resolve_path::PathResolveExt;
use tracing::*;

use gst::prelude::*;

#[derive(Debug)]
pub struct FilePipeline {
    pub state: PipelineState,
}

impl FilePipeline {
    #[instrument(level = "debug")]
    pub fn try_new(
        pipeline_id: &uuid::Uuid,
        video_and_stream_information: &VideoAndStreamInformation,
    ) -> Result<gst::Pipeline> {
        let configuration = match &video_and_stream_information
            .stream_information
            .configuration
        {
            CaptureConfiguration::Video(configuration) => configuration,
            unsupported => {
                return Err(anyhow!("{unsupported:?} is not supported as Fake Pipeline"))
            }
        };

        let video_source = match &video_and_stream_information.video_source {
            VideoSourceType::File(source) => source,
            unsupported => {
                return Err(anyhow!(
                    "SourceType {unsupported:?} is not supported as Fake Pipeline"
                ))
            }
        };

        let filter_name = format!("{PIPELINE_FILTER_NAME}-{pipeline_id}");
        let video_tee_name = format!("{PIPELINE_VIDEO_TEE_NAME}-{pipeline_id}");
        let rtp_tee_name = format!("{PIPELINE_RTP_TEE_NAME}-{pipeline_id}");

        let width = configuration.width;
        let height = configuration.height;
        let encode = &configuration.encode;

        if width % 2 != 0 && height % 2 != 0 {
            return Err(anyhow!(
                "Width and height must be multiples of 2, but got width: {width}, and height: {height}"
            ));
        };

        if !encode.to_string().contains("image") {
            return Err(anyhow!(
                "Format {encode:?} not supported, only images are supported at the moment."
            ));
        };

        let description = format!(
            concat!(
                "multifilesrc location={source}",
                " ! decodebin",
                " ! videoconvert",
                " ! imagefreeze",
                " ! videobox",
                " ! video/x-raw,format=I420,width={width},height={height},framerate=30/1",
                " ! x264enc",
                " ! capsfilter name={filter_name} caps=video/x-h264,width={width},height={height},framerate=30/1",
                " ! tee name={video_tee_name} allow-not-linked=true",
                " ! rtph264pay config-interval=1 pt=96",
                " ! tee name={rtp_tee_name} allow-not-linked=true",
            ),
            source = video_source.source.clone().into_os_string().into_string().unwrap(),
            filter_name = filter_name,
            width = width,
            height = height,
            video_tee_name = video_tee_name,
            rtp_tee_name = rtp_tee_name,
        );

        debug!("Running pipeline: {description:#?}");
        let pipeline = gst::parse::launch(&description)?;

        let pipeline = pipeline
            .downcast::<gst::Pipeline>()
            .expect("Couldn't downcast pipeline");

        if false {
            let rtp_payloader = match &configuration.encode {
                VideoEncodeType::H265 => "rtph265pay".to_string(),
                VideoEncodeType::H264 => "rtph264pay".to_string(),
                VideoEncodeType::Mjpg => "rtpjpegpay".to_string(),
                VideoEncodeType::Yuyv => "rtpvrawpay".to_string(),
                // Well, lets try to encode and see how it goes!
                other => {
                    warn!("Format {other:?} nor supported, going to use rtpjpegpay instead.");
                    "rtpjpegpay".to_string()
                }
            };

            // Fakes (videotestsrc) are only "video/x-raw" or "video/x-bayer",
            // and to be able to encode it, we need to define an available
            // format for both its src the next element's sink pad.
            // We are choosing "UYVY" because it is compatible with the
            // application-rtp template capabilities.
            // For more information: https://gstreamer.freedesktop.org/documentation/additional/design/mediatype-video-raw.html?gi-language=c#formats
            /*
            let description = format!(
                concat!(
                    // Because application-rtp templates doesn't accept "YUY2", we
                    // need to transcode it. We are arbitrarily chosing the closest
                    // format available ("UYVY").
                    " multifilesrc location=\"{source}\" loop=true",
                    " ! decodebin3",
                    // " ! video/x-raw,format=I420",
                    //" ! capsfilter name={filter_name} caps={encode},width={width},height={height},framerate={interval_denominator}/{interval_numerator}",
                    " ! tee name={video_tee_name} allow-not-linked=true",
                    " ! {rtp_payloader} pt=96",
                    " ! tee name={rtp_tee_name} allow-not-linked=true",
                ),
                source = video_source.source.clone().into_os_string().into_string().unwrap(),
                // encode = video_source.configuration.encode.clone().to_codec(),
                // width = configuration.width,
                // height = configuration.height,
                // interval_denominator = configuration.frame_interval.denominator,
                // interval_numerator = configuration.frame_interval.numerator,
                // filter_name = filter_name,
                video_tee_name = video_tee_name,
                rtp_payloader = rtp_payloader,
                rtp_tee_name = rtp_tee_name,
            );*/

            let description = format!(
                concat!(
                    " filesrc location={source}",
                    " ! qtdemux ! video/x-h264 ! queue",
                    " ! tee name={video_tee_name} allow-not-linked=true",
                    " ! rtph264pay config-interval=1 pt=96",
                    " ! tee name={rtp_tee_name} allow-not-linked=true",
                ),
                source = video_source.source.clone().into_os_string().into_string().unwrap(),
                video_tee_name = video_tee_name,
                rtp_tee_name = rtp_tee_name,
            );

            debug!("Running pipeline: {description:#?}");
            let pipeline = gst::parse::launch(&description)?;

            let pipeline = pipeline
                .downcast::<gst::Pipeline>()
                .expect("Couldn't downcast pipeline");
        }

        Ok(pipeline)
    }
}

impl PipelineGstreamerInterface for FilePipeline {
    #[instrument(level = "trace")]
    fn is_running(&self) -> bool {
        self.state.pipeline_runner.is_running()
    }
}
