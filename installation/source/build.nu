#!/usr/bin/env nu

# RustOwl release build script for CI
# Usage: nu build.nu [--profile <release|debug>] [--output-dir <path>]

def get-host-tuple [] {
    let os = (sys host | get name)
    let arch = (sys host | get arch)

    let arch_str = match $arch {
        "x86_64" | "amd64" => "x86_64"
        "aarch64" | "arm64" => "aarch64"
        _ => "x86_64"
    }

    let os_str = match $os {
        "Linux" => "unknown-linux-gnu"
        "Darwin" | "macOS" => "apple-darwin"
        "Windows" => "pc-windows-msvc"
        _ => "unknown-linux-gnu"
    }

    $"($arch_str)-($os_str)"
}

def build-rustowl [profile: string output_dir: string] {
    let host_tuple = (get-host-tuple)
    let os = (sys host | get name)
    let exec_ext = if $os == "Windows" { ".exe" } else { "" }

    print $"Building RustOwl for ($host_tuple) with profile: ($profile)"

    # Build
    cargo build --profile $profile --locked

    let target_dir = $"target/($profile)"
    let rustowl_bin = $"($target_dir)/rustowl($exec_ext)"
    let rustowlc_bin = $"($target_dir)/rustowlc($exec_ext)"

    # Test functionality
    print "Testing functionality..."
    run-external $rustowl_bin "check" "./perf-tests"

    # Create output directory
    let artifact_dir = $"($output_dir)/rustowl-($host_tuple)"
    mkdir $artifact_dir

    # Copy binaries and docs
    cp $rustowl_bin $artifact_dir
    cp $rustowlc_bin $artifact_dir
    cp README.md $artifact_dir
    cp LICENSE $artifact_dir

    # Copy generated files
    let build_out_dirs = (glob "target/**/rustowl-build-time-out")
    if ($build_out_dirs | length) > 0 {
        let build_out = ($build_out_dirs | first)
        let completions_dir = $"($build_out)/completions"
        let man_dir = $"($build_out)/man"
        if ($completions_dir | path exists) { cp -r $completions_dir $artifact_dir }
        if ($man_dir | path exists) { cp -r $man_dir $artifact_dir }
    }

    # Create archive
    let archive_name = if $os == "Windows" {
        $"rustowl-($host_tuple).zip"
    } else {
        $"rustowl-($host_tuple).tar.gz"
    }

    print $"Creating archive: ($archive_name)"
    cd $output_dir

    if $os == "Windows" {
        powershell -c $'Compress-Archive -Path "rustowl-($host_tuple)/*" -DestinationPath "($archive_name)"'
    } else {
        tar -czvf $archive_name $"rustowl-($host_tuple)"
    }

    print $"Build complete. Artifacts in: ($output_dir)"
    print $"  Archive: ($archive_name)"
    print $"  Binary: rustowl-($host_tuple)/rustowl($exec_ext)"
}

def main [
    --profile: string = "release"
    --output-dir: string = "dist"
] {
    build-rustowl $profile $output_dir
}
