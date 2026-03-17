fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(not(target_arch = "wasm32"))]
    tonic_prost_build::compile_protos("proto/farm.proto")?;
    Ok(())
}
