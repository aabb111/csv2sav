fn main() {
    tauri_build::build();

    let mut build = cc::Build::new();
    build
        .warnings(false)
        .include("vendor/readstat/src")
        .include("vendor/readstat/src/spss");

    let is_windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");

    if is_windows {
        // On Windows (MSVC) there is no system iconv.
        // The writer path never calls iconv() with a non-NULL converter,
        // so a stub header is sufficient.
        build.include("vendor/iconv-stub");
    }

    let core_sources = [
        "vendor/readstat/src/CKHashTable.c",
        "vendor/readstat/src/readstat_bits.c",
        "vendor/readstat/src/readstat_convert.c",
        "vendor/readstat/src/readstat_error.c",
        "vendor/readstat/src/readstat_io_unistd.c",
        "vendor/readstat/src/readstat_malloc.c",
        "vendor/readstat/src/readstat_metadata.c",
        "vendor/readstat/src/readstat_parser.c",
        "vendor/readstat/src/readstat_value.c",
        "vendor/readstat/src/readstat_variable.c",
        "vendor/readstat/src/readstat_writer.c",
    ];

    let spss_sources = [
        "vendor/readstat/src/spss/readstat_sav.c",
        "vendor/readstat/src/spss/readstat_sav_compress.c",
        "vendor/readstat/src/spss/readstat_sav_parse.c",
        "vendor/readstat/src/spss/readstat_sav_parse_timestamp.c",
        "vendor/readstat/src/spss/readstat_sav_read.c",
        "vendor/readstat/src/spss/readstat_sav_write.c",
        "vendor/readstat/src/spss/readstat_spss.c",
        "vendor/readstat/src/spss/readstat_spss_parse.c",
        "vendor/readstat/src/spss/readstat_zsav_compress.c",
        "vendor/readstat/src/spss/readstat_zsav_read.c",
        "vendor/readstat/src/spss/readstat_zsav_write.c",
    ];

    for src in core_sources.iter().chain(spss_sources.iter()) {
        build.file(src);
    }

    build.define("HAVE_ZLIB", None);

    if !is_windows {
        build.flag("-std=c99");
    }

    build.compile("readstat");

    if is_windows {
        // Link zlib from vcpkg (installed as zlib:x64-windows-static in CI)
        let vcpkg_root = std::env::var("VCPKG_ROOT")
            .or_else(|_| std::env::var("VCPKG_INSTALLATION_ROOT"))
            .unwrap_or_else(|_| "C:/vcpkg".to_string());
        println!(
            "cargo:rustc-link-search=native={}/installed/x64-windows-static/lib",
            vcpkg_root
        );
        println!("cargo:rustc-link-lib=static=zlib");
    } else {
        println!("cargo:rustc-link-lib=z");
        println!("cargo:rustc-link-lib=iconv");
    }
}
