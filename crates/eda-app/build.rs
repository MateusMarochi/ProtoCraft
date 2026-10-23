fn main() {
    println!("cargo:rerun-if-changed=../../assets/protocraft.ico");
    println!("cargo:rerun-if-env-changed=PROTOCRAFT_REQUIRE_WINRES");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let mut resources = winresource::WindowsResource::new();
    resources
        .set_icon("../../assets/protocraft.ico")
        .set("ProductName", "ProtoCraft")
        .set("FileDescription", "ProtoCraft circuit design software")
        .set("CompanyName", "MateusMarochi")
        .set(
            "LegalCopyright",
            "Copyright (c) ProtoCraft contributors. MIT OR Apache-2.0.",
        )
        .set("OriginalFilename", "protocraft.exe")
        .set("InternalName", "protocraft");

    if let Err(error) = resources.compile() {
        if std::env::var_os("PROTOCRAFT_REQUIRE_WINRES").is_some() {
            panic!("failed to embed ProtoCraft Windows icon and version info: {error}");
        }
        println!("cargo:warning=protocraft.exe built without Windows icon resources: {error}");
    }
}
