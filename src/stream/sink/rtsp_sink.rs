use anyhow::{anyhow, Context, Result};

use tracing::*;

use gst::prelude::*;

use super::sink::SinkInterface;
use crate::stream::{pipeline::pipeline::PIPELINE_TEE_NAME, rtsp::rtsp_server::RTSPServer};

#[derive(Debug)]
pub struct RtspSink {
    sink_id: uuid::Uuid,
    queue: gst::Element,
    path: String,
    tee_src_pad: Option<gst::Pad>,
}
impl SinkInterface for RtspSink {
    #[instrument(level = "debug")]
    fn link(
        &mut self,
        pipeline: &gst::Pipeline,
        pipeline_id: &uuid::Uuid,
        tee_src_pad: gst::Pad,
    ) -> Result<()> {
        let sink_id = &self.get_id();

        // Set Tee's src pad
        if self.tee_src_pad.is_some() {
            return Err(anyhow!(
                "Tee's src pad from RtspSink {sink_id} has already been configured"
            ));
        }
        self.tee_src_pad.replace(tee_src_pad);
        let Some(tee_src_pad) = &self.tee_src_pad else {
            unreachable!()
        };

        // Add the Sink elements to the Pipeline
        let elements = &[&self.queue];
        if let Err(error) = pipeline.add_many(elements) {
            return Err(anyhow!(
                "Failed to add WebRTCBin {sink_id} to Pipeline {pipeline_id}. Reason: {error:#?}"
            ));
        }

        // Link the new Tee's src pad to the queue's sink pad
        let queue_sink_pad = &self
            .queue
            .static_pad("sink")
            .expect("No src pad found on Queue");
        if let Err(error) = tee_src_pad.link(queue_sink_pad) {
            pipeline.remove_many(elements)?;
            return Err(anyhow!(error));
        }

        let pipeline = self
            .queue
            .parent()
            .unwrap()
            .downcast::<gst::Pipeline>()
            .unwrap();

        RTSPServer::add_pipeline(&pipeline, &self.path)?;

        RTSPServer::start_pipeline(&self.path)?;

        Ok(())
    }

    #[instrument(level = "debug")]
    fn unlink(&self, pipeline: &gst::Pipeline, pipeline_id: &uuid::Uuid) -> Result<()> {
        let sink_id = self.get_id();

        let Some(tee_src_pad) = &self.tee_src_pad else {
            warn!("Tried to unlink sink {sink_id} from pipeline {pipeline_id} without a Tee src pad.");
            return Ok(());
        };

        let queue_sink_pad = self
            .queue
            .static_pad("sink")
            .expect("No sink pad found on Queue");
        if let Err(error) = tee_src_pad.unlink(&queue_sink_pad) {
            return Err(anyhow!(
                "Failed unlinking Sink {sink_id} from Tee's source pad. Reason: {error:?}"
            ));
        }
        drop(queue_sink_pad);

        let elements = &[&self.queue];
        if let Err(error) = pipeline.remove_many(elements) {
            return Err(anyhow!(
                "Failed removing RtspSrc element {sink_id} from Pipeline {pipeline_id}. Reason: {error:?}"
            ));
        }

        if let Err(error) = self.queue.set_state(gst::State::Null) {
            return Err(anyhow!(
                "Failed to set queue from sink {sink_id} state to NULL. Reason: {error:#?}"
            ));
        }

        let tee = pipeline
            .by_name(PIPELINE_TEE_NAME)
            .context(format!("no element named {PIPELINE_TEE_NAME:#?}"))?;
        if let Err(error) = tee.remove_pad(tee_src_pad) {
            return Err(anyhow!(
                "Failed removing Tee's source pad. Reason: {error:?}"
            ));
        }

        RTSPServer::stop_pipeline(&self.path)?;

        Ok(())
    }

    #[instrument(level = "debug")]
    fn get_id(&self) -> uuid::Uuid {
        self.sink_id.clone()
    }
}

impl RtspSink {
    #[instrument(level = "debug")]
    pub fn try_new(id: uuid::Uuid, addresses: Vec<url::Url>) -> Result<Self> {
        let queue = gst::ElementFactory::make("queue")
            .name("pay0")
            .property_from_str("leaky", "downstream") // Throw away any data
            .property("flush-on-eos", true)
            .property("max-size-buffers", 0u32) // Disable buffers
            .build()?;

        let path = addresses
            .iter()
            .find_map(|address| (address.scheme() == "rtsp").then_some(address.path().to_string()))
            .context(
                "Failed to find RTSP compatible address. Example: \"rtsp://0.0.0.0:8554/test\"",
            )?;

        Ok(Self {
            sink_id: id,
            queue,
            path,
            tee_src_pad: Default::default(),
        })
    }
}
