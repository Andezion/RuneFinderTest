use titan_client::TitanApiBlocking;
use titan_client::TitanBlockingClient;

fn main() -> Result<(), Box<dyn std::error::Error>> {

    let client = TitanBlockingClient::new("http://localhost:3030");

    let status = client.get_status()?;
    println!("Status: {:?}", status);

    let tip = client.get_tip()?;
    println!("Block Tip: {:?}", tip);

    let address_data = client.get_address("bcrt1qnx4vqlhu0uk6jehxmvyu93qvqxe3hqw0dz2xzp")?;
    println!("Address Data: {:?}", address_data);

    Ok(())
}