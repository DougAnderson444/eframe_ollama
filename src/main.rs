#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use tokio::io::{AsyncBufReadExt as _, BufReader};
use tokio::process::Command;

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> eframe::Result {
    let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();

    tracing::info!("Starting eframe multinode");

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(());

    let _handle = std::thread::spawn(move || {
        // tokio async block so we can use async/await
        // this main() is wrapped in tokio, so we alrady have access to the runtime
        // In the tokio async blockm we send the Command to start the `ollama serve` command
        // and wait for StdErr to output 'Listening on '
        // Once we get that output, we know the server is ready to accept connections
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let mut child = tokio::process::Command::new("ollama_files/bin/ollama")
                .arg("serve")
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .expect("Failed to start ollama");

            tracing::info!("Ollama Server spawned");

            let stdout = child.stdout.take().expect("Failed to capture stdout");
            let stderr = child.stderr.take().expect("Failed to capture stderr");

            let stdout_reader = BufReader::new(stdout);
            let stderr_reader = BufReader::new(stderr);

            let mut stdout_lines = stdout_reader.lines();
            let mut stderr_lines = stderr_reader.lines();

            // loop on tokio select for stdout_lines, stderr_lines next_line()s or termination of the child process
            loop {
                tokio::select! {
                    Ok(Some(line)) = stdout_lines.next_line() => {
                        if line.contains("Listening on 127.0.0.1:11434") {
                            tracing::info!("Ollama Server is running");
                        }
                    }
                    Ok(Some(line)) = stderr_lines.next_line() => {
                        if line.contains("Listening on 127.0.0.1:11434") {
                            tracing::info!("Ollama Server is running");
                        }
                    }
                    _ = shutdown_rx.changed() => {
                        //if let Err(e) = child.kill().await {
                        //    tracing::error!("Failed to kill ollama server: {:?}", e);
                        //}
                        break;
                    }
                    _ = child.wait() => {
                        println!("Child process terminated");
                        break;
                    }

                }
            }

            tracing::info!("Killing ollama server");
            if let Err(e) = Command::new("pkill").args(["-9", "ollama"]).output().await {
                tracing::error!("Failed to kill ollama server: {:?}", e);
            }

            tracing::info!("Ollama server killed");
        });
    });
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

    let shutdown_tx_main = shutdown_tx.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
        let _ = shutdown_tx_main.send(());
    });

    let _ = eframe::run_native(
        "eframe_ollama",
        native_options,
        Box::new(|cc| Ok(Box::new(eframe_template::TemplateApp::new(cc)))),
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
                Box::new(|cc| Ok(Box::new(eframe_template::TemplateApp::new(cc)))),
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
