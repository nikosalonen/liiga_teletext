// src/config.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    // UI settings
    pub page_number: u16,
    pub title: String,
    pub auto_switch_pages: bool,
    pub auto_switch_interval_seconds: u64,

    // Content settings
    pub games_per_page: usize,
    pub subheader: String,

    // Colors (as hex strings, e.g. "#0000FF")
    pub header_bg_color: String,
    pub header_fg_color: String,
    pub result_color: String,

    pub theme: Theme,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Theme {
    pub header_bg: String,
    pub header_fg: String,
    pub subheader_fg: String,
    pub result_fg: String,
    pub text_fg: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            page_number: 235,
            title: "JÄÄKIEKKO".to_string(),
            auto_switch_pages: false,
            auto_switch_interval_seconds: 10,
            games_per_page: 4,
            subheader: "KARSINTA (4 voittoa)".to_string(),
            header_bg_color: "#0000FF".to_string(), // Blue
            header_fg_color: "#FFFFFF".to_string(), // White
            result_color: "#FFFF00".to_string(),    // Yellow
            theme: Theme {
                header_bg: "Blue".to_string(),
                header_fg: "White".to_string(),
                subheader_fg: "Green".to_string(),
                result_fg: "Yellow".to_string(),
                text_fg: "White".to_string(),
            },
        }
    }
}

// Implementation commented out until needed
/*
impl Config {
    pub fn load() -> Self {
        let config_path = Config::get_config_path();

        if Path::new(&config_path).exists() {
            match fs::read_to_string(&config_path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(config) => return config,
                    Err(e) => {
                        eprintln!("Error parsing config file: {}", e);
                        println!("Using default configuration");
                    }
                },
                Err(e) => {
                    eprintln!("Error reading config file: {}", e);
                    println!("Using default configuration");
                }
            }
        } else {
            println!("No config file found. Creating default configuration.");
            let config = Config::default();
            if let Err(e) = config.save() {
                eprintln!("Error saving default config: {}", e);
            }
            return config;
        }

        Config::default()
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config_path = Config::get_config_path();
        let config_dir = Path::new(&config_path).parent().unwrap();

        if !config_dir.exists() {
            fs::create_dir_all(config_dir)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        let mut file = fs::File::create(&config_path)?;
        file.write_all(content.as_bytes())?;

        Ok(())
    }

    fn get_config_path() -> String {
        let home = dirs::home_dir().unwrap_or_else(|| Path::new(".").to_path_buf());
        home.join(".config")
            .join("liiga_teletext")
            .join("config.json")
            .to_string_lossy()
            .to_string()
    }
}
*/
