use std::{
    collections::HashMap,
    ops::DerefMut,
    sync::{Arc, Mutex},
};

use crate::{settings, stream::webrtc::signalling_protocol::BindAnswer};
use crate::{stream::sink::sink::SinkInterface, video::types::VideoSourceType};
use crate::{
    stream::sink::{sink::Sink, webrtc_sink::WebRTCSink},
    video_stream::types::VideoAndStreamInformation,
};

use anyhow::{anyhow, Context, Result};

use gst::{event::Eos, prelude::ElementExtManual, traits::ElementExt};
use tracing::*;

use super::{
    pipeline::pipeline::PipelineGstreamerInterface,
    stream::Stream,
    types::StreamStatus,
    webrtc::{
        self,
        signalling_protocol::RTCSessionDescription,
        signalling_server::{StreamManagementInterface, WebRTCSessionManagementInterface},
        webrtcbin_interface::WebRTCBinInterface,
    },
};

#[derive(Default)]
pub struct Manager {
    streams: HashMap<uuid::Uuid, Stream>,
}

lazy_static! {
    static ref MANAGER: Arc<Mutex<Manager>> = Arc::new(Mutex::new(Manager::default()));
}

impl Manager {
    fn update_settings(&self) {
        let video_and_stream_informations = self
            .streams
            .values()
            .map(|stream| stream.video_and_stream_information.clone())
            .collect();

        settings::manager::set_streams(&video_and_stream_informations);
    }
}

#[instrument(level = "debug")]
pub fn init() {
    debug!("Starting video stream service.");

    if let Err(error) = gst::init() {
        error!("Error! {error}");
    };

    config_gst_plugins();
}

#[instrument(level = "debug")]
fn config_gst_plugins() {
    let plugins_config = crate::cli::manager::gst_feature_rank();

    for config in plugins_config {
        match crate::stream::gst::utils::set_plugin_rank(config.name.as_str(), config.rank) {
            Ok(_) => info!(
                "Gstreamer Plugin {name:#?} configured with rank {rank:#?}.",
                name = config.name,
                rank = config.rank,
            ),
            Err(error) => error!("Error when trying to configure plugin {name:#?} rank to {rank:#?}. Reason: {error}", name = config.name, rank = config.rank, error=error.to_string()),
        }
    }
}

#[instrument(level = "debug")]
pub fn start_default() {
    MANAGER.as_ref().lock().unwrap().streams.clear();

    let mut streams = settings::manager::streams();

    // Update all local video sources to make sure that is available
    streams.iter_mut().for_each(|stream| {
        if let VideoSourceType::Local(source) = &mut stream.video_source {
            if !source.update_device() {
                error!("Source appears to be invalid or not found: {source:#?}");
            }
        }
    });

    // Remove all invalid video_sources
    let streams: Vec<VideoAndStreamInformation> = streams
        .into_iter()
        .filter(|stream| stream.video_source.inner().is_valid())
        .collect();

    debug!("streams: {streams:#?}");

    for stream in streams {
        add_stream_and_start(stream).unwrap_or_else(|error| {
            error!("Not possible to start stream: {error}");
        });
    }
}

#[instrument(level = "debug")]
pub fn streams() -> Vec<StreamStatus> {
    Manager::streams_information()
}

#[instrument(level = "debug")]
pub fn add_stream_and_start(video_and_stream_information: VideoAndStreamInformation) -> Result<()> {
    let stream = Stream::try_new(&video_and_stream_information)?;

    let manager = MANAGER.as_ref().lock().unwrap();
    for stream in manager.streams.values() {
        stream
            .video_and_stream_information
            .conflicts_with(&video_and_stream_information)?
    }
    drop(manager);
    Manager::add_stream(stream)?;

    Ok(())
}

#[instrument(level = "debug")]
pub fn remove_stream_by_name(stream_name: &str) -> Result<()> {
    let manager = MANAGER.as_ref().lock().unwrap();
    if let Some(stream_id) = &manager.streams.iter().find_map(|(id, stream)| {
        if stream.video_and_stream_information.name == *stream_name {
            return Some(id.clone());
        }
        return None;
    }) {
        drop(manager);
        Manager::remove_stream(stream_id)?;
        return Ok(());
    }

    Err(anyhow!("Stream named {stream_name:#?} not found"))
}

impl WebRTCSessionManagementInterface for Manager {
    #[instrument(level = "debug")]
    fn add_session(
        bind: &webrtc::signalling_protocol::BindOffer,
        sender: tokio::sync::mpsc::UnboundedSender<Result<webrtc::signalling_protocol::Message>>,
    ) -> Result<webrtc::signalling_protocol::SessionId> {
        let mut guard = MANAGER.lock().unwrap();
        let manager = guard.deref_mut();

        let producer_id = bind.producer_id;
        let consumer_id = bind.consumer_id;
        let session_id = Self::generate_uuid();

        let stream = manager.streams.get_mut(&producer_id).context(format!(
            "Cannot find any stream with producer {producer_id:#?}"
        ))?;

        let bind = BindAnswer {
            producer_id,
            consumer_id,
            session_id,
        };

        let sink = Sink::WebRTC(WebRTCSink::try_new(bind.clone(), sender)?);
        stream.pipeline.add_sink(sink)?;
        debug!("WebRTC session created: {session_id:#?}");

        Ok(session_id)
    }

    #[instrument(level = "debug")]
    fn remove_session(
        bind: &webrtc::signalling_protocol::BindAnswer,
        _reason: String,
    ) -> Result<()> {
        let mut manager = MANAGER.lock().unwrap();

        let stream = manager
            .streams
            .get_mut(&bind.producer_id)
            .context(format!("Producer {} not found", bind.producer_id))?;

        stream
            .pipeline
            .inner_state_mut()
            .remove_sink(&bind.session_id)
            .context(format!("Cannot remove session {}", bind.session_id))?;

        info!("Session {} successfully removed!", bind.session_id);

        Ok(())
    }

    #[instrument(level = "debug")]
    fn handle_sdp(
        bind: &webrtc::signalling_protocol::BindAnswer,
        sdp: &webrtc::signalling_protocol::RTCSessionDescription,
    ) -> Result<()> {
        let manager = MANAGER.lock().unwrap();

        let sink = manager
            .streams
            .get(&bind.producer_id)
            .context(format!("Producer {} not found", bind.producer_id))?
            .pipeline
            .inner_state_as_ref()
            .sinks
            .get(&bind.session_id)
            .context(format!(
                "Session {} not found in producer {}",
                bind.producer_id, bind.producer_id
            ))?;

        let session = match sink {
            Sink::WebRTC(webrtcsink) => webrtcsink,
            _ => return Err(anyhow!("Only Sink::WebRTC accept SDP")),
        };

        let (sdp, sdp_type) = match sdp {
            RTCSessionDescription::Answer(answer) => {
                (answer.sdp.clone(), gst_webrtc::WebRTCSDPType::Answer)
            }
            RTCSessionDescription::Offer(offer) => {
                (offer.sdp.clone(), gst_webrtc::WebRTCSDPType::Offer)
            }
        };

        let sdp = gst_sdp::SDPMessage::parse_buffer(sdp.as_bytes())?;
        let sdp = gst_webrtc::WebRTCSessionDescription::new(sdp_type, sdp);
        session.handle_sdp(&sdp)
    }

    #[instrument(level = "debug")]
    fn handle_ice(
        bind: &webrtc::signalling_protocol::BindAnswer,
        sdp_m_line_index: u32,
        candidate: &str,
    ) -> Result<()> {
        let manager = MANAGER.lock().unwrap();

        let sink = manager
            .streams
            .get(&bind.producer_id)
            .context(format!("Producer {} not found", bind.producer_id))?
            .pipeline
            .inner_state_as_ref()
            .sinks
            .get(&bind.session_id)
            .context(format!(
                "Session {} not found in producer {}",
                bind.producer_id, bind.producer_id
            ))?;

        let session = match sink {
            Sink::WebRTC(webrtcsink) => webrtcsink,
            _ => return Err(anyhow!("Only Sink::WebRTC accept SDP")),
        };

        session.handle_ice(&sdp_m_line_index, candidate)
    }
}

impl StreamManagementInterface<StreamStatus> for Manager {
    #[instrument(level = "debug")]
    fn add_stream(stream: super::stream::Stream) -> Result<()> {
        let mut manager = MANAGER.lock().unwrap();

        let stream_id = stream.id.clone();
        if manager.streams.insert(stream_id, stream).is_some() {
            return Err(anyhow!("Failed adding stream {stream_id}"));
        }
        manager.update_settings();

        Ok(())
    }

    #[instrument(level = "debug")]
    fn remove_stream(stream_id: &webrtc::signalling_protocol::PeerId) -> Result<()> {
        let mut manager = MANAGER.lock().unwrap();

        if !manager.streams.contains_key(stream_id) {
            return Err(anyhow!("Already removed"));
        }

        let stream = manager
            .streams
            .remove(&stream_id)
            .context(format!("Stream {stream_id} not found"))?;
        manager.update_settings();
        drop(manager);

        let pipeline = &stream.pipeline.inner_state_as_ref().pipeline;
        let pipeline_id = stream_id;
        pipeline.send_event(Eos::new());

        if let Err(error) = stream
            .pipeline
            .inner_state_as_ref()
            .pipeline
            .set_state(gst::State::Null)
        {
            error!("Failed setting Pipeline {pipeline_id} state to NULL. Reason: {error:#?}");
        }
        while pipeline.current_state() != gst::State::Null {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        // Unlink all Sinks
        stream
            .pipeline
            .inner_state_as_ref()
            .sinks
            .values()
            .for_each(|sink| {
                if let Err(error) = sink.unlink(pipeline, pipeline_id) {
                    warn!(
                        "Failed unlinking Sink {} from Pipeline {pipeline_id}. Reason: {error:#?}",
                        sink.get_id()
                    );
                }
            });

        info!("Stream {stream_id} successfully removed!");

        Ok(())
    }

    #[instrument(level = "debug")]
    fn streams_information() -> Vec<StreamStatus> {
        MANAGER
            .lock()
            .unwrap()
            .streams
            .values()
            .map(|stream| StreamStatus {
                id: stream.id,
                running: stream.pipeline.is_running(),
                video_and_stream: stream.video_and_stream_information.clone(),
            })
            .collect()
    }

    #[instrument(level = "debug")]
    fn generate_uuid() -> uuid::Uuid {
        uuid::Uuid::new_v4()
    }
}
