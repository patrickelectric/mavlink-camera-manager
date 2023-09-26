use onvif::soap;

use anyhow::{Context, Result};
use tracing::debug;
use url::Url;

struct Clients {
    devicemgmt: soap::client::Client,
    event: Option<soap::client::Client>,
    deviceio: Option<soap::client::Client>,
    media: Option<soap::client::Client>,
    media2: Option<soap::client::Client>,
    imaging: Option<soap::client::Client>,
    ptz: Option<soap::client::Client>,
    analytics: Option<soap::client::Client>,
}

struct Auth {
    credentials: Option<soap::client::Credentials>,
    url: Box<str>,
}

impl Clients {
    async fn try_new(auth: &Auth) -> Result<Self> {
        let creds = auth.credentials;

        let devicemgmt_uri = Url::parse(&auth.url)?;
        let base_uri = devicemgmt_uri.host_str().context("No host")?;

        let mut out = Self {
            devicemgmt: soap::client::ClientBuilder::new(&devicemgmt_uri)
                .credentials(creds.clone())
                .build(),
            imaging: None,
            ptz: None,
            event: None,
            deviceio: None,
            media: None,
            media2: None,
            analytics: None,
        };

        let services =
            schema::devicemgmt::get_services(&out.devicemgmt, &Default::default()).await?;

        for service in &services.service {
            let service_url = Url::parse(&service.x_addr).map_err(|e| e.to_string())?;
            if !service_url.as_str().starts_with(base_uri.as_str()) {
                return Err(format!(
                    "Service URI {service_url:?} is not within base URI {base_uri:?}"
                ));
            }
            let svc = Some(
                soap::client::ClientBuilder::new(&service_url)
                    .credentials(creds.clone())
                    .build(),
            );
            match service.namespace.as_str() {
                "http://www.onvif.org/ver10/device/wsdl" => {
                    if service_url != devicemgmt_uri {
                        return Err(format!(
                            "advertised device mgmt uri {service_url:?} not expected {devicemgmt_uri:?}"
                        ));
                    }
                }
                "http://www.onvif.org/ver10/events/wsdl" => out.event = svc,
                "http://www.onvif.org/ver10/deviceIO/wsdl" => out.deviceio = svc,
                "http://www.onvif.org/ver10/media/wsdl" => out.media = svc,
                "http://www.onvif.org/ver20/media/wsdl" => out.media2 = svc,
                "http://www.onvif.org/ver20/imaging/wsdl" => out.imaging = svc,
                "http://www.onvif.org/ver20/ptz/wsdl" => out.ptz = svc,
                "http://www.onvif.org/ver20/analytics/wsdl" => out.analytics = svc,
                _ => debug!("unknown service: {:?}", service),
            }
        }

        Ok(out)
    }
}

async fn get_capabilities(clients: &Clients) {
    match schema::devicemgmt::get_capabilities(&clients.devicemgmt, &Default::default()).await {
        Ok(capabilities) => println!("{:#?}", capabilities),
        Err(error) => println!("Failed to fetch capabilities: {}", error),
    }
}

async fn get_device_information(clients: &Clients) -> Result<(), transport::Error> {
    println!(
        "{:#?}",
        &schema::devicemgmt::get_device_information(&clients.devicemgmt, &Default::default())
            .await?
    );
    Ok(())
}

async fn get_service_capabilities(clients: &Clients) {
    match schema::event::get_service_capabilities(&clients.devicemgmt, &Default::default()).await {
        Ok(capability) => println!("devicemgmt: {:#?}", capability),
        Err(error) => println!("Failed to fetch devicemgmt: {}", error),
    }

    if let Some(ref event) = clients.event {
        match schema::event::get_service_capabilities(event, &Default::default()).await {
            Ok(capability) => println!("event: {:#?}", capability),
            Err(error) => println!("Failed to fetch event: {}", error),
        }
    }
    if let Some(ref deviceio) = clients.deviceio {
        match schema::event::get_service_capabilities(deviceio, &Default::default()).await {
            Ok(capability) => println!("deviceio: {:#?}", capability),
            Err(error) => println!("Failed to fetch deviceio: {}", error),
        }
    }
    if let Some(ref media) = clients.media {
        match schema::event::get_service_capabilities(media, &Default::default()).await {
            Ok(capability) => println!("media: {:#?}", capability),
            Err(error) => println!("Failed to fetch media: {}", error),
        }
    }
    if let Some(ref media2) = clients.media2 {
        match schema::event::get_service_capabilities(media2, &Default::default()).await {
            Ok(capability) => println!("media2: {:#?}", capability),
            Err(error) => println!("Failed to fetch media2: {}", error),
        }
    }
    if let Some(ref imaging) = clients.imaging {
        match schema::event::get_service_capabilities(imaging, &Default::default()).await {
            Ok(capability) => println!("imaging: {:#?}", capability),
            Err(error) => println!("Failed to fetch imaging: {}", error),
        }
    }
    if let Some(ref ptz) = clients.ptz {
        match schema::event::get_service_capabilities(ptz, &Default::default()).await {
            Ok(capability) => println!("ptz: {:#?}", capability),
            Err(error) => println!("Failed to fetch ptz: {}", error),
        }
    }
    if let Some(ref analytics) = clients.analytics {
        match schema::event::get_service_capabilities(analytics, &Default::default()).await {
            Ok(capability) => println!("analytics: {:#?}", capability),
            Err(error) => println!("Failed to fetch analytics: {}", error),
        }
    }
}

async fn get_system_date_and_time(clients: &Clients) {
    let date =
        schema::devicemgmt::get_system_date_and_time(&clients.devicemgmt, &Default::default())
            .await;
    println!("{:#?}", date);
}

async fn get_stream_uris(clients: &Clients) -> Result<(), transport::Error> {
    let media_client = clients
        .media
        .as_ref()
        .ok_or_else(|| transport::Error::Other("Client media is not available".into()))?;
    let profiles = schema::media::get_profiles(media_client, &Default::default()).await?;
    debug!("get_profiles response: {:#?}", &profiles);
    let requests: Vec<_> = profiles
        .profiles
        .iter()
        .map(|p: &schema::onvif::Profile| schema::media::GetStreamUri {
            profile_token: schema::onvif::ReferenceToken(p.token.0.clone()),
            stream_setup: schema::onvif::StreamSetup {
                stream: schema::onvif::StreamType::RtpUnicast,
                transport: schema::onvif::Transport {
                    protocol: schema::onvif::TransportProtocol::Rtsp,
                    tunnel: vec![],
                },
            },
        })
        .collect();

    let responses = futures_util::future::try_join_all(
        requests
            .iter()
            .map(|r| schema::media::get_stream_uri(media_client, r)),
    )
    .await?;
    for (p, resp) in profiles.profiles.iter().zip(responses.iter()) {
        println!("token={} name={}", &p.token.0, &p.name.0);
        println!("    {}", &resp.media_uri.uri);
        if let Some(ref v) = p.video_encoder_configuration {
            println!(
                "    {:?}, {}x{}",
                v.encoding, v.resolution.width, v.resolution.height
            );
            if let Some(ref r) = v.rate_control {
                println!("    {} fps, {} kbps", r.frame_rate_limit, r.bitrate_limit);
            }
        }
        if let Some(ref a) = p.audio_encoder_configuration {
            println!(
                "    audio: {:?}, {} kbps, {} kHz",
                a.encoding, a.bitrate, a.sample_rate
            );
        }
    }
    Ok(())
}

async fn get_snapshot_uris(clients: &Clients) -> Result<(), transport::Error> {
    let media_client = clients
        .media
        .as_ref()
        .ok_or_else(|| transport::Error::Other("Client media is not available".into()))?;
    let profiles = schema::media::get_profiles(media_client, &Default::default()).await?;
    debug!("get_profiles response: {:#?}", &profiles);
    let requests: Vec<_> = profiles
        .profiles
        .iter()
        .map(|p: &schema::onvif::Profile| schema::media::GetSnapshotUri {
            profile_token: schema::onvif::ReferenceToken(p.token.0.clone()),
        })
        .collect();

    let responses = futures_util::future::try_join_all(
        requests
            .iter()
            .map(|r| schema::media::get_snapshot_uri(media_client, r)),
    )
    .await?;
    for (p, resp) in profiles.profiles.iter().zip(responses.iter()) {
        println!("token={} name={}", &p.token.0, &p.name.0);
        println!("    snapshot_uri={}", &resp.media_uri.uri);
    }
    Ok(())
}

async fn get_hostname(clients: &Clients) -> Result<(), transport::Error> {
    let resp = schema::devicemgmt::get_hostname(&clients.devicemgmt, &Default::default()).await?;
    debug!("get_hostname response: {:#?}", &resp);
    println!(
        "{}",
        resp.hostname_information
            .name
            .as_deref()
            .unwrap_or("(unset)")
    );
    Ok(())
}

async fn set_hostname(clients: &Clients, hostname: String) -> Result<(), transport::Error> {
    schema::devicemgmt::set_hostname(
        &clients.devicemgmt,
        &schema::devicemgmt::SetHostname { name: hostname },
    )
    .await?;
    Ok(())
}

async fn enable_analytics(clients: &Clients) -> Result<(), transport::Error> {
    let media_client = clients
        .media
        .as_ref()
        .ok_or_else(|| transport::Error::Other("Client media is not available".into()))?;
    let mut config =
        schema::media::get_metadata_configurations(media_client, &Default::default()).await?;
    if config.configurations.len() != 1 {
        println!("Expected exactly one analytics config");
        return Ok(());
    }
    let mut c = config.configurations.pop().unwrap();
    let token_str = c.token.0.clone();
    println!("{:#?}", &c);
    if c.analytics != Some(true) || c.events.is_none() {
        println!(
            "Enabling analytics in metadata configuration {}",
            &token_str
        );
        c.analytics = Some(true);
        c.events = Some(schema::onvif::EventSubscription {
            filter: None,
            subscription_policy: None,
        });
        schema::media::set_metadata_configuration(
            media_client,
            &schema::media::SetMetadataConfiguration {
                configuration: c,
                force_persistence: true,
            },
        )
        .await?;
    } else {
        println!(
            "Analytics already enabled in metadata configuration {}",
            &token_str
        );
    }

    let profiles = schema::media::get_profiles(media_client, &Default::default()).await?;
    let requests: Vec<_> = profiles
        .profiles
        .iter()
        .filter_map(
            |p: &schema::onvif::Profile| match p.metadata_configuration {
                Some(_) => None,
                None => Some(schema::media::AddMetadataConfiguration {
                    profile_token: schema::onvif::ReferenceToken(p.token.0.clone()),
                    configuration_token: schema::onvif::ReferenceToken(token_str.clone()),
                }),
            },
        )
        .collect();
    if !requests.is_empty() {
        println!(
            "Enabling metadata on {}/{} configs",
            requests.len(),
            profiles.profiles.len()
        );
        futures_util::future::try_join_all(
            requests
                .iter()
                .map(|r| schema::media::add_metadata_configuration(media_client, r)),
        )
        .await?;
    } else {
        println!(
            "Metadata already enabled on {} configs",
            profiles.profiles.len()
        );
    }
    Ok(())
}

async fn get_analytics(clients: &Clients) -> Result<(), transport::Error> {
    let media_client = clients
        .media
        .as_ref()
        .ok_or_else(|| transport::Error::Other("Client media is not available".into()))?;
    let config =
        schema::media::get_video_analytics_configurations(media_client, &Default::default())
            .await?;

    println!("{:#?}", &config);
    let c = match config.configurations.first() {
        Some(c) => c,
        None => return Ok(()),
    };
    if let Some(ref a) = clients.analytics {
        let mods = schema::analytics::get_supported_analytics_modules(
            a,
            &schema::analytics::GetSupportedAnalyticsModules {
                configuration_token: schema::onvif::ReferenceToken(c.token.0.clone()),
            },
        )
        .await?;
        println!("{:#?}", &mods);
    }

    Ok(())
}

async fn get_status(clients: &Clients) -> Result<(), transport::Error> {
    if let Some(ref ptz) = clients.ptz {
        let media_client = match clients.media.as_ref() {
            Some(client) => client,
            None => {
                return Err(transport::Error::Other(
                    "Client media is not available".into(),
                ))
            }
        };
        let profile = &schema::media::get_profiles(media_client, &Default::default())
            .await?
            .profiles[0];
        let profile_token = schema::onvif::ReferenceToken(profile.token.0.clone());
        let status =
            &schema::ptz::get_status(ptz, &schema::ptz::GetStatus { profile_token }).await?;
        println!("ptz status: {:#?}", status);
    }
    Ok(())
}
