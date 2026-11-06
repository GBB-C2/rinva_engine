fn main() {
    println!("cargo:rerun-if-changed=app.manifest");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // GetWindowSubclass requires comctl32 v6. Embed activation before DLL loading;
        // activating a context in main would be too late for a static import.
        // GNU needs a COFF resource, while MSVC uses /MANIFEST:EMBED. The helper
        // handles both without requiring windres, rc.exe or a sidecar manifest.
        let manifest = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
            .join("app.manifest");
        embed_manifest::embed_manifest_file(manifest)
            .expect("failed to embed the Common Controls v6 application manifest");
    }
}
