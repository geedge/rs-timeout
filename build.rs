fn main() {
    let path = "./";
    let lib = "timeout";

    println!("cargo:rustc-link-search=all={}", path);
    println!("cargo:rustc-link-lib=dylib={}", lib);
}
