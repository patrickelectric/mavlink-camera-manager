use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::Result;
use async_std::stream::StreamExt;
use tracing::*;

use crate::video::types::VideoSourceType;
use crate::video::video_source_redirect::{VideoSourceRedirect, VideoSourceRedirectType};

lazy_static! {
    static ref MANAGER: Arc<Mutex<Manager>> = Default::default();
}

#[derive(Debug)]
pub struct Manager {
    _server_thread_handle: std::thread::JoinHandle<()>,
}

impl Default for Manager {
    #[instrument(level = "trace")]
    fn default() -> Self {
        Self {
            _server_thread_handle: thread::Builder::new()
                .name("Onvif".to_string())
                .spawn(Manager::run_main_loop)
                .expect("Failed spawing Onvif thread"),
        }
    }
}

impl Manager {
    // Construct our manager, should be done inside main
    #[instrument(level = "debug")]
    pub fn init() {
        MANAGER.as_ref();
    }

    #[instrument(level = "debug", fields(endpoint))]
    fn run_main_loop() {
        tokio::runtime::Builder::new_multi_thread()
            .on_thread_start(|| debug!("Thread started"))
            .on_thread_stop(|| debug!("Thread stopped"))
            .thread_name_fn(|| {
                static ATOMIC_ID: std::sync::atomic::AtomicUsize =
                    std::sync::atomic::AtomicUsize::new(0);
                let id = ATOMIC_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                format!("Onvif-{id}")
            })
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("Failed building a new tokio runtime")
            .block_on(Manager::discover_loop())
            .expect("Error starting Onvif server");
    }

    #[instrument(level = "debug")]
    async fn discover_loop() -> Result<()> {
        use futures::stream::StreamExt;
        use std::net::{IpAddr, Ipv4Addr};

        loop {
            info!("Discovering...");

            const MAX_CONCURRENT_JUMPERS: usize = 100;

            onvif::discovery::DiscoveryBuilder::default()
                .listen_address(IpAddr::V4(Ipv4Addr::UNSPECIFIED))
                .duration(tokio::time::Duration::from_secs(5))
                .run()
                .await?
                .for_each_concurrent(MAX_CONCURRENT_JUMPERS, |device| async move {
                    info!("Device found: {device:#?}");

                    // device.
                })
                .await;
        }
    }
}
