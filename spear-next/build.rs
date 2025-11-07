// Build script for generating protobuf code / 用于生成protobuf代码的构建脚本

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Compile SMS protobuf files / 编译SMS protobuf文件
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(
            &[
                "proto/sms/node.proto",
                "proto/sms/task.proto"
            ],
            &["proto"],
        )?;

    // Compile Spearlet protobuf files / 编译Spearlet protobuf文件
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(
            &[
                "proto/spearlet/object.proto",
                "proto/spearlet/function.proto"
            ],
            &["proto"],
        )?;

    // Tell cargo to rerun this build script if proto files change
    // 告诉cargo在proto文件更改时重新运行此构建脚本
    println!("cargo:rerun-if-changed=proto/");

    Ok(())
}