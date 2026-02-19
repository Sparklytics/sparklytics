// build.rs — Creates a placeholder dashboard/out/ directory so `cargo build`
// succeeds even when the Next.js dashboard hasn't been built yet.
// Run `cd dashboard && npm run build` before `cargo build` for the real UI.

fn main() {
    let manifest_dir = match std::env::var("CARGO_MANIFEST_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };

    let out_dir = std::path::PathBuf::from(&manifest_dir).join("../../dashboard/out");

    if !out_dir.exists() {
        let _ = std::fs::create_dir_all(&out_dir);
        let _ = std::fs::write(
            out_dir.join("index.html"),
            b"<!DOCTYPE html><html><head><title>Sparklytics</title></head>\
              <body><p>Dashboard not built. Run: <code>cd dashboard &amp;&amp; npm run build</code></p></body></html>",
        );
    }

    // Re-run this script if the dashboard output changes.
    println!("cargo:rerun-if-changed={}", out_dir.display());
}
