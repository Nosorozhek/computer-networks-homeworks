mod packet;

use dioxus::prelude::*;
use std::{
    sync::Arc,
};
use tokio::{
    sync::Mutex,
};

use crate::packet::FtpClient;

fn main() {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut host = use_signal(|| "ftp.dlptest.com".to_string());
    let mut port = use_signal(|| "21".to_string());
    let mut username = use_signal(|| "dlpuser".to_string());
    let mut password = use_signal(|| "".to_string());

    let mut is_connected = use_signal(|| false);
    let mut status_msg = use_signal(|| "Connect to a server".to_string());
    let mut file_list = use_signal(|| "Loading...".to_string());

    let mut target_file = use_signal(|| "".to_string());
    let mut editor_content = use_signal(|| "".to_string());
    let mut is_editing = use_signal(|| false);

    let mut client = use_signal(|| None::<Arc<Mutex<FtpClient>>>);

    let refresh_list = move || {
        spawn(async move {
            if let Some(c) = client.read().clone() {
                let mut ftp = c.lock().await;
                if ftp.enter_passive().await.is_ok() {
                    if let Ok(list) = ftp.list().await {
                        file_list.set(list);
                        return;
                    }
                }
                status_msg.set("Failed to retrieve file list".to_string());
            }
        });
    };

    let connect = move |_| {
        status_msg.set("Connecting...".to_string());
        spawn(async move {
            let addr = format!("{}:{}", host(), port());
            match FtpClient::connect(&addr).await {
                Ok(mut ftp) => {
                    if ftp.send_username(&username()).await.is_ok()
                        && ftp.send_password(&password()).await.is_ok()
                    {
                        client.set(Some(Arc::new(Mutex::new(ftp))));
                        is_connected.set(true);
                        status_msg.set("Connected successfully".to_string());
                        refresh_list();
                    } else {
                        status_msg.set("Authentication failed".to_string());
                    }
                }
                Err(e) => status_msg.set(format!("Connection failed: {}", e)),
            }
        });
    };

    let disconnect = move |_| {
        spawn(async move {
            if let Some(c) = client.read().clone() {
                let mut ftp = c.lock().await;
                let _ = ftp.quit().await;
            }
            client.set(None);
            is_connected.set(false);
            status_msg.set("Disconnected".to_string());
        });
    };

    let retrieve_file = move |_| {
        let filename = target_file();
        if filename.is_empty() {
            status_msg.set("Enter a filename".to_string());
            return;
        }
        status_msg.set(format!("Downloading {}...", filename));
        spawn(async move {
            if let Some(c) = client.read().clone() {
                let mut ftp = c.lock().await;
                if ftp.enter_passive().await.is_ok() {
                    match ftp.retrieve(&filename).await {
                        Ok(data) => {
                            let text = String::from_utf8_lossy(&data).to_string();
                            editor_content.set(text);
                            is_editing.set(true);
                            status_msg.set(format!("Retrieved: {}", filename));
                        }
                        Err(e) => status_msg.set(format!("Failed to retrieve: {}", e)),
                    }
                }
            }
        });
    };

    let save_file = move |_| {
        let filename = target_file();
        let content = editor_content();
        if filename.is_empty() {
            status_msg.set("Enter a filename to save".to_string());
            return;
        }
        status_msg.set(format!("Saving {}...", filename));
        spawn(async move {
            if let Some(c) = client.read().clone() {
                let mut ftp = c.lock().await;
                if ftp.enter_passive().await.is_ok() {
                    match ftp.send(&filename, content.into_bytes()).await {
                        Ok(_) => {
                            status_msg.set("File saved to server".to_string());
                            refresh_list();
                            is_editing.set(false);
                        }
                        Err(e) => status_msg.set(format!("Failed to save: {}", e)),
                    }
                }
            }
        });
    };

    let delete_file = move |_| {
        let filename = target_file();
        if filename.is_empty() {
            status_msg.set("Enter a filename to delete".to_string());
            return;
        }
        status_msg.set(format!("Deleting {}...", filename));
        spawn(async move {
            if let Some(c) = client.read().clone() {
                let mut ftp = c.lock().await;
                match ftp.delete(&filename).await {
                    Ok(_) => {
                        status_msg.set("File deleted.".to_string());
                        refresh_list();
                        if is_editing() {
                            is_editing.set(false);
                        }
                    }
                    Err(e) => status_msg.set(format!("Failed to delete: {}", e)),
                }
            }
        });
    };

    rsx! {
        div {
            style: "font-family: sans-serif; padding: 20px; max-width: 900px; margin: 0 auto;",
            h1 { "FTP Client" }

            div {
                style: "padding: 10px; background: #eee; margin-bottom: 20px; border-radius: 5px;",
                strong { "Status: " }
                "{status_msg}"
            }

            if !is_connected() {
                div {
                    style: "display: flex; flex-direction: column; gap: 10px; max-width: 300px;",
                    input { placeholder: "Host", value: "{host}", oninput: move |e| host.set(e.value()) }
                    input { placeholder: "Port", value: "{port}", oninput: move |e| port.set(e.value()) }
                    input { placeholder: "Username", value: "{username}", oninput: move |e| username.set(e.value()) }
                    input { placeholder: "Password", type: "password", value: "{password}", oninput: move |e| password.set(e.value()) }
                    button { onclick: connect, style: "padding: 10px; cursor: pointer;", "Connect" }
                }
            } else {
                div {
                    style: "display: flex; gap: 20px;",
                    div {
                        style: "width: 450px; flex-shrink: 0; border: 1px solid; border-radius: 5px; padding: 10px;",
                        div {
                            style: "display: flex; justify-content: space-between; align-items: center;",
                            h3 { style: "margin-top: 0;", "Server Files" }
                            button { onclick: move |_| refresh_list(), "Refresh" }
                        }
                        pre {
                            style: "padding: 10px; overflow-x: auto; max-height: 400px;",
                            "{file_list}"
                        }
                        button { onclick: disconnect, style: "width: 100%; margin-top: 10px; color: red;", "Disconnect" }
                    }

                    div {
                        style: "flex: 1;",
                        div {
                            style: "margin-bottom: 20px;",
                            h3 { style: "margin-top: 0;", "File Operations" }
                            input {
                                style: "width: 100%; padding: 8px; margin-bottom: 10px;",
                                placeholder: "Enter target filename (e.g. test.txt)",
                                value: "{target_file}",
                                oninput: move |e| target_file.set(e.value())
                            }
                            div {
                                style: "display: flex; gap: 10px;",
                                button { onclick: retrieve_file, "Read" }
                                button { onclick: move |_| is_editing.set(true), "Create / Update" }
                                button { onclick: delete_file, style: "color: red;", "Delete" }
                            }
                        }

                        if is_editing() {
                            div {
                                style: "display: flex; flex-direction: column; gap: 10px; border: 1px solid #007bff; padding: 10px; border-radius: 5px;",
                                h4 { style: "margin: 0;", "Text Editor: {target_file}" }
                                textarea {
                                    style: "display: block; width: 100%; height: 300px; font-family: monospace; padding: 12px; box-sizing: border-box; margin: 0; border: 1px solid #ccc; border-radius: 4px; resize: vertical;",
                                    value: "{editor_content}",
                                    oninput: move |e| editor_content.set(e.value())
                                }
                                div {
                                    style: "display: flex; gap: 10px;",
                                    button { onclick: save_file, style: "background: #007bff; color: white; padding: 8px 16px; border: none; cursor: pointer;", "Save to Server" }
                                    button { onclick: move |_| is_editing.set(false), style: "padding: 8px 16px; cursor: pointer;", "Cancel" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
