// Build script for generating protobuf code / 用于生成protobuf代码的构建脚本

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("SKIP_PROTOC").is_ok() {
        println!("cargo:rerun-if-env-changed=SKIP_PROTOC");
        return Ok(());
    }
    let path = protoc_bin_vendored::protoc_bin_path()?;
    unsafe {
        std::env::set_var("PROTOC", &path);
    }
    // Compile SMS protobuf files / 编译SMS protobuf文件
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(
            &[
                "proto/sms/node.proto",
                "proto/sms/task.proto",
                "proto/sms/placement.proto",
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
                "proto/spearlet/function.proto",
            ],
            &["proto"],
        )?;

    // Tell cargo to rerun this build script if proto files change
    // 告诉cargo在proto文件更改时重新运行此构建脚本
    println!("cargo:rerun-if-changed=proto/");

    // Precompress admin assets
    println!("cargo:rerun-if-changed=assets/admin/");
    let out_dir = std::env::var("OUT_DIR")?;
    let assets = [
        ("assets/admin/index.html", "index.html"),
        ("assets/admin/main.js", "main.js"),
        ("assets/admin/main.css", "main.css"),
    ];
    for (src, name) in assets.iter() {
        let data = std::fs::read(src)?;
        let br_path = std::path::Path::new(&out_dir).join(format!("{}.br", name));
        let gz_path = std::path::Path::new(&out_dir).join(format!("{}.gz", name));
        {
            let mut w =
                brotli::CompressorWriter::new(std::fs::File::create(&br_path)?, 4096, 5, 22);
            use std::io::Write;
            w.write_all(&data)?;
        }
        {
            use flate2::{write::GzEncoder, Compression};
            use std::io::Write;
            let mut w = GzEncoder::new(std::fs::File::create(&gz_path)?, Compression::default());
            w.write_all(&data)?;
            w.finish()?;
        }
    }

    Ok(())
}
