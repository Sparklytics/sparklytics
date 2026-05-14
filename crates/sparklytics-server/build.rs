// build.rs — Creates a placeholder dashboard/out/ directory so debug builds,
// `cargo check`, and tests can run before the Next.js dashboard is built.
// Release builds require the real dashboard output.

fn main() {
    let manifest_dir = match std::env::var("CARGO_MANIFEST_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    let profile = std::env::var("PROFILE").unwrap_or_default();

    let out_dir = std::path::PathBuf::from(&manifest_dir).join("../../dashboard/out");
    let index_path = out_dir.join("index.html");
    let placeholder_marker = "Dashboard not built.";

    if profile == "release" {
        let Ok(index_html) = std::fs::read_to_string(&index_path) else {
            panic!(
                "dashboard/out/index.html is missing. Run `cd dashboard && npm run build` before `cargo build --release`."
            );
        };

        if index_html.contains(placeholder_marker) || !out_dir.join("_next").is_dir() {
            panic!(
                "dashboard/out contains the development placeholder, not the exported dashboard. Run `cd dashboard && npm run build` before `cargo build --release`."
            );
        }
    }

    let _ = std::fs::create_dir_all(&out_dir);

    if !index_path.exists() {
        let _ = std::fs::write(
            &index_path,
            b"<!DOCTYPE html><html><head><title>Sparklytics</title></head>\
              <body><p>Dashboard not built. Run: <code>cd dashboard &amp;&amp; npm run build</code></p></body></html>",
        );
    }

    let public_tracker =
        std::path::PathBuf::from(&manifest_dir).join("../../dashboard/public/s.js");
    let embedded_tracker = out_dir.join("s.js");
    if !embedded_tracker.exists() && public_tracker.exists() {
        let _ = std::fs::copy(public_tracker, embedded_tracker);
    }

    // Re-run this script if the dashboard output changes.
    println!("cargo:rerun-if-changed={}", out_dir.display());
    println!(
        "cargo:rerun-if-changed={}",
        std::path::PathBuf::from(&manifest_dir)
            .join("../../dashboard/public/s.js")
            .display()
    );
}
