use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    // Compile C source file
    let output = Command::new("make")
        // .arg("so") // 生成动态库
        .arg("static") // 生成静态库
        .current_dir("./timeout")
        .output()
        .expect("Failed to compile C source file");

    if !output.status.success() {
        panic!(
            "Failed to compile C source file: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // // Add shared library path to system library search path
    // let output = Command::new("sh")
    //     .arg("-c")
    //     .arg("export LD_LIBRARY_PATH=/workspaces/rs-timeout/timeout:$LD_LIBRARY_PATH")
    //     .output()
    //     .expect("Failed to add shared library path to system library search path");

    // if !output.status.success() {
    //     panic!(
    //         "Failed to add shared library path to system library search path: {}",
    //         String::from_utf8_lossy(&output.stderr)
    //     );
    // }

    // let dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    // println!(
    //     "cargo:rustc-link-search=native={}",
    //     Path::new(&dir).join("timeout").display()
    // );
    // Link shared library
    // let root = String::from(env::var_os("CARGO_MANIFEST_DIR").unwrap().to_str().unwrap());
    // // 将 /lib 添加到链接搜索目录
    // println!("cargo:rustc-link-search=native={}", format!("{}/timeout", &root));

    // println!("cargo:rustc-link-lib=timeout");
    // // println!("cargo:rustc-link-search=native=./timeout");
    // println!("cargo:rerun-if-changed=./timeout");

    // let dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo:rustc-link-search=native=./timeout");
    println!("cargo:rustc-link-lib=static=timeout");
}
