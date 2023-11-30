use std::thread;
use core::time::Duration;
use thirtyfour::prelude::*;
use tracing::*;
use tokio::runtime::Runtime;
use crate::helper;

pub fn start_check_tasks_on_webrtc_reconnects() {
    let mut counter = 0;
    thread::spawn(move || {
        let rt = Runtime::new().unwrap();
        rt.block_on(async move {
            info!("Started webrtc test..");
            let mut caps = DesiredCapabilities::chrome();
            let _ = caps.set_headless();
            let driver = WebDriver::new("http://localhost:9515", caps).await.expect("Failed to create web driver.");
            driver.goto("http://0.0.0.0:6020/webrtc/index.html").await.expect("Failed to connect to local webrtc page.");
            loop {
                for button in ["add-consumer", "add-session", "remove-all-consumers"] {
                    thread::sleep(Duration::from_secs(3));
                    driver.find(By::Id(button)).await.unwrap().click().await.unwrap();
                }
                counter += 1;
                info!("Restarted webrtc {} times", counter);
                if helper::threads::process_task_counter() > 100 {
                    error!("Thead leak detected!");
                    std::process::exit(-1);
                }
            }
        });
        error!("Webrtc test failed internally.");
        std::process::exit(-1);
    });
}
