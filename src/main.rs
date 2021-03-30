use argh::FromArgs;
use color_eyre::eyre::{self, WrapErr};
use frame_metadata::RuntimeMetadataPrefixed;
use subxt_gen::decode_metadata;

#[derive(FromArgs)]
/// Encode runtime metadata
struct SubxtGen {
    /// url of the parachain
    #[argh(option, default = "String::from(\"http://localhost:9933\")")]
    url: String,
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let args: SubxtGen = argh::from_env();

    let metadata = fetch_metadata(&args.url)?;
    let stream = decode_metadata(metadata)?;

    println!("{}", stream);

    Ok(())
}

fn fetch_metadata(url: &str) -> color_eyre::Result<RuntimeMetadataPrefixed> {
    let resp = ureq::post(url)
        .set("Content-Type", "application/json")
        .send_json(ureq::json!({
            "jsonrpc": "2.0",
            "method": "state_getMetadata",
            "id": 1
        }))
        .context("error fetching metadata from the substrate node")?;

    let json: serde_json::Value = resp.into_json()?;
    let hex_data = json["result"]
        .as_str()
        .ok_or(eyre::eyre!("metadata result field should be a string"))?;

    let bytes = hex::decode(hex_data.trim_start_matches("0x"))?;
    let decoded = scale::Decode::decode(&mut &bytes[..])?;
    Ok(decoded)
}
