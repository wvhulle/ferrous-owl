import fs from "node:fs/promises";
import path from "node:path";
import os from "node:os";
import { spawn, spawnSync } from "node:child_process";
import * as vscode from "vscode";
import packageJson from "../package.json";

const version: string = packageJson.version;

const REPO_URL = "https://github.com/wvhulle/ferrous-owl.git";
const CACHE_DIR = path.join(os.homedir(), ".cache", "ferrous-owl");
const CARGO_BIN = path.join(os.homedir(), ".cargo", "bin");
const EXE_EXT = process.platform === "win32" ? ".exe" : "";

const getProjectRootFromExtension = (extensionPath: string): string =>
  path.dirname(path.dirname(extensionPath));

const buildDebugBinary = async (projectRoot: string): Promise<boolean> =>
  vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: "FerrousOwl: Building debug binary",
      cancellable: false,
    },
    async (progress) => {
      try {
        progress.report({ message: "Running cargo build..." });
        
        const cargoBuild = spawn("cargo", ["build"], {
          cwd: projectRoot,
        });

        cargoBuild.stderr.on("data", (data: Buffer) => {
          const line = String(data).trim();
          if (line.includes("Compiling")) {
            progress.report({ message: line });
          }
        });

        await waitForProcess(cargoBuild, "cargo build");
        
        progress.report({ message: "Debug build complete" });
        return true;
      } catch (e) {
        console.error("Debug build failed:", e);
        return false;
      }
    }
  );

const findLocalDevBinary = async (extensionPath: string): Promise<string | null> => {
  const projectRoot = getProjectRootFromExtension(extensionPath);
  const candidate = path.join(projectRoot, "target", "debug", `ferrous-owl${EXE_EXT}`);
  
  if (await exists(candidate)) {
    const ver = getVersionOutput(candidate, ["--version", "--quiet"]);
    if (ver) {
      console.warn(`Found local dev binary: ${candidate}`);
      return candidate;
    }
  }
  
  console.warn(`Debug binary not found at ${candidate}, building...`);
  if (await buildDebugBinary(projectRoot)) {
    if (await exists(candidate)) {
      const ver = getVersionOutput(candidate, ["--version", "--quiet"]);
      if (ver) {
        console.warn(`Built local dev binary: ${candidate}`);
        return candidate;
      }
    }
  }
  
  return null;
};

interface FerrousOwlConfig {
  readonly serverPath: string;
}

const getConfig = (): FerrousOwlConfig => ({
  serverPath: vscode.workspace.getConfiguration("ferrous-owl").get<string>("serverPath", ""),
});

const getVersionOutput = (command: string, args: string[]): string => {
  const result = spawnSync(command, args);
  // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition, @typescript-eslint/strict-boolean-expressions
  return result.stdout ? String(result.stdout).trim() : "";
};

const commandExists = (command: string): boolean =>
  spawnSync(command, ["--version"]).status === 0;

const exists = async (filePath: string): Promise<boolean> =>
  fs.access(filePath).then(() => true).catch(() => false);

const isGitRepo = async (dir: string): Promise<boolean> =>
  exists(path.join(dir, ".git"));

const waitForProcess = (proc: ReturnType<typeof spawn>, name: string): Promise<void> =>
  new Promise((resolve, reject) => {
    proc.on("close", (code) =>
      code === 0 ? resolve() : reject(new Error(`${name} failed with code ${code ?? "unknown"}`))
    );
    proc.on("error", reject);
  });

export const needsUpdate = async (currentVersion: string): Promise<boolean> => {
  if (!currentVersion) {return true;}
  
  console.warn(`Current FerrousOwl version: ${currentVersion}`);
  console.warn(`Extension version: v${version}`);
  
  try {
    const semverParser = await import("semver-parser");
    const current = semverParser.parseSemVer(currentVersion, false);
    const target = semverParser.parseSemVer(version, false);
    return !(
      current.major === target.major &&
      current.minor === target.minor &&
      current.patch === target.patch &&
      JSON.stringify(current.pre) === JSON.stringify(target.pre)
    );
  } catch {
    return true;
  }
};

// TODO: Re-enable when package is published to crates.io
const _tryBinstall = async (): Promise<boolean> =>
  vscode.window.withProgress(
    { location: vscode.ProgressLocation.Notification, title: "FerrousOwl: Trying cargo-binstall..." },
    async (progress) => {
      if (!commandExists("cargo-binstall")) {
        progress.report({ message: "cargo-binstall not found, skipping" });
        return false;
      }
      
      try {
        progress.report({ message: "Installing via cargo-binstall..." });
        const proc = spawn("cargo-binstall", ["--no-confirm", "ferrous-owl"], { stdio: "pipe" });
        await waitForProcess(proc, "cargo-binstall");
        return true;
      } catch {
        progress.report({ message: "cargo-binstall failed" });
        return false;
      }
    }
  );

const cloneOrPullRepo = async (
  progress: vscode.Progress<{ message?: string }>
): Promise<void> => {
  await fs.mkdir(CACHE_DIR, { recursive: true });
  
  if (await isGitRepo(CACHE_DIR)) {
    progress.report({ message: "Pulling latest changes..." });
    const pull = spawn("git", ["pull", "--ff-only"], { cwd: CACHE_DIR });
    try {
      await waitForProcess(pull, "git pull");
    } catch {
      progress.report({ message: "Pull failed, re-cloning..." });
      await fs.rm(CACHE_DIR, { recursive: true, force: true });
      await fs.mkdir(CACHE_DIR, { recursive: true });
      const clone = spawn("git", ["clone", "--depth", "1", REPO_URL, CACHE_DIR]);
      await waitForProcess(clone, "git clone");
    }
  } else {
    progress.report({ message: "Cloning repository..." });
    await fs.rm(CACHE_DIR, { recursive: true, force: true });
    await fs.mkdir(CACHE_DIR, { recursive: true });
    const clone = spawn("git", ["clone", "--depth", "1", REPO_URL, CACHE_DIR]);
    await waitForProcess(clone, "git clone");
  }
};

const buildFromSource = async (): Promise<boolean> =>
  vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: "FerrousOwl: Building from source",
      cancellable: false,
    },
    async (progress) => {
      try {
        await cloneOrPullRepo(progress);
        
        progress.report({ message: "Running cargo build --release (this may take a few minutes)..." });
        
        const cargoBuild = spawn("cargo", ["build", "--release", "--locked"], {
          cwd: CACHE_DIR,
        });

        cargoBuild.stderr.on("data", (data: Buffer) => {
          const line = String(data).trim();
          if (line.includes("Compiling")) {
            progress.report({ message: line });
          }
        });

        await waitForProcess(cargoBuild, "cargo build");
        
        progress.report({ message: "Build complete" });
        return true;
      } catch (e) {
        console.error("Build from source failed:", e);
        return false;
      }
    }
  );

const createSymlink = async (binaryPath: string): Promise<void> => {
  const symlinkPath = path.join(CARGO_BIN, `ferrous-owl${EXE_EXT}`);
  
  if (await exists(symlinkPath)) {
    const stat = await fs.lstat(symlinkPath);
    if (stat.isSymbolicLink() || stat.isFile()) {
      await fs.unlink(symlinkPath);
    }
  }
  
  try {
    await fs.symlink(binaryPath, symlinkPath);
    console.warn(`Created symlink: ${symlinkPath} -> ${binaryPath}`);
  } catch (e) {
    console.warn(`Could not create symlink: ${String(e)}`);
  }
};

const findFerrousOwlBinary = async (): Promise<string | null> => {
  const locations = [
    path.join(CARGO_BIN, `ferrous-owl${EXE_EXT}`),
    path.join(CACHE_DIR, "target", "release", `ferrous-owl${EXE_EXT}`),
  ];
  
  for (const loc of locations) {
    if (await exists(loc)) {
      const ver = getVersionOutput(loc, ["--version", "--quiet"]);
      if (ver) {return loc;}
    }
  }
  
  const globalVer = getVersionOutput("ferrous-owl", ["--version", "--quiet"]);
  if (globalVer) {return "ferrous-owl";}
  
  return null;
};

export const installFerrousOwl = async (): Promise<string> => {
  if (!commandExists("cargo") || !commandExists("git")) {
    throw new Error(
      "FerrousOwl requires cargo and git. Please install Rust via rustup.rs and ensure git is available."
    );
  }

  // TODO: Re-enable binstall when package is published
  // if (await tryBinstall()) {
  //   const binary = await findRustowlBinary();
  //   if (binary) {
  //     return binary;
  //   }
  // }

  if (await buildFromSource()) {
    const targetBinary = path.join(CACHE_DIR, "target", "release", `ferrous-owl${EXE_EXT}`);
    if (await exists(targetBinary)) {
      // Create symlink in background, don't wait - avoids disrupting any running server
      void createSymlink(targetBinary);
      // Return the direct path to the built binary
      return targetBinary;
    }
  }

  void vscode.window.showErrorMessage(
    "FerrousOwl installation failed. Please install manually:\n" +
    "git clone https://github.com/wvhulle/ferrous-owl.git ~/.cache/ferrous-owl\n" +
    "cd ~/.cache/ferrous-owl && cargo build --release --locked"
  );
  
  throw new Error("Failed to install FerrousOwl");
};

export const bootstrapFerrousOwl = async (
  extensionPath: string,
  extensionMode?: vscode.ExtensionMode,
): Promise<string> => {
  const config = getConfig();

  if (config.serverPath) {
    const ver = getVersionOutput(config.serverPath, ["--version", "--quiet"]);
    if (ver) {
      console.warn(`Using configured serverPath: ${config.serverPath}`);
      return config.serverPath;
    }
    throw new Error(`Configured serverPath "${config.serverPath}" is not a valid ferrous-owl executable`);
  }

  const isDevelopment = extensionMode === vscode.ExtensionMode.Development;
  
  if (isDevelopment && extensionPath) {
    const localBinary = await findLocalDevBinary(extensionPath);
    if (localBinary) {
      console.warn(`Development mode: using local binary ${localBinary}`);
      return localBinary;
    }
    console.warn("Development mode: no local binary found, falling back to normal bootstrap");
  }

  const existingBinary = await findFerrousOwlBinary();
  
  if (existingBinary) {
    const currentVersion = getVersionOutput(existingBinary, ["--version", "--quiet"]);
    const updateNeeded = await needsUpdate(currentVersion);
    
    if (updateNeeded) {
      // Don't auto-update - just warn the user and use existing binary
      // Auto-updating can kill running servers in other VS Code windows
      void vscode.window.showWarningMessage(
        `FerrousOwl update available (${currentVersion} → v${version}). Run "FerrousOwl: Update" command to update.`,
        "Update Now"
      ).then(async (choice) => {
        if (choice === "Update Now") {
          await vscode.commands.executeCommand("ferrous-owl.update");
        }
      });
    }
    
    return existingBinary;
  }

  // No existing binary found - install
  return installFerrousOwl();
};
