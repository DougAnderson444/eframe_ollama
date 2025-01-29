//! Launch and kill Ollama in Linux using Tokio.
use tokio::io::{AsyncBufReadExt as _, BufReader};
use tokio::process::Command;

pub fn launch_ollama() -> tokio::sync::watch::Sender<()> {
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
                        // We can't do this because the `ollama_llama_server` keeps going after the child process is killed
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
            // Kill all processes with the name ollama in it using pkill -9 ollama
            if let Err(e) = Command::new("pkill").args(["-9", "ollama"]).output().await {
                tracing::error!("Failed to kill ollama server: {:?}", e);
            }

            tracing::info!("Ollama server killed");
        });
    });

    shutdown_tx
}
