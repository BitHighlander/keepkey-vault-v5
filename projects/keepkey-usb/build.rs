use std::path::PathBuf;

fn main() -> std::io::Result<()> {
    // Use the protocol definitions from the device-protocol directory.
    let proto_dir: PathBuf = ["..", "..", "..", "device-protocol"].iter().collect();

    // Set protoc environment variables for vendored protoc
    std::env::set_var("PROTOC", protoc_bin_vendored::protoc_bin_path().unwrap());
    std::env::set_var(
        "PROTOC_INCLUDE",
        protoc_bin_vendored::include_path().unwrap(),
    );

    // Configure prost build with serde support
    let mut config = prost_build::Config::new();
    config.type_attribute(".", "#[::serde_with::serde_as]");
    config.type_attribute(".", "#[::serde_with::skip_serializing_none]");
    config.type_attribute(".", "#[derive(::serde::Serialize)]");
    config.type_attribute(".", "#[serde(rename_all = \"camelCase\")]");
    config.field_attribute(
        ".CoinType.contract_address",
        "#[serde_as(as = \"Option<::serde_with::hex::Hex>\")]",
    );
    config.btree_map(["."]);

    // Compile all protocol files including chain-specific ones
    config.compile_protos(
        &[
            proto_dir.join("types.proto"),
            proto_dir.join("messages.proto"),
            proto_dir.join("messages-binance.proto"),
            proto_dir.join("messages-cosmos.proto"),
            proto_dir.join("messages-eos.proto"),
            proto_dir.join("messages-ethereum.proto"),
            proto_dir.join("messages-mayachain.proto"),
            proto_dir.join("messages-nano.proto"),
            proto_dir.join("messages-osmosis.proto"),
            proto_dir.join("messages-ripple.proto"),
            proto_dir.join("messages-tendermint.proto"),
            proto_dir.join("messages-thorchain.proto"),
        ],
        &[proto_dir],
    )?;

    Ok(())
}
