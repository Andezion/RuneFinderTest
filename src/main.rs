use crate::rune_find1::{rune_find1};
use crate::rune_find2::{rune_find2};

use titan_client::TitanApi;
use titan_client::TitanClient;
use tokio;

mod rune_find1;
mod rune_find2;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let _ = rune_find1();
    // let _ = rune_find2();

    let client = TitanClient::new("http://localhost:3030");

    let status = client.get_status().await?;
    println!("Status: {:?}", status);


    let tip = client.get_tip().await?;
    println!("Block Tip: {:?}", tip);

    let address_data = client.get_address("your-bitcoin-address").await?;
    println!("Address Data: {:?}", address_data);

    Ok(())
}