#[cfg(feature = "build_cc")]
fn main() {
    let (debug, optim_level) = if cfg!(feature = "debug") {
        (true, "-O0")
    } else {
        (false, "-O3")
    };

    let mut build = cc::Build::new();
    build.include("snmalloc/src")
        .file("snmalloc/src/snmalloc/override/rust.cc")
        .cpp(true)
        .debug(debug);

    configure_compiler(&mut build, optim_level);
    configure_features(&mut build);

    let target = if cfg!(feature = "check") {
        "snmallocshim-rust"
    } else {
        "snmallocshim-checks-rust"
    };
    
    build.compile(target);
    configure_linking();
}

#[cfg(feature = "build_cc")]
fn configure_compiler(build: &mut cc::Build, optim_level: &str) {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").expect("target_os not defined!");
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").expect("target_env not defined!");
    let target_family = std::env::var("CARGO_CFG_TARGET_FAMILY").expect("target family not set");

    if target_os == "windows" {
        configure_windows_build(build);
    }

    build.flag_if_supported(optim_level)
        .flag_if_supported("-fomit-frame-pointer");

    if target_env == "msvc" {
        configure_msvc_build(build);
    }

    // C++ Standard selection
    let cpp_std = if cfg!(feature = "usecxx17") {
        ["-std=c++17", "/std:c++17"]
    } else {
        ["-std=c++20", "/std:c++20"]
    };
    cpp_std.iter().for_each(|std| { build.flag_if_supported(std); });

    // Unix-specific TLS model
    if (target_family == "unix" || target_env == "gnu") && target_os != "haiku" {
        let tls_model = if cfg!(feature = "local_dynamic_tls") {
            "-ftls-model=local-dynamic"
        } else {
            "-ftls-model=initial-exec"
        };
        build.flag_if_supported(tls_model);
    }
}

#[cfg(feature = "build_cc")]
fn configure_windows_build(build: &mut cc::Build) {
    if let Ok(msystem) = std::env::var("MSYSTEM") {
        match msystem.as_str() {
            "CLANG64" | "CLANGARM64" => {
                build.flag_if_supported("-flto")
                    .flag_if_supported("-fuse-ld=lld")
                    .flag_if_supported("-stdlib=libc++")
                    .flag_if_supported("-Wno-error=unknown-pragmas")
                    .flag_if_supported("-Qunused-arguments");
            }
            "UCRT64" => {
                build.flag_if_supported("-flto")
                    .flag_if_supported("-fuse-ld=lld")
                    .flag_if_supported("-Wno-error=unknown-pragmas");
            }
            _ => {}
        }
    }

    build.flag_if_supported("-mcx16")
        .flag_if_supported("-fno-exceptions")
        .flag_if_supported("-fno-rtti");
}

#[cfg(feature = "build_cc")]
fn configure_msvc_build(build: &mut cc::Build) {
    let msvc_flags = [
        "/nologo", "/W4", "/WX", "/wd4127", "/wd4324", "/wd4201",
        "/Ob2", "/DNDEBUG", "/EHsc", "/Gd", "/TP", "/Gm-", "/GS",
        "/fp:precise", "/Zc:wchar_t", "/Zc:forScope", "/Zc:inline"
    ];
    msvc_flags.iter().for_each(|flag| { build.flag_if_supported(flag); });
}

#[cfg(feature = "build_cc")]
fn configure_features(build: &mut cc::Build) {
    if cfg!(feature = "native-cpu") {
        build.define("SNMALLOC_OPTIMISE_FOR_CURRENT_MACHINE", "ON")
            .flag_if_supported("-march=native");
    }
    if cfg!(feature = "qemu") {
        build.define("SNMALLOC_QEMU_WORKAROUND", "ON");
    }
    if cfg!(feature = "lto") {
        build.define("SNMALLOC_IPO", "ON");
    }
    if cfg!(feature = "notls") {
        build.define("SNMALLOC_ENABLE_DYNAMIC_LOADING", "ON");
    }
    if cfg!(feature = "win8compat") {
        build.define("WINVER", "0x0603");
    }
    build.define(
        "SNMALLOC_USE_WAIT_ON_ADDRESS",
        if cfg!(feature = "usewait-on-address") { "1" } else { "0" }
    );
}

#[cfg(feature = "build_cc")]
fn configure_linking() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").expect("target_os not defined!");
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").expect("target_env not defined!");

    if target_env == "msvc" && !cfg!(feature = "win8compat") {
        println!("cargo:rustc-link-lib=dylib=mincore");
    }
    if target_os == "windows" && target_env == "gnu" {
        println!("cargo:rustc-link-lib=dylib=atomic");
    }
    if target_os == "linux" {
        println!("cargo:rustc-link-lib=dylib=atomic");
    }
    if !cfg!(target_os = "freebsd") && cfg!(all(unix, not(target_os = "macos"))) {
        if cfg!(target_env = "gnu") {
            println!("cargo:rustc-link-lib=c_nonshared");
        }
    } else if !cfg!(windows) {
        let cxxlib = if cfg!(any(target_os = "macos", target_os = "openbsd")) {
            "c++"
        } else {
            "stdc++"
        };
        println!("cargo:rustc-link-lib={}", cxxlib);
    }
}
#[cfg(not(feature = "build_cc"))]
fn main() {
    // Clean build directory if exists
    if let Ok(metadata) = std::fs::metadata("build") {
        if metadata.is_dir() {
            let _ = std::fs::remove_dir_all("build");
        }
    }

    let mut config = cmake::Config::new("snmalloc");
    let build_type = if cfg!(feature = "debug") {
        "Debug"
    } else {
        "Release"
    };

    // Basic configuration
    config
        .define("SNMALLOC_RUST_SUPPORT", "ON")
        .profile(build_type)
        .generator("Ninja")
        .very_verbose(true)
        .define("CMAKE_SH", "CMAKE_SH-NOTFOUND");

    // MSYS2 specific configuration
    if let Ok(msystem) = std::env::var("MSYSTEM") {
        match msystem.as_str() {
            "CLANG64" | "CLANGARM64" => {
                config.define("CMAKE_CXX_COMPILER", "clang++")
                      .define("CMAKE_C_COMPILER", "clang")
                      .define("CMAKE_CXX_FLAGS", 
                          "-fuse-ld=lld -stdlib=libc++ -mcx16 -Wno-error=unknown-pragmas -Qunused-arguments")
                      .define("CMAKE_C_FLAGS",
                          "-fuse-ld=lld -Wno-error=unknown-pragmas -Qunused-arguments")
                      .define("CMAKE_EXE_LINKER_FLAGS",
                          "-fuse-ld=lld -stdlib=libc++");
            }
            "UCRT64" => {
                config
                    .define("CMAKE_CXX_FLAGS", "-fuse-ld=lld -Wno-error=unknown-pragmas")
                    .define("CMAKE_SYSTEM_NAME", "Windows")
                    .define("CMAKE_C_FLAGS", "-fuse-ld=lld -Wno-error=unknown-pragmas");
            }
            _ => {}
        }
    }

    let triple = std::env::var("TARGET").expect("TARGET not set");
    if triple.contains("android") {
        configure_android(&mut config, &triple);
    }

    if cfg!(all(windows, target_env = "msvc")) {
        config
            .define("CMAKE_CXX_FLAGS_RELEASE", "/O2 /Ob2 /DNDEBUG /EHsc")
            .define("CMAKE_C_FLAGS_RELEASE", "/O2 /Ob2 /DNDEBUG /EHsc");
        if cfg!(feature = "win8compat") {
            config.define("WIN8COMPAT", "ON");
        }
    }

    if cfg!(feature = "native-cpu") {
        config.define("SNMALLOC_OPTIMISE_FOR_CURRENT_MACHINE", "ON");
    }
    if cfg!(feature = "usecxx17") {
        config.define("SNMALLOC_USE_CXX17", "ON");
    }
    if cfg!(feature = "stats") {
        config.define("USE_SNMALLOC_STATS", "ON");
    }
    if cfg!(feature = "qemu") {
        config.define("SNMALLOC_QEMU_WORKAROUND", "ON");
    }
    config.define(
        "SNMALLOC_USE_WAIT_ON_ADDRESS",
        if cfg!(feature = "usewait-on-address") {
            "1"
        } else {
            "0"
        },
    );

    let target = if cfg!(feature = "check") {
        "snmallocshim-checks-rust"
    } else {
        "snmallocshim-rust"
    };

    let mut dst = config.build_target(target).build();
    dst.push("build");

    println!("cargo:rustc-link-lib=static={}", target);
    if cfg!(all(windows, target_env = "msvc")) {
        if !cfg!(feature = "win8compat") {
            println!("cargo:rustc-link-lib=dylib=mincore");
        }
        println!(
            "cargo:rustc-link-search=native={}/{}",
            dst.display(),
            build_type
        );
    } else {
        println!("cargo:rustc-link-search=native={}", dst.display());
        if cfg!(all(windows, target_env = "gnu")) {
            ["bcrypt", "atomic", "winpthread", "gcc_s"]
                .iter()
                .for_each(|lib| println!("cargo:rustc-link-lib=dylib={}", lib));
        }
        if cfg!(target_os = "linux") {
            println!("cargo:rustc-link-lib=dylib=atomic");
        }
        if !cfg!(target_os = "freebsd") && cfg!(all(unix, not(target_os = "macos"))) {
            if cfg!(target_env = "gnu") {
                println!("cargo:rustc-link-lib=c_nonshared");
            }
        } else if !cfg!(windows) {
            let cxxlib = if cfg!(any(target_os = "macos", target_os = "openbsd")) {
                "c++"
            } else {
                "stdc++"
            };
            println!("cargo:rustc-link-lib={}", cxxlib);
        }
    }
}

#[cfg(not(feature = "build_cc"))]
fn configure_android(config: &mut cmake::Config, triple: &str) {
    let ndk = std::env::var("ANDROID_NDK").expect("ANDROID_NDK environment variable not set");
    config.define(
        "CMAKE_TOOLCHAIN_FILE",
        format!("{}/build/cmake/android.toolchain.cmake", ndk),
    );

    if let Ok(platform) = std::env::var("ANDROID_PLATFORM") {
        config.define("ANDROID_PLATFORM", platform);
    }

    if cfg!(feature = "android-lld") {
        config.define("ANDROID_LD", "lld");
    }

    let abi = match triple {
        t if t.contains("aarch64") => "arm64-v8a",
        t if t.contains("armv7") => {
            config.define("ANDROID_ARM_MODE", "arm");
            "armeabi-v7a"
        }
        t if t.contains("x86_64") => "x86_64",
        t if t.contains("i686") => "x86",
        t if t.contains("neon") => "armeabi-v7a with NEON",
        t if t.contains("arm") => "armeabi-v7a",
        _ => panic!("Unsupported Android architecture"),
    };
    config.define("ANDROID_ABI", abi);
}
