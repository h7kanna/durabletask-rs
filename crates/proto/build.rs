fn main() -> Result<(), anyhow::Error> {
    println!("cargo:rerun-if-changed=../../submodules/durabletask-protobuf");
    tonic_prost_build::configure()
        .build_server(false)
        .build_client(true)
        .build_transport(true)
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .extern_path(".google.protobuf.Duration", "::prost_wkt_types::Duration")
        .extern_path(".google.protobuf.Timestamp", "::prost_wkt_types::Timestamp")
        .protoc_arg("--experimental_allow_proto3_optional")
        .compile_protos(
            &["../../submodules/durabletask-protobuf/protos/orchestrator_service.proto"],
            &["../../submodules/durabletask-protobuf"],
        )?;
    Ok(())
}
