fn main() -> Result<(), anyhow::Error> {
    println!("cargo:rerun-if-changed=../../submodules/durabletask-protobuf");
    tonic_build::configure()
        .build_server(false)
        .build_client(true)
        .build_transport(true)
        .protoc_arg("--experimental_allow_proto3_optional")
        .compile_protos(
            &["../../submodules/durabletask-protobuf/protos/orchestrator_service.proto"],
            &["../../submodules/durabletask-protobuf"],
        )?;
    Ok(())
}
