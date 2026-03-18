fn main() {
    // Download pdfium.dll for Windows
    #[cfg(target_os = "windows")]
    {
        let out_dir = std::env::var("OUT_DIR").unwrap();
        let pdfium_path = std::path::Path::new(&out_dir).parent().unwrap().parent().unwrap().parent().unwrap();

        // PDFium version to download (using a known working version)
        let pdfium_version = "6164";
        let pdfium_url = format!(
            "https://github.com/nicholasoxford/pdfium-binaries/releases/download/chromium%2F{}/pdfium-mt-x64.dll.zip",
            pdfium_version
        );

        let dll_path = pdfium_path.join("pdfium.dll");

        if !dll_path.exists() {
            println!("cargo:warning=Downloading pdfium.dll...");

            // Download the zip file
            let response = ureq::get(&pdfium_url).call().expect("Failed to download pdfium");

            // Read the response body
            let mut buffer: Vec<u8> = Vec::new();
            std::io::Read::read_to_end(&mut response.into_reader(), &mut buffer).expect("Failed to read response");

            // Extract the DLL from the zip
            let reader = std::io::Cursor::new(&buffer);
            let mut archive = zip::ZipArchive::new(reader).expect("Failed to open zip archive");

            for i in 0..archive.len() {
                let mut file = archive.by_index(i).unwrap();
                let outpath = match file.enclosed_name() {
                    Some(path) => path.to_path_buf(),
                    None => continue,
                };

                if outpath.extension().map(|e| e == "dll").unwrap_or(false) {
                    let mut outfile = std::fs::File::create(&dll_path).expect("Failed to create dll file");
                    std::io::copy(&mut file, &mut outfile).expect("Failed to write dll");
                    println!("cargo:warning=Extracted pdfium.dll to {:?}", dll_path);
                    break;
                }
            }
        }

        // Copy to target directory for runtime
        let target_dll = std::path::Path::new("target/release/pdfium.dll");
        if dll_path.exists() && !target_dll.exists() {
            let _ = std::fs::copy(&dll_path, target_dll);
        }
    }

    tauri_build::build()
}