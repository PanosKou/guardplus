// build.rs
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use the vendored protoc
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    std::env::set_var("PROTOC", &protoc);

    // Compile echo.proto into Rust code under OUT_DIR
    tonic_build::configure()
        .build_server(true)
        .compile(&["proto/echo.proto"], &["proto"])?;
    println!("cargo:rerun-if-changed=proto/echo.proto");
    Ok(())
}
