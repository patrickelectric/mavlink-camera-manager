use super::types::*;
use super::video_source::{VideoSource, VideoSourceAvailable};

use anyhow::Result;
use onvif::discovery;
use paperclip::actix::Apiv2Schema;
use serde::{Deserialize, Serialize};
use tracing::*;

#[derive(Apiv2Schema, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum VideoSourceRedirectType {
    Redirect(String),
    Onvif(url::Url),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VideoSourceRedirect {
    pub name: String,
    pub source: VideoSourceRedirectType,
}

impl VideoSource for VideoSourceRedirect {
    fn name(&self) -> &String {
        &self.name
    }

    fn source_string(&self) -> &str {
        match &self.source {
            VideoSourceRedirectType::Redirect(string) => string,
            VideoSourceRedirectType::Onvif(url) => url.as_str(),
        }
    }

    fn formats(&self) -> Vec<Format> {
        match &self.source {
            VideoSourceRedirectType::Redirect(_) | VideoSourceRedirectType::Onvif(_) => {
                vec![]
            }
        }
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
        match &self.source {
            VideoSourceRedirectType::Redirect(_) | VideoSourceRedirectType::Onvif(_) => true,
        }
    }

    fn is_shareable(&self) -> bool {
        true
    }
}

impl VideoSourceAvailable for VideoSourceRedirect {
    fn cameras_available() -> Vec<VideoSourceType> {
        let mut sources = vec![VideoSourceType::Redirect(VideoSourceRedirect {
            name: "Redirect source".into(),
            source: VideoSourceRedirectType::Redirect("Redirect".into()),
        })];

        // let (tx, mut rx) = tokio::sync::oneshot::channel();

        // std::thread::spawn(move || {
        //     tokio::task::spawn_blocking(|| async move {
        //         if let Err(error) = tx.send(discover().await) {
        //             error!("Failed sending discovered Onvif sources: {error:?}")
        //         }
        //     });
        // });
        // info!("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");

        // loop {
        //     match rx.try_recv() {
        //         Ok(Ok(onvif_sources)) => {
        //             sources.append(onvif_sources.as_mut());
        //             break;
        //         }
        //         Ok(Err(error)) => error!("Failed discovering Onvif sources: {error:?}"),
        //         Err(_) => (),
        //     }
        //     std::thread::sleep(std::time::Duration::from_millis(100));
        // }
        // info!("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");

        sources
    }
}

// async fn discover() -> Result<Vec<VideoSourceType>> {
//     use futures::stream::iter;
//     use futures::stream::StreamExt;

//     let videos = discovery::DiscoveryBuilder::default()
//         .duration(std::time::Duration::from_secs(5))
//         .listen_address("0.0.0.0".parse().unwrap())
//         .run()
//         .await?
//         .map(|device| {
//             iter(device.urls.into_iter().map(move |url| {
//                 VideoSourceType::Redirect(VideoSourceRedirect {
//                     name: device.name.clone().unwrap_or("Unnamed".into()),
//                     source: VideoSourceRedirectType::Onvif(url.clone()),
//                 })
//             }))
//         })
//         .flatten()
//         .collect()
//         .await;

//     Ok(videos)
// }
