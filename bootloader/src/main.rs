use std::path::PathBuf;

fn main() {
    let uefi = std::env::var("UEFI_IMAGE").map(PathBuf::from).ok();
    let bios = std::env::var("BIOS_IMAGE").map(PathBuf::from).ok();

    println!("Aperture OS boot images built.");
    if let Some(path) = uefi {
        println!("  UEFI: {}", path.display());
    }
    if let Some(path) = bios {
        println!("  BIOS: {}", path.display());
    }
    println!("Run ./run-qemu.sh to start the OS in QEMU.");
}
