use std::path::PathBuf;
use std::env;

fn main() -> std::io::Result<()> {
    // Get the current directory and build the absolute path to device-protocol
    let current_dir = env::current_dir()?;
    let vault_root = current_dir.parent().unwrap().parent().unwrap();
    let proto_dir = vault_root.join("device-protocol");
    
    // Verify the protocol directory exists
    if !proto_dir.exists() {
        eprintln!("ERROR: Protocol directory does not exist: {:?}", proto_dir);
        eprintln!("Current dir: {:?}", current_dir);
        eprintln!("Vault root: {:?}", vault_root);
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Protocol directory not found: {:?}", proto_dir)
        ));
    }
    
    // Verify types.proto exists
    let types_proto = proto_dir.join("types.proto");
    if !types_proto.exists() {
        eprintln!("ERROR: types.proto does not exist: {:?}", types_proto);
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("types.proto not found: {:?}", types_proto)
        ));
    }
    
    println!("cargo:warning=Using protocol directory: {:?}", proto_dir);
    println!("cargo:warning=types.proto found at: {:?}", types_proto);

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
