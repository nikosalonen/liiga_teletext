// src/config.rs
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub api_domain: String,
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let config_path = dirs::config_dir()
            .ok_or("Could not find config directory")?
            .join("liiga_teletext")
            .join("config.toml");

        if !config_path.exists() {
            // Create default config if it doesn't exist
            fs::create_dir_all(config_path.parent().unwrap())?;
            fs::write(&config_path, include_str!("../config.toml"))?;
        }

        let contents = fs::read_to_string(config_path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
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
