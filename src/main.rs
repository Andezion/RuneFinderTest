use crate::rune_find1::{rune_find1};
use crate::rune_find2::{rune_find2};

use titan_client::TitanClient;
use titan_client::TitanApi;

mod rune_find1;
mod rune_find2;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let _ = rune_find1();
    // let _ = rune_find2();

    let client = TitanClient::new("http://127.0.0.1:3030");

    let runes = client.get_runes(None).await?;
    println!("Runes list: {:?}", runes);

    if let Some(first_rune) = runes.items.first() {
        println!("First rune: {:?}", first_rune);

        let rune_info = client.get_rune(&first_rune.id).await?;
        println!("Rune info: {:?}", rune_info);

        let rune_txs = client.get_rune_transactions(&first_rune.id, None).await?;
        println!("Rune transactions: {:?}", rune_txs);
    } else {
        println!("No runes found in indexer.");
    }

    Ok(())
}