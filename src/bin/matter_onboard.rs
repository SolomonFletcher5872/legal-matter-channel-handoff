use std::{env, fs};

use legal_matter_onboarding::{InfraiClient, MatterIntake, onboard_matter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input_path = env::args()
        .nth(1)
        .ok_or("usage: matter_onboard <matter.json>")?;
    let intake: MatterIntake = serde_json::from_str(&fs::read_to_string(input_path)?)?;
    let infrai = InfraiClient::from_env()?;
    let receipt = onboard_matter(&infrai, intake).await?;
    println!("{}", serde_json::to_string_pretty(&receipt)?);
    Ok(())
}
