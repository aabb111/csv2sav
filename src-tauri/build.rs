fn main() {
    tauri_build::build();

    let mut build = cc::Build::new();
    build
        .warnings(false)
        .include("vendor/readstat/src")
        .include("vendor/readstat/src/spss");

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

    if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
        build.flag("-std=c99");
    }

    build.compile("readstat");

    println!("cargo:rustc-link-lib=z");
    println!("cargo:rustc-link-lib=iconv");
}
