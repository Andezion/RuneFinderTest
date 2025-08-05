use reqwest::Client;
use std::error::Error;
use tokio;
use serde::{Deserialize, Serialize};
use hex::FromHex;

#[derive(Debug, Deserialize, Serialize)]
pub struct Transaction {
    pub txid: String,
    pub version: u32,
    pub vin: Vec<TxIn>,
    pub vout: Vec<TxOut>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TxIn {
    pub txid: String,
    pub vout: u32,
    #[serde(rename = "scriptsig")]
    pub script_sig: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TxOut {
    #[serde(rename = "scriptpubkey")]
    pub script_pubkey: String,
    pub value: u64,
}

#[derive(Debug)]
pub struct RuneEvent {
    pub rune_id: String,
    pub amount: u128,
    pub event_type: String,
}

pub const API_URL: &str = "https://blockstream.info/api";

pub fn extract_tlv(data: &[u8]) -> Vec<(u8, Vec<u8>)> {
    let mut i = 0;
    let mut result = vec![];

    while i + 2 <= data.len() {
        let t = data[i];
        let l = data[i + 1] as usize;
        i += 2;

        if i + l > data.len() || l > 100 {
            break;
        }

        let v = data[i..i + l].to_vec();
        result.push((t, v));
        i += l;
    }

    result
}

pub fn has_rune_marker(data: &[u8]) -> bool {
    if data.len() < 4 {
        return false;
    }

    for window in data.windows(4) {
        if window == [0x52, 0x55, 0x4E, 0x45] ||
            window == [0x00, 0x00, 0x00, 0x00] ||
            (window[0] == 0x00 && window[1] > 0 && window[1] < 10)
        {
            return true;
        }
    }

    false
}

pub fn parse_rune_data(script_hex: &str) -> Option<RuneEvent> {
    let bytes = Vec::from_hex(script_hex).ok()?;

    if bytes.first() != Some(&0x6a) {
        return None;
    }

    if bytes.len() < 10 {
        return None;
    }

    let payload = &bytes[2..];

    if !has_rune_marker(payload) && payload.len() < 20 {
        return None;
    }

    let tlv = extract_tlv(payload);

    if tlv.is_empty() {

        if payload.len() >= 16 {
            return Some(RuneEvent {
                rune_id: hex::encode(&payload[..16]),
                amount: payload.len() as u128 * 42,
                event_type: "HEURISTIC".to_string(),
            });
        }
        return None;
    }

    let mut rune_id = None;
    let mut amount = None;

    for (t, v) in tlv {
        match t {
            0x00..=0x0F => {
                if v.len() >= 4 && v.len() <= 32 {
                    rune_id = Some(hex::encode(&v));
                }
            }
            0x10..=0x1F => {
                if !v.is_empty() && v.len() <= 16 {
                    amount = Some(v.iter().fold(0u128, |acc, &b| (acc << 8) | (b as u128)));
                }
            }
            _ => continue,
        }
    }

    if let (Some(id), Some(amt)) = (rune_id.clone(), amount) {
        Some(RuneEvent {
            rune_id: id,
            amount: amt,
            event_type: "TLV_STRUCTURED".to_string(),
        })
    } else if let Some(id) = rune_id {
        Some(RuneEvent {
            rune_id: id,
            amount: 1000,
            event_type: "TLV_PARTIAL".to_string(),
        })
    } else {

        if payload.len() >= 8 {
            let interesting_data = payload.iter()
                .enumerate()
                .find(|&(_, &b)| b > 0 && b != 0xFF)
                .map(|(i, _)| i)
                .unwrap_or(0);

            if interesting_data < payload.len() - 8 {
                return Some(RuneEvent {
                    rune_id: hex::encode(&payload[interesting_data..interesting_data + 8]),
                    amount: payload[interesting_data] as u128 * 100,
                    event_type: "PATTERN_MATCH".to_string(),
                });
            }
        }
        None
    }
}

pub fn validate_rune_event(tx: &Transaction, rune_event: &RuneEvent) -> bool {
    if tx.vin.is_empty() || tx.vout.is_empty() {
        return false;
    }

    if rune_event.rune_id.is_empty() || rune_event.rune_id.len() < 8 {
        return false;
    }

    if rune_event.amount == 0 {
        return false;
    }

    if !rune_event.rune_id.chars().all(|c| c.is_ascii_hexdigit()) {
        return false;
    }

    match rune_event.event_type.as_str() {
        "TLV_STRUCTURED" => rune_event.amount < 1_000_000_000_000,
        "TLV_PARTIAL" => rune_event.rune_id.len() >= 16,
        "HEURISTIC" | "PATTERN_MATCH" => rune_event.rune_id.len() >= 12,
        _ => true,
    }
}

pub async fn get_recent_block_hashes(client: &Client, count: usize) -> Result<Vec<String>, Box<dyn Error>> {
    let mut blocks = Vec::new();

    let last_block_hash = client
        .get(&format!("{}/blocks/tip/hash", API_URL))
        .send()
        .await?
        .text()
        .await?;

    println!("Получаем {} последних блоков, начиная с: {}", count, last_block_hash);

    let mut current_hash = last_block_hash;

    for i in 0..count {
        blocks.push(current_hash.clone());

        if i < count - 1 {
            #[derive(Deserialize)]
            struct BlockInfo {
                previousblockhash: Option<String>,
            }

            match client
                .get(&format!("{}/block/{}", API_URL, current_hash))
                .send()
                .await?
                .json::<BlockInfo>()
                .await
            {
                Ok(block_info) => {
                    if let Some(prev_hash) = block_info.previousblockhash {
                        current_hash = prev_hash;
                    } else {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    }

    Ok(blocks)
}

#[tokio::main]
pub async fn rune_find2() -> Result<(), Box<dyn Error>> {
    let client = Client::new();


    let block_hashes = get_recent_block_hashes(&client, 3).await?;

    let mut total_rune_events = 0;
    let mut total_processed = 0;
    let mut total_successful = 0;
    let mut op_return_found = 0;

    for (block_num, block_hash) in block_hashes.iter().enumerate() {
        println!("\nАнализируем блок {} ({}...)", block_num + 1, &block_hash[..16]);

        let tx_ids: Vec<String> = client
            .get(&format!("{}/block/{}/txids", API_URL, block_hash))
            .send()
            .await?
            .json()
            .await?;

        println!("Найдено {} транзакций", tx_ids.len());

        let mut block_rune_events = 0;
        let tx_limit = std::cmp::min(150, tx_ids.len());

        for (tx_index, txid) in tx_ids.iter().take(tx_limit).enumerate() {
            total_processed += 1;

            match client
                .get(&format!("{}/tx/{}", API_URL, txid))
                .send()
                .await
            {
                Ok(response) => {
                    match response.json::<Transaction>().await {
                        Ok(tx) => {
                            total_successful += 1;

                            for (vout_index, vout) in tx.vout.iter().enumerate() {
                                if vout.script_pubkey.starts_with("6a") {
                                    op_return_found += 1;
                                }

                                if let Some(rune_event) = parse_rune_data(&vout.script_pubkey) {
                                    if validate_rune_event(&tx, &rune_event) {
                                        println!("\n---- Rune ----");
                                        println!("   Блок: {}...", &block_hash[..16]);
                                        println!("   Транзакция: {}", tx.txid);
                                        println!("   Выход №: {}", vout_index);
                                        println!("   Тип: {}", rune_event.event_type);
                                        println!("   ID: {}", rune_event.rune_id);
                                        println!("   Количество: {}", rune_event.amount);
                                        println!("   Скрипт: {}", &vout.script_pubkey);
                                        block_rune_events += 1;
                                        total_rune_events += 1;
                                    }
                                }
                            }
                        }
                        Err(_) => {}
                    }
                }
                Err(_) => {}
            }
        }

        println!("Блок {} завершен, найдено Rune событий: {}", block_num + 1, block_rune_events);
        println!("OP_RETURN выходов в блоке: {}", op_return_found);
        op_return_found = 0;
    }

    println!("\nСтатистика:");
    println!("   Проанализировано блоков: {}", block_hashes.len());
    println!("   Обработано транзакций: {}", total_processed);
    println!("   Успешно спарсено: {}", total_successful);
    println!("   Найдено потенциальных Rune событий: {}", total_rune_events);

    Ok(())
}