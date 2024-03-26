use futures::stream::{self, TryStreamExt};
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, Write};
use walkdir::WalkDir;
use dotenv::dotenv;
use std::env;

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
    dotenv().ok(); // Load .env file variables into the environment

    let client = Client::new();
    // Clone these strings for use inside the async closure
    let server_url = env::var("SERVER_URL").expect("SERVER_URL must be set");
    let api_key = env::var("API_KEY").expect("API_KEY must be set");

    let folder_path = "ef";
    let output_dir = "TypeScript";
    fs::create_dir_all(output_dir)?;

    let cs_files = WalkDir::new(folder_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "cs"))
        .collect::<Vec<_>>();

    let requests = stream::iter(cs_files).then(|entry| {
        let file_path = entry.path().to_path_buf();
        let output_dir = output_dir.to_string();
        let client = client.clone();
        // Clone server_url and api_key for each usage in the loop
        let server_url = server_url.clone();
        let api_key = api_key.clone();
        async move {
            let content = fs::read_to_string(&file_path)?;
            let res = client
                .post(&server_url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&CompletionRequest {
                    model: "gpt-3.5-turbo-0125",
                    messages: vec![
                        Message {
                            role: "system",
                            content: "You are an expert C# and Angular developer with over 30 years of experience. You are tasked with converting C# models into the TypeScript Type for an Angular DTO. Remember to match the nullable properties, and make sure string are nullable. Respond only with the file exactly as how you want the .ts file to appear.",
                        },
                        Message {
                            role: "user",
                            content: &format!("Convert this C# model to a TypeScript type:\n\n{}", content),
                        },
                    ],
                })
                .send()
                .await
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?; // Handle errors from sending the request

            // Deserialize the response body into CompletionResponse struct
            let response_body = res
                .json::<CompletionResponse>()
                .await
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?; // Handle errors from deserialization

            // Use the deserialized response
            let ts_content = response_body.choices.get(0)
                .map_or("", |choice| &choice.message.content);
            let ts_file_path = format!(
                "{}/{}.ts",
                output_dir,
                file_path.file_stem().unwrap().to_str().unwrap()
            );
            let mut file = File::create(ts_file_path)?;
            file.write_all(ts_content.as_bytes())?;
            Ok::<(), io::Error>(())
        }
    });

    requests.try_collect::<Vec<_>>().await?;
    Ok(())
}
