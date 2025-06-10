// // build.rs
// fn main() -> Result<(), Box<dyn std::error::Error>> {
//     // Use the vendored protoc binary
//     let protoc = protoc_bin_vendored::protoc_bin_path()
//         .expect("protoc binary missing");
//     tonic_build::configure()
//         .proto_path(protoc)
//         .build_server(true)
//         .compile_protos(&["proto/echo.proto"], &["proto"])?;
//     // Re-run this script if the .proto changes
//     println!("cargo:rerun-if-changed=proto/echo.proto");
//     Ok(())
// }
// build.rs

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1) Vendor in a protoc binary so no system install is needed
    let protoc_path = protoc_bin_vendored::protoc_bin_path()
        .expect("protoc-bin-vendored failed to provide `protoc`") ;
    // Tell Prost to use this binary
    std::env::set_var("PROTOC", &protoc_path);

    // 2) Compile .proto → Rust (Tonic + Prost)
    tonic_build::configure()
        // Optionally tweak where `include_proto!` looks; defaults to `super`
        .proto_path("proto")
        // Generate both client and server code
        .build_server(true)
        .compile(&["proto/echo.proto"], &["proto"])?;

    // 3) Re-run this script if the proto changes
    println!("cargo:rerun-if-changed=proto/echo.proto");

    Ok(())
}
