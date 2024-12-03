#![allow(dead_code)]

use std::env;
use std::fs;

#[derive(Debug)]
enum Compiler {
    Clang,
    Gcc,
    Msvc,
    Unknown
}

struct BuildConfig {
    debug: bool,
    optim_level: &'static str,
    target_os: String,
    target_env: String,
    target_family: String,
    target: String,
    out_dir: String,
    build_type: String,
    msystem: Option<String>,
    cmake_cxx_standard: &'static str,
    target_lib: &'static str,
    features: BuildFeatures,
    #[cfg(feature = "build_cc")]
    builder: cc::Build,
    #[cfg(not(feature = "build_cc"))]
    builder: cmake::Config,
    compiler: Compiler
}
impl std::fmt::Debug for BuildConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuildConfig")
            .field("debug", &self.debug)
            .field("optim_level", &self.optim_level)
            .field("target_os", &self.target_os)
            .field("target_env", &self.target_env)
            .field("target_family", &self.target_family)
            .field("out_dir", &self.out_dir)
            .field("build_type", &self.build_type)
            .field("msystem", &self.msystem)
            .field("cmake_cxx_standard", &self.cmake_cxx_standard)
            .field("target_lib", &self.target_lib)
            .field("features", &self.features)
            .finish()
    }
}

#[derive(Debug)]
struct BuildFeatures {
    native_cpu: bool,
    qemu: bool,
    wait_on_address: bool,
    lto: bool,
    notls: bool,
    win8compat: bool,
    stats: bool,
    android_lld: bool,
    local_dynamic_tls: bool,
}

impl BuildConfig {
    fn new() -> Self {
        let debug = cfg!(feature = "debug");
        #[cfg(feature = "build_cc")]
        let builder = cc::Build::new();
        
        #[cfg(not(feature = "build_cc"))]
        let builder = Config::new("snmalloc");
        let mut config = Self {
            debug,
            optim_level: if debug { "-O0" } else { "-O3" },
            target_os: env::var("CARGO_CFG_TARGET_OS").expect("target_os not defined!"),
            target_env: env::var("CARGO_CFG_TARGET_ENV").expect("target_env not defined!"),
            target_family: env::var("CARGO_CFG_TARGET_FAMILY").expect("target family not set"),
            target: env::var("TARGET").expect("TARGET not set"),
            out_dir: env::var("OUT_DIR").unwrap(),
            build_type: if debug { "Debug" } else { "Release" }.to_string(),
            msystem: env::var("MSYSTEM").ok(),
            cmake_cxx_standard: if cfg!(feature = "usecxx17") { "17" } else { "20" },
            target_lib: if cfg!(feature = "check") {
                "snmallocshim-checks-rust"
            } else {
                "snmallocshim-rust"
            },
            features: BuildFeatures::new(),
            builder,
            compiler: Compiler::Unknown,
        };
        config.compiler = config.detect_compiler();
        config.embed_build_info();
        config
    }
    fn detect_compiler(&self) -> Compiler {
        if self.is_msvc() {
            return Compiler::Msvc;
        }

        // Check for clang first since it can masquerade as gcc
        if let Ok(cc) = env::var("CC") {
            if cc.contains("clang") {
                return Compiler::Clang;
            }
            if cc.contains("gcc") {
                return Compiler::Gcc;
            }
        }

        Compiler::Unknown
    }
    fn embed_build_info(&self) {
        println!("cargo:rustc-env=BUILD_TARGET_OS={}", self.target_os);
        println!("cargo:rustc-env=BUILD_TARGET_ENV={}", self.target_env);
        println!("cargo:rustc-env=BUILD_TARGET_FAMILY={}", self.target_family);
        println!("cargo:rustc-env=BUILD_TARGET={}", self.target);
        println!("cargo:rustc-env=BUILD_CC={:#?}", self.compiler);
        println!("cargo:rustc-env=BUILD_TYPE={}", self.build_type);
        println!("cargo:rustc-env=BUILD_DEBUG={}", self.debug);
        println!("cargo:rustc-env=BUILD_OPTIM_LEVEL={}", self.optim_level);
        println!("cargo:rustc-env=BUILD_CXX_STANDARD={}", self.cmake_cxx_standard);
        
        if let Some(ms) = &self.msystem {
            println!("cargo:rustc-env=BUILD_MSYSTEM={}", ms);
        }
    }

    fn get_cpp_flags(&self) -> [&'static str; 2] {
        if cfg!(feature = "usecxx17") {
            ["-std=c++17", "/std:c++17"]
        } else {
            ["-std=c++20", "/std:c++20"]
        }
    }

    fn is_msvc(&self) -> bool {
        self.target_env == "msvc"
    }

    fn is_gnu(&self) -> bool {
        self.target_env == "gnu"
    }

    fn is_windows(&self) -> bool {
        self.target_os == "windows"
    }

    fn is_linux(&self) -> bool {
        self.target_os == "linux"
    }

    fn is_unix(&self) -> bool {
        self.target_family == "unix"
    }

    fn is_clang_msys(&self) -> bool {
        self.msystem.as_deref().map_or(false, |s| s.contains("CLANG"))
    }

    fn is_ucrt64(&self) -> bool {
        self.msystem.as_deref() == Some("UCRT64")
    }
}
fn configure_msys2(config: &mut BuildConfig) {
    if let Some(msystem) = &config.msystem {
        match msystem.as_str() {
            "CLANG64" | "CLANGARM64" => {
                let defines = vec![
                    ("CMAKE_CXX_COMPILER", "clang++"),
                    ("CMAKE_C_COMPILER", "clang"),
                    ("CMAKE_CXX_FLAGS", "-fuse-ld=lld -stdlib=libc++ -mcx16 -Wno-error=unknown-pragmas -Qunused-arguments"),
                    ("CMAKE_C_FLAGS", "-fuse-ld=lld -Wno-error=unknown-pragmas -Qunused-arguments"),
                    ("CMAKE_EXE_LINKER_FLAGS", "-fuse-ld=lld -stdlib=libc++")
                ];
                apply_defines(&mut config.builder, &defines);
            }
            "UCRT64" => {
                let defines = vec![
                    ("CMAKE_CXX_FLAGS", "-fuse-ld=lld -Wno-error=unknown-pragmas"),
                    ("CMAKE_SYSTEM_NAME", "Windows"),
                    ("CMAKE_C_FLAGS", "-fuse-ld=lld -Wno-error=unknown-pragmas")
                ];
                apply_defines(&mut config.builder, &defines);
            }
            _ => {}
        }
    }
}
fn configure_features(config: &mut BuildConfig) {
    if config.features.native_cpu {
        config.builder.define("SNMALLOC_OPTIMISE_FOR_CURRENT_MACHINE", "ON");
        #[cfg(feature = "build_cc")]
        config.builder.flag_if_supported("-march=native");
    }

    if config.features.qemu {
        config.builder.define("SNMALLOC_QEMU_WORKAROUND", "ON");
    }

    if config.features.lto {
        match config.compiler {
            Compiler::Unknown => {
                // Skip LTO for unknown compiler
            }
            Compiler::Msvc => {
                // Skip LTO for unknown compiler
            }
            _ => {
                println!("{:?}",config.compiler);
                // Enable LTO for known compilers (Clang, Gcc, Msvc)
                config.builder.define("SNMALLOC_IPO", "ON");
            }
        }
    }

    if config.features.notls {
        config.builder.define("SNMALLOC_ENABLE_DYNAMIC_LOADING", "ON");
    }

    if config.features.win8compat {
        #[cfg(feature = "build_cc")]
        config.builder.define("WINVER", "0x0603");
        #[cfg(not(feature = "build_cc"))]
        config.builder.define("WIN8COMPAT", "ON");
    }

    config.builder.define(
        "SNMALLOC_USE_WAIT_ON_ADDRESS",
        if config.features.wait_on_address { "1" } else { "0" }
    );

    #[cfg(not(feature = "build_cc"))]
    {
        config.builder.define("CMAKE_CXX_STANDARD", config.cmake_cxx_standard);
        if config.features.stats {
            config.builder.define("USE_SNMALLOC_STATS", "ON");
        }
    }
}

fn configure_platform(config: &mut BuildConfig) {
    if config.is_windows() {
        let common_flags = vec![
            "-mcx16",
            "-fno-exceptions",
            "-fno-rtti",
            "-pthread"
        ];
        
        for flag in common_flags {
            config.builder.flag_if_supported(flag);
        }

        if let Some(msystem) = &config.msystem {
            match msystem.as_str() {
                "CLANG64" | "CLANGARM64" => {
                    let clang_flags = vec![
                        "-flto",
                        "-fuse-ld=lld",
                        "-stdlib=libc++",
                        "-Wno-error=unknown-pragmas",
                        "-Qunused-arguments"
                    ];
                    for flag in clang_flags {
                        config.builder.flag_if_supported(flag);
                    }
                }
                "UCRT64" => {
                    let ucrt_flags = vec![
                        "-Wno-error=unknown-pragmas",
                        "-fuse-ld=lld",
                        "-Qunused-arguments"
                    ];
                    for flag in ucrt_flags {
                        config.builder.flag_if_supported(flag);
                    }
                }
                _ => {}
            }
        }
    } else if config.is_linux() || config.is_unix() {
        let unix_flags = vec![
            "-fPIC",
            "-pthread",
            "-fno-exceptions",
            "-fno-rtti",
            "-mcx16",
            "-Wno-unused-parameter"
        ];
        for flag in unix_flags {
            config.builder.flag_if_supported(flag);
        }
    }

    if config.is_msvc() {
        let msvc_flags = vec![
            "/nologo", "/W4", "/WX", "/wd4127", "/wd4324", "/wd4201",
            "/Ob2", "/DNDEBUG", "/EHsc", "/Gd", "/TP", "/Gm-", "/GS",
            "/fp:precise", "/Zc:wchar_t", "/Zc:forScope", "/Zc:inline"
        ];
        for flag in msvc_flags {
            config.builder.flag_if_supported(flag);
        }
        
        config.builder
            .define("CMAKE_CXX_FLAGS_RELEASE", "/O2 /Ob2 /DNDEBUG /EHsc")
            .define("CMAKE_C_FLAGS_RELEASE", "/O2 /Ob2 /DNDEBUG /EHsc");
    }

    if config.target.contains("android") {
        configure_android(config);
    }
}

fn configure_android(config: &mut BuildConfig) {
    let ndk = env::var("ANDROID_NDK").expect("ANDROID_NDK environment variable not set");
    let toolchain_path = format!("{}/build/cmake/android.toolchain.cmake", ndk);
    
    // Convert String to &str before passing to define
    config.builder.define("CMAKE_TOOLCHAIN_FILE", &*toolchain_path);

    if let Ok(platform) = env::var("ANDROID_PLATFORM") {
        // Convert String to &str before passing to define
        config.builder.define("ANDROID_PLATFORM", &*platform);
    }

    if cfg!(feature = "android-lld") {
        config.builder.define("ANDROID_LD", "lld");
    }

    let abi = match config.target.as_str() {
        t if t.contains("aarch64") => "arm64-v8a",
        t if t.contains("armv7") => {
            config.builder.define("ANDROID_ARM_MODE", "arm");
            "armeabi-v7a"
        }
        t if t.contains("x86_64") => "x86_64",
        t if t.contains("i686") => "x86",
        t if t.contains("neon") => "armeabi-v7a with NEON",
        t if t.contains("arm") => "armeabi-v7a",
        _ => panic!("Unsupported Android architecture"),
    };
    config.builder.define("ANDROID_ABI", abi);
}

// Helper function to apply defines regardless of builder type
fn apply_defines<T>(builder: &mut T, defines: &[(&str, &str)])
where
    T: BuilderDefine,
{
    for (key, value) in defines {
        builder.define(key, value);
    }
}

trait BuilderDefine {
    fn define(&mut self, key: &str, value: &str) -> &mut Self;
    fn flag_if_supported(&mut self, flag: &str) -> &mut Self;
}

#[cfg(feature = "build_cc")]
impl BuilderDefine for cc::Build {
    fn define(&mut self, key: &str, value: &str) -> &mut Self {
        self.define(key, Some(value))
    }
    fn flag_if_supported(&mut self, flag: &str) -> &mut Self {
        self.flag_if_supported(flag)
    }
}

#[cfg(not(feature = "build_cc"))]
impl BuilderDefine for cmake::Config {
    fn define(&mut self, key: &str, value: &str) -> &mut Self {
        self.define(key, value)
    }
    fn flag_if_supported(&mut self, _flag: &str) -> &mut Self {
        self
    }
}

impl BuildFeatures {
    fn new() -> Self {
        Self {
            native_cpu: cfg!(feature = "native-cpu"),
            qemu: cfg!(feature = "qemu"),
            wait_on_address: cfg!(feature = "usewait-on-address"),
            lto: cfg!(feature = "lto"),
            notls: cfg!(feature = "notls"),
            win8compat: cfg!(feature = "win8compat"),
            stats: cfg!(feature = "stats"),
            android_lld: cfg!(feature = "android-lld"),
            local_dynamic_tls: cfg!(feature = "local_dynamic_tls"),
        }
    }
}


fn configure_linking(config: &BuildConfig, dst: Option<&std::path::PathBuf>) {
    println!("cargo:rustc-link-lib=static={}", config.target_lib);

    if config.is_msvc() {
        if !config.features.win8compat {
            println!("cargo:rustc-link-lib=mincore");
        }
        if let Some(dst) = dst {
            println!(
                "cargo:rustc-link-search=native={}/{}",
                dst.display(),
                config.build_type
            );
        }
    } else {
        if let Some(dst) = dst {
            println!("cargo:rustc-link-search=native={}", dst.display());
        }

        // FreeBSD specific handling
        if cfg!(target_os = "freebsd") {
            println!("cargo:rustc-link-search=native=/usr/local/lib");
            println!("cargo:rustc-link-lib=c++");
            return;
        }

        // Windows GNU handling
        if config.is_windows() && config.is_gnu() {
            println!("cargo:rustc-link-lib=bcrypt");
            println!("cargo:rustc-link-lib=winpthread");

            if config.is_clang_msys() {
                // CLANG64/CLANGARM64 specific
                println!("cargo:rustc-link-lib=c++");
            } else if config.is_ucrt64() {
                // UCRT64 specific
                println!("cargo:rustc-link-lib=stdc++");
            } else {
                // Regular MinGW
                println!("cargo:rustc-link-lib=stdc++");
                println!("cargo:rustc-link-lib=atomic");
            }
        }

        // Linux specific handling
        if config.is_linux() {
            println!("cargo:rustc-link-lib=atomic");
            println!("cargo:rustc-link-lib=stdc++");
            println!("cargo:rustc-link-lib=pthread");
            
            if cfg!(feature = "usecxx17") && !config.is_clang_msys() {
                println!("cargo:rustc-link-lib=gcc");
            }
        }

        // General Unix handling (excluding FreeBSD and macOS)
        if config.is_unix() && !cfg!(target_os = "macos") && !cfg!(target_os = "freebsd") {
            if config.is_gnu() {
                println!("cargo:rustc-link-lib=c_nonshared");
            }
        } else if !config.is_windows() {
            let cxxlib = if cfg!(any(target_os = "macos", target_os = "openbsd")) {
                "c++"
            } else {
                "stdc++"
            };
            println!("cargo:rustc-link-lib={}", cxxlib);
        }
    }
}
fn configure_compiler_flags(config: &mut BuildConfig) {
    config.builder
        .flag_if_supported(config.optim_level)
        .flag_if_supported("-fomit-frame-pointer");

    for std in config.get_cpp_flags() {
        config.builder.flag_if_supported(std);
    }
}

fn configure_tls(config: &mut BuildConfig) {
    if (config.is_unix() || config.is_gnu()) && config.target_os != "haiku" {
        let tls_model = if config.features.local_dynamic_tls {
            "-ftls-model=local-dynamic"
        } else {
            "-ftls-model=initial-exec"
        };
        config.builder.flag_if_supported(tls_model);
    }
}
#[cfg(feature = "build_cc")]
use cc;

#[cfg(feature = "build_cc")]
fn main() {
    let mut config = BuildConfig::new();
    config.builder
        .include("snmalloc/src")
        .file("snmalloc/src/snmalloc/override/rust.cc")
        .cpp(true)
        .debug(config.debug);

    configure_platform(&mut config);
    configure_compiler_flags(&mut config);
    configure_tls(&mut config);
    configure_features(&mut config);

    config.builder.out_dir(&config.out_dir);
    config.builder.compile(config.target_lib);
    configure_linking(&config, None);
}

#[cfg(not(feature = "build_cc"))]
use cmake::Config;

#[cfg(not(feature = "build_cc"))]
fn main() {
    let mut config = BuildConfig::new();
   
    config.builder
        .define("SNMALLOC_RUST_SUPPORT", "ON")
        .profile(&config.build_type)
        .very_verbose(true)
        .define("CMAKE_SH", "CMAKE_SH-NOTFOUND");

    configure_msys2(&mut config);
    configure_platform(&mut config);
    configure_compiler_flags(&mut config);
    configure_tls(&mut config);
    configure_features(&mut config);

    let mut dst = config.builder.build_target(config.target_lib).build();
    dst.push("build");
    configure_linking(&config, Some(&dst));
}
