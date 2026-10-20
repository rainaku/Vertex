fn main() {
    let json = serde_json::to_string_pretty(&vertex_core::Registry::default().contract()).unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../frontend/src/lib/conversion-contract.json");
    std::fs::write(path, format!("{json}\n")).unwrap();
}
