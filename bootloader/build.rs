use std::path::PathBuf;
use std::process::Command;

fn main() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("bootloader package should be inside a workspace")
        .to_path_buf();

    let kernel_elf = workspace_root
        .join("target")
        .join("x86_64-unknown-none")
        .join("debug")
        .join("kernel");

    // Build the kernel binary for the x86_64-unknown-none target.
    let status = Command::new("cargo")
        .arg("build")
        .arg("-p")
        .arg("kernel")
        .arg("-Z")
        .arg("build-std=core,compiler_builtins,alloc")
        .arg("-Z")
        .arg("build-std-features=compiler-builtins-mem")
        .arg("--target")
        .arg("x86_64-unknown-none")
        .current_dir(&workspace_root)
        .status()
        .expect("Failed to execute cargo build for kernel");

    if !status.success() {
        panic!("Kernel build failed");
    }

    if !kernel_elf.exists() {
        panic!("Kernel ELF not found at {}", kernel_elf.display());
    }

    let uefi_path = workspace_root.join("target/aperture-uefi.img");
    let bios_path = workspace_root.join("target/aperture-bios.img");

    std::fs::create_dir_all(workspace_root.join("target"))
        .expect("Failed to create target directory");

    let mut builder = bootloader::DiskImageBuilder::new(kernel_elf.clone());

    builder
        .create_uefi_image(&uefi_path)
        .expect("Failed to create UEFI disk image");

    builder
        .create_bios_image(&bios_path)
        .expect("Failed to create BIOS disk image");

    println!("cargo:rerun-if-changed={}", kernel_elf.display());
    println!("cargo:rustc-env=UEFI_IMAGE={}", uefi_path.display());
    println!("cargo:rustc-env=BIOS_IMAGE={}", bios_path.display());

    eprintln!("UEFI image: {}", uefi_path.display());
    eprintln!("BIOS image: {}", bios_path.display());
}
