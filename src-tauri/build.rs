fn main() {
    tauri_build::build();

    let is_windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");

    // Build vendored zlib (all platforms, avoids system zlib version differences)
    let mut zlib_build = cc::Build::new();
    zlib_build.warnings(false);
    if !is_windows {
        zlib_build.flag("-std=c99");
    }
    let zlib_sources = [
        "vendor/zlib/adler32.c",
        "vendor/zlib/compress.c",
        "vendor/zlib/crc32.c",
        "vendor/zlib/deflate.c",
        "vendor/zlib/infback.c",
        "vendor/zlib/inffast.c",
        "vendor/zlib/inflate.c",
        "vendor/zlib/inftrees.c",
        "vendor/zlib/trees.c",
        "vendor/zlib/uncompr.c",
        "vendor/zlib/zutil.c",
    ];
    for src in &zlib_sources {
        zlib_build.file(src);
    }
    zlib_build.include("vendor/zlib");
    zlib_build.compile("z");

    // Build readstat C library
    let mut build = cc::Build::new();
    build
        .warnings(false)
        .include("vendor/readstat/src")
        .include("vendor/readstat/src/spss")
        .include("vendor/zlib"); // zlib.h for readstat_sav_write.c

    if is_windows {
        // Windows has no system iconv; writer path never calls iconv() with a
        // non-NULL converter so a stub header is sufficient.
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
        // macOS needs iconv from system
        println!("cargo:rustc-link-lib=iconv");
    }

    build.compile("readstat");
}
