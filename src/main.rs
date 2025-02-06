#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use futures::{AsyncReadExt, AsyncWriteExt, StreamExt};
use libp2p::StreamProtocol;

use futures::stream::BoxStream;
use futures::TryStreamExt;
use std::io::{Error, ErrorKind};
use tokio::io::Result;

const LLAMA_PROTOCOL: StreamProtocol = StreamProtocol::new("/llama");

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> eframe::Result {
    use std::net::{Ipv4Addr, Ipv6Addr};

    use libp2p::{multiaddr::Protocol, Multiaddr, Stream};
    use libp2p_webrtc::tokio::Certificate;
    use ollama_rs::generation::completion::request::GenerationRequest;

    let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();

    tracing::info!("Starting eframe multinode");

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_min_inner_size([300.0, 220.0])
            .with_icon(
                // NOTE: Adding an icon is optional
                eframe::icon_data::from_png_bytes(&include_bytes!("../assets/icon-256.png")[..])
                    .expect("Failed to load icon"),
            ),
        ..Default::default()
    };

    let shutdown_tx = ollama_launcher::launch_ollama();

    let mut swarm = libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_quic()
        .with_other_transport(|id_keys| {
            Ok(libp2p_webrtc::tokio::Transport::new(
                id_keys.clone(),
                Certificate::generate(&mut rand::thread_rng()).unwrap(),
            ))
        })
        .unwrap()
        .with_behaviour(|_| libp2p_stream::Behaviour::new())
        .unwrap()
        .build();

    let peer_id = *swarm.local_peer_id();
    tracing::info!("Local peer id: {:?}", peer_id);

    let address_webrtc = Multiaddr::from(Ipv6Addr::UNSPECIFIED)
        .with(Protocol::Udp(0))
        .with(Protocol::WebRTCDirect);

    let addr_webrtc_ipv4 = Multiaddr::from(Ipv4Addr::UNSPECIFIED)
        .with(Protocol::Udp(0))
        .with(Protocol::WebRTCDirect);

    for addr in [
        address_webrtc,
        addr_webrtc_ipv4,
        // address_quic, address_tcp
    ] {
        swarm.listen_on(addr).unwrap();
    }

    let mut incoming_streams = swarm
        .behaviour()
        .new_control()
        .accept(LLAMA_PROTOCOL)
        .unwrap();

    // Deal with incoming streams.
    tokio::spawn(async move {
        while let Some((peer, stream)) = incoming_streams.next().await {
            match ollama_process(stream).await {
                Ok(_) => {
                    tracing::info!(%peer, "Received response");
                }
                Err(e) => {
                    tracing::warn!(%peer, "Processing failed: {e}");
                    continue;
                }
            }
        }
    });

    async fn ollama_process(mut stream: Stream) -> tokio::io::Result<()> {
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await?;

        let prompt = String::from_utf8(buf).expect("Invalid UTF-8 sequence");
        let ollama = ollama_rs::Ollama::default();

        // Create and transform the stream immediately to use io::Error
        let generation = ollama
            .generate_stream(GenerationRequest::new(
                "llama3.1:latest".to_string(),
                prompt,
            ))
            .await
            .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?;

        // Transform the stream to use io::Error and box it
        let mut safe_stream: BoxStream<'_, Result<Vec<_>>> = generation
            .map(|r| r.map_err(|e| Error::new(ErrorKind::Other, e.to_string())))
            .boxed();

        // Now we can safely process the transformed stream
        // try_next() properly handles the error types, gets around OllamaError being !Send
        while let Some(chunk) = safe_stream.try_next().await? {
            for response in chunk {
                stream.write_all(response.response.as_bytes()).await?;
                stream.flush().await?;
            }
        }

        Ok(())
    }

    let _ = eframe::run_native(
        "eframe_ollama",
        native_options,
        Box::new(|cc| Ok(Box::new(eframe_ollama::TemplateApp::new(cc)))),
    );

    // Shutdown the ollama server
    shutdown_tx
        .send(())
        .expect("Failed to send shutdown signal");

    Ok(())
}

// When compiling to web using trunk:
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("No window")
            .document()
            .expect("No document");

        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("Failed to find the_canvas_id")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("the_canvas_id was not a HtmlCanvasElement");

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(eframe_ollama::TemplateApp::new(cc)))),
            )
            .await;

        // Remove the loading text and spinner:
        if let Some(loading_text) = document.get_element_by_id("loading_text") {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html(
                        "<p> The app has crashed. See the developer console for details. </p>",
                    );
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}
