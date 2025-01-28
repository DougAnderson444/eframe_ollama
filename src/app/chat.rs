//! Chat widget.
//! By default uses Ollama::default() to connect to the Ollama server.

use futures::StreamExt as _;
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::Mutex as AsyncMutex;

use ollama_rs::{
    generation::chat::{request::ChatMessageRequest, ChatMessage, ChatMessageResponseStream},
    Ollama,
};

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
#[derive(Default)]
pub struct ChatWidget {
    /// Out Ollama instance.
    #[serde(skip)] // This how you opt-out of serialization of a field
    ollama: Arc<AsyncMutex<Ollama>>,
    /// Our prompt
    prompt: String,
    /// The current response
    response: Arc<RwLock<String>>,
    /// Previous response history
    history: Arc<Mutex<Vec<ChatMessage>>>,
}

impl ChatWidget {
    pub fn ui(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        // Chat should look like:
        // At the verry bottom, the single line textedit with the Send button on the far right.
        // Filling the space above that, the chat history, with the most recent message at the bottom,
        // and the history scrolling up as new messages are added.
        // The chat history should be a scrollable region, with the most recent message at the bottom.
        // Since chat responses are streamed, when Send button is clicked we should start a new stream
        // by spawning a new tokio task to handle the stream.
        // When the stream response is complete, we should update the chat history with the response.
        // Scrollable chat history

        egui::TopBottomPanel::bottom("chat_prompt").show_inside(ui, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.prompt);
                if ui.button("Send").clicked() {
                    let ollama = self.ollama.clone();
                    let history = self.history.clone();
                    let response = self.response.clone();
                    let prompt = self.prompt.clone();
                    let ctx = ctx.clone();
                    tracing::info!("Sending chat message: {:?}", prompt);
                    tokio::spawn(async move {
                        start_chat(ctx, ollama, history, response, prompt).await;
                    });
                }
            });
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.vertical(|ui| {
                // History with a "Clear Hostory" button
                ui.horizontal(|ui| {
                    ui.label("Chat history:");
                    if ui.button("Clear History").clicked() {
                        self.history.lock().unwrap().clear();
                        self.response.write().unwrap().clear();
                    }
                });
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for message in self.history.lock().unwrap().iter() {
                        ui.label(message.content.as_str());
                    }

                    // show the responses are they stream in
                    ui.label(self.response.read().unwrap().clone());
                });
            });
        });
    }
}

async fn start_chat(
    ctx: egui::Context,
    ollama: Arc<AsyncMutex<Ollama>>,
    history: Arc<Mutex<Vec<ChatMessage>>>,
    response: Arc<RwLock<String>>,
    input: String,
) {
    let mut stream: ChatMessageResponseStream = match ollama
        .lock()
        .await
        .send_chat_messages_with_history_stream(
            history.clone(),
            ChatMessageRequest::new(
                "llama3.1:latest".to_string(),
                vec![ChatMessage::user(input.to_string())],
            ),
        )
        .await
    {
        Ok(stream) => stream,
        Err(e) => {
            history
                .lock()
                .unwrap()
                .push(ChatMessage::user(format!("Error: {}", e)));
            return;
        }
    };

    while let Some(Ok(res)) = stream.next().await {
        tracing::info!("Got response: {:?}", res);
        let mut response_lock = response.write().unwrap();
        *response_lock += res.message.content.as_str();
        ctx.request_repaint();
    }

    // clear the response
    let mut response_lock = response.write().unwrap();
    *response_lock = "".to_string();
}
