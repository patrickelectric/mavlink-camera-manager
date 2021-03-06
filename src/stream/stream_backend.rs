use super::types::*;
use super::video_stream_udp::VideoStreamUdp;
use crate::video::{
    types::{VideoEncodeType, VideoSourceType},
    video_source_gst::{VideoSourceGst, VideoSourceGstType},
    video_source_local::VideoSourceLocal,
};
use crate::video_stream::types::VideoAndStreamInformation;
use log::*;
use simple_error::SimpleError;

pub trait StreamBackend {
    fn start(&mut self) -> bool;
    fn stop(&mut self) -> bool;
    fn is_running(&self) -> bool;
    fn restart(&mut self);
    fn set_pipeline_description(&mut self, description: &str);
    fn pipeline(&self) -> String;
}

pub fn new(
    video_and_stream_information: &VideoAndStreamInformation,
) -> Result<StreamType, SimpleError> {
    check_endpoints(video_and_stream_information)?;
    check_encode(video_and_stream_information)?;
    check_scheme(video_and_stream_information)?;
    return create_stream(video_and_stream_information);
}

fn check_endpoints(
    video_and_stream_information: &VideoAndStreamInformation,
) -> Result<(), SimpleError> {
    let endpoints = &video_and_stream_information.stream_information.endpoints;

    if endpoints.is_empty() {
        return Err(SimpleError::new("Endpoints are empty".to_string()));
    }

    let endpoints_have_same_scheme = endpoints
        .windows(2)
        .all(|win| win[0].scheme() == win[1].scheme());
    if !endpoints_have_same_scheme {
        return Err(SimpleError::new(format!(
            "Endpoints scheme are not the same: {:#?}",
            endpoints
        )));
    }

    return Ok(());
}

fn check_encode(
    video_and_stream_information: &VideoAndStreamInformation,
) -> Result<(), SimpleError> {
    let encode = video_and_stream_information
        .stream_information
        .configuration
        .encode
        .clone();

    if let VideoEncodeType::UNKNOWN(name) = encode {
        return Err(SimpleError::new(format!(
            "Encode is not supported: {}",
            name
        )));
    }

    if VideoEncodeType::H264 != encode {
        return Err(SimpleError::new(format!(
            "Only H264 encode is supported now, used: {:?}",
            encode
        )));
    }

    return Ok(());
}

fn check_scheme(
    video_and_stream_information: &VideoAndStreamInformation,
) -> Result<(), SimpleError> {
    let endpoints = &video_and_stream_information.stream_information.endpoints;
    let encode = video_and_stream_information
        .stream_information
        .configuration
        .encode
        .clone();
    let scheme = endpoints.first().unwrap().scheme();

    match scheme {
        "rtsp" => {
            if endpoints.len() > 1 {
                return Err(SimpleError::new(format!(
                    "Multiple RTSP endpoints are not acceptable: {:#?}",
                    endpoints
                )));
            }
        }
        "udp" => {
            if VideoEncodeType::H264 != encode {
                return Err(SimpleError::new(format!("Endpoint with udp scheme only supports H264 encode. Encode: {:?}, Endpoints: {:#?}", encode, endpoints)));
            }

            if VideoEncodeType::H265 == encode {
                return Err(SimpleError::new("Endpoint with udp scheme only supports H264, encode type is H265, the scheme should be udp265.".to_string()));
            }

            //UDP endpoints should contain both host and port
            let no_host_or_port = endpoints
                .iter()
                .any(|endpoint| endpoint.host().is_none() || endpoint.port().is_none());

            if no_host_or_port {
                return Err(SimpleError::new(format!(
                    "Endpoint with udp scheme should contain host and port. Endpoints: {:#?}",
                    endpoints
                )));
            }
        }
        "udp265" => {
            if VideoEncodeType::H265 != encode {
                return Err(SimpleError::new(format!("Endpoint with udp265 scheme only supports H265 encode. Encode: {:?}, Endpoints: {:#?}", encode, endpoints)));
            }
        }
        _ => {
            return Err(SimpleError::new(format!(
                "Scheme is not accepted as stream endpoint: {}",
                scheme
            )));
        }
    }

    return Ok(());
}

fn create_udp_stream(
    video_and_stream_information: &VideoAndStreamInformation,
) -> Result<StreamType, SimpleError> {
    let encode = video_and_stream_information
        .stream_information
        .configuration
        .encode
        .clone();
    let endpoints = &video_and_stream_information.stream_information.endpoints;
    let configuration = &video_and_stream_information
        .stream_information
        .configuration;
    let video_source = &video_and_stream_information.video_source;

    let video_format = match video_source {
        VideoSourceType::Local(local_device) => {
            if VideoEncodeType::H264 == encode {
                format!(
                    concat!(
                        "v4l2src device={device}",
                        " ! video/x-h264,width={width},height={height},framerate={interval_denominator}/{interval_numerator}",
                    ),
                    device = &local_device.device_path,
                    width = configuration.width,
                    height = configuration.height,
                    interval_denominator = configuration.frame_interval.denominator,
                    interval_numerator = configuration.frame_interval.numerator,
                )
            } else {
                return Err(SimpleError::new(format!(
                    "Unsupported encode for UDP endpoint: {:?}",
                    encode
                )));
            }
        }
        VideoSourceType::Gst(gst_source) => match &gst_source.source {
            VideoSourceGstType::Fake(pattern) => {
                format!(
                        concat!(
                            "videotestsrc pattern={pattern}",
                            " ! video/x-raw,width={width},height={height},framerate={interval_denominator}/{interval_numerator}",
                            " ! videoconvert",
                            " !  x264enc bitrate=5000",
                            " ! video/x-h264, profile=baseline",
                        ),
                        pattern = pattern,
                        width = configuration.width,
                        height = configuration.height,
                        interval_denominator = configuration.frame_interval.denominator,
                        interval_numerator = configuration.frame_interval.numerator,
                    )
            }
            _ => {
                return Err(SimpleError::new(format!(
                    "Unsupported GST source for UDP endpoint: {:#?}",
                    gst_source
                )));
            }
        },
    };

    if VideoEncodeType::H264 == encode {
        let udp_encode = concat!(
            " ! h264parse",
            " ! queue",
            " ! rtph264pay config-interval=10 pt=96",
        );

        let clients: Vec<String> = endpoints
            .iter()
            .map(|endpoint| format!("{}:{}", endpoint.host().unwrap(), endpoint.port().unwrap()))
            .collect();
        let clients = clients.join(",");

        let udp_sink = format!(" ! multiudpsink clients={}", clients);

        let pipeline = [&video_format, udp_encode, &udp_sink].join("");
        info!("Created pipeline: {}", pipeline);
        let mut stream = VideoStreamUdp::default();
        stream.set_pipeline_description(&pipeline);
        return Ok(StreamType::UDP(stream));
    }

    return Err(SimpleError::new(format!(
        "Unsupported encode: {:?}",
        encode
    )));
}

fn create_stream(
    video_and_stream_information: &VideoAndStreamInformation,
) -> Result<StreamType, SimpleError> {
    // The scheme was validated by "new" function
    let endpoint = &video_and_stream_information
        .stream_information
        .endpoints
        .iter()
        .next()
        .unwrap();
    match endpoint.scheme() {
        "udp" => create_udp_stream(video_and_stream_information),
        something => Err(SimpleError::new(format!(
            "Unsupported scheme: {}",
            something
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video::{
        types::{CaptureConfiguration, FrameInterval},
        video_source_local::{VideoSourceLocal, VideoSourceLocalType},
    };

    use url::Url;

    #[test]
    fn test_udp() {
        let result = create_stream(&VideoAndStreamInformation {
            name: "Test".into(),
            stream_information: StreamInformation {
                endpoints: vec![Url::parse("udp://192.168.0.1:42").unwrap()],
                configuration: CaptureConfiguration {
                    encode: VideoEncodeType::H264,
                    height: 720,
                    width: 1080,
                    frame_interval: FrameInterval {
                        numerator: 1,
                        denominator: 30,
                    },
                },
            },
            video_source: VideoSourceType::Local(VideoSourceLocal {
                name: "PotatoCam".into(),
                device_path: "/dev/video42".into(),
                typ: VideoSourceLocalType::Unknown("TestPotatoCam".into()),
            }),
        });

        assert!(result.is_ok());
        let result = result.unwrap();

        let StreamType::UDP(video_stream_udp) = result;
        assert_eq!(video_stream_udp.pipeline(), "v4l2src device=/dev/video42 ! video/x-h264,width=1080,height=720,framerate=30/1 ! h264parse ! queue ! rtph264pay config-interval=10 pt=96 ! multiudpsink clients=192.168.0.1:42");
    }
}
