use std::{
    error::Error,
    fs,
    io::{stdout, Write},
    path::PathBuf,
    thread::sleep,
    time::Duration,
};

use crossterm::{execute, terminal::*};

use directories::ProjectDirs;

use clap::{Parser, Subcommand};
use demand::Input;
use ratatui::{
    backend::CrosstermBackend, buffer::Buffer, layout::Rect, widgets::Widget, Frame, Terminal,
};
use serde::{Deserialize, Serialize};

#[derive(Subcommand)]
enum Cmds {
    /// Login to Meater Cloud
    Login {
        /// Your Meater Cloud email
        email: String,
    },

    /// Monitor the BBQ
    Bbq,
}

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    commands: Cmds,
}

const MEATER_API: &str = "https://public-api.cloud.meater.com/v1";

#[derive(Debug, Serialize, Deserialize)]
struct LoginBody<'a> {
    email: &'a str,
    password: &'a str,
}

#[derive(Debug, Serialize, Deserialize)]
struct Meta {}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TokenData {
    token: String,
    user_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginResponse<'a> {
    status: &'a str,
    status_code: u16,
    data: TokenData,
    meta: Option<Meta>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MeaterData {
    devices: Vec<Device>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Device {
    id: String,
    temperature: Temperature,
    cook: Cook,
    updated_at: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Temperature {
    internal: f32,
    ambient: f32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Cook {
    id: String,
    name: String,
    state: String,
    temperature: CookTemperature,
    time: Time,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CookTemperature {
    target: f32,
    peak: f32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Time {
    elapsed: i32,
    remaining: i32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MeaterResponse {
    status: String,
    status_code: u16,
    data: MeaterData,
    // updated_at: u64,
}

impl Widget for MeaterResponse {
    fn render(self, area: Rect, buf: &mut Buffer) {
        todo!()
    }
}

// {
//     "status": "OK",
//     "statusCode": 200,
//     "data": {
//         "devices": [
//             {
//                 "id": "<Device ID>",
//                 "temperature": {
//                     "internal": 0,
//                     "ambient": 0,
//                 },
//                 "cook": {
//                     "id": "<Cook ID>",
//                     "name": "<Cook name>",
//                     "state": "<Cook state>",
//                     "temperature": {
//                         "target": 0,
//                         "peak": 0
//                     },
//                     "time": {
//                         "elapsed": 0,
//                         "remaining": 0
//                     }
//                 },
//                 "updated_at": 123456789
//             },
//             {
//                 "id": "<Device ID>",
//                 "temperature": {
//                     "internal": 0,
//                     "ambient": 0,
//                 },
//                 "cook": null,
//                 "updated_at": 123456789
//             },
//         ]
//     },
//     "meta": {}
// }

struct App {
    meater: Option<MeaterResponse>,
}

impl App {
    pub fn new() -> Self {
        App { meater: None }
    }

    pub fn run(&self, bearer: String) -> Result<(), Box<dyn Error>> {
        // let mut terminal = Terminal::new(CrosstermBackend::new(stdout())).expect("No TUI");

        // execute!(stdout(), EnterAlternateScreen)?;
        // enable_raw_mode()?;

        loop {
            let meat_status_url = format!("{MEATER_API}/devices");
            let devices_response = ureq::get(&meat_status_url)
                .set("Authorization", &bearer)
                .call();

            if devices_response.is_err() {
                println!("BBQ Error: {}", devices_response.unwrap_err());
                continue;
            }

            // dbg!(&devices_response);

            let response = devices_response.unwrap();
            let json = &response.into_string().unwrap();

            // dbg!(json);

            let meater: MeaterResponse = serde_json::from_str(json)?;
            // let meater = meater;

            // terminal.draw(|frame| self.render(frame))?;
            // Test display
            // if meater.is_none() {
            //     continue;
            // }

            // let meater = meater.unwrap();

            if let Some(device) = meater.data.devices.first() {
                println!(
                    "Internal temperature is: {}",
                    c_to_f(device.temperature.internal)
                );
                println!(
                    "Ambient temperature is: {}",
                    c_to_f(device.temperature.ambient)
                );
            }

            sleep(Duration::from_secs(30));
        }
    }

    // fn render(&mut self, frame: &mut Frame) {
    //     frame.render_widget(self, frame.size());
    // }
}

// impl Widget for &mut App {
//     fn render(self, area: Rect, buf: &mut Buffer) {
//         // Test display
//         if let None = self.meater {}
//
//         let meater = self.meater.unwrap();
//
//         if let Some(device) = meater.data.devices.first() {
//             println!(
//                 "Internal temperature is: {}",
//                 c_to_f(device.temperature.internal)
//             );
//             println!(
//                 "Ambient temperature is: {}",
//                 c_to_f(device.temperature.ambient)
//             );
//         }
//     }
// }

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    match cli.commands {
        Cmds::Login { email } => do_login(&email),
        Cmds::Bbq => do_bbq(),
    }
}

fn do_bbq() -> Result<(), Box<dyn Error>> {
    let mut token_path = get_data_directory(Some("token")).unwrap();
    token_path.push("data.json");

    let data = fs::read_to_string(token_path);
    if data.is_err() {
        panic!("Could not find token, please do login first");
    }
    let token_data: TokenData = serde_json::from_str(&data.unwrap())?;
    let bearer = format!("Bearer {}", token_data.token);

    let app = App::new();
    app.run(bearer)
}

fn do_login(email: &str) -> Result<(), Box<dyn Error>> {
    let t = Input::new("Enter your Meater Cloud password:")
        .placeholder("Enter password")
        .prompt("Password: ")
        .password(true);
    let password = t.run().expect("error running input");
    let login_url = format!("{MEATER_API}/login");
    let login_body = LoginBody {
        email,
        password: &password,
    };

    let body = serde_json::to_string(&login_body).unwrap();
    let login_response = ureq::post(&login_url)
        .set("Content-Type", "application/json")
        .send(body.as_bytes());
    if login_response.is_err() {
        let error = login_response.unwrap_err();
        panic!("Could not log in: {:?}", error.to_string());
    }

    let r = &login_response?.into_string()?;
    let response = serde_json::from_str::<LoginResponse>(r)?;
    let mut dir = get_data_directory(Some("token"))?;
    dir.push("data.json");

    let write_result = fs::write(dir, serde_json::to_string(&response.data)?);
    if write_result.is_err() {
        panic!("Was not able to complete login process, please try again");
    }

    println!("Logged in to Meater Cloud");

    Ok(())
}

fn get_data_directory(path: Option<&str>) -> Result<PathBuf, Box<dyn Error>> {
    if let Some(project_directories) = ProjectDirs::from("com", "s9tpepper", "Meater") {
        let mut data_directory = project_directories.data_dir().to_path_buf();
        if let Some(path) = path {
            data_directory.push(path);
        }

        if !data_directory.exists() {
            std::fs::create_dir_all(&data_directory)?;
        }

        return Ok(data_directory);
    }

    Err("Could not get data directory".into())
}

fn c_to_f(celsius: f32) -> f32 {
    ((9.0 / 5.0) * celsius) + 32.0
}
