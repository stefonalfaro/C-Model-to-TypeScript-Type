use std::io::{self, Write};
use std::fs::{self, File};
use log::info;
use serde::{Serialize, Deserialize};
use reqwest::Client;
use walkdir::WalkDir;
use dotenv::dotenv;
use std::env;
use futures::stream::{self, StreamExt};

#[derive(Serialize)]
struct CompletionRequest<'a> {
    model: &'a str,
    messages: Vec<Message<'a>>,
}

#[derive(Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct CompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: MessageContent,
}

#[derive(Deserialize)]
struct MessageContent {
    content: String,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init(); // Initialize the logger
    dotenv().ok(); // Load .env file variables into the environment

    let client = Client::new();
    let server_url = env::var("SERVER_URL").expect("SERVER_URL must be set");
    let api_key = env::var("API_KEY").expect("API_KEY must be set");

    let folder_path = "EF";
    let output_dir = "TypeScript";
    fs::create_dir_all(output_dir)?;

    let cs_files = WalkDir::new(folder_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "cs"))
        .collect::<Vec<_>>();

    stream::iter(cs_files).for_each_concurrent(2, |entry| {
        let file_path = entry.path().to_path_buf();
        let output_dir_cloned = output_dir.clone();
        let client_cloned = client.clone();
        let server_url_cloned = server_url.clone();
        let api_key_cloned = api_key.clone();

        async move {
            let content = match fs::read_to_string(&file_path) {
                Ok(content) => content,
                Err(e) => {
                    eprintln!("Error reading file {:?}: {}", file_path, e);
                    return;
                }
            };

            info!("Successfully read file: {:?}", file_path);

            let res = match client_cloned
                .post(&server_url_cloned)
                .header("Authorization", format!("Bearer {}", api_key_cloned))
                .header("Content-Type", "application/json")
                .json(&CompletionRequest {
                    model: "gpt-3.5-turbo-0125",
                    messages: vec![
                        Message {
                            role: "system",
                            content: "You are an expert C# and Angular developer with over 30 years of experience. You are tasked with converting C# models into the TypeScript Type for an Angular DTO. Remember to match the nullable properties, and make sure string are nullable. Respond only with the file exactly as how you want the .ts file to appear.\n\nDo not import anything. Use only TypeScript values.",
                        },
                        Message {
                            role: "user",
                            content: &format!("Convert this C# model to a TypeScript type:\n\n{}", content),
                        },
                    ],
                })
                .send()
                .await {
                    Ok(res) => res,
                    Err(e) => {
                        eprintln!("Error sending request for file {:?}: {}", file_path, e);
                        return;
                    }
                };

            info!("Request sent for file: {:?}", file_path);

            let response_body = match res.json::<CompletionResponse>().await {
                Ok(body) => body,
                Err(e) => {
                    eprintln!("Error decoding response for file {:?}: {}", file_path, e);
                    return;
                }
            };

            info!("Response received and decoded for file: {:?}", file_path);

            if let Some(choice) = response_body.choices.get(0) {
                let ts_content = &choice.message.content;
                let ts_file_path = format!("{}/{}.ts", output_dir_cloned, file_path.file_stem().unwrap().to_str().unwrap());
                if let Err(e) = File::create(&ts_file_path).and_then(|mut file| file.write_all(ts_content.as_bytes())) {
                    eprintln!("Error writing to file {:?}: {}", ts_file_path, e);
                }

                info!("TypeScript file generated: {:?}", ts_file_path);

            } else {
                eprintln!("No choices found in response for file {:?}", file_path);
            }
        }
    }).await;

    Ok(())
}
