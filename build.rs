use clap::CommandFactory;
use clap_complete::generate_to;
use std::env;
use std::fs;
use std::io::Error;
use std::process::Command;

fn main() -> Result<(), Error> {
    // Declare custom cfg flags to avoid warnings
    println!("cargo::rustc-check-cfg=cfg(miri)");

    let toolchain = get_toolchain();
    println!("cargo::rustc-env=RUSTOWL_TOOLCHAIN={toolchain}");
    println!("cargo::rustc-env=TOOLCHAIN_CHANNEL={}", get_channel());
    if let Some(date) = get_toolchain_date() {
        println!("cargo::rustc-env=TOOLCHAIN_DATE={date}");
    }

    let host_tuple = get_host_tuple();
    println!("cargo::rustc-env=HOST_TUPLE={host_tuple}");

    #[cfg(target_os = "macos")]
    {
        println!("cargo::rustc-link-arg-bin=rustowlc=-Wl,-rpath,@executable_path/../lib");
    }
    #[cfg(target_os = "linux")]
    {
        println!("cargo::rustc-link-arg-bin=rustowlc=-Wl,-rpath,$ORIGIN/../lib");
    }
    #[cfg(target_os = "windows")]
    {
        println!("cargo::rustc-link-arg-bin=rustowlc=/LIBPATH:..\\bin");
    }

    Ok(())
}

// get toolchain
// Priority: RUSTUP_TOOLCHAIN > TOOLCHAIN_CHANNEL > rust-toolchain.toml
fn get_toolchain() -> String {
    if let Ok(v) = env::var("RUSTUP_TOOLCHAIN") {
        v
    } else if let Ok(v) = env::var("TOOLCHAIN_CHANNEL") {
        format!("{v}-{}", get_host_tuple())
    } else {
        // Read from rust-toolchain.toml
        let content = std::fs::read_to_string("./rust-toolchain.toml").unwrap_or_default();
        let channel = content
            .lines()
            .find(|l| l.starts_with("channel"))
            .and_then(|l| l.split('"').nth(1))
            .unwrap_or("stable");
        format!("{}-{}", channel.trim(), get_host_tuple())
    }
}
fn get_channel() -> String {
    get_toolchain()
        .split("-")
        .next()
        .expect("failed to obtain channel from toolchain")
        .to_owned()
}
fn get_toolchain_date() -> Option<String> {
    let r = regex::Regex::new(r#"\d\d\d\d-\d\d-\d\d"#).unwrap();
    r.find(&get_toolchain()).map(|v| v.as_str().to_owned())
}
fn get_host_tuple() -> String {
    Command::new(env::var("RUSTC").unwrap_or("rustc".to_string()))
        .arg("--print")
        .arg("host-tuple")
        .output()
        .map(|v| String::from_utf8(v.stdout).unwrap().trim().to_string())
        .expect("failed to obtain host-tuple")
}
