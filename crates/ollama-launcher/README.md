# Ollama Launcher

Spawns a new OS thread (Linux for now) and runs Ollama in it using Tokio.

Kills the processes with `ollama` in the name when it gets the signal.

# Setup 

Ollama launcher assumes you've got ollama in:

```
ollama_files/bin/ollama
```

Which can be setup by running the just commands located in the [justfile](./justfile):

```sh 
just install_ollama_linux
```

# Usage

```rust 
// main.rs 

// sandwich around yuor eframe launch:
let shutdown_tx = ollama_launcher::launch_ollama();

let _ = eframe::run_native(
    "PeerPiper-Multinode",
    native_options,
    Box::new(|cc| Ok(Box::new(eframe_multinode::MultinodeApp::new(cc)))),
);

// Shutdown the ollama server
shutdown_tx
    .send(())
    .expect("Failed to send shutdown signal");
```

## TODO:

- [ ] Other OS (Window, Mac)
- [ ] Flag tokio feature?
- [ ] Use [https://github.com/tauri-apps/tauri-plugin-shell](https://github.com/tauri-apps/tauri-plugin-shell) as insipiration for robust/better handling.

