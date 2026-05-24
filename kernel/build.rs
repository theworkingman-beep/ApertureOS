use std::path::Path;

fn main() {
    let ws_path = Path::new("../target/vibeos-x86_64/release/windowserver");
    let shell_path = Path::new("../target/vibeos-x86_64/release/desktop_shell");

    if ws_path.exists() && shell_path.exists() {
        println!("cargo:rustc-cfg=feature=\"userspace_gui\"");
        println!("cargo:rerun-if-changed={}", ws_path.display());
        println!("cargo:rerun-if-changed={}", shell_path.display());
    }
}