use anathema::backend::tui::TuiBackend;
use anathema::runtime::{Emitter, Runtime};
use anathema::state::{State, Value};
use anathema::templates::Document;
use anathema::widgets::components::{Component, ComponentId};
use anathema::widgets::Elements;

use std::{
    error::Error,
    fs::{self, read_to_string},
    path::PathBuf,
    thread::sleep,
    time::Duration,
};

use directories::ProjectDirs;

use clap::{Parser, Subcommand};
use demand::Input;
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
    cook: Option<Cook>,
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

struct App {
    // meater: Option<MeaterResponse>,
}

// -----------------------------------------------------------------------------
//   - State -
//   Holds the temperature
// -----------------------------------------------------------------------------
#[derive(State)]
struct TempState {
    internal_temp: Value<f32>,
    ambient_temp: Value<f32>,
    target_temp: Value<f32>,
    time_elapsed: Value<String>,
    time_remaining: Value<String>,
    cook_info: Value<String>,
    internal_temp_color: Value<u8>,
}

// -----------------------------------------------------------------------------
//   - Component -
//   Accepts incomming messages and updates
//   the state (the temp)
// -----------------------------------------------------------------------------
struct Temp;
impl Component for Temp {
    type Message = MeaterResponse;
    type State = TempState;

    fn message(
        &mut self,
        message: Self::Message,
        state: Option<&mut Self::State>,
        // NOTE: Not sure what this one is for yet
        _elements: Elements<'_, '_>,
        // mut elements: Elements<'_, '_>,
    ) {
        let update = state.unwrap();
        // TODO: fix unwraps
        let device = message.data.devices.first().unwrap();
        update
            .internal_temp
            .set(c_to_f(device.temperature.internal));
        update.ambient_temp.set(c_to_f(device.temperature.ambient));

        update
            .internal_temp_color
            .set(calculate_internal_temp_color(device));

        if let Some(cook) = &device.cook {
            update.target_temp.set(c_to_f(cook.temperature.target));
            update.time_elapsed.set(display_time(cook.time.elapsed));
            update.time_remaining.set(display_time(cook.time.remaining));
            update
                .cook_info
                .set(format!("{}: {}", cook.name, cook.state));
        } else {
            update.target_temp.set(0.0);
            update.time_elapsed.set("0".to_string());
            update.time_remaining.set("0".to_string());
            update.cook_info.set("".to_string());
        }
    }
}

fn calculate_internal_temp_color(device: &Device) -> u8 {
    if let Some(cook) = &device.cook {
        let internal_temp = c_to_f(device.temperature.internal);
        let target_temp = c_to_f(cook.temperature.target);
        let percentage = ((internal_temp / target_temp) * 100.0).ceil() as u8;

        // TEMP_COLORS
        // bg: #FF2233 }, // 0 - 50
        // bg: #FF5F00 }, // 51 - 59
        // bg: #FF9933 }, // 60 - 69
        // bg: #FFCC33 }, // 70 - 79
        // bg: #FFFF33 }, // 80 - 87
        // bg: #B2FF66 }, // 88 - 94
        // bg: #66FF66 }, // 95 - 100

        match percentage {
            0..=50 => 0,
            51..=59 => 1,
            60..=69 => 2,
            70..=79 => 3,
            80..=87 => 4,
            88..=94 => 5,
            95.. => 6,
        }
    } else {
        6
    }
}

#[cfg(test)]
fn get_test_device(internal: f32, target: f32) -> Device {
    Device {
        id: 0.to_string(),
        temperature: Temperature {
            internal,
            ambient: 30.0,
        },
        cook: Some(Cook {
            id: "0".to_string(),
            name: "Test Cook".to_string(),
            state: "Configured".to_string(),
            temperature: CookTemperature { target, peak: 21.0 },
            time: Time {
                elapsed: 1124123,
                remaining: 142112,
            },
        }),
        updated_at: Some(141321412341),
    }
}

#[test]
fn calc_temp_color_green() {
    let device = get_test_device(51.0, 52.0);

    let color = calculate_internal_temp_color(&device);

    assert_eq!(6, color);
}

#[test]
fn calc_temp_color_light_green() {
    let device = get_test_device(31.88, 35.56);

    let color = calculate_internal_temp_color(&device);

    assert_eq!(5, color);
}

#[test]
fn calc_temp_color_red() {
    let device = get_test_device(21.0, 95.0);

    let color = calculate_internal_temp_color(&device);

    assert_eq!(0, color);
}

#[test]
fn calc_temp_color_red_max_value() {
    // 70 F / 140 F
    let device = get_test_device(21.1, 60.0);

    let color = calculate_internal_temp_color(&device);

    assert_eq!(0, color);
}

#[test]
fn calc_temp_color_light_red() {
    // 71.6 F / 140 F
    let device = get_test_device(22.0, 60.0);

    let color = calculate_internal_temp_color(&device);

    assert_eq!(1, color);
}

fn display_time(time: i32) -> String {
    if time == -1 {
        return "Estimating...".to_string();
    }

    let secs: u64 = time.try_into().unwrap_or(0);

    if secs == 0 {
        return "00:00:00".to_string();
    }

    let seconds = secs % 60;
    let minutes = (secs / 60) % 60;
    let hours = (secs / 60) / 60;

    format!("{:0>2}:{:0>2}:{:0>2}", hours, minutes, seconds)
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    match cli.commands {
        Cmds::Login { email } => do_login(&email),
        Cmds::Bbq => do_bbq(),
    }
}

// -----------------------------------------------------------------------------
//   - Fake temp updates -
// -----------------------------------------------------------------------------
fn update_temp(emitter: Emitter, recipient: ComponentId) {
    std::thread::spawn(move || {
        let mut token_path = get_data_directory(Some("token")).unwrap();
        token_path.push("data.json");

        let data = fs::read_to_string(token_path);
        if data.is_err() {
            panic!("Could not find token, please do login first");
        }

        // TODO: fix unwrap
        let token_data: TokenData = serde_json::from_str(&data.unwrap()).unwrap();
        let bearer = format!("Bearer {}", token_data.token);

        loop {
            let meat_status_url = format!("{MEATER_API}/devices");
            let devices_response = ureq::get(&meat_status_url)
                .set("Authorization", &bearer)
                .call();

            if devices_response.is_err() {
                panic!("BBQ Error: {}", devices_response.unwrap_err());
            }

            let response = devices_response.unwrap();
            let json = &response.into_string().unwrap();

            let meater = serde_json::from_str::<MeaterResponse>(json);
            match meater {
                Ok(message) => {
                    let _ = emitter.emit(message, recipient);
                }

                // TODO: do something when it errors, not sure what yet...
                Err(_) => {
                    println!("Oopsies");
                }
            }

            sleep(Duration::from_secs(2));
        }
    });
}

impl App {
    pub fn new() -> Self {
        App {}
    }

    pub fn run(&self) -> Result<(), Box<dyn Error>> {
        let template = read_to_string("template.aml").unwrap();
        let mut doc = Document::new("@main");

        // Get the component id, this is the recipient we will send the
        // data to
        let temp_id = doc.add_component("main", template);

        let tui = TuiBackend::builder()
            .enable_alt_screen()
            .enable_raw_mode()
            .hide_cursor()
            .finish();

        if let Err(ref error) = tui {
            println!("GOT ERROR");
            println!("Error starting TUI backend: {error:?}");
        }

        let backend = tui.unwrap();

        let mut runtime = Runtime::new(doc, backend).unwrap();
        runtime.register_component(
            temp_id,
            Temp,
            TempState {
                internal_temp: 0.0.into(),
                ambient_temp: 0.0.into(),
                target_temp: 0.0.into(),
                time_elapsed: "00:00:00".to_string().into(),
                time_remaining: "00:00:00".to_string().into(),
                cook_info: "".to_string().into(),
                internal_temp_color: 0.into(),
            },
        );

        // Get an emitter from the runtime. This is how
        // we can send messages to components from the "outside".
        let emitter = runtime.emitter();

        update_temp(emitter, temp_id.into());

        let _ = runtime.run();

        Ok(())
    }
}

fn do_bbq() -> Result<(), Box<dyn Error>> {
    let app = App::new();
    app.run()
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
