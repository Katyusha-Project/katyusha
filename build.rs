fn main() {
    // Compile the C core and link it as a static library.
    cc::Build::new()
        .file("c_src/core.c")
        .warnings(true)
        .flag_if_supported("-O2")
        .compile("katyusha_core");

    println!("cargo:rerun-if-changed=c_src/core.c");
    println!("cargo:rerun-if-changed=c_src/core.h");
}
